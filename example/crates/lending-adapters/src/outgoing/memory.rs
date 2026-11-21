//! Lending's tables in the kernel's in-memory store, and the unit its use cases work with.

use std::collections::HashMap;
use std::sync::Arc;

use dca_building_blocks::application::TransactionUnit;
use dca_building_blocks::ddd::tactical::Entity;
use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Repository, Store};
use kernel_adapters::outgoing::inbox::TransactionInbox;
use kernel_adapters::outgoing::memory::{MemoryUnit, Staged};
use kernel_adapters::outgoing::outbox::{Subscriptions, TransactionEvents};
use kernel_application::OrderingKey;
use kernel_domain::CopyId;
use lending_application::fulfil_reservation::FulfilReservationUnit;
use lending_application::lend_copy::LendCopyUnit;
use lending_application::reserve_copy::ReserveCopyUnit;
use lending_application::shared::{
    LendableCopyRepository, LoanLedger, LoanRecord, ReservationRepository,
};
use lending_application::shelve_copy::ShelveCopyUnit;
use lending_domain::copy::{CopyEvent, LendableCopy};
use lending_domain::member::MemberId;
use lending_domain::reservation::{Reservation, ReservationEvent, ReservationId};

/// Lending's tables.
#[derive(Debug, Clone, Default)]
pub struct LendingTables {
    copies: HashMap<CopyId, LendableCopy>,
    reservations: HashMap<ReservationId, Reservation>,
    ledger: Vec<LoanRecord>,
}

impl LendingTables {
    /// The copy with this id.
    pub fn copy(&self, id: &CopyId) -> Option<&LendableCopy> {
        self.copies.get(id)
    }

    /// The reservation with this id.
    pub fn reservation(&self, id: &ReservationId) -> Option<&Reservation> {
        self.reservations.get(id)
    }

    /// How many loans the ledger records.
    pub fn loans_recorded(&self) -> usize {
        self.ledger.len()
    }
}

/// The copies as a transaction sees them.
pub struct MemoryCopies<'a> {
    copies: &'a mut HashMap<CopyId, LendableCopy>,
}

impl OutputPort for MemoryCopies<'_> {}

impl Repository<LendableCopy> for MemoryCopies<'_> {
    async fn find_by_id(&mut self, id: &CopyId) -> Result<Option<LendableCopy>, PortError> {
        Ok(self.copies.get(id).cloned())
    }

    async fn save(&mut self, copy: &LendableCopy) -> Result<(), PortError> {
        self.copies.insert(*copy.id(), copy.clone());
        Ok(())
    }

    async fn delete_by_id(&mut self, id: &CopyId) -> Result<(), PortError> {
        self.copies.remove(id);
        Ok(())
    }
}

impl LendableCopyRepository for MemoryCopies<'_> {}

/// The reservations as a transaction sees them.
pub struct MemoryReservations<'a> {
    reservations: &'a mut HashMap<ReservationId, Reservation>,
}

impl OutputPort for MemoryReservations<'_> {}

impl Repository<Reservation> for MemoryReservations<'_> {
    async fn find_by_id(&mut self, id: &ReservationId) -> Result<Option<Reservation>, PortError> {
        Ok(self.reservations.get(id).cloned())
    }

    async fn save(&mut self, reservation: &Reservation) -> Result<(), PortError> {
        self.reservations
            .insert(*reservation.id(), reservation.clone());
        Ok(())
    }

    async fn delete_by_id(&mut self, id: &ReservationId) -> Result<(), PortError> {
        self.reservations.remove(id);
        Ok(())
    }
}

impl ReservationRepository for MemoryReservations<'_> {
    async fn find_by_id_for_member(
        &mut self,
        id: &ReservationId,
        member: &MemberId,
    ) -> Result<Option<Reservation>, PortError> {
        Ok(self
            .reservations
            .get(id)
            .filter(|reservation| reservation.member() == member)
            .cloned())
    }
}

/// The ledger as a transaction sees it.
pub struct MemoryLedger<'a> {
    records: &'a mut Vec<LoanRecord>,
}

impl OutputPort for MemoryLedger<'_> {}
impl Store for MemoryLedger<'_> {}

impl LoanLedger for MemoryLedger<'_> {
    async fn append(&mut self, record: LoanRecord) -> Result<(), PortError> {
        self.records.push(record);
        Ok(())
    }

    async fn loans_recorded_for(&mut self, member: &MemberId) -> Result<usize, PortError> {
        Ok(self.records.iter().filter(|r| &r.member == member).count())
    }
}

/// The unit every lending use case works with: the transaction's copy of the tables.
pub struct LendingUnit {
    staged: Staged<LendingTables>,
    subscriptions: Arc<Subscriptions>,
}

impl LendingUnit {
    fn copies(&mut self) -> MemoryCopies<'_> {
        MemoryCopies {
            copies: &mut self.staged.context.copies,
        }
    }

    fn reservations(&mut self) -> MemoryReservations<'_> {
        MemoryReservations {
            reservations: &mut self.staged.context.reservations,
        }
    }

    fn copy_events(&mut self) -> TransactionEvents<'_, CopyEvent> {
        TransactionEvents::new(&self.staged.outbox, &self.subscriptions, by_copy, ())
    }

    fn reservation_events(&mut self) -> TransactionEvents<'_, ReservationEvent> {
        TransactionEvents::new(&self.staged.outbox, &self.subscriptions, by_reservation, ())
    }

    fn inbox(&mut self) -> TransactionInbox<'_> {
        self.staged.inbox.port()
    }
}

fn by_copy(event: &CopyEvent) -> Option<OrderingKey> {
    Some(OrderingKey::new(format!("copy-{}", event.copy().as_u128())))
}

fn by_reservation(event: &ReservationEvent) -> Option<OrderingKey> {
    Some(OrderingKey::new(format!(
        "reservation-{}",
        event.reservation().as_u128()
    )))
}

impl MemoryUnit for LendingUnit {
    type Tables = LendingTables;

    fn open(staged: Staged<LendingTables>, subscriptions: Arc<Subscriptions>) -> Self {
        Self {
            staged,
            subscriptions,
        }
    }

    fn close(self) -> Staged<LendingTables> {
        self.staged
    }
}

impl TransactionUnit for LendingUnit {}

impl ShelveCopyUnit for LendingUnit {
    type Copies<'a> = MemoryCopies<'a>;
    type Events<'a> = TransactionEvents<'a, CopyEvent>;
    type Inbox<'a> = TransactionInbox<'a>;
    fn copies(&mut self) -> MemoryCopies<'_> {
        LendingUnit::copies(self)
    }
    fn events(&mut self) -> TransactionEvents<'_, CopyEvent> {
        self.copy_events()
    }
    fn inbox(&mut self) -> TransactionInbox<'_> {
        LendingUnit::inbox(self)
    }
}

impl ReserveCopyUnit for LendingUnit {
    type Copies<'a> = MemoryCopies<'a>;
    type Reservations<'a> = MemoryReservations<'a>;
    type Events<'a> = TransactionEvents<'a, ReservationEvent>;
    fn copies(&mut self) -> MemoryCopies<'_> {
        LendingUnit::copies(self)
    }
    fn reservations(&mut self) -> MemoryReservations<'_> {
        LendingUnit::reservations(self)
    }
    fn events(&mut self) -> TransactionEvents<'_, ReservationEvent> {
        self.reservation_events()
    }
}

impl LendCopyUnit for LendingUnit {
    type Copies<'a> = MemoryCopies<'a>;
    type Events<'a> = TransactionEvents<'a, CopyEvent>;
    type Ledger<'a> = MemoryLedger<'a>;
    fn copies(&mut self) -> MemoryCopies<'_> {
        LendingUnit::copies(self)
    }
    fn events(&mut self) -> TransactionEvents<'_, CopyEvent> {
        self.copy_events()
    }
    fn ledger(&mut self) -> MemoryLedger<'_> {
        MemoryLedger {
            records: &mut self.staged.context.ledger,
        }
    }
}

impl FulfilReservationUnit for LendingUnit {
    type Reservations<'a> = MemoryReservations<'a>;
    type Events<'a> = TransactionEvents<'a, ReservationEvent>;
    type Inbox<'a> = TransactionInbox<'a>;
    fn reservations(&mut self) -> MemoryReservations<'_> {
        LendingUnit::reservations(self)
    }
    fn events(&mut self) -> TransactionEvents<'_, ReservationEvent> {
        self.reservation_events()
    }
    fn inbox(&mut self) -> TransactionInbox<'_> {
        LendingUnit::inbox(self)
    }
}
