//! The catalogue's tables in the kernel's in-memory store, and the unit its use cases work with.

use std::collections::HashMap;
use std::sync::Arc;

use catalogue_application::acquire_copy::AcquireCopyUnit;
use catalogue_application::register_title::RegisterTitleUnit;
use catalogue_application::shared::{TitleNameTaken, TitleRepository};
use catalogue_domain::title::{Title, TitleEvent};
use dca_building_blocks::application::TransactionUnit;
use dca_building_blocks::ddd::tactical::Entity;
use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Repository};
use kernel_adapters::outgoing::memory::{MemoryUnit, Staged};
use kernel_adapters::outgoing::outbox::{IntegrationOutbox, Subscriptions, TransactionEvents};
use kernel_application::OrderingKey;
use kernel_domain::TitleId;

use super::publication::PublishCopyAcquired;

/// The catalogue's tables.
#[derive(Debug, Clone, Default)]
pub struct CatalogueTables {
    titles: HashMap<TitleId, Title>,
}

impl CatalogueTables {
    /// The title with this id.
    pub fn title(&self, id: &TitleId) -> Option<&Title> {
        self.titles.get(id)
    }
}

/// The titles as a transaction sees them.
pub struct MemoryTitles<'a> {
    titles: &'a mut HashMap<TitleId, Title>,
}

impl OutputPort for MemoryTitles<'_> {}

impl Repository<Title> for MemoryTitles<'_> {
    async fn find_by_id(&mut self, id: &TitleId) -> Result<Option<Title>, PortError> {
        Ok(self.titles.get(id).cloned())
    }

    async fn save(&mut self, title: &Title) -> Result<(), PortError> {
        self.titles.insert(*title.id(), title.clone());
        Ok(())
    }

    async fn delete_by_id(&mut self, id: &TitleId) -> Result<(), PortError> {
        self.titles.remove(id);
        Ok(())
    }
}

impl TitleRepository for MemoryTitles<'_> {
    async fn add(&mut self, title: &Title) -> Result<Result<(), TitleNameTaken>, PortError> {
        if self.titles.values().any(|held| held.name() == title.name()) {
            return Ok(Err(TitleNameTaken {
                name: title.name().clone(),
            }));
        }
        self.titles.insert(*title.id(), title.clone());
        Ok(Ok(()))
    }
}

/// The unit every catalogue use case works with: the transaction's copy of the tables.
pub struct CatalogueUnit {
    staged: Staged<CatalogueTables>,
    subscriptions: Arc<Subscriptions>,
}

impl CatalogueUnit {
    fn titles(&mut self) -> MemoryTitles<'_> {
        MemoryTitles {
            titles: &mut self.staged.context.titles,
        }
    }

    /// The title events of the transaction: the copies acquired are published to the other contexts
    /// in the transaction, through its outbox, the rest is registered for the catalogue's own
    /// listeners after it.
    fn events(&mut self) -> TitleEvents<'_> {
        let outbox = &self.staged.outbox;
        let subscriptions = &*self.subscriptions;
        TransactionEvents::new(
            outbox,
            subscriptions,
            by_title,
            IntegrationOutbox::new(outbox, subscriptions),
        )
        .reacting_with(&PublishCopyAcquired)
    }
}

/// The title events a catalogue transaction publishes; the listeners inside it reach the
/// transaction's integration event publisher.
pub type TitleEvents<'a> = TransactionEvents<'a, TitleEvent, IntegrationOutbox<'a>>;

fn by_title(event: &TitleEvent) -> Option<OrderingKey> {
    Some(OrderingKey::new(format!(
        "title-{}",
        event.title().as_u128()
    )))
}

impl MemoryUnit for CatalogueUnit {
    type Tables = CatalogueTables;

    fn open(staged: Staged<CatalogueTables>, subscriptions: Arc<Subscriptions>) -> Self {
        Self {
            staged,
            subscriptions,
        }
    }

    fn close(self) -> Staged<CatalogueTables> {
        self.staged
    }
}

impl TransactionUnit for CatalogueUnit {}

impl RegisterTitleUnit for CatalogueUnit {
    type Titles<'a> = MemoryTitles<'a>;
    type Events<'a> = TitleEvents<'a>;
    fn titles(&mut self) -> MemoryTitles<'_> {
        CatalogueUnit::titles(self)
    }
    fn events(&mut self) -> TitleEvents<'_> {
        CatalogueUnit::events(self)
    }
}

impl AcquireCopyUnit for CatalogueUnit {
    type Titles<'a> = MemoryTitles<'a>;
    type Events<'a> = TitleEvents<'a>;
    fn titles(&mut self) -> MemoryTitles<'_> {
        CatalogueUnit::titles(self)
    }
    fn events(&mut self) -> TitleEvents<'_> {
        CatalogueUnit::events(self)
    }
}
