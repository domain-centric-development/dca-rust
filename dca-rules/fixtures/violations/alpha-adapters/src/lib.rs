pub mod incoming {
    use alpha_application::do_thing::Runner;
    use alpha_application::WrongRepository;
    use alpha_domain::Calculator;
    use alpha_infrastructure::Pool;
    use beta_api::events::Notice;

    use crate::outgoing::Store1;

    /// DCA-MAP-010: the outbound external system's contract named on the incoming side.
    pub fn vendor() -> vendor_sdk::Client {
        vendor_sdk::Client
    }

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

    /// DCA-ERR-010: a handler that drops the input port's failure.
    pub mod ask {
        use alpha_application::other::OtherInputPort;

        pub fn ask(port: &dyn OtherInputPort) -> u16 {
            if let Ok(_) = port.run() { 200 } else { 500 }
        }

        pub struct Ports {
            pub other: Box<dyn OtherInputPort>,
        }
        pub struct State<S>(pub S);

        pub fn ask_state(State(ports): State<Ports>) -> u16 {
            ports.other.run().map_or(500, |_| 200)
        }

        pub fn ask_held(ports: &Ports) -> Option<u16> {
            ports.other.run().ok()
        }
    }

    /// DCA-CYC-004: two incoming adapter modules that call each other.
    pub mod first {
        pub fn one() -> u16 {
            super::second::two()
        }
    }

    pub mod second {
        pub fn two() -> u16 {
            0
        }

        pub fn again() -> u16 {
            super::first::one()
        }
    }

    /// DCA-USE-018: one adapter drives the commands of two features.
    pub fn sweep_and_patch(
        sweep: &dyn alpha_application::tidy::sweep::SweepInputPort,
        patch: &dyn alpha_application::mend::patch::PatchInputPort,
    ) {
        let _ = (sweep, patch);
    }

    /// DCA-HEX-014: a handler registered for GET that builds a command; the POST handler may.
    pub mod web {
        use alpha_application::do_thing::RunCommand;
        use axum::routing::get;

        pub fn routes() -> axum::Router {
            axum::Router::new().route("/run", get(run).post(create))
        }

        pub fn run() -> u16 {
            let _command: Option<RunCommand> = None;
            200
        }

        pub fn create() -> u16 {
            let _command: Option<RunCommand> = None;
            201
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
    // DCA-MAP-008: the events channel of an anti-corruption layer outside the incoming module.
    use beta_api::events::Notice;
    // DCA-MAP-011: the api channel of beta, which alpha does not declare.
    use beta_api::set_limit;

    pub struct Store1;

    /// DCA-HEX-016: prints the domain's variant names; Display through `{}` is not listed.
    pub fn phase_label(phase: alpha_domain::Phase) -> String {
        let _shown = format!("{}", alpha_domain::Phase::Open);
        let _named = format!("{p:?}", p = alpha_domain::Phase::Closed);
        format!("{:?}", phase)
    }

    pub fn phase_text(phase: &alpha_domain::Phase) -> String {
        phase.to_string()
    }

    /// DCA-HEX-021: an outgoing adapter changes the aggregate it stores; clearing its recorded
    /// events through the building block passes.
    pub fn store(thing: &mut alpha_domain::Thing) {
        thing.parts_mut();
        let _ = thing.take_domain_events();
    }

    /// DCA-CYC-003: two outgoing adapter modules that call each other.
    pub mod east {
        pub fn east() -> u8 {
            super::west::west()
        }
    }

    pub mod west {
        pub fn west() -> u8 {
            0
        }

        pub fn back() -> u8 {
            super::east::east()
        }
    }

    /// DCA-ERR-002: an output-port failure the adapter declares itself instead of beside the port.
    #[derive(Debug)]
    pub struct StoreRefusedThing;
    impl std::error::Error for StoreRefusedThing {}
    impl dca_building_blocks::application::OutputPortFailure for StoreRefusedThing {}
}

/// DCA-HEX-018: a handler registered outside the incoming module.
#[actix_web::get("/health")]
pub fn health() -> u16 {
    200
}

/// DCA-ADV-021: an outgoing adapter computes with a domain service while it writes.
pub mod export {
    use alpha_domain::Calculator;

    pub fn export(calculator: &Calculator) {
        calculator.calculate();
    }
}
