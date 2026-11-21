//! What lending refuses.

use core::fmt;

use dca_building_blocks::ddd::tactical::DomainFailure;

/// What lending refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LendingFailure {
    /// The copy is out on loan.
    NotOnShelf,
    /// The copy is on the shelf, there is nothing to return.
    NotOnLoan,
    /// The reservation was fulfilled already.
    ReservationClosed,
}

impl fmt::Display for LendingFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LendingFailure::NotOnShelf => f.write_str("the copy is out on loan"),
            LendingFailure::NotOnLoan => f.write_str("the copy is not on loan"),
            LendingFailure::ReservationClosed => {
                f.write_str("the reservation was fulfilled already")
            }
        }
    }
}

impl std::error::Error for LendingFailure {}
impl DomainFailure for LendingFailure {}
