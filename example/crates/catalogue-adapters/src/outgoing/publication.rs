//! Publishes the catalogue's integration events: translated from its domain events inside the
//! publishing transaction, so they leave the context exactly when what caused them is committed.

use catalogue_domain::title::TitleEvent;
use catalogue_events::CopyAcquired;
use dca_building_blocks::application::{BoxFuture, EventListener};
use dca_building_blocks::hexagonal::port_out::{IntegrationEventPublisher, PortError};

/// Publishes a copy acquired to the other contexts. It reacts inside the transaction and calls an
/// output port only: the transaction's integration event publisher, which the domain event
/// publisher hands it as its unit.
#[derive(Debug, Clone, Copy, Default)]
pub struct PublishCopyAcquired;

impl<P: IntegrationEventPublisher<CopyAcquired> + Send> EventListener<TitleEvent, P>
    for PublishCopyAcquired
{
    fn on<'a>(
        &'a self,
        event: &'a TitleEvent,
        publisher: &'a mut P,
    ) -> BoxFuture<'a, Result<(), PortError>> {
        Box::pin(async move {
            let TitleEvent::CopyAcquired(acquired) = event else {
                return Ok(());
            };
            let published = CopyAcquired {
                event: acquired.event,
                title: acquired.title.as_u128(),
                copy: acquired.copy.as_u128(),
                occurred_on: acquired.occurred_on,
            };
            publisher.publish(published).await
        })
    }
}
