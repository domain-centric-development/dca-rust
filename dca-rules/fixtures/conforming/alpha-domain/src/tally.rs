use dca_building_blocks::ddd::tactical::DomainFailure;

/// A module's own failure named `Error`: a `use std::error::Error;` inside one function does not
/// shadow it for the module.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("empty")]
    Empty,
}

impl DomainFailure for Error {}

pub fn has_cause(e: &Error) -> bool {
    use std::error::Error;
    e.source().is_some()
}
