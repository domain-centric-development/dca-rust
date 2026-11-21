use core::fmt;
use core::marker::PhantomData;

use alpha_domain::thing::{Label, PartCatalog, PartId, Sizing, ThingError, ThingEvent, ThingId};
use dca_building_blocks::application::{
    BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit, UseCaseFailure,
};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, PortError};

use crate::shared::{Clock, IdSource, ThingRepository};

#[derive(Debug, Clone)]
pub struct AddPartCommand {
    pub thing: ThingId,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct AddPartResult {
    pub part: PartId,
    pub crowded: bool,
    pub listed: bool,
}

#[derive(Debug)]
pub enum AddPartError {
    NotFound,
    Refused(ThingError),
    Unavailable(PortError),
}

impl fmt::Display for AddPartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not added")
    }
}

impl std::error::Error for AddPartError {}
impl UseCaseFailure for AddPartError {}

impl From<PortError> for AddPartError {
    fn from(e: PortError) -> Self {
        AddPartError::Unavailable(e)
    }
}

pub trait AddPartInputPort: UseCase<AddPartCommand, Output = Result<AddPartResult, AddPartError>> {}

/// What adding a part works with inside its transaction: the things and the event publisher, as
/// the transaction sees them.
pub trait AddPartUnit: TransactionUnit + Send {
    type Things<'a>: ThingRepository + Send
    where
        Self: 'a;
    type Events<'a>: DomainEventPublisher<ThingEvent> + Send
    where
        Self: 'a;
    fn things(&mut self) -> Self::Things<'_>;
    fn events(&mut self) -> Self::Events<'_>;
}

pub struct AddPartUseCase<U, T, C, I, K> {
    transactions: T,
    clock: C,
    ids: I,
    sizing: Sizing,
    catalog: K,
    unit: PhantomData<fn() -> U>,
}

impl<U, T, C, I, K> AddPartUseCase<U, T, C, I, K> {
    pub fn new(transactions: T, clock: C, ids: I, sizing: Sizing, catalog: K) -> Self {
        Self { transactions, clock, ids, sizing, catalog, unit: PhantomData }
    }
}

impl<U, T, C, I, K> InputPort for AddPartUseCase<U, T, C, I, K> {}

impl<U, T, C, I, K> UseCase<AddPartCommand> for AddPartUseCase<U, T, C, I, K>
where
    U: AddPartUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
    K: PartCatalog + Sync,
{
    type Output = Result<AddPartResult, AddPartError>;

    fn execute(&self, command: AddPartCommand) -> BoxFuture<'_, Self::Output> {
        let label = match Label::new(&command.label) {
            Ok(label) => label,
            Err(refused) => return Box::pin(async move { Err(AddPartError::Refused(refused)) }),
        };
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                let mut thing = unit.things().find_by_id(&command.thing).await?.ok_or(AddPartError::NotFound)?;
                let listed = self.sizing.is_listed(&thing, &self.catalog, &label).await;
                let part = self.ids.next_part_id();
                thing.add_part(part, label, self.ids.next_event_id(), self.clock.now()).map_err(AddPartError::Refused)?;
                unit.things().save(&thing).await?;
                unit.events().publish_and_clear_events(&mut thing).await?;
                Ok(AddPartResult { part, crowded: self.sizing.is_crowded(&thing), listed })
            }
            .scope_boxed()
        }))
    }
}

impl<U, T, C, I, K> AddPartInputPort for AddPartUseCase<U, T, C, I, K>
where
    U: AddPartUnit,
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    I: IdSource + Sync,
    K: PartCatalog + Sync,
{
}
