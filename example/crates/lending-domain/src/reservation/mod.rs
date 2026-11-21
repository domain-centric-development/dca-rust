//! A member's reservation of a copy: it belongs to the member and is reached only through them.

use dca_building_blocks::ddd::tactical::{
    AggregateRoot, DomainEvent, DomainEvents, Entity, EventId, Id, Owned, Timestamp, Value,
};
use kernel_domain::CopyId;

use crate::failure::LendingFailure;
use crate::member::MemberId;

/// A reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReservationId(u128);

impl ReservationId {
    /// The identity from its 128 bits.
    pub const fn from_u128(bits: u128) -> ReservationId {
        ReservationId(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for ReservationId {}
impl Id for ReservationId {}

/// A member reserved a copy.
#[derive(Debug, Clone)]
pub struct ReservationPlaced {
    pub event: EventId,
    pub reservation: ReservationId,
    pub member: MemberId,
    pub copy: CopyId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for ReservationPlaced {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// The member collected the reserved copy.
#[derive(Debug, Clone)]
pub struct ReservationFulfilled {
    pub event: EventId,
    pub reservation: ReservationId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for ReservationFulfilled {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// What a reservation records.
#[derive(Debug, Clone)]
pub enum ReservationEvent {
    ReservationPlaced(ReservationPlaced),
    ReservationFulfilled(ReservationFulfilled),
}

impl ReservationEvent {
    /// The reservation the event is about.
    pub fn reservation(&self) -> &ReservationId {
        match self {
            ReservationEvent::ReservationPlaced(e) => &e.reservation,
            ReservationEvent::ReservationFulfilled(e) => &e.reservation,
        }
    }
}

impl DomainEvent for ReservationEvent {
    fn event_id(&self) -> EventId {
        match self {
            ReservationEvent::ReservationPlaced(e) => e.event_id(),
            ReservationEvent::ReservationFulfilled(e) => e.event_id(),
        }
    }
    fn occurred_on(&self) -> Timestamp {
        match self {
            ReservationEvent::ReservationPlaced(e) => e.occurred_on(),
            ReservationEvent::ReservationFulfilled(e) => e.occurred_on(),
        }
    }
}

/// A member's reservation of a copy.
#[derive(Debug, Clone)]
pub struct Reservation {
    id: ReservationId,
    member: MemberId,
    copy: CopyId,
    fulfilled: bool,
    events: DomainEvents<ReservationEvent>,
}

impl Reservation {
    /// A member reserves a copy.
    pub fn place(
        id: ReservationId,
        member: MemberId,
        copy: CopyId,
        event: EventId,
        at: Timestamp,
    ) -> Reservation {
        let mut reservation = Reservation {
            id,
            member,
            copy,
            fulfilled: false,
            events: DomainEvents::new(),
        };
        reservation
            .events
            .record(ReservationEvent::ReservationPlaced(ReservationPlaced {
                event,
                reservation: id,
                member,
                copy,
                occurred_on: at,
            }));
        reservation
    }

    /// The member it belongs to.
    pub fn member(&self) -> &MemberId {
        &self.member
    }

    /// The copy reserved.
    pub fn copy(&self) -> &CopyId {
        &self.copy
    }

    /// Whether the member collected the copy.
    pub fn is_fulfilled(&self) -> bool {
        self.fulfilled
    }

    /// The member collected the copy; a reservation is fulfilled once.
    pub fn fulfil(&mut self, event: EventId, at: Timestamp) -> Result<(), LendingFailure> {
        if self.fulfilled {
            return Err(LendingFailure::ReservationClosed);
        }
        self.fulfilled = true;
        self.events.record(ReservationEvent::ReservationFulfilled(
            ReservationFulfilled {
                event,
                reservation: self.id,
                occurred_on: at,
            },
        ));
        Ok(())
    }
}

impl Entity for Reservation {
    type Id = ReservationId;
    fn id(&self) -> &ReservationId {
        &self.id
    }
}

impl AggregateRoot for Reservation {
    type Event = ReservationEvent;
    fn domain_events(&self) -> &[ReservationEvent] {
        &self.events
    }
    fn take_domain_events(&mut self) -> Vec<ReservationEvent> {
        self.events.take()
    }
}

impl Owned for Reservation {
    type OwnerId = MemberId;
}
