pub struct Pool;

/// DCA-ADV-021: the composition root may construct the domain service, but it calls it.
pub mod wiring {
    use alpha_domain::Calculator;

    pub fn wire() -> Calculator {
        let calculator = Calculator::new();
        calculator.calculate();
        calculator
    }
}

/// DCA-HEX-019: an output port implemented in the composition root is reported; the bridge that
/// drives an input port through a bound on its impl block is not.
pub mod ports {
    use alpha_application::other::OtherInputPort;
    use dca_building_blocks::hexagonal::port_out::OutputPort;

    pub struct Relay;
    impl OutputPort for Relay {}

    pub struct Bridge<P> {
        port: P,
    }
    impl<P: OtherInputPort> OutputPort for Bridge<P> {}
}
