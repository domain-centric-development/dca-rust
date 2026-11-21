//! What more than one use case of lending works with. Lending answers its refusals as variants of a
//! closed result; its one failure is that the store cannot answer.

use core::fmt;
use std::future::Future;

use dca_building_blocks::application::{Refusal, UseCaseFailure};
use dca_building_blocks::ddd::tactical::Timestamp;
use dca_building_blocks::hexagonal::port_out::{PortError, Repository, Store};
use kernel_domain::CopyId;
use lending_domain::copy::{LendableCopy, LoanId};
use lending_domain::member::MemberId;
use lending_domain::reservation::{Reservation, ReservationId};

/// The copies lending knows.
pub trait LendableCopyRepository: Repository<LendableCopy> {}

/// The reservations: each belongs to its member and is looked up through them.
pub trait ReservationRepository: Repository<Reservation> {
    /// The member's reservation with this id; another member's is not found.
    fn find_by_id_for_member(
        &mut self,
        id: &ReservationId,
        member: &MemberId,
    ) -> impl Future<Output = Result<Option<Reservation>, PortError>> + Send;
}

/// One loan as the ledger records it.
#[derive(Debug, Clone)]
pub struct LoanRecord {
    pub loan: LoanId,
    pub copy: CopyId,
    pub member: MemberId,
    pub lent_on: Timestamp,
}

/// The ledger of every loan made: appended to, never changed.
pub trait LoanLedger: Store {
    /// Records a loan.
    fn append(&mut self, record: LoanRecord) -> impl Future<Output = Result<(), PortError>> + Send;

    /// How many loans the ledger records for the member.
    fn loans_recorded_for(
        &mut self,
        member: &MemberId,
    ) -> impl Future<Output = Result<usize, PortError>> + Send;
}

/// Lending cannot answer now: its store is unavailable.
#[derive(Debug)]
pub struct LendingUnavailable(pub PortError);

impl fmt::Display for LendingUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("lending is unavailable")
    }
}

impl std::error::Error for LendingUnavailable {}
impl UseCaseFailure for LendingUnavailable {}

impl From<PortError> for LendingUnavailable {
    fn from(failure: PortError) -> Self {
        LendingUnavailable(failure)
    }
}

/// Refused: lending knows no such copy.
#[derive(Debug, Clone)]
pub struct UnknownCopy {
    pub copy: CopyId,
}

impl Refusal for UnknownCopy {}
