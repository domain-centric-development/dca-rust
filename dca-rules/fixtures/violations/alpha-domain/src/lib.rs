use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dca_building_blocks::ddd::strategic::OpenHostService;
use dca_building_blocks::ddd::tactical::*;
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository, Store};

pub mod utils {
    pub struct Bucket;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThingId(u128);
impl Value for ThingId {}
impl Id for ThingId {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OtherId(u128);
impl Value for OtherId {}
impl Id for OtherId {}

pub struct Other {
    id: OtherId,
}
impl Entity for Other {
    type Id = OtherId;
    fn id(&self) -> &OtherId { &self.id }
}
impl AggregateRoot for Other {
    type Event = ThingEvent;
    fn domain_events(&self) -> &[ThingEvent] { &[] }
    fn take_domain_events(&mut self) -> Vec<ThingEvent> { vec![] }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Thing {
    id: ThingId,
    pub label: String,
    other: Other,
    port: Box<dyn OutputPort>,
    shared: Rc<RefCell<u32>>,
    parts: Vec<Part>,
}

impl Thing {
    pub fn parts_mut(&mut self) -> &mut Vec<Part> { &mut self.parts }
    pub async fn load(&self) {}
    pub fn rename(&self, service: &Calculator) { service.calculate(); }
}
impl Entity for Thing {
    type Id = ThingId;
    fn id(&self) -> &ThingId { &self.id }
}
impl AggregateRoot for Thing {
    type Event = ThingEvent;
    fn domain_events(&self) -> &[ThingEvent] { &[] }
    fn take_domain_events(&mut self) -> Vec<ThingEvent> { vec![] }
}

pub struct Part {
    pub id: ThingId,
    owner: Other,
}
impl Part {
    pub fn new() -> Self { todo!() }
    pub fn set_name(&mut self, name: Name) {}
}
impl Entity for Part {
    type Id = ThingId;
    fn id(&self) -> &ThingId { &self.id }
}

pub struct NoId {
    name: Name,
}
impl Entity for NoId {
    type Id = ThingId;
    fn id(&self) -> &ThingId { todo!() }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Name {
    pub text: Title,
    part: Option<Part>,
    cache: Cell<u8>,
}
impl Name {
    pub fn set_text(&mut self, text: Title) {}
    pub fn new(text: Title) -> Self { todo!() }
    pub fn from_input(text: Title) -> Self { todo!() }
}
impl Value for Name {}

#[derive(Debug, Clone, PartialEq)]
pub struct Title(String);
impl Value for Title {}

pub struct ThingAggregateRoot;

pub trait LocalPort: OutputPort {}
pub trait ThingRepository: Repository<Thing> {}
pub trait DomainStore: Store {}

pub struct MemoryRepository;
impl OutputPort for MemoryRepository {}
impl Repository<Thing> for MemoryRepository {}
impl ThingRepository for MemoryRepository {}

pub struct ThingHelper;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThingEvent {
    schema_version: u32,
    cache: Cell<u8>,
}
impl DomainEvent for ThingEvent {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}
impl ThingEvent {
    pub fn touch(&mut self) {}
}

#[derive(sqlx::FromRow)]
pub struct Calculator {
    count: Cell<u32>,
}
impl DomainService for Calculator {}
impl Calculator {
    pub fn calculate(&self) {}
}

#[derive(sqlx::FromRow)]
pub struct Maker;
impl Factory for Maker {}
impl Maker {
    pub fn tweak(&mut self) {}
}

#[derive(sqlx::FromRow)]
pub struct ActiveSpecification;

#[derive(sqlx::FromRow)]
pub struct Stored {
    id: ThingId,
}

#[derive(Debug)]
pub struct Broken;
impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}
impl std::error::Error for Broken {}

#[derive(Debug)]
pub struct HttpRefusal;
impl std::error::Error for HttpRefusal {}
impl DomainFailure for HttpRefusal {}

pub struct Gate;
impl OpenHostService for Gate {}

pub struct ThingDto;
pub struct DoCommand;
pub struct FindQuery;
pub struct DoneResult;
pub struct ThingResponse;
pub struct ThingConverter;
pub struct ThingViewModel;

pub struct Holder {
    dto: ThingDto,
}

#[derive(Debug, Clone)]
pub enum ThingEvents {
    Changed { at: Timestamp },
}
impl DomainEvent for ThingEvents {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}
