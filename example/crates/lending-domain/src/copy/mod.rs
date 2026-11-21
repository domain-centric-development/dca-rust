//! A copy as lending sees it: on the shelf, or out on loan to a member.

mod event;
mod loan;
mod terms;

pub use crate::failure::LendingFailure;
pub use event::{CopyEvent, CopyLent, CopyReturned, CopyShelved};
pub use loan::{ActiveLoan, Loan, LoanId, ReturnedLoan};
pub use terms::{LoanPeriod, LoanTerms};

use dca_building_blocks::ddd::tactical::{
    AggregateRoot, DomainEvents, Entity, EventId, Specification, Timestamp,
};
use kernel_domain::{CopyId, TitleId};

use crate::member::MemberId;
use crate::reservation::ReservationId;

/// A copy that may be lent: the aggregate root over its loans.
#[derive(Debug, Clone)]
pub struct LendableCopy {
    id: CopyId,
    title: TitleId,
    loans: Vec<Loan>,
    events: DomainEvents<CopyEvent>,
}

impl LendableCopy {
    /// A copy put on the shelf, ready to be lent.
    pub fn shelve(id: CopyId, title: TitleId, event: EventId, at: Timestamp) -> LendableCopy {
        let mut copy = LendableCopy {
            id,
            title,
            loans: Vec::new(),
            events: DomainEvents::new(),
        };
        copy.events.record(CopyEvent::CopyShelved(CopyShelved {
            event,
            copy: id,
            title,
            occurred_on: at,
        }));
        copy
    }

    /// The title it is a copy of.
    pub fn title(&self) -> &TitleId {
        &self.title
    }

    /// Its loans, the running one last.
    pub fn loans(&self) -> &[Loan] {
        &self.loans
    }

    /// Lends the copy to a member for a period, collecting their reservation if they name one.
    /// Answers when it is due back.
    pub fn lend(
        &mut self,
        loan: LoanId,
        member: MemberId,
        reservation: Option<ReservationId>,
        period: LoanPeriod,
        event: EventId,
        at: Timestamp,
    ) -> Result<Timestamp, LendingFailure> {
        if !OnShelfSpecification.is_satisfied_by(self) {
            return Err(LendingFailure::NotOnShelf);
        }
        let due = period.due_from(at);
        self.loans.push(Loan::begin(loan, member, due));
        self.events.record(CopyEvent::CopyLent(CopyLent {
            event,
            copy: self.id,
            loan,
            member,
            reservation,
            due,
            occurred_on: at,
        }));
        Ok(due)
    }

    /// Takes the copy back.
    pub fn take_back(&mut self, event: EventId, at: Timestamp) -> Result<(), LendingFailure> {
        let Some(running) = self.loans.pop_if(|loan| loan.is_active()) else {
            return Err(LendingFailure::NotOnLoan);
        };
        let loan = *running.id();
        self.loans.push(running.returned(at));
        self.events.record(CopyEvent::CopyReturned(CopyReturned {
            event,
            copy: self.id,
            loan,
            occurred_on: at,
        }));
        Ok(())
    }
}

impl Entity for LendableCopy {
    type Id = CopyId;
    fn id(&self) -> &CopyId {
        &self.id
    }
}

impl AggregateRoot for LendableCopy {
    type Event = CopyEvent;
    fn domain_events(&self) -> &[CopyEvent] {
        &self.events
    }
    fn take_domain_events(&mut self) -> Vec<CopyEvent> {
        self.events.take()
    }
}

/// A copy on the shelf: none of its loans is running.
#[derive(Debug, Clone, Copy, Default)]
pub struct OnShelfSpecification;

impl Specification<LendableCopy> for OnShelfSpecification {
    fn is_satisfied_by(&self, candidate: &LendableCopy) -> bool {
        !candidate.loans.iter().any(Loan::is_active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_unix_millis(millis)
    }

    fn shelved() -> LendableCopy {
        let mut copy = LendableCopy::shelve(
            CopyId::from_u128(1),
            TitleId::from_u128(2),
            EventId::from_u128(1),
            at(0),
        );
        copy.take_domain_events();
        copy
    }

    #[test]
    fn a_copy_on_the_shelf_is_lent_and_due_after_the_period() {
        let mut copy = shelved();
        let due = copy.lend(
            LoanId::from_u128(3),
            MemberId::from_u128(4),
            None,
            LoanPeriod::of_days(1),
            EventId::from_u128(2),
            at(0),
        );
        assert_eq!(due, Ok(at(86_400_000)));
        assert!(!OnShelfSpecification.is_satisfied_by(&copy));
        assert!(matches!(copy.domain_events(), [CopyEvent::CopyLent(_)]));
    }

    #[test]
    fn a_copy_out_on_loan_is_not_lent_again() {
        let mut copy = shelved();
        let first = copy.lend(
            LoanId::from_u128(3),
            MemberId::from_u128(4),
            None,
            LoanPeriod::of_days(1),
            EventId::from_u128(2),
            at(0),
        );
        assert!(first.is_ok());
        let second = copy.lend(
            LoanId::from_u128(5),
            MemberId::from_u128(6),
            None,
            LoanPeriod::of_days(1),
            EventId::from_u128(3),
            at(1),
        );
        assert_eq!(second, Err(LendingFailure::NotOnShelf));
    }

    #[test]
    fn a_returned_loan_leaves_the_copy_on_the_shelf() {
        let mut copy = shelved();
        assert_eq!(
            copy.take_back(EventId::from_u128(2), at(1)),
            Err(LendingFailure::NotOnLoan)
        );
        let lent = copy.lend(
            LoanId::from_u128(3),
            MemberId::from_u128(4),
            None,
            LoanPeriod::of_days(1),
            EventId::from_u128(3),
            at(0),
        );
        assert!(lent.is_ok());
        assert_eq!(copy.take_back(EventId::from_u128(4), at(5)), Ok(()));
        assert!(OnShelfSpecification.is_satisfied_by(&copy));
        assert!(
            matches!(copy.loans(), [Loan::Returned(returned)] if returned.returned_on() == at(5))
        );
    }
}
