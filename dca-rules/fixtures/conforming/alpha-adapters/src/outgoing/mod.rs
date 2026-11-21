use std::collections::HashMap;

use alpha_application::add_part::AddPartUnit;
use alpha_application::remove_thing::RemoveThingUnit;
use alpha_application::shared::ThingRepository;
use dca_building_blocks::application::TransactionUnit;
use alpha_domain::thing::{KeeperId, Label, PartCatalog, Thing, ThingEvent, ThingId};
use dca_building_blocks::ddd::tactical::DomainGateway;
use dca_building_blocks::ddd::tactical::Entity;
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, OutputPort, PortError, Repository};

#[derive(Default)]
pub struct InMemoryThingRepository {
    things: HashMap<ThingId, Thing>,
}

impl OutputPort for InMemoryThingRepository {}

impl Repository<Thing> for InMemoryThingRepository {
    async fn find_by_id(&mut self, id: &ThingId) -> Result<Option<Thing>, PortError> {
        Ok(self.things.get(id).cloned())
    }
    async fn save(&mut self, thing: &Thing) -> Result<(), PortError> {
        self.things.insert(thing.id().clone(), thing.clone());
        Ok(())
    }
    async fn delete_by_id(&mut self, id: &ThingId) -> Result<(), PortError> {
        self.things.remove(id);
        Ok(())
    }
}

impl ThingRepository for InMemoryThingRepository {
    async fn find_by_id_for_keeper(&mut self, keeper: &KeeperId, id: &ThingId) -> Result<Option<Thing>, PortError> {
        Ok(self.find_by_id(id).await?.filter(|thing| thing.keeper() == keeper))
    }
}

/// The application's own repository trait, for the `&mut` a unit hands out.
impl ThingRepository for &mut InMemoryThingRepository {
    async fn find_by_id_for_keeper(&mut self, keeper: &KeeperId, id: &ThingId) -> Result<Option<Thing>, PortError> {
        (**self).find_by_id_for_keeper(keeper, id).await
    }
}

/// The events of the transaction, kept until it commits.
#[derive(Default)]
pub struct InMemoryThingEvents {
    published: Vec<ThingEvent>,
}

impl OutputPort for InMemoryThingEvents {}

impl DomainEventPublisher<ThingEvent> for InMemoryThingEvents {
    async fn publish(&mut self, event: ThingEvent) -> Result<(), PortError> {
        self.published.push(event);
        Ok(())
    }
}

/// The unit adding a part works with: the in-memory things and the events of the transaction.
#[derive(Default)]
pub struct InMemoryAddPartUnit {
    things: InMemoryThingRepository,
    events: InMemoryThingEvents,
}

impl TransactionUnit for InMemoryAddPartUnit {}

impl AddPartUnit for InMemoryAddPartUnit {
    type Things<'a> = &'a mut InMemoryThingRepository;
    type Events<'a> = &'a mut InMemoryThingEvents;
    fn things(&mut self) -> &mut InMemoryThingRepository {
        &mut self.things
    }
    fn events(&mut self) -> &mut InMemoryThingEvents {
        &mut self.events
    }
}

/// The unit removing a thing works with: it owns the things for the transaction's length.
#[derive(Default)]
pub struct InMemoryRemoveThingUnit {
    things: HashMap<ThingId, Thing>,
}

/// The things as the transaction sees them, borrowed from the unit.
pub struct InMemoryThings<'a> {
    things: &'a mut HashMap<ThingId, Thing>,
}

impl OutputPort for InMemoryThings<'_> {}

impl Repository<Thing> for InMemoryThings<'_> {
    async fn find_by_id(&mut self, id: &ThingId) -> Result<Option<Thing>, PortError> {
        Ok(self.things.get(id).cloned())
    }
    async fn save(&mut self, thing: &Thing) -> Result<(), PortError> {
        let _ = thing;
        Ok(())
    }
    async fn delete_by_id(&mut self, id: &ThingId) -> Result<(), PortError> {
        self.things.remove(id);
        Ok(())
    }
}

impl TransactionUnit for InMemoryRemoveThingUnit {}

impl RemoveThingUnit for InMemoryRemoveThingUnit {
    type Things<'a> = InMemoryThings<'a>;
    fn things(&mut self) -> InMemoryThings<'_> {
        InMemoryThings { things: &mut self.things }
    }
}

/// The part catalogue, answered from a fixed list.
pub struct FixedPartCatalog {
    labels: Vec<Label>,
}

impl DomainGateway for FixedPartCatalog {}

impl PartCatalog for FixedPartCatalog {
    async fn lists(&self, label: &Label) -> bool {
        self.labels.contains(label)
    }
}

/// Reacts inside the publishing transaction and reaches an output port of that transaction through
/// the unit the publisher hands it - no input port (DCA-HEX-017).
pub mod reaction {
    use alpha_application::shared::ThingRepository;
    use alpha_domain::thing::ThingEvent;
    use dca_building_blocks::application::{BoxFuture, EventListener};
    use dca_building_blocks::hexagonal::port_out::PortError;

    pub struct CheckThingStored;

    impl<U: ThingRepository + Send> EventListener<ThingEvent, U> for CheckThingStored {
        fn on<'a>(&'a self, event: &'a ThingEvent, things: &'a mut U) -> BoxFuture<'a, Result<(), PortError>> {
            Box::pin(async move {
                let _ = (event, things);
                Ok(())
            })
        }
    }
}
