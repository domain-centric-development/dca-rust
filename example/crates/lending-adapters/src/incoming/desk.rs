//! The lending desk: members borrow and reserve copies here.

use std::sync::Arc;

use kernel_domain::CopyId;
use lending_application::lend_copy::{LendCopyCommand, LendCopyInputPort, LendCopyResult};
use lending_application::reserve_copy::{
    ReserveCopyCommand, ReserveCopyInputPort, ReserveCopyResult,
};
use lending_application::shared::LendingUnavailable;
use lending_domain::copy::LendingFailure;
use lending_domain::member::MemberId;
use lending_domain::reservation::ReservationId;

/// What the desk answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeskResponse {
    /// Done: the new loan's or reservation's id.
    Done { id: u128 },
    /// Refused, with the reason's code.
    Refused { reason: &'static str },
    /// Lending cannot answer now.
    Unavailable,
}

/// The desk over lending's input ports.
pub struct LendingDesk {
    lend: Arc<dyn LendCopyInputPort + Send + Sync>,
    reserve: Arc<dyn ReserveCopyInputPort + Send + Sync>,
}

impl LendingDesk {
    /// The desk over these ports.
    pub fn new(
        lend: Arc<dyn LendCopyInputPort + Send + Sync>,
        reserve: Arc<dyn ReserveCopyInputPort + Send + Sync>,
    ) -> Self {
        Self { lend, reserve }
    }

    /// A member borrows a copy, collecting their reservation if they name one.
    pub async fn lend(&self, member: u128, copy: u128, reservation: Option<u128>) -> DeskResponse {
        let command = LendCopyCommand {
            copy: CopyId::from_u128(copy),
            member: MemberId::from_u128(member),
            reservation: reservation.map(ReservationId::from_u128),
        };
        match self.lend.execute(command).await {
            Ok(LendCopyResult::Lent(lent)) => DeskResponse::Done {
                id: lent.loan.as_u128(),
            },
            Ok(LendCopyResult::UnknownCopy(_)) => DeskResponse::Refused {
                reason: "unknown-copy",
            },
            Ok(LendCopyResult::NotLendable(refused)) => DeskResponse::Refused {
                reason: match refused.failure {
                    LendingFailure::NotOnShelf => "not-on-shelf",
                    LendingFailure::NotOnLoan => "not-on-loan",
                    LendingFailure::ReservationClosed => "reservation-closed",
                },
            },
            Ok(LendCopyResult::MemberSuspended(_)) => DeskResponse::Refused {
                reason: "member-suspended",
            },
            Ok(LendCopyResult::LoanLimitReached(_)) => DeskResponse::Refused {
                reason: "loan-limit-reached",
            },
            Err(LendingUnavailable(_)) => DeskResponse::Unavailable,
        }
    }

    /// A member reserves a copy.
    pub async fn reserve(&self, member: u128, copy: u128) -> DeskResponse {
        let command = ReserveCopyCommand {
            member: MemberId::from_u128(member),
            copy: CopyId::from_u128(copy),
        };
        match self.reserve.execute(command).await {
            Ok(ReserveCopyResult::Reserved(reserved)) => DeskResponse::Done {
                id: reserved.reservation.as_u128(),
            },
            Ok(ReserveCopyResult::UnknownCopy(_)) => DeskResponse::Refused {
                reason: "unknown-copy",
            },
            Err(LendingUnavailable(_)) => DeskResponse::Unavailable,
        }
    }
}
