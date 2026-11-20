use std::cell::Cell;

use alpha_domain::{Part, Thing, ThingEvent};
use dca_building_blocks::application::UseCaseFailure;
use dca_building_blocks::ddd::tactical::{DomainEvent, DomainFailure, DomainService, EventId, Factory, IntegrationEvent, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository, Store};

/// DCA-CYC-002: the shared module and a use case name each other.
pub mod shared {
    pub trait Helper {
        fn query(&self) -> crate::lookup::LookupQuery;
    }
}

pub trait InputPort {}

pub trait LooseRepository {}
pub trait ThingsRepositoryPort {}
pub trait CacheStore {}
pub trait Stash: Store {
    fn save(&self);
}
pub struct Bucket;
impl OutputPort for Bucket {}
impl Store for Bucket {}

pub trait WrongRepository: Repository<Thing> {
    fn part(&self) -> Option<Part>;
}
pub trait PartRepository: Repository<Part> {}

pub struct OpenSpecification;

#[derive(Debug)]
pub struct LostFailure;
impl std::error::Error for LostFailure {}
impl DomainFailure for LostFailure {}

#[derive(Debug)]
pub struct Oops;
impl std::error::Error for Oops {}

pub struct Planner;
impl DomainService for Planner {}

pub struct OutsideFactory;
impl Factory for OutsideFactory {}

#[derive(Debug, Clone)]
pub struct Happened {
    at: Timestamp,
}
impl DomainEvent for Happened {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}

#[derive(Debug, Clone)]
pub struct Published {
    schema_version: u32,
}
impl IntegrationEvent for Published {
    const TYPE: &'static str = "published";
    const VERSION: u32 = 1;
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}
impl Published {
    pub fn touch(&mut self) {}
}

pub struct RootUseCase;
impl dca_building_blocks::hexagonal::port_in::InputPort for RootUseCase {}

pub mod do_thing {
    use std::cell::Cell;

    use alpha_domain::{Thing, ThingEvent};
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::DomainEventPublisher;

    use crate::WrongRepository;

    pub trait DoThingPort: InputPort {}

    pub struct RunCommand {
        cache: Cell<u8>,
    }
    impl RunCommand {
        pub fn bump(&mut self) {}
    }

    pub struct RunQuery {
        cache: Cell<u8>,
    }

    pub struct RunResult {
        thing: Thing,
        cache: Cell<u8>,
    }

    pub struct RunDto;

    pub struct Runner<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> {
        things: R,
        events: P,
        other: Box<dyn crate::other::OtherInputPort>,
    }
    impl<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> InputPort for Runner<R, P> {}
    impl<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> Runner<R, P> {
        pub fn run(&self, thing: &Thing) {
            self.things.save(thing);
        }
    }
}

pub mod other {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use super::do_thing::RunCommand;

    pub trait OtherInputPort: InputPort {}

    pub struct OnArrivalUseCase;
    impl InputPort for OnArrivalUseCase {}

    pub struct SetLimitUseCase;
    impl InputPort for SetLimitUseCase {}

    /// DCA-USE-021: a use case that counts its calls in itself.
    pub struct CountingUseCase {
        calls: std::cell::Cell<u32>,
    }
    impl InputPort for CountingUseCase {}

    /// DCA-USE-022: a use case that answers with a bare number.
    pub trait CountInputPort:
        dca_building_blocks::hexagonal::port_in::UseCase<RunCommand, Output = Result<u32, String>>
    {
    }
}

pub mod lookup {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use crate::WrongRepository;

    pub struct LookupQuery;

    pub fn helped(helper: &dyn crate::shared::Helper) -> LookupQuery {
        helper.query()
    }

    pub struct LookupUseCase<R: WrongRepository> {
        things: R,
    }
    impl<R: WrongRepository> InputPort for LookupUseCase<R> {}
    impl<R: WrongRepository> LookupUseCase<R> {
        pub fn lookup(&self, query: LookupQuery) {
            self.things.delete_by_id(todo!());
        }
    }
}

/// DCA-ERR-008: a closed result whose refusing variant carries no Refusal.
pub mod hold_thing {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    pub struct HoldThingCommand {
        pub thing: u64,
    }

    pub enum HoldThingResult {
        Held(u64),
        AlreadyHeld(u64),
    }

    pub trait HoldThingInputPort: InputPort {
        fn hold(&self, command: HoldThingCommand) -> HoldThingResult;
    }
}

/// DCA-ERR-003: implements the classification instead of a leaf.
#[derive(Debug)]
pub struct QuotaExceeded;
impl std::error::Error for QuotaExceeded {}
impl dca_building_blocks::application::InputPortFailure for QuotaExceeded {}

/// DCA-ERR-009: a use case that invents a domain failure for what the repository did not find.
pub mod remove_thing {
    use alpha_domain::{Thing, ThingRefused};

    pub struct RemoveThingUseCase;

    impl RemoveThingUseCase {
        pub fn remove(&self, id: u128, found: Option<Thing>) -> Result<(), ThingRefused> {
            let _thing = found.ok_or_else(|| ThingRefused::Unknown(id))?;
            Err(ThingRefused::Gone)
        }
    }
}

/// DCA-USE-018: two features, each with a command port.
pub mod tidy {
    pub mod sweep {
        pub struct SweepCommand;
        pub trait SweepInputPort: dca_building_blocks::hexagonal::port_in::InputPort {
            fn sweep(&self, command: SweepCommand);
        }
    }
}

pub mod mend {
    pub mod patch {
        pub struct PatchCommand;
        pub trait PatchInputPort: dca_building_blocks::hexagonal::port_in::InputPort {
            fn patch(&self, command: PatchCommand);
        }
    }
}

/// DCA-ERR-010: a port's failure answered without naming it.
pub mod keep_thing {
    use alpha_domain::Thing;

    use crate::WrongRepository;

    pub struct KeepThingUseCase<R: WrongRepository> {
        things: R,
    }

    impl<R: WrongRepository> KeepThingUseCase<R> {
        pub fn keep(&self, thing: &Thing) -> bool {
            let _ = self.things.save(thing);
            match self.things.save(thing) {
                Ok(()) => true,
                Err(_) => false,
            }
        }

        pub fn try_keep(&self, thing: &Thing) -> bool {
            self.things.save(thing).is_ok()
        }

        pub fn keep_anyway(&self, thing: &Thing) {
            let _ = std::panic::catch_unwind(|| self.things.save(thing));
        }

        pub fn keep_named(&self, thing: &Thing) -> Result<(), String> {
            self.things.save(thing).map_err(|e| e.to_string())?;
            match self.things.save(thing) {
                Ok(()) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}
