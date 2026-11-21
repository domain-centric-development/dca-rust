//! The catalogue's integration events: their type and version are the contract.

use dca_building_blocks::ddd::tactical::{EventId, IntegrationEvent, Timestamp};

/// The library acquired a copy of a title; it may be lent from now on.
#[derive(Debug, Clone)]
pub struct CopyAcquired {
    pub event: EventId,
    pub title: u128,
    pub copy: u128,
    pub occurred_on: Timestamp,
}

impl IntegrationEvent for CopyAcquired {
    const TYPE: &'static str = "catalogue.copy-acquired";
    const VERSION: u32 = 1;
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}
