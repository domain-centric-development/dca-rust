//! Tactical building blocks: values, identities, entities, aggregates, events, services, failures.

use core::fmt::Debug;
use core::hash::Hash;

/// A value object: equal when its attributes are equal, never changed after it is built.
///
/// `PartialEq` as supertrait makes a value without equality unkompilable.
pub trait Value: PartialEq + Clone + Debug {}

/// A typed identity. An entity is identified by one, never by a bare string or number.
pub trait Id: Value + Eq + Hash {}

/// An object with an identity that stays the same while its state changes.
pub trait Entity {
    /// The entity's typed identity.
    type Id: Id;

    /// The identity.
    fn id(&self) -> &Self::Id;

    /// Whether `other` is the same entity, whatever state either is in.
    fn same_identity_as(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

/// The root of an aggregate: the only entry for changes, and the collector of what happened.
pub trait AggregateRoot: Entity {
    /// The events this aggregate records.
    type Event: DomainEvent;

    /// The events recorded since they were last taken.
    fn domain_events(&self) -> &[Self::Event];

    /// Hands the recorded events over and forgets them.
    fn take_domain_events(&mut self) -> Vec<Self::Event>;
}

/// An identity for an event, the 128 bits of a UUID without depending on a UUID crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId(u128);

impl EventId {
    /// An event id from its 128 bits.
    pub const fn from_u128(bits: u128) -> Self {
        Self(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for EventId {}
impl Id for EventId {}

/// A point in time, milliseconds since the Unix epoch, UTC.
///
/// The domain receives it as a value; it never reads a clock itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    /// A timestamp from milliseconds since the Unix epoch.
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Milliseconds since the Unix epoch.
    pub const fn as_unix_millis(&self) -> i64 {
        self.0
    }
}

impl Value for Timestamp {}

/// Something that happened in the domain, recorded by an aggregate.
pub trait DomainEvent: Clone + Debug {
    /// The event's identity.
    fn event_id(&self) -> EventId;

    /// When it happened.
    fn occurred_on(&self) -> Timestamp;
}

/// A published fact another bounded context may consume; its type and schema version are part of
/// the contract and live in the type, not in the payload.
pub trait IntegrationEvent: Clone + Debug {
    /// The event type as consumers know it.
    const TYPE: &'static str;

    /// The schema version of the payload.
    const VERSION: u32;

    /// The event's identity.
    fn event_id(&self) -> EventId;

    /// When it happened.
    fn occurred_on(&self) -> Timestamp;
}

/// Domain logic that belongs to no single entity or value and works on aggregates or entities.
pub trait DomainService {}

/// Builds an aggregate or entity whose construction is too involved for the type itself.
pub trait Factory {}

/// A predicate over a domain object, named in the ubiquitous language.
pub trait Specification<T> {
    /// Whether the candidate satisfies the specification.
    fn is_satisfied_by(&self, candidate: &T) -> bool;
}

/// A failure the domain names: an invariant the operation would break.
pub trait DomainFailure: std::error::Error {}
