use dca_building_blocks::ddd::tactical::{DomainEvent, EventId, Timestamp};
use kernel_domain::{CopyId, TitleId};

use super::LoanId;
use crate::member::MemberId;
use crate::reservation::ReservationId;

/// A copy arrived on the shelf, ready to be lent.
#[derive(Debug, Clone)]
pub struct CopyShelved {
    pub event: EventId,
    pub copy: CopyId,
    pub title: TitleId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for CopyShelved {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// A member borrowed a copy, collecting their reservation if they name one.
#[derive(Debug, Clone)]
pub struct CopyLent {
    pub event: EventId,
    pub copy: CopyId,
    pub loan: LoanId,
    pub member: MemberId,
    pub reservation: Option<ReservationId>,
    pub due: Timestamp,
    pub occurred_on: Timestamp,
}

impl DomainEvent for CopyLent {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// A copy came back.
#[derive(Debug, Clone)]
pub struct CopyReturned {
    pub event: EventId,
    pub copy: CopyId,
    pub loan: LoanId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for CopyReturned {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// What a lendable copy records.
#[derive(Debug, Clone)]
pub enum CopyEvent {
    CopyShelved(CopyShelved),
    CopyLent(CopyLent),
    CopyReturned(CopyReturned),
}

impl CopyEvent {
    /// The copy the event is about.
    pub fn copy(&self) -> &CopyId {
        match self {
            CopyEvent::CopyShelved(e) => &e.copy,
            CopyEvent::CopyLent(e) => &e.copy,
            CopyEvent::CopyReturned(e) => &e.copy,
        }
    }
}

impl DomainEvent for CopyEvent {
    fn event_id(&self) -> EventId {
        match self {
            CopyEvent::CopyShelved(e) => e.event_id(),
            CopyEvent::CopyLent(e) => e.event_id(),
            CopyEvent::CopyReturned(e) => e.event_id(),
        }
    }
    fn occurred_on(&self) -> Timestamp {
        match self {
            CopyEvent::CopyShelved(e) => e.occurred_on(),
            CopyEvent::CopyLent(e) => e.occurred_on(),
            CopyEvent::CopyReturned(e) => e.occurred_on(),
        }
    }
}
