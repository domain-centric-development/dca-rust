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

/// A point in time on the UTC time line: seconds since the Unix epoch and the nanoseconds within the
/// second, the precision of Java's `Instant`.
///
/// The domain receives it as a value; it never reads a clock itself. An adapter's clock converts
/// the operating system's time with `Timestamp::from(SystemTime::now())`.
///
/// ```
/// use dca_building_blocks::ddd::tactical::Timestamp;
///
/// let t = Timestamp::from_unix(1_700_000_000, 250_000_000);
/// assert_eq!(t.as_unix_millis(), 1_700_000_000_250);
/// assert_eq!(Timestamp::from_unix_millis(-1).subsec_nanos(), 999_000_000);
/// assert!(Timestamp::from_unix_millis(1) > Timestamp::from_unix(0, 999_999));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp {
    seconds: i64,
    nanos: u32,
}

const NANOS_PER_SECOND: u32 = 1_000_000_000;

impl Timestamp {
    /// A timestamp from seconds since the Unix epoch and the nanoseconds within that second.
    ///
    /// # Panics
    ///
    /// When `nanos` is a second or more.
    pub const fn from_unix(seconds: i64, nanos: u32) -> Self {
        assert!(
            nanos < NANOS_PER_SECOND,
            "nanos within a second are below 1_000_000_000"
        );
        Self { seconds, nanos }
    }

    /// A timestamp from milliseconds since the Unix epoch.
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self {
            seconds: millis.div_euclid(1000),
            nanos: millis.rem_euclid(1000) as u32 * 1_000_000,
        }
    }

    /// Whole seconds since the Unix epoch, rounded down.
    pub const fn unix_seconds(&self) -> i64 {
        self.seconds
    }

    /// The nanoseconds within the second.
    pub const fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// Milliseconds since the Unix epoch, rounded down.
    pub const fn as_unix_millis(&self) -> i64 {
        self.seconds * 1000 + (self.nanos / 1_000_000) as i64
    }
}

impl From<std::time::SystemTime> for Timestamp {
    fn from(time: std::time::SystemTime) -> Self {
        match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(since) => Self {
                seconds: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
                nanos: since.subsec_nanos(),
            },
            Err(before) => {
                let before = before.duration();
                let seconds = i64::try_from(before.as_secs()).unwrap_or(i64::MAX);
                match before.subsec_nanos() {
                    0 => Self {
                        seconds: -seconds,
                        nanos: 0,
                    },
                    n => Self {
                        seconds: -seconds - 1,
                        nanos: NANOS_PER_SECOND - n,
                    },
                }
            }
        }
    }
}

impl From<Timestamp> for std::time::SystemTime {
    fn from(t: Timestamp) -> Self {
        let epoch = std::time::UNIX_EPOCH;
        let whole = std::time::Duration::from_secs(t.seconds.unsigned_abs());
        let within = std::time::Duration::from_nanos(u64::from(t.nanos));
        if t.seconds >= 0 {
            epoch + whole + within
        } else {
            epoch - whole + within
        }
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
    use super::{DomainEvents, Timestamp};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    #[test]
    fn a_timestamp_keeps_nanoseconds_and_converts_to_system_time_both_ways() {
        for t in [
            Timestamp::from_unix(1_700_000_000, 123_456_789),
            Timestamp::from_unix(-5, 1),
            Timestamp::from_unix(-5, 0),
            Timestamp::from_unix_millis(-1),
        ] {
            assert_eq!(Timestamp::from(SystemTime::from(t)), t);
        }
        let before = UNIX_EPOCH - Duration::from_millis(1500);
        assert_eq!(Timestamp::from(before), Timestamp::from_unix_millis(-1500));
    }
}
