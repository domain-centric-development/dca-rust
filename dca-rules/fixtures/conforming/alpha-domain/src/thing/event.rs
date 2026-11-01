use dca_building_blocks::ddd::tactical::{DomainEvent, EventId, Timestamp};

use super::{PartId, ThingId};

#[derive(Debug, Clone)]
pub struct ThingCreated {
    pub event: EventId,
    pub thing: ThingId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for ThingCreated {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

#[derive(Debug, Clone)]
pub struct PartAdded {
    pub event: EventId,
    pub thing: ThingId,
    pub part: PartId,
    pub occurred_on: Timestamp,
}

impl DomainEvent for PartAdded {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

#[derive(Debug, Clone)]
pub enum ThingEvent {
    ThingCreated(ThingCreated),
    PartAdded(PartAdded),
}

impl DomainEvent for ThingEvent {
    fn event_id(&self) -> EventId {
        match self {
            ThingEvent::ThingCreated(e) => e.event_id(),
            ThingEvent::PartAdded(e) => e.event_id(),
        }
    }
    fn occurred_on(&self) -> Timestamp {
        match self {
            ThingEvent::ThingCreated(e) => e.occurred_on(),
            ThingEvent::PartAdded(e) => e.occurred_on(),
        }
    }
}
