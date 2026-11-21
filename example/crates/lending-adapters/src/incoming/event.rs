//! The event consumers: they react after the publishing transaction committed, delivered from the
//! outbox, and drive a use case that runs in a transaction of its own. The catalogue's event is
//! translated here into lending's terms.

use std::sync::Arc;

use catalogue_events::CopyAcquired;
use dca_building_blocks::application::{BoxFuture, EventListener, TransactionalEventListener};
use dca_building_blocks::ddd::tactical::IntegrationEvent;
use dca_building_blocks::hexagonal::port_out::PortError;
use kernel_application::{ListenerId, Received};
use kernel_domain::{CopyId, TitleId};
use lending_application::fulfil_reservation::{
    FulfilReservationCommand, FulfilReservationInputPort,
};
use lending_application::shared::LendingUnavailable;
use lending_application::shelve_copy::{ShelveCopyCommand, ShelveCopyInputPort};
use lending_domain::copy::CopyEvent;

/// The stable name of the listener that shelves the copies the catalogue acquired.
pub const SHELVE_ACQUIRED_COPY: ListenerId = ListenerId::new("lending.shelve-acquired-copy");

/// The stable name of the listener that fulfils the reservation a member collected.
pub const FULFIL_COLLECTED_RESERVATION: ListenerId =
    ListenerId::new("lending.fulfil-collected-reservation");

/// Puts every copy the catalogue acquired on lending's shelf.
pub struct ShelveAcquiredCopy {
    shelve: Arc<dyn ShelveCopyInputPort + Send + Sync>,
}

impl ShelveAcquiredCopy {
    /// The listener over the shelving input port.
    pub fn new(shelve: Arc<dyn ShelveCopyInputPort + Send + Sync>) -> Self {
        Self { shelve }
    }
}

impl EventListener<CopyAcquired> for ShelveAcquiredCopy {
    fn on<'a>(
        &'a self,
        event: &'a CopyAcquired,
        _: &'a mut (),
    ) -> BoxFuture<'a, Result<(), PortError>> {
        Box::pin(async move {
            let command = ShelveCopyCommand {
                received: Received::new(event.event_id(), SHELVE_ACQUIRED_COPY),
                copy: CopyId::from_u128(event.copy),
                title: TitleId::from_u128(event.title),
            };
            match self.shelve.execute(command).await {
                Ok(_) => Ok(()),
                Err(LendingUnavailable(failure)) => Err(failure),
            }
        })
    }
}

impl TransactionalEventListener<CopyAcquired> for ShelveAcquiredCopy {}

/// Fulfils the reservation a member named when they borrowed the copy.
pub struct FulfilCollectedReservation {
    fulfil: Arc<dyn FulfilReservationInputPort + Send + Sync>,
}

impl FulfilCollectedReservation {
    /// The listener over the fulfilling input port.
    pub fn new(fulfil: Arc<dyn FulfilReservationInputPort + Send + Sync>) -> Self {
        Self { fulfil }
    }
}

impl EventListener<CopyEvent> for FulfilCollectedReservation {
    fn on<'a>(
        &'a self,
        event: &'a CopyEvent,
        _: &'a mut (),
    ) -> BoxFuture<'a, Result<(), PortError>> {
        Box::pin(async move {
            let CopyEvent::CopyLent(lent) = event else {
                return Ok(());
            };
            let Some(reservation) = lent.reservation else {
                return Ok(());
            };
            let command = FulfilReservationCommand {
                received: Received::new(lent.event, FULFIL_COLLECTED_RESERVATION),
                member: lent.member,
                reservation,
            };
            match self.fulfil.execute(command).await {
                Ok(_) => Ok(()),
                Err(LendingUnavailable(failure)) => Err(failure),
            }
        })
    }
}

impl TransactionalEventListener<CopyEvent> for FulfilCollectedReservation {}
