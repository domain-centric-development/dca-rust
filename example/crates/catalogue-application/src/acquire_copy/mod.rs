//! Adds a copy the library acquired to its title.

use core::fmt;
use core::marker::PhantomData;

use catalogue_domain::title::{CatalogueFailure, ShelfMark, ShelfMarks, Title, TitleEvent};
use dca_building_blocks::application::{
    BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit, UseCaseFailure,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, PortError, Repository};
use kernel_application::{Clock, IdSource};
use kernel_domain::{CopyId, TitleId};

#[derive(Debug, Clone)]
pub struct AcquireCopyCommand {
    pub title: TitleId,
}

#[derive(Debug, Clone)]
pub struct AcquireCopyResult {
    pub copy: CopyId,
    pub shelf_mark: ShelfMark,
}

#[derive(Debug)]
pub enum AcquireCopyError {
    TitleNotFound,
    Refused(CatalogueFailure),
    Unavailable(PortError),
}

impl fmt::Display for AcquireCopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AcquireCopyError::TitleNotFound => f.write_str("no such title"),
            AcquireCopyError::Refused(refused) => refused.fmt(f),
            AcquireCopyError::Unavailable(_) => f.write_str("the catalogue is unavailable"),
        }
    }
}

impl std::error::Error for AcquireCopyError {}
impl UseCaseFailure for AcquireCopyError {}

impl From<PortError> for AcquireCopyError {
    fn from(failure: PortError) -> Self {
        AcquireCopyError::Unavailable(failure)
    }
}

pub trait AcquireCopyInputPort:
    UseCase<AcquireCopyCommand, Output = Result<AcquireCopyResult, AcquireCopyError>>
{
}

/// Every use case that answers the command this way is the port: the bounds are written once,
/// on its `UseCase` impl.
impl<P> AcquireCopyInputPort for P where
    P: UseCase<AcquireCopyCommand, Output = Result<AcquireCopyResult, AcquireCopyError>>
{
}

/// What acquiring a copy works with inside its transaction.
pub trait AcquireCopyUnit: TransactionUnit + Send {
    type Titles<'a>: Repository<Title> + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<TitleEvent> + Send
    where
        Self: 'a;
    fn titles(&mut self) -> Self::Titles<'_>;
    fn events(&mut self) -> Self::Events<'_>;
}

pub struct AcquireCopyUseCase<U, T, C, I, M> {
    transactions: T,
    clock: C,
    ids: I,
    marks: M,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I, M> AcquireCopyUseCase<U, T, C, I, M> {
    pub fn new(transactions: T, clock: C, ids: I, marks: M) -> Self {
        Self {
            transactions,
            clock,
            ids,
            marks,
            unit: PhantomData,
        }
    }
}

impl<U, T, C, I, M> InputPort for AcquireCopyUseCase<U, T, C, I, M> {}

impl<U, T, C, I, M> UseCase<AcquireCopyCommand> for AcquireCopyUseCase<U, T, C, I, M>
where
    U: AcquireCopyUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
    M: ShelfMarks + Sync,
{
    type Output = Result<AcquireCopyResult, AcquireCopyError>;

    fn execute(&self, command: AcquireCopyCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                let mut title = unit
                    .titles()
                    .find_by_id(&command.title)
                    .await?
                    .ok_or(AcquireCopyError::TitleNotFound)?;
                let copy = CopyId::from_u128(self.ids.next_id());
                let shelf_mark = title
                    .acquire_copy(
                        &self.marks,
                        copy,
                        self.ids.next_event_id(),
                        self.clock.now(),
                    )
                    .map_err(AcquireCopyError::Refused)?;
                unit.titles().save(&title).await?;
                unit.events().publish_and_clear_events(&mut title).await?;
                Ok(AcquireCopyResult { copy, shelf_mark })
            }
            .scope_boxed()
        }))
    }
}
