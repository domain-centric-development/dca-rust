mod event;
mod failure;
mod part;

pub use event::{PartAdded, ThingCreated, ThingEvent};
pub use failure::ThingError;
pub use part::Part;

use dca_building_blocks::ddd::tactical::{AggregateRoot, Entity, EventId, Id, Owned, Timestamp, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThingId(u128);
impl Value for ThingId {}
impl Id for ThingId {}

/// The one a thing belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeeperId(u128);
impl Value for KeeperId {}
impl Id for KeeperId {}

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
    keeper: KeeperId,
    parts: Vec<Part>,
    archived: bool,
    events: Vec<ThingEvent>,
}

impl Thing {
    pub fn create(id: ThingId, keeper: KeeperId, event: EventId, at: Timestamp) -> Thing {
        Thing { id, keeper, parts: Vec::new(), archived: false, events: vec![ThingEvent::ThingCreated(ThingCreated { event, thing: id, occurred_on: at })] }
    }

    /// Rebuilds a stored thing; its yes/no fact comes back as the `bool` it was.
    pub fn reconstitute(id: ThingId, keeper: KeeperId, parts: Vec<Part>, archived: bool) -> Thing {
        Thing { id, keeper, parts, archived, events: Vec::new() }
    }

    pub fn add_part(&mut self, part: PartId, label: Label, event: EventId, at: Timestamp) -> Result<(), ThingError> {
        if self.parts.iter().any(|p| p.label() == &label) {
            return Err(ThingError::DuplicateLabel);
        }
        self.parts.push(Part::new(part, label));
        self.events.push(ThingEvent::PartAdded(PartAdded { event, thing: self.id, part, occurred_on: at }));
        Ok(())
    }

    pub fn keeper(&self) -> &KeeperId {
        &self.keeper
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    pub fn archive(&mut self) {
        self.archived = true;
    }

    pub fn is_archived(&self) -> bool {
        self.archived
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

impl Owned for Thing {
    type OwnerId = KeeperId;
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
