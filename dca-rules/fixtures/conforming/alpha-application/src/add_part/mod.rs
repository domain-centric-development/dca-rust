use core::fmt;

use alpha_domain::thing::{Label, PartId, Sizing, ThingError, ThingEvent, ThingId};
use dca_building_blocks::application::{TransactionBoundary, UseCaseFailure};
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

pub struct AddPartUseCase<R, P, T, C, I> {
    things: R,
    events: P,
    transactions: T,
    clock: C,
    ids: I,
    sizing: Sizing,
}

impl<R, P, T, C, I> AddPartUseCase<R, P, T, C, I> {
    pub fn new(things: R, events: P, transactions: T, clock: C, ids: I, sizing: Sizing) -> Self {
        Self { things, events, transactions, clock, ids, sizing }
    }
}

impl<R, P, T, C, I> InputPort for AddPartUseCase<R, P, T, C, I> {}

impl<R, P, T, C, I> UseCase<AddPartCommand> for AddPartUseCase<R, P, T, C, I>
where
    R: ThingRepository,
    P: DomainEventPublisher<ThingEvent>,
    T: TransactionBoundary,
    C: Clock,
    I: IdSource,
{
    type Output = Result<AddPartResult, AddPartError>;

    fn execute(&self, command: AddPartCommand) -> Self::Output {
        let label = Label::new(&command.label).map_err(AddPartError::Refused)?;
        self.transactions.in_transaction(|| {
            let mut thing = self.things.find_by_id(&command.thing)?.ok_or(AddPartError::NotFound)?;
            let part = self.ids.next_part_id();
            thing.add_part(part, label, self.ids.next_event_id(), self.clock.now()).map_err(AddPartError::Refused)?;
            self.things.save(&thing)?;
            self.events.publish_and_clear_events(&mut thing)?;
            Ok(AddPartResult { part, crowded: self.sizing.is_crowded(&thing) })
        })
    }
}

impl<R, P, T, C, I> AddPartInputPort for AddPartUseCase<R, P, T, C, I>
where
    R: ThingRepository,
    P: DomainEventPublisher<ThingEvent>,
    T: TransactionBoundary,
    C: Clock,
    I: IdSource,
{
}
