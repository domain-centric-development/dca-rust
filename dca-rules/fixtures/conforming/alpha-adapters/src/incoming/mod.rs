use alpha_application::add_part::{AddPartCommand, AddPartError, AddPartInputPort};
use alpha_domain::thing::{ThingError, ThingId};

pub struct PartResponse {
    pub status: u16,
}

pub async fn add_part(port: &dyn AddPartInputPort, thing: u128, label: String) -> PartResponse {
    match port.execute(AddPartCommand { thing: ThingId::from(thing), label }).await {
        Ok(_) => PartResponse { status: 201 },
        Err(AddPartError::NotFound) => PartResponse { status: 404 },
        Err(AddPartError::Refused(_)) => PartResponse { status: 409 },
        Err(AddPartError::Unavailable(_)) => PartResponse { status: 503 },
    }
}

/// Matches a domain failure by its variants: a pattern names a variant, it constructs nothing.
pub fn refusal_code(failure: &ThingError) -> &'static str {
    match failure {
        ThingError::BlankLabel => "blank-label",
        ThingError::DuplicateLabel => "duplicate-label",
    }
}

/// A claims-only check declared in the signature: the extractor refuses a caller without the role
/// before the handler runs.
pub mod admin {
    pub struct Accountant;
    pub struct RequireRole<R>(pub R);

    pub fn close_books(_gate: RequireRole<Accountant>) -> u16 {
        200
    }
}

/// The event consumer: translates beta's integration event into alpha's own terms. The listener
/// names the channel, the translator beside it alpha's domain, and the listener names the
/// translator - together the translation site DCA-MAP-008 asks for.
pub mod event {
    pub mod listener {
        use beta_events::PartShipped;

        pub fn on_part_shipped(event: &PartShipped) -> super::translator::ShippedPart {
            super::translator::shipped(event.part)
        }
    }

    pub mod translator {
        use alpha_domain::thing::ThingId;

        pub struct ShippedPart {
            pub thing: ThingId,
        }

        pub fn shipped(part: u128) -> ShippedPart {
            ShippedPart { thing: ThingId::from(part) }
        }
    }
}

/// The routes: a command behind POST, a read behind GET.
pub mod web {
    use axum::routing::{get, post};

    pub fn router() -> axum::Router {
        axum::Router::new()
            .route("/things/{thing}/parts", post(super::add_part))
            .route("/health", get(health))
    }

    pub fn health() -> u16 {
        200
    }
}

/// Adds a first part to every thing created, after the thing's transaction committed.
pub mod listener {
    use alpha_application::add_part::{AddPartCommand, AddPartError, AddPartInputPort};
    use alpha_domain::thing::ThingCreated;
    use dca_building_blocks::application::{
        BoxFuture, EventListener, TransactionalEventListener,
    };
    use dca_building_blocks::hexagonal::port_out::PortError;

    pub struct AddFirstPart {
        port: Box<dyn AddPartInputPort>,
    }

    impl EventListener<ThingCreated> for AddFirstPart {
        fn on<'a>(&'a self, event: &'a ThingCreated, _: &'a mut ()) -> BoxFuture<'a, Result<(), PortError>> {
            Box::pin(async move {
                match self.port.execute(AddPartCommand { thing: event.thing.clone(), label: "first".into() }).await {
                    Ok(_) => Ok(()),
                    Err(AddPartError::NotFound) => Ok(()),
                    Err(AddPartError::Refused(_)) => Ok(()),
                    Err(AddPartError::Unavailable(e)) => Err(e),
                }
            })
        }
    }

    impl TransactionalEventListener<ThingCreated> for AddFirstPart {}
}
