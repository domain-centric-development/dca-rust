mod event;
mod failure;
mod part;

pub use event::{PartAdded, ThingCreated, ThingEvent};
pub use failure::ThingError;
pub use part::Part;

use std::future::Future;

use dca_building_blocks::ddd::tactical::{
    AggregateRoot, DomainGateway, DomainService, Entity, EventId, Id, Owned, Timestamp, Value,
};

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

/// A domain service: the use case asks it; the composition root only builds it (DCA-ADV-021).
#[derive(Debug, Clone, Copy)]
pub struct Sizing;

impl DomainService for Sizing {}

impl Sizing {
    /// The sizing with its usual limits; it names its type as the return type, not `Self`.
    pub const fn usual() -> Sizing {
        Sizing
    }

    pub fn is_crowded(&self, thing: &Thing) -> bool {
        thing.parts().len() > 1
    }

    /// Whether the label names a part the catalogue lists - a fact the thing does not hold, asked
    /// of the gateway and awaited here, in the domain service.
    pub async fn is_listed<C: PartCatalog + Sync>(&self, thing: &Thing, catalog: &C, label: &Label) -> bool {
        let _ = thing;
        catalog.lists(label).await
    }
}

/// A label's digest, computed by a gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelDigest(u64);
impl Value for LabelDigest {}

impl LabelDigest {
    pub fn new(bits: u64) -> LabelDigest {
        LabelDigest(bits)
    }
}

/// Digests a label: a synchronous domain gateway, a computation bound to a technology. The model
/// takes it as a parameter and calls it.
pub trait LabelHasher: DomainGateway {
    fn digest(&self, label: &Label) -> LabelDigest;
}

impl Label {
    pub fn digest<H: LabelHasher>(&self, hasher: &H) -> LabelDigest {
        hasher.digest(self)
    }
}

impl Thing {
    pub fn digest_label<H: LabelHasher>(&self, hasher: &H, label: &Label) -> LabelDigest {
        label.digest(hasher)
    }
}

/// Two states with names of their own: no boolean in disguise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hatch {
    Open,
    Closed,
}

/// The catalogue of the parts there are: a domain gateway, read-only, answered from outside.
pub trait PartCatalog: DomainGateway {
    fn lists(&self, label: &Label) -> impl Future<Output = bool> + Send;
}

/// An entity as a state machine: a pub enum whose variants each wrap a state with private fields,
/// so only this module builds one - no open variant, no unnamed way in.
pub enum Seal {
    Open(OpenSeal),
    Broken(BrokenSeal),
}

pub struct OpenSeal {
    id: ThingId,
}

pub struct BrokenSeal {
    id: ThingId,
}

impl Entity for Seal {
    type Id = ThingId;
    fn id(&self) -> &ThingId {
        // A block-local glob over the enum brings in its variants, nothing else of the module.
        use Seal::*;
        match self {
            Open(s) => &s.id,
            Broken(s) => &s.id,
        }
    }
}
