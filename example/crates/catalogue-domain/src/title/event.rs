use dca_building_blocks::ddd::tactical::{DomainEvent, EventId, Timestamp};
use kernel_domain::{CopyId, TitleId};

use super::{ShelfMark, TitleName};

/// A title entered the catalogue.
#[derive(Debug, Clone)]
pub struct TitleRegistered {
    pub event: EventId,
    pub title: TitleId,
    pub name: TitleName,
    pub occurred_on: Timestamp,
}

impl DomainEvent for TitleRegistered {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// The library acquired a copy of a title.
#[derive(Debug, Clone)]
pub struct CopyAcquired {
    pub event: EventId,
    pub title: TitleId,
    pub copy: CopyId,
    pub shelf_mark: ShelfMark,
    pub occurred_on: Timestamp,
}

impl DomainEvent for CopyAcquired {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

/// What a title records.
#[derive(Debug, Clone)]
pub enum TitleEvent {
    TitleRegistered(TitleRegistered),
    CopyAcquired(CopyAcquired),
}

impl TitleEvent {
    /// The title the event is about.
    pub fn title(&self) -> &TitleId {
        match self {
            TitleEvent::TitleRegistered(e) => &e.title,
            TitleEvent::CopyAcquired(e) => &e.title,
        }
    }
}

impl DomainEvent for TitleEvent {
    fn event_id(&self) -> EventId {
        match self {
            TitleEvent::TitleRegistered(e) => e.event_id(),
            TitleEvent::CopyAcquired(e) => e.event_id(),
        }
    }
    fn occurred_on(&self) -> Timestamp {
        match self {
            TitleEvent::TitleRegistered(e) => e.occurred_on(),
            TitleEvent::CopyAcquired(e) => e.occurred_on(),
        }
    }
}
