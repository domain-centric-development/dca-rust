use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dca_building_blocks::ddd::strategic::OpenHostService;
use dca_building_blocks::ddd::tactical::*;
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository, Store};

pub mod utils {
    pub struct Bucket;
}

/// DCA-LAY-004: the domain runs its own transaction.
pub fn atomically(boundary: &dyn dca_building_blocks::application::TransactionBoundary) {
    let _ = boundary;
}

/// DCA-CYC-001: two domain modules that name each other.
pub mod left {
    pub struct Left(pub crate::right::Right);
}

pub mod right {
    pub struct Right;

    pub fn unwrap(left: crate::left::Left) -> Right {
        left.0
    }
}

/// DCA-CYC-001 through a `use` inside a function body and a path inside `assert_eq!`.
pub mod up {
    pub fn lift() -> u8 {
        use crate::down::sink;
        sink()
    }
}

pub mod down {
    pub fn sink() -> u8 {
        0
    }

    pub fn check() {
        assert_eq!(crate::up::lift(), 0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThingId(u128);
impl Value for ThingId {}
impl Id for ThingId {}

/// The one a thing belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeeperId(u128);
impl Value for KeeperId {}
impl Id for KeeperId {}

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
/// DCA-TAC-023: a root built through `Default` and a conversion.
impl Default for Other {
    fn default() -> Self { todo!() }
}
impl From<OtherId> for Other {
    fn from(id: OtherId) -> Self { Other { id } }
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
    pub fn restore(id: ThingId) -> Self { todo!() }
    pub fn parts_mut(&mut self) -> &mut Vec<Part> { &mut self.parts }
    pub async fn load(&self) {}
    pub fn rename(&self, service: &Calculator) { service.calculate(); }
}
impl Entity for Thing {
    type Id = ThingId;
    fn id(&self) -> &ThingId { &self.id }
}
/// Owned, but `WrongRepository` loads it by id alone.
impl Owned for Thing {
    type OwnerId = KeeperId;
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
/// DCA-TAC-005: and through a conversion.
impl TryFrom<u128> for Part {
    type Error = Broken;
    fn try_from(id: u128) -> Result<Self, Broken> { todo!() }
}
impl Part {
    pub fn new() -> Self { todo!() }
    pub fn set_name(&mut self, name: Name) {}
}
impl Entity for Part {
    type Id = ThingId;
    fn id(&self) -> &ThingId { &self.id }
}

/// DCA-TAC-005: an entity built from outside through a derived `Default`.
#[derive(Default)]
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
    pub fn new() -> Self { Calculator { count: Cell::new(0) } }
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

/// DCA-ERR-009: a domain failure only a use case can raise - the domain never searches.
#[derive(Debug)]
pub enum ThingRefused {
    Unknown(u128),
    Gone,
}
impl std::fmt::Display for ThingRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }
}
impl std::error::Error for ThingRefused {}
impl DomainFailure for ThingRefused {}

/// DCA-TAC-027: the model receives a gateway only as a parameter - `price` passes; a field, a
/// static, a locator, a method handing one out and a construction do not.
pub trait Rates: DomainGateway {
    fn rate(&self) -> u32;
}

pub struct FixedRates;
impl DomainGateway for FixedRates {}
impl Rates for FixedRates {
    fn rate(&self) -> u32 {
        1
    }
}

pub static RATES: FixedRates = FixedRates;

pub fn rates() -> FixedRates {
    FixedRates
}

impl Thing {
    pub fn price<R: Rates>(&self, rates: &R) -> u32 {
        rates.rate()
    }
    pub fn price_from_static(&self) -> u32 {
        RATES.rate()
    }
    pub fn price_from_locator(&self) -> u32 {
        rates().rate()
    }
    pub fn rate_source(&self) -> &'static FixedRates {
        &RATES
    }
}

/// DCA-TAC-027: a gateway read from a field of a value handed in, and one obtained from a trait's
/// associated function through a generic parameter and through the trait's path.
pub struct RateContext<R: Rates> {
    pub rates: R,
}
pub trait RateLocator {
    fn locate() -> FixedRates;
}
impl Thing {
    pub fn price_from_context<R: Rates>(&self, ctx: &RateContext<R>) -> u32 {
        ctx.rates.rate()
    }
    pub fn price_from_trait<L: RateLocator>(&self) -> u32 {
        L::locate().rate()
    }
    pub fn price_from_qualified(&self) -> u32 {
        <FixedRates as RateLocator>::locate().rate()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quote<R: Rates> {
    rates: R,
}
impl<R: Rates> Value for Quote<R> {}
impl<R: Rates> Quote<R> {
    pub fn fixed(&self) -> u32 {
        FixedRates.rate()
    }
}

/// DCA-ONI-004: an async runtime named in the domain.
pub struct Waiter {
    lock: tokio::sync::Mutex<u8>,
}

/// DCA-HEX-016: an enum an adapter should map, not print.
#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Open,
    Closed,
}

/// DCA-ERR-007: a failure that carries prose and the aggregate itself.
#[derive(Debug)]
pub struct LabelRefused {
    reason: String,
    thing: Thing,
}
impl std::error::Error for LabelRefused {}
impl DomainFailure for LabelRefused {}

/// DCA-TAC-025: a yes/no fact kept in a private `bool` field passes; a `bool` parameter and a `pub`
/// `bool` field are flag arguments. The reconstituting factory `restore` takes its `bool` - its bare
/// count is reported.
pub struct Lamp {
    lit: bool,
    pub dimmed: bool,
}
impl Lamp {
    pub fn restore(lit: bool, dimmed: bool, hours: u32) -> Self { let _ = hours; Lamp { lit, dimmed } }
    pub fn switch(&mut self, lit: bool) { self.lit = lit; }
    pub fn is_lit(&self) -> bool { self.lit }
    fn dim_to(&mut self, level: u8) { self.dimmed = level > 0; }
}

/// DCA-TAC-025: booleans in disguise - one fact and its negation.
pub enum Visibility { Visible, NotVisible }
pub enum Power { On, Off }
pub enum Latch { Locked, Unlocked }

/// DCA-TAC-025: a domain trait whose method takes and returns bare values, asynchronously too.
pub trait Weigher {
    fn weigh(&self, grams: u32) -> String;
    fn tare(&self) -> impl core::future::Future<Output = u64> + Send;
}

/// DCA-TAC-006: a setter an entity takes from a trait of the workspace, written in the impl and as
/// the trait's default. `Display` is no trait of the workspace and is not read.
pub trait Relabel {
    fn set_label(&mut self, label: Title);
}
impl Relabel for Part {
    fn set_label(&mut self, label: Title) { let _ = label; }
}
pub trait Retitle {
    fn set_title(&mut self, title: Title) { let _ = title; }
}
impl Retitle for Part {}

/// DCA-ADV-020: a domain service operation from a trait of the workspace that takes no aggregate.
pub trait Pricing {
    fn quote_for(&self, label: Title) -> Title;
}
impl Pricing for Calculator {
    fn quote_for(&self, label: Title) -> Title { label }
}

/// DCA-NAM-007/008/010: traits carry the technical suffixes too.
pub trait PartDto {}
pub trait PartConverter {}
pub trait PartUtil {}

/// DCA-STR-005: a trait carrying the open host service role, declared in the domain.
pub trait Window: OpenHostService {}

/// DCA-ADV-004: framework metadata on an event's variant. DCA-ADV-011: on a service's impl block
/// and on its method.
#[derive(Debug, Clone)]
pub enum Moved {
    #[sqlx(rename = "away")]
    Away(ThingEvent),
}
impl DomainEvent for Moved {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}

pub struct Mover;
impl DomainService for Mover {}
#[sqlx::instrument]
impl Mover {
    #[sqlx::trace]
    pub fn shift(&self, thing: &Thing) { let _ = thing; }
}

/// DCA-ADV-019: an entity that holds a domain service in a field and calls it.
pub struct Bin {
    id: ThingId,
    mover: Mover,
}
impl Entity for Bin {
    type Id = ThingId;
    fn id(&self) -> &ThingId { &self.id }
}
impl Bin {
    pub fn tidy(&self) { self.mover.shift(todo!()); }
}

/// DCA-ONI-004: a future taken, held and handed out outside a gateway and a domain service.
pub struct Pending {
    work: core::pin::Pin<Box<dyn core::future::Future<Output = ()>>>,
}
pub fn wait_for(work: impl core::future::Future<Output = ()>) { let _ = work; }
pub fn later() -> dca_building_blocks::application::BoxFuture<'static, ()> { todo!() }

/// DCA-ERR-001: a derived error in the domain that is no domain failure.
#[derive(Debug, thiserror::Error)]
pub enum Jammed {
    Stuck,
}

/// DCA-ERR-004: a domain failure that carries a framework derive.
#[derive(Debug, sqlx::Type)]
pub struct Spilled;
impl std::error::Error for Spilled {}
impl DomainFailure for Spilled {}

/// DCA-ERR-007: only `extra` and `label` are listed; ids, values, numbers, enums, time types, the
/// wrapped failure and containers of those pass.
#[derive(Debug)]
pub struct Overweight {
    limit: u32,
    thing: ThingId,
    phase: Phase,
    at: Timestamp,
    raw: Vec<u8>,
    names: Option<Name>,
    cause: ThingRefused,
    extra: Holder,
    label: Box<str>,
}
impl std::error::Error for Overweight {}
impl DomainFailure for Overweight {}

/// DCA-TAC-004: a field that holds ids is not the id; an enum entity has its id in every variant.
pub struct Pile {
    ids: Vec<ThingId>,
}
impl Entity for Pile {
    type Id = ThingId;
    fn id(&self) -> &ThingId { todo!() }
}
pub enum Slot {
    Filled { id: ThingId, part: Title },
    Empty,
}
impl Entity for Slot {
    type Id = ThingId;
    fn id(&self) -> &ThingId { todo!() }
}

/// DCA-TAC-024: `new` beside a named factory that takes the same types, written differently.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag(String);
impl Value for Tag {}
impl Tag {
    pub fn new(text: &str, note: String) -> Self { todo!() }
    pub fn from_text<'a>(text: &'a str, note: std::string::String) -> Self { todo!() }
}
