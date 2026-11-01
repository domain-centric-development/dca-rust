use dca_building_blocks::ddd::tactical::{EventId, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository};
use alpha_domain::thing::{PartId, Thing};

pub trait ThingRepository: Repository<Thing> {}

pub trait Clock: OutputPort {
    fn now(&self) -> Timestamp;
}

pub trait IdSource: OutputPort {
    fn next_part_id(&self) -> PartId;
    fn next_event_id(&self) -> EventId;
}
