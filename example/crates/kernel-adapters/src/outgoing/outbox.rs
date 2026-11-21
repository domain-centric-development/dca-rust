//! The outbox: the deliveries a committed event still owes its transactional listeners, written in
//! the publishing transaction - by the domain event publisher for a context's own listeners, and by
//! the integration event publisher for the listeners of other contexts.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use dca_building_blocks::application::EventListener;
use dca_building_blocks::ddd::tactical::{DomainEvent, IntegrationEvent};
use dca_building_blocks::hexagonal::port_out::{
    DomainEventPublisher, IntegrationEventPublisher, OutputPort, PortError,
};
use kernel_application::{ListenerId, OrderingKey};

/// An event as the outbox keeps it: any event type, shared by the entries of its listeners.
pub(crate) type Payload = Arc<dyn Any + Send + Sync>;

/// One delivery an event owes one listener.
#[derive(Clone)]
pub(crate) struct OutboxEntry {
    pub(crate) sequence: u64,
    pub(crate) listener: ListenerId,
    pub(crate) order: Option<OrderingKey>,
    pub(crate) payload: Payload,
    pub(crate) delivered: bool,
}

/// The outbox table, as a transaction sees it.
#[derive(Clone, Default)]
pub struct OutboxTable {
    entries: Vec<OutboxEntry>,
    next: u64,
}

impl OutboxTable {
    fn add(&mut self, listener: ListenerId, order: Option<OrderingKey>, payload: Payload) {
        self.next += 1;
        self.entries.push(OutboxEntry {
            sequence: self.next,
            listener,
            order,
            payload,
            delivered: false,
        });
    }

    /// How many deliveries are still to make.
    pub fn pending(&self) -> usize {
        self.entries.iter().filter(|e| !e.delivered).count()
    }

    /// The listeners the deliveries still to make go to, in the order they were registered.
    pub fn pending_listeners(&self) -> Vec<ListenerId> {
        self.entries
            .iter()
            .filter(|e| !e.delivered)
            .map(|e| e.listener)
            .collect()
    }

    pub(crate) fn undelivered(&self) -> Vec<OutboxEntry> {
        self.entries
            .iter()
            .filter(|e| !e.delivered)
            .cloned()
            .collect()
    }

    pub(crate) fn mark_delivered(&mut self, sequence: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.sequence == sequence) {
            entry.delivered = true;
        }
    }

    /// Forgets that the deliveries were made: what a relay finds that stopped after delivering and
    /// before recording it. The listeners then receive the events again - delivery is at least once.
    pub fn forget_deliveries(&mut self) {
        for entry in &mut self.entries {
            entry.delivered = false;
        }
    }
}

/// Which listeners react after the commit to which event type: the wiring the publishers read when
/// they register deliveries.
#[derive(Debug, Clone, Default)]
pub struct Subscriptions {
    listeners: HashMap<TypeId, Vec<ListenerId>>,
}

impl Subscriptions {
    /// Subscribes a transactional listener to the events of type `E`, by its stable name.
    pub fn subscribe<E: 'static>(mut self, listener: ListenerId) -> Self {
        self.listeners
            .entry(TypeId::of::<E>())
            .or_default()
            .push(listener);
        self
    }

    /// The listeners of `E`.
    pub fn of<E: 'static>(&self) -> &[ListenerId] {
        self.listeners
            .get(&TypeId::of::<E>())
            .map_or(&[], Vec::as_slice)
    }
}

fn locked(table: &Mutex<OutboxTable>) -> std::sync::MutexGuard<'_, OutboxTable> {
    table.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The outbox over the transaction's table: no port, a part of the publishers' adapters.
pub struct TransactionOutbox<'a> {
    table: &'a Mutex<OutboxTable>,
}

impl<'a> TransactionOutbox<'a> {
    /// The outbox over the transaction's table.
    pub fn new(table: &'a Mutex<OutboxTable>) -> Self {
        Self { table }
    }
}

impl TransactionOutbox<'_> {
    /// Records that `event` is to be delivered to `listener` once the transaction commits, after
    /// the earlier entries with the same `order` key, if it has one.
    fn register<E: Clone + Send + Sync + 'static>(
        &mut self,
        event: &E,
        listener: ListenerId,
        order: Option<OrderingKey>,
    ) {
        locked(self.table).add(listener, order, Arc::new(event.clone()));
    }
}

/// Publishes integration events through the transaction's outbox, one delivery per subscribed
/// listener of another context: they leave the context only if the transaction commits.
pub struct IntegrationOutbox<'a> {
    outbox: TransactionOutbox<'a>,
    subscriptions: &'a Subscriptions,
}

impl<'a> IntegrationOutbox<'a> {
    /// The publisher over the transaction's table.
    pub fn new(table: &'a Mutex<OutboxTable>, subscriptions: &'a Subscriptions) -> Self {
        Self {
            outbox: TransactionOutbox::new(table),
            subscriptions,
        }
    }
}

impl OutputPort for IntegrationOutbox<'_> {}

impl<E: IntegrationEvent> IntegrationEventPublisher<E> for IntegrationOutbox<'_> {
    async fn publish(&mut self, event: E) -> Result<(), PortError> {
        for listener in self.subscriptions.of::<E>() {
            self.outbox.register(&event, *listener, None);
        }
        Ok(())
    }
}

/// The domain event publisher a transaction unit hands out: it calls the listeners that react
/// inside the transaction, handing each the transaction's ports `U`, then registers one outbox entry
/// per listener that reacts after the commit, ordered by the key `ordering` reads from the event.
pub struct TransactionEvents<'a, E, U = ()> {
    outbox: TransactionOutbox<'a>,
    unit: U,
    reacting: Vec<&'a (dyn EventListener<E, U> + Send + Sync)>,
    after_commit: &'a [ListenerId],
    ordering: fn(&E) -> Option<OrderingKey>,
}

impl<'a, E: DomainEvent, U> TransactionEvents<'a, E, U> {
    /// The publisher over the transaction's outbox, for the listeners subscribed to `E`; the
    /// listeners that react inside the transaction reach its ports through `unit`.
    pub fn new(
        table: &'a Mutex<OutboxTable>,
        subscriptions: &'a Subscriptions,
        ordering: fn(&E) -> Option<OrderingKey>,
        unit: U,
    ) -> Self {
        Self {
            outbox: TransactionOutbox::new(table),
            unit,
            reacting: Vec::new(),
            after_commit: subscriptions.of::<E>(),
            ordering,
        }
    }

    /// Adds a listener that reacts inside the transaction. It is built once and serves every
    /// transaction; the publisher only borrows it.
    pub fn reacting_with(mut self, listener: &'a (dyn EventListener<E, U> + Send + Sync)) -> Self {
        self.reacting.push(listener);
        self
    }
}

impl<E, U> OutputPort for TransactionEvents<'_, E, U> {}

impl<E: DomainEvent, U: Send> DomainEventPublisher<E> for TransactionEvents<'_, E, U> {
    async fn publish(&mut self, event: E) -> Result<(), PortError> {
        for listener in &self.reacting {
            listener.on(&event, &mut self.unit).await?;
        }
        for listener in self.after_commit {
            self.outbox
                .register(&event, *listener, (self.ordering)(&event));
        }
        Ok(())
    }
}
