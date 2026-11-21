//! The shared kernel's domain: the rules on a domain crate read it.
use dca_building_blocks::ddd::tactical::{DomainEvent, DomainService, EventId, Factory, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Store};

pub struct Shared;

/// DCA-TAC-001: named an aggregate root, not one.
pub struct LedgerAggregateRoot;

/// DCA-ADV-004/011/015/018: framework metadata on the kernel's event, service, factory and
/// specification.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Noted {
    occurred_on: Timestamp,
}
impl DomainEvent for Noted {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { self.occurred_on }
}

#[derive(sqlx::FromRow)]
pub struct Sorting;
impl DomainService for Sorting {}

#[derive(sqlx::FromRow)]
pub struct NotedFactory;
impl Factory for NotedFactory {}

#[derive(sqlx::FromRow)]
pub struct OpenSpecification;

/// DCA-STR-013: a contract in the kernel's events module without the integration event identity.
/// DCA-STR-007: an integration event is a context's published contract; the shared kernel holds none.
pub mod events {
    use dca_building_blocks::ddd::tactical::{EventId, IntegrationEvent, Timestamp};

    #[derive(Debug, Clone)]
    pub struct Bulletin {
        event: EventId,
        occurred_on: Timestamp,
    }
    impl IntegrationEvent for Bulletin {
        const TYPE: &'static str = "kernel.bulletin";
        const VERSION: u32 = 1;
        fn event_id(&self) -> EventId {
            self.event
        }
        fn occurred_on(&self) -> Timestamp {
            self.occurred_on
        }
    }
}

/// An output port implemented in the shared kernel's domain, not in its outgoing adapters.
pub struct DomainLedger;
impl OutputPort for DomainLedger {}
impl Store for DomainLedger {}
impl DomainLedger {
    pub async fn note(&mut self, event: &Noted) -> Result<(), PortError> {
        let _ = event;
        Ok(())
    }
}
