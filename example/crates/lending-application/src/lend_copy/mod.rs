//! A member borrows a copy - for the period the terms of lending give them, within their limit of
//! loans.

use core::marker::PhantomData;

use dca_building_blocks::application::{
    BoxFuture, ClosedResult, Refusal, ScopedFutureExt, TransactionBoundary, TransactionUnit,
};
use dca_building_blocks::ddd::tactical::Timestamp;
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};
use kernel_application::{Clock, IdSource};
use kernel_domain::CopyId;
use lending_domain::copy::{CopyEvent, LendingFailure, LoanId, LoanTerms};
use lending_domain::member::{MemberId, MemberStandings};
use lending_domain::reservation::ReservationId;

use crate::shared::{
    LendableCopyRepository, LendingUnavailable, LoanLedger, LoanRecord, UnknownCopy,
};

/// How many loans the ledger may record for one member.
pub const LOAN_LIMIT: usize = 2;

#[derive(Debug, Clone)]
pub struct LendCopyCommand {
    pub copy: CopyId,
    pub member: MemberId,
    /// The reservation the member collects, if any.
    pub reservation: Option<ReservationId>,
}

#[derive(Debug, Clone)]
pub struct Lent {
    pub loan: LoanId,
    pub due: Timestamp,
}

/// Refused: the domain does not lend the copy now.
#[derive(Debug, Clone)]
pub struct CopyNotLendable {
    pub copy: CopyId,
    pub failure: LendingFailure,
}

impl Refusal for CopyNotLendable {}

/// Refused: the member is suspended.
#[derive(Debug, Clone)]
pub struct MemberSuspended {
    pub member: MemberId,
}

impl Refusal for MemberSuspended {}

/// Refused: the member has borrowed as many copies as they may.
#[derive(Debug, Clone)]
pub struct LoanLimitReached {
    pub member: MemberId,
    pub limit: usize,
}

impl Refusal for LoanLimitReached {}

#[derive(Debug, Clone)]
pub enum LendCopyResult {
    Lent(Lent),
    UnknownCopy(UnknownCopy),
    NotLendable(CopyNotLendable),
    MemberSuspended(MemberSuspended),
    LoanLimitReached(LoanLimitReached),
}

impl ClosedResult for LendCopyResult {
    fn refusal(&self) -> Option<&dyn Refusal> {
        match self {
            LendCopyResult::Lent(_) => None,
            LendCopyResult::UnknownCopy(refused) => Some(refused),
            LendCopyResult::NotLendable(refused) => Some(refused),
            LendCopyResult::MemberSuspended(refused) => Some(refused),
            LendCopyResult::LoanLimitReached(refused) => Some(refused),
        }
    }
}

pub trait LendCopyInputPort:
    UseCase<LendCopyCommand, Output = Result<LendCopyResult, LendingUnavailable>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> LendCopyInputPort for P where
    P: UseCase<LendCopyCommand, Output = Result<LendCopyResult, LendingUnavailable>>
{
}

/// What lending a copy works with inside its transaction.
pub trait LendCopyUnit: TransactionUnit + Send {
    type Copies<'a>: LendableCopyRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<CopyEvent> + Send
    where
        Self: 'a;
    type Ledger<'a>: LoanLedger + Send
    where
        Self: 'a;
    fn copies(&mut self) -> Self::Copies<'_>;
    fn events(&mut self) -> Self::Events<'_>;
    fn ledger(&mut self) -> Self::Ledger<'_>;
}

pub struct LendCopyUseCase<U, T, C, I, S> {
    transactions: T,
    clock: C,
    ids: I,
    terms: LoanTerms,
    standings: S,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I, S> LendCopyUseCase<U, T, C, I, S> {
    pub fn new(transactions: T, clock: C, ids: I, terms: LoanTerms, standings: S) -> Self {
        Self {
            transactions,
            clock,
            ids,
            terms,
            standings,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I, S> InputPort for LendCopyUseCase<U, T, C, I, S> {}

impl<U, T, C, I, S> UseCase<LendCopyCommand> for LendCopyUseCase<U, T, C, I, S>
where
    U: LendCopyUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
    S: MemberStandings + Sync,
{
    type Output = Result<LendCopyResult, LendingUnavailable>;

    fn execute(&self, command: LendCopyCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction_closed(move |unit| {
            async move {
                let member = command.member;
                let Some(mut copy) = unit.copies().find_by_id(&command.copy).await? else {
                    return Ok(LendCopyResult::UnknownCopy(UnknownCopy {
                        copy: command.copy,
                    }));
                };
                let Some(period) = self.terms.period_for(&copy, &member, &self.standings).await
                else {
                    return Ok(LendCopyResult::MemberSuspended(MemberSuspended { member }));
                };
                let loan = LoanId::from_u128(self.ids.next_id());
                let now = self.clock.now();
                let lent = copy.lend(
                    loan,
                    member,
                    command.reservation,
                    period,
                    self.ids.next_event_id(),
                    now,
                );
                let due = match lent {
                    Ok(due) => due,
                    Err(failure) => {
                        return Ok(LendCopyResult::NotLendable(CopyNotLendable {
                            copy: command.copy,
                            failure,
                        }));
                    }
                };
                unit.copies().save(&copy).await?;
                unit.events().publish_and_clear_events(&mut copy).await?;
                let record = LoanRecord {
                    loan,
                    copy: command.copy,
                    member,
                    lent_on: now,
                };
                unit.ledger().append(record).await?;
                // The ledger counts the loan just made: over the limit, the transaction is rolled
                // back - the copy, its event and the record alike.
                if unit.ledger().loans_recorded_for(&member).await? > LOAN_LIMIT {
                    return Ok(LendCopyResult::LoanLimitReached(LoanLimitReached {
                        member,
                        limit: LOAN_LIMIT,
                    }));
                }
                Ok(LendCopyResult::Lent(Lent { loan, due }))
            }
            .scope_boxed()
        }))
    }
}
