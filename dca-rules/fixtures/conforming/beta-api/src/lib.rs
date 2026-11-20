pub mod events {
    use dca_building_blocks::ddd::tactical::{EventId, IntegrationEvent, Timestamp};

    #[derive(Debug, Clone)]
    pub struct PartShipped {
        pub event: EventId,
        pub part: u128,
        pub occurred_on: Timestamp,
    }

    impl IntegrationEvent for PartShipped {
        const TYPE: &'static str = "beta.part-shipped";
        const VERSION: u32 = 1;
        fn event_id(&self) -> EventId {
            self.event
        }
        fn occurred_on(&self) -> Timestamp {
            self.occurred_on
        }
    }
}
