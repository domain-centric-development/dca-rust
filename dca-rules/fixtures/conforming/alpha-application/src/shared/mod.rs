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

/// Declared by the repository port beside it: a number only the store can see is taken. The adapter
/// returns it, the use case matches it.
#[derive(Debug)]
pub struct ThingNumberTaken {
    pub number: u64,
}

impl core::fmt::Display for ThingNumberTaken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("thing number taken")
    }
}

impl std::error::Error for ThingNumberTaken {}
impl dca_building_blocks::application::OutputPortFailure for ThingNumberTaken {}
