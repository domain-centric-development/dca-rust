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
