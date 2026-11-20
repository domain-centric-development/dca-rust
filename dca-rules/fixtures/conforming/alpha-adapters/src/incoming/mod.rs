use alpha_application::add_part::{AddPartCommand, AddPartError, AddPartInputPort};
use alpha_domain::thing::ThingId;

pub struct PartResponse {
    pub status: u16,
}

pub fn add_part(port: &dyn AddPartInputPort, thing: u128, label: String) -> PartResponse {
    match port.execute(AddPartCommand { thing: ThingId::from(thing), label }) {
        Ok(_) => PartResponse { status: 201 },
        Err(AddPartError::NotFound) => PartResponse { status: 404 },
        Err(AddPartError::Refused(_)) => PartResponse { status: 409 },
        Err(AddPartError::Unavailable(_)) => PartResponse { status: 503 },
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

/// The event consumer: translates beta's integration event into alpha's own terms.
pub mod event {
    use beta_api::events::PartShipped;

    pub struct ShippedPart {
        pub part: u128,
    }

    pub fn on_part_shipped(event: &PartShipped) -> ShippedPart {
        ShippedPart { part: event.part }
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
