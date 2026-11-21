//! A member reserves a copy.

use core::marker::PhantomData;

use dca_building_blocks::application::{
    BoxFuture, ClosedResult, Refusal, ScopedFutureExt, TransactionBoundary, TransactionUnit,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};
use kernel_application::{Clock, IdSource};
use kernel_domain::CopyId;
use lending_domain::member::MemberId;
use lending_domain::reservation::{Reservation, ReservationEvent, ReservationId};

use crate::shared::{
    LendableCopyRepository, LendingUnavailable, ReservationRepository, UnknownCopy,
};

#[derive(Debug, Clone)]
pub struct ReserveCopyCommand {
    pub member: MemberId,
    pub copy: CopyId,
}

#[derive(Debug, Clone)]
pub struct Reserved {
    pub reservation: ReservationId,
}

#[derive(Debug, Clone)]
pub enum ReserveCopyResult {
    Reserved(Reserved),
    UnknownCopy(UnknownCopy),
}

impl ClosedResult for ReserveCopyResult {
    fn refusal(&self) -> Option<&dyn Refusal> {
        match self {
            ReserveCopyResult::Reserved(_) => None,
            ReserveCopyResult::UnknownCopy(refused) => Some(refused),
        }
    }
}

pub trait ReserveCopyInputPort:
    UseCase<ReserveCopyCommand, Output = Result<ReserveCopyResult, LendingUnavailable>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> ReserveCopyInputPort for P where
    P: UseCase<ReserveCopyCommand, Output = Result<ReserveCopyResult, LendingUnavailable>>
{
}

/// What reserving a copy works with inside its transaction.
pub trait ReserveCopyUnit: TransactionUnit + Send {
    type Copies<'a>: LendableCopyRepository + Send
    where
        Self: 'a;
    type Reservations<'a>: ReservationRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<ReservationEvent> + Send
    where
        Self: 'a;
    fn copies(&mut self) -> Self::Copies<'_>;
    fn reservations(&mut self) -> Self::Reservations<'_>;
    fn events(&mut self) -> Self::Events<'_>;
}

pub struct ReserveCopyUseCase<U, T, C, I> {
    transactions: T,
    clock: C,
    ids: I,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I> ReserveCopyUseCase<U, T, C, I> {
    pub fn new(transactions: T, clock: C, ids: I) -> Self {
        Self {
            transactions,
            clock,
            ids,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I> InputPort for ReserveCopyUseCase<U, T, C, I> {}

impl<U, T, C, I> UseCase<ReserveCopyCommand> for ReserveCopyUseCase<U, T, C, I>
where
    U: ReserveCopyUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
{
    type Output = Result<ReserveCopyResult, LendingUnavailable>;

    fn execute(&self, command: ReserveCopyCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction_closed(move |unit| {
            async move {
                if unit.copies().find_by_id(&command.copy).await?.is_none() {
                    return Ok(ReserveCopyResult::UnknownCopy(UnknownCopy {
                        copy: command.copy,
                    }));
                }
                let id = ReservationId::from_u128(self.ids.next_id());
                let mut reservation = Reservation::place(
                    id,
                    command.member,
                    command.copy,
                    self.ids.next_event_id(),
                    self.clock.now(),
                );
                unit.reservations().save(&reservation).await?;
                unit.events()
                    .publish_and_clear_events(&mut reservation)
                    .await?;
                Ok(ReserveCopyResult::Reserved(Reserved { reservation: id }))
            }
            .scope_boxed()
        }))
    }
}
