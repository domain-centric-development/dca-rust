//! The composition root: builds both contexts over their stores, the desks that drive them and the
//! relays that carry events after the commit - the catalogue's to lending, lending's to itself.

use std::sync::Arc;

use catalogue_adapters::incoming::desk::CatalogueDesk;
use catalogue_adapters::outgoing::memory::{CatalogueTables, CatalogueUnit};
use catalogue_adapters::outgoing::shelving::InitialsShelfMarks;
use catalogue_application::acquire_copy::AcquireCopyUseCase;
use catalogue_application::register_title::RegisterTitleUseCase;
use catalogue_domain::title::TitleFactory;
use catalogue_events::CopyAcquired;
use dca_building_blocks::ddd::tactical::Entity;
use dca_building_blocks::ddd::tactical::Specification;
use dca_building_blocks::hexagonal::port_out::PortError;
use kernel_adapters::outgoing::memory::MemoryDatabase;
use kernel_adapters::outgoing::outbox::Subscriptions;
use kernel_adapters::outgoing::relay::Relay;
use kernel_adapters::outgoing::system::{SequentialIds, SystemClock};
use kernel_domain::{CopyId, TitleId};
use lending_adapters::incoming::desk::LendingDesk;
use lending_adapters::incoming::event::{
    FULFIL_COLLECTED_RESERVATION, FulfilCollectedReservation, SHELVE_ACQUIRED_COPY,
    ShelveAcquiredCopy,
};
use lending_adapters::outgoing::membership::SuspensionList;
use lending_adapters::outgoing::memory::{LendingTables, LendingUnit};
use lending_application::fulfil_reservation::FulfilReservationUseCase;
use lending_application::lend_copy::LendCopyUseCase;
use lending_application::reserve_copy::ReserveCopyUseCase;
use lending_application::shelve_copy::ShelveCopyUseCase;
use lending_domain::copy::{CopyEvent, LoanPeriod, LoanTerms, OnShelfSpecification};
use lending_domain::member::MemberId;
use lending_domain::reservation::ReservationId;

/// The library, wired.
pub struct Library {
    pub catalogue: CatalogueDesk,
    pub lending: LendingDesk,
    catalogue_store: MemoryDatabase<CatalogueTables>,
    lending_store: MemoryDatabase<LendingTables>,
    catalogue_relay: Relay<CatalogueTables>,
    lending_relay: Relay<LendingTables>,
}

impl Library {
    /// The library with empty stores, these members suspended.
    pub fn open(suspended: &[u128]) -> Library {
        let clock = SystemClock;
        let ids = SequentialIds::default();

        let catalogue_store = MemoryDatabase::new(
            CatalogueTables::default(),
            Subscriptions::default().subscribe::<CopyAcquired>(SHELVE_ACQUIRED_COPY),
        );
        let lending_store = MemoryDatabase::new(
            LendingTables::default(),
            Subscriptions::default().subscribe::<CopyEvent>(FULFIL_COLLECTED_RESERVATION),
        );

        let catalogue = CatalogueDesk::new(
            Arc::new(RegisterTitleUseCase::<CatalogueUnit, _, _, _>::new(
                catalogue_store.clone(),
                clock,
                ids.clone(),
                TitleFactory,
            )),
            Arc::new(AcquireCopyUseCase::<CatalogueUnit, _, _, _, _>::new(
                catalogue_store.clone(),
                clock,
                ids.clone(),
                InitialsShelfMarks,
            )),
        );

        let standings = SuspensionList::new(suspended.iter().copied().map(MemberId::from_u128));
        let lending = LendingDesk::new(
            Arc::new(LendCopyUseCase::<LendingUnit, _, _, _, _>::new(
                lending_store.clone(),
                clock,
                ids.clone(),
                LoanTerms::new(LoanPeriod::of_days(21)),
                standings,
            )),
            Arc::new(ReserveCopyUseCase::<LendingUnit, _, _, _>::new(
                lending_store.clone(),
                clock,
                ids.clone(),
            )),
        );

        let shelve = Arc::new(ShelveCopyUseCase::<LendingUnit, _, _, _>::new(
            lending_store.clone(),
            clock,
            ids.clone(),
        ));
        let fulfil = Arc::new(FulfilReservationUseCase::<LendingUnit, _, _, _>::new(
            lending_store.clone(),
            clock,
            ids,
        ));
        let catalogue_relay = Relay::new(&catalogue_store).route::<CopyAcquired, _>(
            SHELVE_ACQUIRED_COPY,
            Arc::new(ShelveAcquiredCopy::new(shelve)),
        );
        let lending_relay = Relay::new(&lending_store).route::<CopyEvent, _>(
            FULFIL_COLLECTED_RESERVATION,
            Arc::new(FulfilCollectedReservation::new(fulfil)),
        );

        Library {
            catalogue,
            lending,
            catalogue_store,
            lending_store,
            catalogue_relay,
            lending_relay,
        }
    }

    /// Runs both relays once: what the catalogue committed reaches lending, then what lending
    /// committed reaches its own listeners. Answers how many deliveries were made.
    pub async fn relay(&self) -> Result<usize, PortError> {
        let from_catalogue = self.catalogue_relay.deliver().await?;
        let within_lending = self.lending_relay.deliver().await?;
        Ok(from_catalogue + within_lending)
    }

    /// Forgets the deliveries both relays made, as a relay that stopped before recording them.
    pub async fn lose_relay_acknowledgements(&self) {
        self.catalogue_store.forget_deliveries().await;
        self.lending_store.forget_deliveries().await;
    }

    /// The deliveries the catalogue still owes.
    pub async fn catalogue_deliveries_pending(&self) -> usize {
        self.catalogue_store
            .read(|staged| staged.outbox_table().pending())
            .await
    }

    /// The deliveries lending still owes its own listeners.
    pub async fn lending_deliveries_pending(&self) -> usize {
        self.lending_store
            .read(|staged| staged.outbox_table().pending())
            .await
    }

    /// How many copies the catalogue holds of the title.
    pub async fn holdings_of(&self, title: u128) -> Option<usize> {
        self.catalogue_store
            .read(|staged| {
                staged
                    .context
                    .title(&TitleId::from_u128(title))
                    .map(|t| t.holdings().len())
            })
            .await
    }

    /// Whether lending knows the copy, and whether it is on the shelf.
    pub async fn on_shelf(&self, copy: u128) -> Option<bool> {
        self.lending_store
            .read(|staged| {
                staged
                    .context
                    .copy(&CopyId::from_u128(copy))
                    .map(|c| OnShelfSpecification.is_satisfied_by(c))
            })
            .await
    }

    /// The loans lending made of the copy.
    pub async fn loans_of(&self, copy: u128) -> usize {
        self.lending_store
            .read(|staged| {
                staged
                    .context
                    .copy(&CopyId::from_u128(copy))
                    .map_or(0, |c| c.loans().len())
            })
            .await
    }

    /// Whether the reservation was fulfilled.
    pub async fn reservation_fulfilled(&self, reservation: u128) -> Option<bool> {
        self.lending_store
            .read(|staged| {
                staged
                    .context
                    .reservation(&ReservationId::from_u128(reservation))
                    .map(|r| r.is_fulfilled())
            })
            .await
    }

    /// How many loans the ledger records.
    pub async fn loans_recorded(&self) -> usize {
        self.lending_store
            .read(|staged| staged.context.loans_recorded())
            .await
    }

    /// How many deliveries lending's inbox received.
    pub async fn deliveries_received(&self) -> usize {
        self.lending_store.read(|staged| staged.inbox.len()).await
    }

    /// The title's id, as the catalogue holds it.
    pub async fn title_held(&self, title: u128) -> bool {
        self.catalogue_store
            .read(|staged| {
                staged
                    .context
                    .title(&TitleId::from_u128(title))
                    .is_some_and(|t| t.id().as_u128() == title)
            })
            .await
    }
}
