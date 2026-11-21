use core::fmt;

use dca_building_blocks::ddd::tactical::DomainFailure;

/// What the catalogue refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueFailure {
    /// A title needs a name.
    BlankName,
    /// The title holds this copy already.
    CopyHeldAlready,
}

impl fmt::Display for CatalogueFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogueFailure::BlankName => f.write_str("a title needs a name"),
            CatalogueFailure::CopyHeldAlready => f.write_str("the title holds this copy already"),
        }
    }
}

impl std::error::Error for CatalogueFailure {}
impl DomainFailure for CatalogueFailure {}
