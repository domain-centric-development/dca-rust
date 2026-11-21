use dca_building_blocks::ddd::tactical::{EventId, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Repository};
use alpha_domain::thing::{KeeperId, PartId, Thing, ThingId};

/// A thing belongs to its keeper: the scoped lookup takes the keeper first, the position is free.
pub trait ThingRepository: Repository<Thing> {
    fn find_by_id_for_keeper(
        &mut self,
        keeper: &KeeperId,
        id: &ThingId,
    ) -> impl std::future::Future<Output = Result<Option<Thing>, PortError>> + Send;
}

pub trait Clock: OutputPort {
    fn now(&self) -> Timestamp;
}

pub trait IdSource: OutputPort {
    fn next_part_id(&self) -> PartId;
    fn next_event_id(&self) -> EventId;
}

/// Declared by the repository port beside it: a number only the store can see is taken. The adapter
/// returns it, the use case matches it.
#[derive(Debug)]
pub struct ThingNumberTaken {
    pub number: u64,
}

impl core::fmt::Display for ThingNumberTaken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("thing number taken")
    }
}

impl std::error::Error for ThingNumberTaken {}
impl dca_building_blocks::application::OutputPortFailure for ThingNumberTaken {}

/// A use-case failure that keeps its cause, the usual way: a boxed error, an io error, a duration
/// (DCA-ERR-007).
#[derive(Debug)]
pub struct StorageFailed {
    pub source: Box<dyn std::error::Error + Send + Sync>,
}
impl core::fmt::Display for StorageFailed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("storage failed")
    }
}
impl std::error::Error for StorageFailed {}
impl dca_building_blocks::application::UseCaseFailure for StorageFailed {}

#[derive(Debug)]
pub struct TooLate {
    pub waited: std::time::Duration,
    pub cause: std::io::Error,
}
impl core::fmt::Display for TooLate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("too late")
    }
}
impl std::error::Error for TooLate {}
impl dca_building_blocks::application::UseCaseFailure for TooLate {}

/// Names a part, if the catalogue knows it: an `Option`, no failure to answer (DCA-ERR-010).
pub trait PartNames: OutputPort {
    fn name_of(&self, part: &PartId) -> Option<ThingId>;
}

pub fn direct<N: PartNames>(names: &N, part: &PartId) -> u8 {
    match names.name_of(part) {
        Some(_) => 1,
        None => 0,
    }
}

pub fn aliased<N: PartNames>(names: &N, part: &PartId) -> u8 {
    let found = names.name_of(part);
    match found {
        Some(_) => 1,
        None => 0,
    }
}

/// A port call's result bound to `found` and propagated; closure parameters, loop, arm and
/// `if let` bindings of the same name are other values (DCA-ERR-010).
pub async fn count_found<R: ThingRepository>(
    things: &mut R,
    keeper: &KeeperId,
    id: &ThingId,
    others: &[Result<u8, u8>],
) -> Result<usize, PortError> {
    let found = things.find_by_id_for_keeper(keeper, id).await;
    let mut count = others.iter().filter(|found| found.is_ok()).count();
    for found in others {
        count += usize::from(found.unwrap_or(0));
    }
    if let Some(found) = others.first() {
        count += usize::from(found.is_ok());
    }
    match others.last() {
        Some(found) => count += usize::from(found.is_err()),
        None => {}
    }
    Ok(count + usize::from(found?.is_some()))
}

/// A catch-all arm that only cleans up and hands what it caught on unchanged does not answer the
/// failure (DCA-ERR-010): every exit, the tail and each `return`, is the binding itself.
pub async fn look_up_or_undo<R: ThingRepository>(
    things: &mut R,
    keeper: &KeeperId,
    id: &ThingId,
    undo: impl Fn(),
) -> Result<Option<Thing>, PortError> {
    let first = match things.find_by_id_for_keeper(keeper, id).await {
        Ok(Some(thing)) => Some(thing),
        other => {
            undo();
            return other;
        }
    };
    match things.find_by_id_for_keeper(keeper, id).await {
        Ok(found) => Ok(found.or(first)),
        caught => {
            if first.is_some() {
                return caught;
            }
            undo();
            caught
        }
    }
}
