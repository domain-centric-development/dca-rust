use core::fmt;

use dca_building_blocks::ddd::tactical::DomainFailure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThingError {
    LimitReached,
}

impl fmt::Display for ThingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("refused")
    }
}

impl std::error::Error for ThingError {}
impl DomainFailure for ThingError {}
