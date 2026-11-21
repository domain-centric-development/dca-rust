//! What the catalogue and lending share: the identity of a title and of a copy.

use dca_building_blocks::ddd::tactical::{Id, Value};

/// A title the library holds - a book, whatever number of copies it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TitleId(u128);

impl TitleId {
    /// The identity from its 128 bits.
    pub const fn from_u128(bits: u128) -> TitleId {
        TitleId(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for TitleId {}
impl Id for TitleId {}

/// One physical copy of a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CopyId(u128);

impl CopyId {
    /// The identity from its 128 bits.
    pub const fn from_u128(bits: u128) -> CopyId {
        CopyId(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for CopyId {}
impl Id for CopyId {}
