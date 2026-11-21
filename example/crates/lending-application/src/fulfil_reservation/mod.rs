//! Closes the reservation a member collected - driven by the copy lent, after lending committed the
//! loan, and passed over when the same delivery arrives again.

use core::marker::PhantomData;

use dca_building_blocks::application::{
    BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};
use kernel_application::{Clock, Delivery, IdSource, Inbox, Received};
use lending_domain::member::MemberId;
use lending_domain::reservation::{ReservationEvent, ReservationId};

use crate::shared::{LendingUnavailable, ReservationRepository};

#[derive(Debug, Clone)]
pub struct FulfilReservationCommand {
    /// The delivery that asks for it: the event's id and the listener's name.
    pub received: Received,
    pub member: MemberId,
    pub reservation: ReservationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfilReservationResult {
    Fulfilled,
    /// The member holds no such reservation.
    NotFound,
    /// It was fulfilled before.
    AlreadyFulfilled,
    AlreadyReceived,
}

pub trait FulfilReservationInputPort:
    UseCase<FulfilReservationCommand, Output = Result<FulfilReservationResult, LendingUnavailable>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> FulfilReservationInputPort for P where
    P: UseCase<
            FulfilReservationCommand,
            Output = Result<FulfilReservationResult, LendingUnavailable>,
        >
{
}

/// What fulfilling a reservation works with inside its transaction.
pub trait FulfilReservationUnit: TransactionUnit + Send {
    type Reservations<'a>: ReservationRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<ReservationEvent> + Send
    where
        Self: 'a;
    type Inbox<'a>: Inbox + Send
    where
        Self: 'a;
    fn reservations(&mut self) -> Self::Reservations<'_>;
    fn events(&mut self) -> Self::Events<'_>;
    fn inbox(&mut self) -> Self::Inbox<'_>;
}

pub struct FulfilReservationUseCase<U, T, C, I> {
    transactions: T,
    clock: C,
    ids: I,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I> FulfilReservationUseCase<U, T, C, I> {
    pub fn new(transactions: T, clock: C, ids: I) -> Self {
        Self {
            transactions,
            clock,
            ids,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I> InputPort for FulfilReservationUseCase<U, T, C, I> {}

impl<U, T, C, I> UseCase<FulfilReservationCommand> for FulfilReservationUseCase<U, T, C, I>
where
    U: FulfilReservationUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
{
    type Output = Result<FulfilReservationResult, LendingUnavailable>;

    fn execute(&self, command: FulfilReservationCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                match unit.inbox().receive(command.received).await? {
                    Delivery::Repeated => return Ok(FulfilReservationResult::AlreadyReceived),
                    Delivery::First => {}
                }
                let found = unit
                    .reservations()
                    .find_by_id_for_member(&command.reservation, &command.member)
                    .await?;
                let Some(mut reservation) = found else {
                    return Ok(FulfilReservationResult::NotFound);
                };
                if reservation
                    .fulfil(self.ids.next_event_id(), self.clock.now())
                    .is_err()
                {
                    return Ok(FulfilReservationResult::AlreadyFulfilled);
                }
                unit.reservations().save(&reservation).await?;
                unit.events()
                    .publish_and_clear_events(&mut reservation)
                    .await?;
                Ok(FulfilReservationResult::Fulfilled)
            }
            .scope_boxed()
        }))
    }
}
