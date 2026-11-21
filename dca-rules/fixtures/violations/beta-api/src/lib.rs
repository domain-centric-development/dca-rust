pub fn set_limit() {}

/// DCA-NAM-013: published operations named after a field - a method of an api type, a method of an
/// api trait.
pub struct Desk;
impl Desk {
    pub fn update_count(&self) {}
}
pub trait Counter {
    fn set_count(&self);
}

/// DCA-STR-007: an integration event in a module of the api crate - the events crate is its place.
pub mod events {
    use dca_building_blocks::ddd::tactical::{EventId, IntegrationEvent, Timestamp};

    #[derive(Debug, Clone)]
    pub struct LimitRaised {
        pub event: EventId,
        pub occurred_on: Timestamp,
    }

    impl IntegrationEvent for LimitRaised {
        const TYPE: &'static str = "beta.limit-raised";
        const VERSION: u32 = 1;
        fn event_id(&self) -> EventId {
            self.event
        }
        fn occurred_on(&self) -> Timestamp {
            self.occurred_on
        }
    }
}
