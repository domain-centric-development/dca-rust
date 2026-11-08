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
