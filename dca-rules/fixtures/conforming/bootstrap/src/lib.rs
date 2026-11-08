pub use alpha_adapters::outgoing::InMemoryThingRepository;

/// DCA-ADV-021: the composition root builds the domain service to hand it to a use case.
pub fn sizing() -> alpha_domain::thing::Sizing {
    alpha_domain::thing::Sizing
}
