use std::cell::Cell;

use alpha_domain::{Part, Thing, ThingEvent};
use dca_building_blocks::application::UseCaseFailure;
use dca_building_blocks::ddd::tactical::{DomainEvent, DomainFailure, DomainService, EventId, Factory, IntegrationEvent, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository, Store};

pub mod shared {
    pub trait Helper {}
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
}

pub mod lookup {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use crate::WrongRepository;

    pub struct LookupQuery;

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
