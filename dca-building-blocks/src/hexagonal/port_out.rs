//! Output ports: what the application needs from outside, declared in the application, implemented
//! by outgoing adapters.

use crate::ddd::tactical::{AggregateRoot, DomainEvent, IntegrationEvent};
use core::fmt;

/// Marks a trait as an output port.
pub trait OutputPort {}

/// An output port could not do its work: the store was unreachable, the broker refused. Not a
/// domain failure; a use case passes it on as its own "unavailable" case.
#[derive(Debug)]
pub struct PortError {
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PortError {
    /// A port error with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// A port error caused by another error.
    pub fn caused_by(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
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

/// Loads and saves one aggregate type by its identity. `T: AggregateRoot` makes a repository for
/// anything but an aggregate root unkompilable.
pub trait Repository<T: AggregateRoot>: OutputPort {
    /// The aggregate with this identity, if there is one.
    fn find_by_id(&self, id: &T::Id) -> Result<Option<T>, PortError>;

    /// Saves the aggregate's current state.
    fn save(&self, aggregate: &T) -> Result<(), PortError>;

    /// Removes the aggregate with this identity.
    fn delete_by_id(&self, id: &T::Id) -> Result<(), PortError>;
}

/// Keeps a group of records where no single one has an identity of interest, like an operations log.
/// It reads and appends; it never saves or deletes an element.
pub trait Store: OutputPort {}

/// Publishes an aggregate's domain events after it was saved.
pub trait DomainEventPublisher<E: DomainEvent>: OutputPort {
    /// Publishes one event.
    fn publish(&self, event: E) -> Result<(), PortError>;

    /// Takes the aggregate's recorded events and publishes each.
    fn publish_and_clear_events<A: AggregateRoot<Event = E>>(
        &self,
        aggregate: &mut A,
    ) -> Result<(), PortError> {
        for event in aggregate.take_domain_events() {
            self.publish(event)?;
        }
        Ok(())
    }
}

/// Publishes integration events for other bounded contexts.
pub trait IntegrationEventPublisher<E: IntegrationEvent>: OutputPort {
    /// Publishes one event.
    fn publish(&self, event: E) -> Result<(), PortError>;
}
