pub mod incoming {
    use alpha_application::do_thing::Runner;
    use alpha_application::WrongRepository;
    use alpha_domain::Calculator;
    use alpha_infrastructure::Pool;
    use beta_api::events::Notice;

    use crate::outgoing::Store1;

    /// DCA-HEX-020: one handler decides a claims-only check in its body; the one with the extractor
    /// and the one routed behind the layer declare it and are not listed.
    pub mod admin {
        pub struct Caller;
        impl Caller {
            pub fn has_role(&self, _role: &str) -> bool {
                false
            }
        }
        pub struct Accountant;
        pub struct RequireRole<R>(pub R);
        pub struct Router;
        impl Router {
            pub fn new() -> Router {
                Router
            }
            pub fn route(self, _path: &str, _handler: fn(&Caller) -> u16) -> Router {
                self
            }
            pub fn route_layer(self, _layer: u8) -> Router {
                self
            }
        }
        pub fn require_role(_role: &str) -> u8 {
            0
        }

        pub fn close_books(caller: &Caller) -> u16 {
            if !caller.has_role("accountant") {
                return 403;
            }
            200
        }

        pub fn reopen_books(_gate: RequireRole<Accountant>, caller: &Caller) -> u16 {
            if caller.has_role("auditor") { 409 } else { 200 }
        }

        pub fn archive_books(caller: &Caller) -> u16 {
            if caller.has_role("archivist") { 200 } else { 403 }
        }

        pub fn router() -> Router {
            Router::new()
                .route("/books/archive", archive_books)
                .route_layer(require_role("accountant"))
        }
    }

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

/// DCA-ADV-021: an outgoing adapter computes with a domain service while it writes.
pub mod export {
    use alpha_domain::Calculator;

    pub fn export(calculator: &Calculator) {
        calculator.calculate();
    }
}
