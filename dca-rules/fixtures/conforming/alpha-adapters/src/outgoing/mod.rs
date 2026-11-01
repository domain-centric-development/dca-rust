use std::collections::HashMap;
use std::sync::Mutex;

use alpha_application::shared::ThingRepository;
use alpha_domain::thing::{Thing, ThingId};
use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Repository};

#[derive(Default)]
pub struct InMemoryThingRepository {
    things: Mutex<HashMap<ThingId, Thing>>,
}

impl OutputPort for InMemoryThingRepository {}

impl Repository<Thing> for InMemoryThingRepository {
    fn find_by_id(&self, id: &ThingId) -> Result<Option<Thing>, PortError> {
        Ok(self.things.lock().map_err(|_| PortError::new("poisoned"))?.get(id).cloned())
    }
    fn save(&self, thing: &Thing) -> Result<(), PortError> {
        let _ = thing;
        Ok(())
    }
    fn delete_by_id(&self, id: &ThingId) -> Result<(), PortError> {
        let _ = id;
        Ok(())
    }
}

impl ThingRepository for InMemoryThingRepository {}
