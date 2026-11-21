pub mod driving {
    /// DCA-HEX-008: a repository by its name outside the configured outgoing module.
    pub struct StrayThingRepo;

    pub mod pages {
        /// DCA-NAM-011 passes a view model in the configured web module.
        pub struct ThingViewModel;
    }
}

pub mod driven {
    use alpha_application::common::ThingRepo;

    /// In the configured outgoing module: DCA-HEX-008 and DCA-TAC-015 pass it.
    pub struct InMemoryThingRepo;
    impl ThingRepo for InMemoryThingRepo {}
}
