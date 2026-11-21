//! Puts a copy the catalogue acquired on lending's shelf - driven by the catalogue's event, after
//! the catalogue committed, and passed over when the same delivery arrives again.

use core::marker::PhantomData;

use dca_building_blocks::application::{
    BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};
use kernel_application::{Clock, Delivery, IdSource, Inbox, Received};
use kernel_domain::{CopyId, TitleId};
use lending_domain::copy::{CopyEvent, LendableCopy};

use crate::shared::{LendableCopyRepository, LendingUnavailable};

#[derive(Debug, Clone)]
pub struct ShelveCopyCommand {
    /// The delivery that asks for it: the event's id and the listener's name.
    pub received: Received,
    pub copy: CopyId,
    pub title: TitleId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelveCopyResult {
    Shelved,
    AlreadyReceived,
}

pub trait ShelveCopyInputPort:
    UseCase<ShelveCopyCommand, Output = Result<ShelveCopyResult, LendingUnavailable>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> ShelveCopyInputPort for P where
    P: UseCase<ShelveCopyCommand, Output = Result<ShelveCopyResult, LendingUnavailable>>
{
}

/// What shelving a copy works with inside its transaction - the inbox among it.
pub trait ShelveCopyUnit: TransactionUnit + Send {
    type Copies<'a>: LendableCopyRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<CopyEvent> + Send
    where
        Self: 'a;
    type Inbox<'a>: Inbox + Send
    where
        Self: 'a;
    fn copies(&mut self) -> Self::Copies<'_>;
    fn events(&mut self) -> Self::Events<'_>;
    fn inbox(&mut self) -> Self::Inbox<'_>;
}

pub struct ShelveCopyUseCase<U, T, C, I> {
    transactions: T,
    clock: C,
    ids: I,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I> ShelveCopyUseCase<U, T, C, I> {
    pub fn new(transactions: T, clock: C, ids: I) -> Self {
        Self {
            transactions,
            clock,
            ids,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I> InputPort for ShelveCopyUseCase<U, T, C, I> {}

impl<U, T, C, I> UseCase<ShelveCopyCommand> for ShelveCopyUseCase<U, T, C, I>
where
    U: ShelveCopyUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
{
    type Output = Result<ShelveCopyResult, LendingUnavailable>;

    fn execute(&self, command: ShelveCopyCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                match unit.inbox().receive(command.received).await? {
                    Delivery::Repeated => return Ok(ShelveCopyResult::AlreadyReceived),
                    Delivery::First => {}
                }
                let mut copy = LendableCopy::shelve(
                    command.copy,
                    command.title,
                    self.ids.next_event_id(),
                    self.clock.now(),
                );
                unit.copies().save(&copy).await?;
                unit.events().publish_and_clear_events(&mut copy).await?;
                Ok(ShelveCopyResult::Shelved)
            }
            .scope_boxed()
        }))
    }
}
