mod event;
mod failure;
mod part;

pub use event::{PartAdded, ThingCreated, ThingEvent};
pub use failure::ThingError;
pub use part::Part;

use dca_building_blocks::ddd::tactical::{AggregateRoot, Entity, EventId, Id, Timestamp, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThingId(u128);
impl Value for ThingId {}
impl Id for ThingId {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartId(u128);
impl Value for PartId {}
impl Id for PartId {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);
impl Label {
    pub fn new(text: &str) -> Result<Label, ThingError> {
        if text.trim().is_empty() { Err(ThingError::BlankLabel) } else { Ok(Label(text.trim().to_string())) }
    }
}
impl Value for Label {}

#[derive(Debug, Clone)]
pub struct Thing {
    id: ThingId,
    parts: Vec<Part>,
    events: Vec<ThingEvent>,
}

impl Thing {
    pub fn create(id: ThingId, event: EventId, at: Timestamp) -> Thing {
        Thing { id, parts: Vec::new(), events: vec![ThingEvent::ThingCreated(ThingCreated { event, thing: id, occurred_on: at })] }
    }

    pub fn add_part(&mut self, part: PartId, label: Label, event: EventId, at: Timestamp) -> Result<(), ThingError> {
        if self.parts.iter().any(|p| p.label() == &label) {
            return Err(ThingError::DuplicateLabel);
        }
        self.parts.push(Part::new(part, label));
        self.events.push(ThingEvent::PartAdded(PartAdded { event, thing: self.id, part, occurred_on: at }));
        Ok(())
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
}

impl PartialEq for Thing {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Entity for Thing {
    type Id = ThingId;
    fn id(&self) -> &ThingId {
        &self.id
    }
}

impl AggregateRoot for Thing {
    type Event = ThingEvent;
    fn domain_events(&self) -> &[ThingEvent] {
        &self.events
    }
    fn take_domain_events(&mut self) -> Vec<ThingEvent> {
        std::mem::take(&mut self.events)
    }
}
