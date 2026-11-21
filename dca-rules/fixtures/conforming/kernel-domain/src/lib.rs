use dca_building_blocks::ddd::tactical::{
    AggregateRoot, DomainEvent, DomainService, Entity, EventId, Factory, Id, Specification,
    Timestamp, Value,
};

#[derive(Debug, Clone)]
pub struct Recorded {
    event: EventId,
    occurred_on: Timestamp,
}
impl DomainEvent for Recorded {
    fn event_id(&self) -> EventId {
        self.event
    }
    fn occurred_on(&self) -> Timestamp {
        self.occurred_on
    }
}

pub struct Ordering;
impl DomainService for Ordering {}

pub struct RecordedFactory;
impl Factory for RecordedFactory {}

pub struct RecentSpecification;
impl Specification<Recorded> for RecentSpecification {
    fn is_satisfied_by(&self, candidate: &Recorded) -> bool {
        let _ = candidate;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LedgerId(u128);
impl Value for LedgerId {}
impl Id for LedgerId {}

pub struct Ledger {
    id: LedgerId,
    events: Vec<Recorded>,
}
impl Entity for Ledger {
    type Id = LedgerId;
    fn id(&self) -> &LedgerId {
        &self.id
    }
}
impl AggregateRoot for Ledger {
    type Event = Recorded;
    fn domain_events(&self) -> &[Recorded] {
        &self.events
    }
    fn take_domain_events(&mut self) -> Vec<Recorded> {
        std::mem::take(&mut self.events)
    }
}
