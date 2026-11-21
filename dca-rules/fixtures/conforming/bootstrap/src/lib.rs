pub use alpha_adapters::outgoing::InMemoryThingRepository;

/// DCA-ADV-021: the composition root builds the domain service to hand it to a use case.
pub fn sizing() -> alpha_domain::thing::Sizing {
    alpha_domain::thing::Sizing
}

/// DCA-ADV-021: a constructor that returns the type by its name is a constructor too.
pub fn usual_sizing() -> alpha_domain::thing::Sizing {
    alpha_domain::thing::Sizing::usual()
}

/// DCA-HEX-019: the bridge - an output port answered by driving a use case - is wiring and sits in
/// the composition root.
pub struct AddingClock<P> {
    add: P,
}
impl<P: alpha_application::add_part::AddPartInputPort> dca_building_blocks::hexagonal::port_out::OutputPort
    for AddingClock<P>
{
}
impl<P: alpha_application::add_part::AddPartInputPort> alpha_application::shared::Clock for AddingClock<P> {
    fn now(&self) -> dca_building_blocks::ddd::tactical::Timestamp {
        todo!()
    }
}
