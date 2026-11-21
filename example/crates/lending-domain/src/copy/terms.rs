use dca_building_blocks::ddd::tactical::{DomainService, Timestamp, Value};

use super::LendableCopy;
use crate::member::{MemberId, MemberStandings, Standing};

const MILLIS_PER_DAY: i64 = 24 * 60 * 60 * 1000;

/// How long a loan runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoanPeriod(u16);

impl LoanPeriod {
    /// A period of whole days.
    pub const fn of_days(days: u16) -> LoanPeriod {
        LoanPeriod(days)
    }

    /// The day a loan that starts at `start` is due back.
    pub fn due_from(&self, start: Timestamp) -> Timestamp {
        Timestamp::from_unix_millis(start.as_unix_millis() + i64::from(self.0) * MILLIS_PER_DAY)
    }

    fn halved(&self) -> LoanPeriod {
        LoanPeriod((self.0 / 2).max(1))
    }
}

impl Value for LoanPeriod {}

/// The library's terms of lending: how long a member may keep a copy, if at all. It asks the
/// membership office for the member's standing and awaits the answer - the one asynchronous part
/// of the domain, beside the gateway it asks.
#[derive(Debug, Clone, Copy)]
pub struct LoanTerms {
    regular: LoanPeriod,
}

impl DomainService for LoanTerms {}

/// How many loans make a copy popular, with a shorter period.
const POPULAR_AFTER: usize = 3;

impl LoanTerms {
    /// Terms with this regular period.
    pub const fn new(regular: LoanPeriod) -> LoanTerms {
        LoanTerms { regular }
    }

    /// The period the member may keep the copy, or nothing for a suspended member. A popular copy
    /// goes out for half the period.
    pub async fn period_for<S: MemberStandings + Sync>(
        &self,
        copy: &LendableCopy,
        member: &MemberId,
        standings: &S,
    ) -> Option<LoanPeriod> {
        match standings.standing_of(member).await {
            Standing::Suspended => None,
            Standing::Good if copy.loans().len() >= POPULAR_AFTER => Some(self.regular.halved()),
            Standing::Good => Some(self.regular),
        }
    }
}
