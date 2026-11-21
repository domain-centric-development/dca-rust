//! Output ports: what the application needs from outside, declared in the application, implemented
//! by outgoing adapters.

use crate::ddd::tactical::{AggregateRoot, DomainEvent, IntegrationEvent};
use core::fmt;
use std::future::Future;

/// Marks a trait as an output port.
pub trait OutputPort {}

/// An output port could not do its work: the store was unreachable, the broker refused. Not a
/// domain failure; a use case passes it on as its own "unavailable" case.
///
/// It is for a failure of the system's own infrastructure, which no caller can answer but by
/// trying again later. A failure the caller can answer - a conflict, a uniqueness only the store
/// sees, a foreign system that refused - is part of the port's contract and declared as an
/// [`OutputPortFailure`](crate::application::OutputPortFailure) instead.
///
/// ```
/// use dca_building_blocks::hexagonal::port_out::{PortError, PortErrorKind};
///
/// let down = PortError::new("the store is down");
/// assert_eq!(down.kind(), PortErrorKind::Unavailable);
/// assert_eq!(down.to_string(), "the store is down");
///
/// let lost = PortError::rollback_failed(down);
/// assert_eq!(lost.kind(), PortErrorKind::RollbackFailed);
/// assert!(std::error::Error::source(&lost).is_some());
/// ```
#[derive(Debug)]
pub struct PortError {
    kind: PortErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// What kind of [`PortError`] it is: what a caller that must tell them apart reads, instead of the
/// message.
///
/// More kinds may come; a `match` over it needs a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PortErrorKind {
    /// The port could not do its work: the store was unreachable, a transaction could not begin or
    /// commit. Trying again later may succeed.
    Unavailable,
    /// The work of a transaction failed and its rollback failed too: what the store holds is no
    /// longer known.
    RollbackFailed,
}

impl PortError {
    /// A port error with a message, of the kind [`PortErrorKind::Unavailable`].
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: PortErrorKind::Unavailable,
            message: message.into(),
            source: None,
        }
    }

    /// A transaction whose work failed and whose rollback failed too, of the kind
    /// [`PortErrorKind::RollbackFailed`]: what the store holds is no longer known. A boundary
    /// returns it in place of the work's failure.
    pub fn rollback_failed(cause: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            kind: PortErrorKind::RollbackFailed,
            ..Self::caused_by("the transaction failed and could not be rolled back", cause)
        }
    }

    /// A port error caused by another error, of the kind [`PortErrorKind::Unavailable`].
    pub fn caused_by(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind: PortErrorKind::Unavailable,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// What kind of port error it is.
    pub fn kind(&self) -> PortErrorKind {
        self.kind
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

// Every port is asynchronous and takes `&mut self`: an adapter a transaction unit hands out may
// hold the transaction's connection by `&mut` and run its queries on it. Every returned future is
// `Send`, so a use case built on the ports runs on a multi-threaded runtime. An adapter over memory
// or files implements them as well - its work done inline when it is quick, behind the runtime's
// blocking pool when it is not.
//
// A `&mut` to a port is the port: a transaction unit whose adapter owns its ports hands each out as
// `type Things<'a> = &'a mut Adapter`. A port trait of the application's own that extends one of
// these is implemented for the `&mut` by the adapter.

impl<P: OutputPort + ?Sized> OutputPort for &mut P {}

/// Loads and saves one aggregate type by its identity. `T: AggregateRoot` makes a repository for
/// anything but an aggregate root unkompilable.
pub trait Repository<T: AggregateRoot>: OutputPort {
    /// The aggregate with this identity, if there is one.
    fn find_by_id(
        &mut self,
        id: &T::Id,
    ) -> impl Future<Output = Result<Option<T>, PortError>> + Send;

    /// Saves the aggregate's current state.
    fn save(&mut self, aggregate: &T) -> impl Future<Output = Result<(), PortError>> + Send;

    /// Removes the aggregate with this identity.
    fn delete_by_id(&mut self, id: &T::Id) -> impl Future<Output = Result<(), PortError>> + Send;
}

impl<T: AggregateRoot, R: Repository<T> + ?Sized> Repository<T> for &mut R {
    fn find_by_id(
        &mut self,
        id: &T::Id,
    ) -> impl Future<Output = Result<Option<T>, PortError>> + Send {
        (**self).find_by_id(id)
    }

    fn save(&mut self, aggregate: &T) -> impl Future<Output = Result<(), PortError>> + Send {
        (**self).save(aggregate)
    }

    fn delete_by_id(&mut self, id: &T::Id) -> impl Future<Output = Result<(), PortError>> + Send {
        (**self).delete_by_id(id)
    }
}

/// Keeps a group of records where no single one has an identity of interest, like an operations log.
/// It reads and appends; it never saves or deletes an element.
pub trait Store: OutputPort {}

impl<S: Store + ?Sized> Store for &mut S {}

/// Publishes an aggregate's domain events after it was saved, inside the transaction: it calls the
/// [`EventListener`]s that react in the transaction, handing each the transaction's ports, and
/// records, in the same transaction, a delivery for each [`TransactionalEventListener`] that reacts
/// after the commit - in an outbox, the adapter's concern. The use case calls
/// [`DomainEventPublisher::publish_and_clear_events`] and knows no listener.
///
/// [`EventListener`]: crate::application::EventListener
/// [`TransactionalEventListener`]: crate::application::TransactionalEventListener
pub trait DomainEventPublisher<E: DomainEvent>: OutputPort {
    /// Publishes one event.
    fn publish(&mut self, event: E) -> impl Future<Output = Result<(), PortError>> + Send;

    /// Takes the aggregate's recorded events and publishes each, in order.
    fn publish_and_clear_events<A: AggregateRoot<Event = E>>(
        &mut self,
        aggregate: &mut A,
    ) -> impl Future<Output = Result<(), PortError>> + Send
    where
        Self: Send,
        E: Send,
    {
        let events = aggregate.take_domain_events();
        async move {
            for event in events {
                self.publish(event).await?;
            }
            Ok(())
        }
    }
}

impl<E: DomainEvent, P: DomainEventPublisher<E> + ?Sized> DomainEventPublisher<E> for &mut P {
    fn publish(&mut self, event: E) -> impl Future<Output = Result<(), PortError>> + Send {
        (**self).publish(event)
    }
}

/// Publishes integration events for other bounded contexts. Its adapter records each event in the
/// open transaction - in an outbox, one entry per subscriber - so the event leaves the context only
/// if the transaction commits.
pub trait IntegrationEventPublisher<E: IntegrationEvent>: OutputPort {
    /// Publishes one event.
    fn publish(&mut self, event: E) -> impl Future<Output = Result<(), PortError>> + Send;
}

impl<E: IntegrationEvent, P: IntegrationEventPublisher<E> + ?Sized> IntegrationEventPublisher<E>
    for &mut P
{
    fn publish(&mut self, event: E) -> impl Future<Output = Result<(), PortError>> + Send {
        (**self).publish(event)
    }
}
