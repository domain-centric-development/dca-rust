//! The output ports every context's use cases share, and the vocabulary of event delivery: which
//! listener a delivery goes to, in which order, and whether it arrived before.

use dca_building_blocks::ddd::tactical::{EventId, Timestamp};
use std::future::Future;

use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Store};

/// What time it is: the domain never reads a clock, the use case asks this port.
pub trait Clock: OutputPort {
    /// Now.
    fn now(&self) -> Timestamp;
}

/// Fresh identities, for aggregates and for events.
pub trait IdSource: OutputPort {
    /// The bits of a new identity; the use case wraps them in the id type it needs.
    fn next_id(&self) -> u128;

    /// A new event identity.
    fn next_event_id(&self) -> EventId;
}

/// The stable name of a listener that reacts after the commit: what the outbox records for a
/// delivery still to make and the inbox for one made. It names what the listener does, not a type
/// path, so a renamed or moved type keeps its pending deliveries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ListenerId(&'static str);

impl ListenerId {
    /// A listener id from its stable name.
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The name.
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

/// What orders deliveries: the entries with the same key reach their listener one after the other,
/// in the order they were registered - the identity of the aggregate whose events they are.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderingKey(String);

impl OrderingKey {
    /// A key from any stable text, such as an aggregate's identity.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

/// Which delivery a listener-driven use case runs for: the event's id and the listener's name. The
/// command carries it, and the use case hands it to the [`Inbox`] before it does anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Received {
    event: EventId,
    listener: ListenerId,
}

impl Received {
    /// The delivery of `event` to `listener`.
    pub const fn new(event: EventId, listener: ListenerId) -> Self {
        Self { event, listener }
    }
}

/// Whether a delivery reached its listener before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// New: the listener reacts.
    First,
    /// Received before: the listener has reacted already and does nothing.
    Repeated,
}

/// The record of the deliveries each listener received, kept in the listener's transaction so a
/// rolled-back reaction is received again. Delivery is at least once; this is how a repeat is
/// passed over. It appends and reads, so it is a store.
pub trait Inbox: Store {
    /// Records the delivery and answers whether it is new.
    fn receive(
        &mut self,
        received: Received,
    ) -> impl Future<Output = Result<Delivery, PortError>> + Send;
}
