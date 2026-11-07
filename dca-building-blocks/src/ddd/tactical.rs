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

/// An aggregate root that belongs to someone: reachable only through its owner.
///
/// Its repository declares a lookup that takes the aggregate's id *and* the owner's id, and a use
/// case acting for a caller loads it through that lookup - never by id alone. An object loaded by
/// its id alone is reachable by anyone who knows or guesses the id: the insecure direct object
/// reference of broken access control (OWASP Top 10, A01).
///
/// The associated type names the owner's id type: `impl Owned for MedicalRecord { type OwnerId =
/// PatientId; }`, `impl Owned for Workspace { type OwnerId = TenantId; }`. The trait declares no
/// method; the owner is a field of the aggregate like any other.
///
/// What it says, and what not:
///
/// - "Has an owner", not "always the same owner". The owner may change, but only through an
///   explicit operation of the aggregate - a draft a visitor started as a guest, taken over by the
///   account they sign into - called after the use case proved the caller's right to the change.
///   That proof is logic of the use case, which no marker can verify; its tests prove it.
/// - An owner need not be an account. A guest given a pseudonymous identity on the first request
///   is an owner: the aggregate belongs to that identity, provided it comes from a signed token and
///   is not guessable, never from a value the client names.
/// - Where no identity exists and the aggregate's id itself is the secret - a long random id in a
///   link, "whoever holds it may" - there is no owner to scope a lookup by, and the aggregate is not
///   `Owned`. Its safety is the id's unguessability.
///
/// ```ignore
/// impl Owned for MedicalRecord {
///     type OwnerId = PatientId;
/// }
///
/// pub trait MedicalRecordRepository: Repository<MedicalRecord> {
///     fn find_by_id_for_patient(&self, id: &MedicalRecordId, patient: &PatientId)
///         -> Result<Option<MedicalRecord>, PortError>;
/// }
/// ```
///
/// Forgetting the `impl Owned` silences every rule that selects on it; marking the aggregates that
/// belong to someone is part of modelling them.
pub trait Owned: AggregateRoot {
    /// The id type of the owner.
    type OwnerId: Id;
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
