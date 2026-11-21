//! Removes a thing: a use case over its own transaction unit, whose repository borrows the unit's
//! transaction.

use core::fmt;
use core::marker::PhantomData;
use std::future::Future;

use alpha_domain::thing::{Thing, ThingId};
use dca_building_blocks::application::{
    ScopedFutureExt, TransactionBoundary, TransactionUnit, UseCaseFailure,
};
use dca_building_blocks::hexagonal::port_in::InputPort;
use dca_building_blocks::hexagonal::port_out::{PortError, Repository};

pub struct RemoveThingCommand {
    pub thing: ThingId,
}

#[derive(Debug)]
pub enum RemoveThingError {
    Unavailable(PortError),
}

impl fmt::Display for RemoveThingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not removed")
    }
}

impl std::error::Error for RemoveThingError {}
impl UseCaseFailure for RemoveThingError {}

impl From<PortError> for RemoveThingError {
    fn from(e: PortError) -> Self {
        RemoveThingError::Unavailable(e)
    }
}

/// What removing a thing works with inside its transaction: the things, borrowed from it.
pub trait RemoveThingUnit: TransactionUnit + Send {
    type Things<'a>: Repository<Thing> + Send
    where
        Self: 'a;
    fn things(&mut self) -> Self::Things<'_>;
}

pub trait RemoveThingInputPort: InputPort {
    fn remove(
        &self,
        command: RemoveThingCommand,
    ) -> impl Future<Output = Result<(), RemoveThingError>> + Send;
}

pub struct RemoveThingUseCase<T, U> {
    transactions: T,
    unit: PhantomData<fn() -> U>,
}

impl<T, U> RemoveThingUseCase<T, U> {
    pub fn new(transactions: T) -> Self {
        Self { transactions, unit: PhantomData }
    }
}

impl<T, U> InputPort for RemoveThingUseCase<T, U> {}

impl<T, U> RemoveThingInputPort for RemoveThingUseCase<T, U>
where
    T: TransactionBoundary<U> + Sync,
    U: RemoveThingUnit,
{
    fn remove(
        &self,
        command: RemoveThingCommand,
    ) -> impl Future<Output = Result<(), RemoveThingError>> + Send {
        self.transactions.in_transaction(move |unit| {
            async move {
                unit.things().delete_by_id(&command.thing).await?;
                Ok(())
            }
            .scope_boxed()
        })
    }
}
