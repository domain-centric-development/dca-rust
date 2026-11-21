//! Enters a new title into the catalogue.

use core::fmt;
use core::marker::PhantomData;

use catalogue_domain::title::{CatalogueFailure, TitleEvent, TitleFactory, TitleName};
use dca_building_blocks::application::{
    BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit, UseCaseFailure,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, PortError};
use kernel_application::{Clock, IdSource};
use kernel_domain::TitleId;

use crate::shared::{TitleNameTaken, TitleRepository};

#[derive(Debug, Clone)]
pub struct RegisterTitleCommand {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct RegisterTitleResult {
    pub title: TitleId,
}

#[derive(Debug)]
pub enum RegisterTitleError {
    Refused(CatalogueFailure),
    NameTaken(TitleNameTaken),
    Unavailable(PortError),
}

impl fmt::Display for RegisterTitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterTitleError::Refused(refused) => refused.fmt(f),
            RegisterTitleError::NameTaken(taken) => taken.fmt(f),
            RegisterTitleError::Unavailable(_) => f.write_str("the catalogue is unavailable"),
        }
    }
}

impl std::error::Error for RegisterTitleError {}
impl UseCaseFailure for RegisterTitleError {}

impl From<PortError> for RegisterTitleError {
    fn from(failure: PortError) -> Self {
        RegisterTitleError::Unavailable(failure)
    }
}

pub trait RegisterTitleInputPort:
    UseCase<RegisterTitleCommand, Output = Result<RegisterTitleResult, RegisterTitleError>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> RegisterTitleInputPort for P where
    P: UseCase<RegisterTitleCommand, Output = Result<RegisterTitleResult, RegisterTitleError>>
{
}

/// What registering a title works with inside its transaction.
pub trait RegisterTitleUnit: TransactionUnit + Send {
    type Titles<'a>: TitleRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<TitleEvent> + Send
    where
        Self: 'a;
    fn titles(&mut self) -> Self::Titles<'_>;
    fn events(&mut self) -> Self::Events<'_>;
}

pub struct RegisterTitleUseCase<U, T, C, I> {
    transactions: T,
    clock: C,
    ids: I,
    factory: TitleFactory,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I> RegisterTitleUseCase<U, T, C, I> {
    pub fn new(transactions: T, clock: C, ids: I, factory: TitleFactory) -> Self {
        Self {
            transactions,
            clock,
            ids,
            factory,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I> InputPort for RegisterTitleUseCase<U, T, C, I> {}

impl<U, T, C, I> UseCase<RegisterTitleCommand> for RegisterTitleUseCase<U, T, C, I>
where
    U: RegisterTitleUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
{
    type Output = Result<RegisterTitleResult, RegisterTitleError>;

    fn execute(&self, command: RegisterTitleCommand) -> BoxFuture<'_, Self::Output> {
        let name = match TitleName::new(&command.name) {
            Ok(name) => name,
            Err(refused) => {
                return Box::pin(async move { Err(RegisterTitleError::Refused(refused)) });
            }
        };
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                let id = TitleId::from_u128(self.ids.next_id());
                let mut title =
                    self.factory
                        .register(id, name, self.ids.next_event_id(), self.clock.now());
                unit.titles()
                    .add(&title)
                    .await?
                    .map_err(RegisterTitleError::NameTaken)?;
                unit.events().publish_and_clear_events(&mut title).await?;
                Ok(RegisterTitleResult { title: id })
            }
            .scope_boxed()
        }))
    }
}
