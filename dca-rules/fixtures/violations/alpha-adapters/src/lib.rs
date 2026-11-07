pub mod incoming {
    use alpha_application::do_thing::Runner;
    use alpha_application::WrongRepository;
    use alpha_domain::Calculator;
    use alpha_infrastructure::Pool;
    use beta_api::events::Notice;

    use crate::outgoing::Store1;

    /// DCA-ERR-006: answers every port failure the same way.
    pub mod generic {
        use dca_building_blocks::application::PortFailure;

        pub fn answer(_failure: &dyn PortFailure) -> u16 {
            409
        }
    }
}

pub mod outgoing {
    use alpha_application::other::OtherInputPort;

    pub struct Store1;

    /// DCA-ERR-002: an output-port failure the adapter declares itself instead of beside the port.
    #[derive(Debug)]
    pub struct StoreRefusedThing;
    impl std::error::Error for StoreRefusedThing {}
    impl dca_building_blocks::application::OutputPortFailure for StoreRefusedThing {}
}

