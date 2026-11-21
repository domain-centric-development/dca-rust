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

/// The events an aggregate recorded and has not handed over yet: a field of the aggregate root,
/// to which its [`AggregateRoot`] methods delegate.
///
/// It records, shows what it holds and hands it over; it decides nothing. The aggregate records an
/// event in the method that changes its state, the use case hands the events on after saving
/// (`publish_and_clear_events`), and a rebuilt aggregate starts with none.
///
/// ```
/// use dca_building_blocks::ddd::tactical::{
///     AggregateRoot, DomainEvent, DomainEvents, Entity, EventId, Id, Timestamp, Value,
/// };
///
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// pub struct ShelfId(u128);
/// impl Value for ShelfId {}
/// impl Id for ShelfId {}
///
/// #[derive(Debug, Clone)]
/// pub struct ShelfEmptied {
///     event: EventId,
///     occurred_on: Timestamp,
/// }
/// impl DomainEvent for ShelfEmptied {
///     fn event_id(&self) -> EventId { self.event }
///     fn occurred_on(&self) -> Timestamp { self.occurred_on }
/// }
///
/// pub struct Shelf {
///     id: ShelfId,
///     events: DomainEvents<ShelfEmptied>,
/// }
///
/// impl Shelf {
///     pub fn empty(&mut self, event: EventId, at: Timestamp) {
///         self.events.record(ShelfEmptied { event, occurred_on: at });
///     }
/// }
///
/// impl Entity for Shelf {
///     type Id = ShelfId;
///     fn id(&self) -> &ShelfId { &self.id }
/// }
///
/// impl AggregateRoot for Shelf {
///     type Event = ShelfEmptied;
///     fn domain_events(&self) -> &[ShelfEmptied] { &self.events }
///     fn take_domain_events(&mut self) -> Vec<ShelfEmptied> { self.events.take() }
/// }
///
/// let mut shelf = Shelf { id: ShelfId(1), events: DomainEvents::new() };
/// shelf.empty(EventId::from_u128(7), Timestamp::from_unix_millis(0));
/// assert_eq!(shelf.domain_events().len(), 1);
/// assert_eq!(shelf.take_domain_events().len(), 1);
/// assert!(shelf.domain_events().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEvents<E> {
    recorded: Vec<E>,
}

impl<E> DomainEvents<E> {
    /// No events recorded.
    pub const fn new() -> Self {
        Self {
            recorded: Vec::new(),
        }
    }

    /// Records one event, after the ones recorded before it.
    pub fn record(&mut self, event: E) {
        self.recorded.push(event);
    }

    /// The events recorded, in the order they were recorded.
    pub fn as_slice(&self) -> &[E] {
        &self.recorded
    }

    /// Hands the recorded events over, in the order they were recorded, and forgets them.
    pub fn take(&mut self) -> Vec<E> {
        core::mem::take(&mut self.recorded)
    }
}

impl<E> Default for DomainEvents<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> core::ops::Deref for DomainEvents<E> {
    type Target = [E];

    fn deref(&self) -> &[E] {
        &self.recorded
    }
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
///
/// An event is data that outlives the call that recorded it and moves between threads - into the
/// outbox, to a relay, to a listener - so it is `Send + Sync + 'static`: it owns what it holds.
pub trait DomainEvent: Clone + Debug + Send + Sync + 'static {
    /// The event's identity.
    fn event_id(&self) -> EventId;

    /// When it happened.
    fn occurred_on(&self) -> Timestamp;
}

/// A published fact another bounded context may consume; its type and schema version are part of
/// the contract and live in the type, not in the payload.
///
/// Like a [`DomainEvent`] it owns what it holds and moves between threads.
pub trait IntegrationEvent: Clone + Debug + Send + Sync + 'static {
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
///
/// A domain service may be asynchronous - the one part of the domain that is: when it needs a fact
/// the model does not hold, it asks a [`DomainGateway`] and awaits the answer. The aggregates,
/// entities, values, events, failures, factories and specifications stay synchronous; the service
/// hands them what the gateway answered.
pub trait DomainService {}

/// A port the domain declares to obtain a fact it does not hold, or to delegate a computation bound
/// to a technology - a password hash, a tax rate, an availability - in the domain's own language.
/// The trait sits in the domain crate; its implementation sits in an outgoing adapter.
///
/// **It never writes to the outside.** A gateway enriches the model so the model can decide; it
/// creates, changes or removes no external state. Persisting, sending and publishing are the
/// application's, through its output ports.
///
/// Its methods take `&self` - a gateway only reads - and are synchronous for a computation in the
/// process, asynchronous for a fact from outside:
///
/// ```
/// use std::future::Future;
///
/// use dca_building_blocks::ddd::tactical::DomainGateway;
///
/// # pub struct Region; pub struct Rate; pub struct Password; pub struct Digest;
/// pub trait TaxRates: DomainGateway {
///     fn rate_for(&self, region: &Region) -> impl Future<Output = Option<Rate>> + Send;
/// }
///
/// pub trait PasswordHasher: DomainGateway {
///     fn digest(&self, password: &Password) -> Digest;
/// }
/// ```
///
/// Who calls it: a [`DomainService`], the use case that hands the service the gateway, or a method
/// of an aggregate, entity, value, factory or specification that receives the gateway as a
/// parameter. The model is synchronous, so it calls only a synchronous gateway; a domain service
/// awaits an asynchronous one and hands the model the answer. The model never holds a gateway in a
/// field, never hands one out, and never looks one up - no registry, no static, no construction in
/// place.
pub trait DomainGateway {}

/// Builds an aggregate or entity whose construction is too involved for the type itself.
pub trait Factory {}

/// A predicate over a domain object, named in the ubiquitous language.
pub trait Specification<T> {
    /// Whether the candidate satisfies the specification.
    fn is_satisfied_by(&self, candidate: &T) -> bool;
}

/// A failure the domain names: an invariant the operation would break.
pub trait DomainFailure: std::error::Error {}

#[cfg(test)]
mod tests {
    use super::DomainEvents;

    #[test]
    fn records_in_order_and_shows_what_it_holds() {
        let mut events = DomainEvents::new();
        events.record("first");
        events.record("second");
        assert_eq!(events.as_slice(), ["first", "second"]);
        assert_eq!(events.len(), 2);
        assert_eq!(events.first(), Some(&"first"));
    }

    #[test]
    fn take_hands_the_events_over_and_forgets_them() {
        let mut events = DomainEvents::default();
        events.record(1);
        events.record(2);
        assert_eq!(events.take(), vec![1, 2]);
        assert!(events.is_empty());
        assert!(events.take().is_empty());
    }

    #[test]
    fn a_clone_holds_the_same_events_and_stays_apart() {
        let mut events = DomainEvents::new();
        events.record(1);
        let copy = events.clone();
        events.record(2);
        assert_eq!(copy.as_slice(), [1]);
        assert_eq!(format!("{copy:?}"), "DomainEvents { recorded: [1] }");
    }

    #[test]
    fn needs_no_default_of_the_event() {
        struct Plain;
        let events: DomainEvents<Plain> = DomainEvents::default();
        assert!(events.is_empty());
    }
}
