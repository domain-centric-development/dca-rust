//! A store in memory with transactions: a transaction works on a copy of the tables and replaces
//! them on commit; one transaction runs at a time.

use std::sync::{Arc, Mutex, PoisonError};

use dca_building_blocks::application::{ScopedBoxFuture, TransactionBoundary};
use dca_building_blocks::hexagonal::port_out::PortError;
use tokio::sync::Mutex as AsyncMutex;

use super::inbox::InboxTable;
use super::outbox::{OutboxEntry, OutboxTable, Subscriptions};

/// A context's tables with the kernel's outbox and inbox beside them, so an aggregate and the
/// deliveries its events owe are committed together.
pub struct Staged<C> {
    /// The context's own tables.
    pub context: C,
    /// The outbox, shared by the publishers of one transaction.
    pub outbox: Mutex<OutboxTable>,
    /// The inbox.
    pub inbox: InboxTable,
}

impl<C> Staged<C> {
    fn new(context: C) -> Self {
        Self {
            context,
            outbox: Mutex::new(OutboxTable::default()),
            inbox: InboxTable::default(),
        }
    }

    /// The outbox table.
    pub fn outbox_table(&self) -> OutboxTable {
        self.outbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl<C: Clone> Clone for Staged<C> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            outbox: Mutex::new(self.outbox_table()),
            inbox: self.inbox.clone(),
        }
    }
}

/// A use case's transaction unit over the in-memory store: opened over a copy of the tables and
/// closed into the tables to commit.
pub trait MemoryUnit: Send + Sized {
    /// The context's tables.
    type Tables: Clone + Send + 'static;

    /// The unit over a transaction's copy of the tables.
    fn open(staged: Staged<Self::Tables>, subscriptions: Arc<Subscriptions>) -> Self;

    /// The tables as the transaction leaves them.
    fn close(self) -> Staged<Self::Tables>;
}

/// The in-memory store of one context. Clones share the store.
pub struct MemoryDatabase<C> {
    state: Arc<AsyncMutex<Staged<C>>>,
    subscriptions: Arc<Subscriptions>,
}

impl<C> Clone for MemoryDatabase<C> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            subscriptions: Arc::clone(&self.subscriptions),
        }
    }
}

impl<C: Clone + Send + 'static> MemoryDatabase<C> {
    /// A store over the context's empty tables, its publishers registering deliveries for the
    /// subscribed listeners.
    pub fn new(context: C, subscriptions: Subscriptions) -> Self {
        Self {
            state: Arc::new(AsyncMutex::new(Staged::new(context))),
            subscriptions: Arc::new(subscriptions),
        }
    }

    /// Reads the committed tables.
    pub async fn read<R>(&self, read: impl FnOnce(&Staged<C>) -> R) -> R {
        read(&*self.state.lock().await)
    }

    /// Forgets the deliveries the relay made - see [`OutboxTable::forget_deliveries`].
    pub async fn forget_deliveries(&self) {
        let state = self.state.lock().await;
        state
            .outbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .forget_deliveries();
    }

    pub(crate) async fn undelivered(&self) -> Vec<OutboxEntry> {
        self.read(|staged| staged.outbox_table().undelivered())
            .await
    }

    pub(crate) async fn mark_delivered(&self, sequence: u64) {
        let state = self.state.lock().await;
        state
            .outbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .mark_delivered(sequence);
    }
}

impl<C, U> TransactionBoundary<U> for MemoryDatabase<C>
where
    C: Clone + Send + 'static,
    U: MemoryUnit<Tables = C>,
{
    async fn in_transaction<'scope, T, E, F>(&self, work: F) -> Result<T, E>
    where
        T: Send + 'scope,
        E: Send + From<PortError> + 'scope,
        F: for<'unit> FnOnce(&'unit mut U) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
            + Send
            + 'scope,
    {
        let mut committed = self.state.lock().await;
        let mut unit = U::open(committed.clone(), Arc::clone(&self.subscriptions));
        let outcome = work(&mut unit).await;
        if outcome.is_ok() {
            *committed = unit.close();
        }
        outcome
    }
}
