use std::cell::Cell;

use alpha_domain::{Part, Thing, ThingEvent};
use dca_building_blocks::application::UseCaseFailure;
use dca_building_blocks::ddd::tactical::{DomainEvent, DomainFailure, DomainService, EventId, Factory, IntegrationEvent, Timestamp};
use dca_building_blocks::hexagonal::port_out::{OutputPort, Repository, Store};

/// DCA-CYC-002: the shared module and a use case name each other.
pub mod shared {
    pub trait Helper {
        fn query(&self) -> crate::lookup::LookupQuery;
    }
}

pub trait InputPort {}

pub trait LooseRepository {}
pub trait ThingsRepositoryPort {}
pub trait CacheStore {}
pub trait Stash: Store {
    fn save(&self);
}
pub struct Bucket;
impl OutputPort for Bucket {}
impl Store for Bucket {}

pub trait WrongRepository: Repository<Thing> {
    fn part(&self) -> Option<Part>;
}
pub trait PartRepository: Repository<Part> {}

pub struct OpenSpecification;

#[derive(Debug)]
pub struct LostFailure;
impl std::error::Error for LostFailure {}
impl DomainFailure for LostFailure {}

#[derive(Debug)]
pub struct Oops;
impl std::error::Error for Oops {}

pub struct Planner;
impl DomainService for Planner {}

pub struct OutsideFactory;
impl Factory for OutsideFactory {}

#[derive(Debug, Clone)]
pub struct Happened {
    at: Timestamp,
}
impl DomainEvent for Happened {
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}

#[derive(Debug, Clone)]
pub struct Published {
    schema_version: u32,
}
impl IntegrationEvent for Published {
    const TYPE: &'static str = "published";
    const VERSION: u32 = 1;
    fn event_id(&self) -> EventId { todo!() }
    fn occurred_on(&self) -> Timestamp { todo!() }
}
impl Published {
    pub fn touch(&mut self) {}
}

pub struct RootUseCase;
impl dca_building_blocks::hexagonal::port_in::InputPort for RootUseCase {}

pub mod do_thing {
    use std::cell::Cell;

    use alpha_domain::{Thing, ThingEvent};
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::DomainEventPublisher;

    use crate::WrongRepository;

    pub trait DoThingPort: InputPort {}

    pub struct RunCommand {
        cache: Cell<u8>,
    }
    impl RunCommand {
        pub fn bump(&mut self) {}
    }

    pub struct RunQuery {
        cache: Cell<u8>,
    }

    pub struct RunResult {
        thing: Thing,
        cache: Cell<u8>,
    }

    pub struct RunDto;

    pub struct Runner<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> {
        things: R,
        events: P,
        other: Box<dyn crate::other::OtherInputPort>,
    }
    impl<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> InputPort for Runner<R, P> {}
    impl<R: WrongRepository, P: DomainEventPublisher<ThingEvent>> Runner<R, P> {
        pub fn run(&self, thing: &Thing) {
            self.things.save(thing);
        }
    }
}

pub mod other {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use super::do_thing::RunCommand;

    pub trait OtherInputPort: InputPort {}

    pub struct OnArrivalUseCase;
    impl InputPort for OnArrivalUseCase {}

    pub struct SetLimitUseCase;
    impl InputPort for SetLimitUseCase {}

    /// DCA-USE-021: a use case that counts its calls in itself.
    pub struct CountingUseCase {
        calls: std::cell::Cell<u32>,
    }
    impl InputPort for CountingUseCase {}

    /// DCA-USE-022: a use case that answers with a bare number.
    pub trait CountInputPort:
        dca_building_blocks::hexagonal::port_in::UseCase<RunCommand, Output = Result<u32, String>>
    {
    }
}

pub mod lookup {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use crate::WrongRepository;

    pub struct LookupQuery;

    pub fn helped(helper: &dyn crate::shared::Helper) -> LookupQuery {
        helper.query()
    }

    pub struct LookupUseCase<R: WrongRepository> {
        things: R,
    }
    impl<R: WrongRepository> InputPort for LookupUseCase<R> {}
    impl<R: WrongRepository> LookupUseCase<R> {
        pub fn lookup(&self, query: LookupQuery) {
            self.things.delete_by_id(todo!());
        }
    }
}

/// DCA-ERR-008: a closed result whose refusing variant carries no Refusal.
pub mod hold_thing {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    pub struct HoldThingCommand {
        pub thing: u64,
    }

    pub enum HoldThingResult {
        Held(u64),
        AlreadyHeld(u64),
    }

    pub trait HoldThingInputPort: InputPort {
        fn hold(&self, command: HoldThingCommand) -> HoldThingResult;
    }
}

/// DCA-ERR-003: implements the classification instead of a leaf.
#[derive(Debug)]
pub struct QuotaExceeded;
impl std::error::Error for QuotaExceeded {}
impl dca_building_blocks::application::InputPortFailure for QuotaExceeded {}

/// DCA-ERR-009: a use case that invents a domain failure for what the repository did not find.
pub mod remove_thing {
    use alpha_domain::{Thing, ThingRefused};

    pub struct RemoveThingUseCase;

    impl RemoveThingUseCase {
        pub fn remove(&self, id: u128, found: Option<Thing>) -> Result<(), ThingRefused> {
            let _thing = found.ok_or_else(|| ThingRefused::Unknown(id))?;
            Err(ThingRefused::Gone)
        }
    }
}

/// DCA-USE-018: two features, each with a command port.
pub mod tidy {
    pub mod sweep {
        pub struct SweepCommand;
        pub trait SweepInputPort: dca_building_blocks::hexagonal::port_in::InputPort {
            fn sweep(&self, command: SweepCommand);
        }
    }
}

pub mod mend {
    pub mod patch {
        pub struct PatchCommand;
        pub trait PatchInputPort: dca_building_blocks::hexagonal::port_in::InputPort {
            fn patch(&self, command: PatchCommand);
        }
    }
}

/// DCA-ERR-010: a port's failure answered without naming it.
pub mod keep_thing {
    use alpha_domain::Thing;
    use dca_building_blocks::hexagonal::port_out::PortError;

    use crate::WrongRepository;

    pub struct KeepThingUseCase<R: WrongRepository> {
        things: R,
    }

    impl<R: WrongRepository> KeepThingUseCase<R> {
        pub async fn keep(&mut self, thing: &Thing) -> bool {
            let _ = self.things.save(thing).await;
            match self.things.save(thing).await {
                Ok(()) => true,
                Err(_) => false,
            }
        }

        pub async fn keep_or_drop(&mut self, thing: &Thing) -> bool {
            match self.things.save(thing).await {
                Ok(()) => true,
                other => {
                    drop(other);
                    false
                }
            }
        }

        /// Cleans up, then hands on something else than what it caught: the failure is answered.
        pub async fn keep_or_wrap(&mut self, thing: &Thing) -> Result<(), String> {
            match self.things.save(thing).await {
                Ok(()) => Ok(()),
                other => {
                    drop(thing.clone());
                    other.map_err(|e| e.to_string())
                }
            }
        }

        /// Hands it on in one exit, swallows it in the other.
        pub async fn keep_or_pretend(&mut self, thing: &Thing, strict: bool) -> Result<(), PortError> {
            match self.things.save(thing).await {
                Ok(()) => Ok(()),
                other => {
                    if strict {
                        return other;
                    }
                    Ok(())
                }
            }
        }

        pub async fn try_keep(&mut self, thing: &Thing) -> bool {
            self.things.save(thing).await.is_ok()
        }

        pub fn keep_anyway(&mut self, thing: &Thing) {
            let _ = std::panic::catch_unwind(|| self.things.save(thing));
        }

        pub async fn keep_named(&mut self, thing: &Thing) -> Result<(), String> {
            self.things.save(thing).await.map_err(|e| e.to_string())?;
            match self.things.save(thing).await {
                Ok(()) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

/// DCA-RS-007: a unit shared by every use case, at the crate root, that hands out a helper besides
/// the repository.
pub trait SharedUnit: dca_building_blocks::application::TransactionUnit {
    type Things: WrongRepository;
    type Helper: shared::Helper;
    fn things(&mut self) -> &mut Self::Things;
    fn helper(&mut self) -> &mut Self::Helper;
}

/// DCA-USE-009 and DCA-ERR-010 through a transaction unit: the save is dropped and the events are
/// never handed on.
pub mod store_thing {
    use core::marker::PhantomData;

    use alpha_domain::Thing;
    use dca_building_blocks::application::{ScopedFutureExt, TransactionBoundary, TransactionUnit};
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::PortError;

    pub trait StoreThingUnit: TransactionUnit {
        type Things<'a>: crate::WrongRepository
        where
            Self: 'a;
        fn things(&mut self) -> Self::Things<'_>;
    }

    pub struct StoreThingUseCase<T, U> {
        transactions: T,
        unit: PhantomData<U>,
    }

    impl<T, U> InputPort for StoreThingUseCase<T, U> {}

    impl<T: TransactionBoundary<U>, U: StoreThingUnit> StoreThingUseCase<T, U> {
        pub async fn store(&self, thing: &Thing) -> Result<(), PortError> {
            self.transactions
                .in_transaction(|unit| {
                    async move {
                        let _ = unit.things().save(thing).await;
                        Ok(())
                    }
                    .scope_boxed()
                })
                .await
        }
    }
}

/// DCA-RS-007 and DCA-ERR-010 through a unit whose accessors borrow it: a helper handed out, and a
/// removal whose failure is dropped.
pub mod purge_thing {
    use core::marker::PhantomData;
    use std::future::Future;

    use alpha_domain::{Thing, ThingId};
    use dca_building_blocks::application::{ScopedFutureExt, TransactionBoundary, TransactionUnit};
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Repository};

    /// Also extends a helper and hands out a bare output port, which takes no part in the transaction.
    pub trait PurgeThingUnit: TransactionUnit + Send + crate::shared::Helper {
        type Things<'a>: Repository<Thing> + Send
        where
            Self: 'a;
        type Helper<'a>: crate::shared::Helper
        where
            Self: 'a;
        type Pending<'a>: OutputPort + Send
        where
            Self: 'a;
        fn things(&mut self) -> Self::Things<'_>;
        fn helper(&mut self) -> Self::Helper<'_>;
        fn pending(&mut self) -> Self::Pending<'_>;
    }

    pub struct PurgeThingUseCase<T, U> {
        transactions: T,
        unit: PhantomData<fn() -> U>,
    }

    impl<T, U> InputPort for PurgeThingUseCase<T, U> {}

    impl<T: TransactionBoundary<U> + Sync, U: PurgeThingUnit> PurgeThingUseCase<T, U> {
        pub fn purge(&self, id: ThingId) -> impl Future<Output = Result<(), PortError>> + Send {
            self.transactions.in_transaction(move |unit| {
                async move {
                    let _ = unit.things().delete_by_id(&id).await;
                    Ok(())
                }
                .scope_boxed()
            })
        }
    }
}

/// DCA-USE-017: a use case exposes the operation of a workspace trait that is no input port - in
/// the impl and as the trait's default. Its own input port's method and `Clone` are not counted.
pub mod sort_parts {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    pub trait SortPartsInputPort: InputPort {
        fn sort(&self);
    }

    pub trait Peek {
        fn peek(&self);
        fn glance(&self) {}
    }

    #[derive(Clone)]
    pub struct SortPartsUseCase;
    impl InputPort for SortPartsUseCase {}
    impl SortPartsInputPort for SortPartsUseCase {
        fn sort(&self) {}
    }
    impl Peek for SortPartsUseCase {
        fn peek(&self) {}
    }
}

use thiserror::Error as ThisError;

/// DCA-ERR-003: a derived error in the application that is no port failure.
#[derive(Debug, ThisError)]
pub struct Blocked;

/// DCA-ERR-004: a use-case failure that answers the web itself.
#[derive(Debug)]
pub struct Overdue;
impl std::error::Error for Overdue {}
impl UseCaseFailure for Overdue {}
impl axum::response::IntoResponse for Overdue {
    fn into_response(self) -> axum::response::Response { todo!() }
}

/// DCA-USE-016: a use case that holds another through a bound on its impl block, and one that
/// takes another as a parameter. DCA-NAM-012/013: types named as use cases without the role.
pub mod chain {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use crate::other::OtherInputPort;

    pub struct ChainUseCase<P> {
        next: P,
    }
    impl<P> InputPort for ChainUseCase<P> {}
    impl<P: OtherInputPort> ChainUseCase<P> {
        pub fn run(&self) {}
    }

    pub struct RelayUseCase;
    impl InputPort for RelayUseCase {}
    impl RelayUseCase {
        fn relay(&self, next: &dyn OtherInputPort) { let _ = next; }
    }

    pub struct WhenDueUseCase;
    pub struct UpdateLimitUseCase;
}

/// DCA-USE-009, DCA-USE-012, DCA-USE-019 and DCA-ERR-010 through locals: a port reached through a
/// local bound to the field or to the unit's accessor, and a port call's result bound first and
/// answered later. `settle` binds the result, then shadows it with what `?` left - the later
/// `let _ =` drops nothing.
pub mod alias_thing {
    use core::marker::PhantomData;

    use alpha_domain::Thing;
    use dca_building_blocks::application::{ScopedFutureExt, TransactionBoundary};
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::PortError;

    use crate::WrongRepository;

    pub struct AliasQuery;

    pub struct AliasThingUseCase<R: WrongRepository, T, U> {
        things: R,
        transactions: T,
        unit: PhantomData<U>,
    }

    impl<R: WrongRepository, T, U> InputPort for AliasThingUseCase<R, T, U> {}

    impl<R: WrongRepository, T: TransactionBoundary<U>, U: crate::store_thing::StoreThingUnit>
        AliasThingUseCase<R, T, U>
    {
        pub fn find(&self, query: AliasQuery) {
            let _ = query;
        }

        pub async fn stash(&self, thing: &Thing) -> bool {
            let things = &self.things;
            let saved = things.save(thing).await;
            match saved {
                Ok(()) => true,
                Err(_) => false,
            }
        }

        pub async fn peek(&self, thing: &Thing) -> bool {
            let saved = self.things.save(thing).await;
            if let Ok(()) = saved { true } else { false }
        }

        pub async fn settle(&self, thing: &Thing) -> Result<(), PortError> {
            let settled = self.things.save(thing).await;
            let settled = settled.map_err(|e| e)?;
            let _ = settled;
            Ok(())
        }

        pub async fn tuck(&self, thing: &Thing) -> Result<(), PortError> {
            self.transactions
                .in_transaction(|unit| {
                    async move {
                        let tucked = unit.things();
                        let _ = tucked.save(thing).await;
                        Ok(())
                    }
                    .scope_boxed()
                })
                .await
        }
    }
}

/// DCA-CYC-005: two use cases of one feature, below the configured container `usecases`, that name
/// each other - a cycle inside one feature.
pub mod usecases {
    pub mod sorting {
        pub mod sort_one {
            pub struct SortOneUseCase;
            impl dca_building_blocks::hexagonal::port_in::InputPort for SortOneUseCase {}
            pub fn next() -> Option<super::sort_two::SortTwoUseCase> {
                None
            }
        }

        pub mod sort_two {
            pub struct SortTwoUseCase;
            impl dca_building_blocks::hexagonal::port_in::InputPort for SortTwoUseCase {}
            pub fn back() -> Option<super::sort_one::SortOneUseCase> {
                None
            }
        }
    }
}

/// DCA-USE-023: a port declared in one use case's module and used by another; a port of the use
/// case's own passes.
pub mod install {
    use dca_building_blocks::hexagonal::port_in::InputPort;
    use dca_building_blocks::hexagonal::port_out::OutputPort;

    pub trait Source: OutputPort {
        fn read(&self) -> u32;
    }

    pub trait OwnLog: OutputPort {
        fn note(&self);
    }

    pub struct InstallUseCase<S: Source, L: OwnLog> {
        pub source: S,
        pub log: L,
    }

    impl<S: Source, L: OwnLog> InputPort for InstallUseCase<S, L> {}
}

pub mod check {
    use dca_building_blocks::hexagonal::port_in::InputPort;

    use crate::install::Source; // DCA-USE-023

    pub struct CheckUseCase<S: Source> {
        pub source: S,
    }

    impl<S: Source> InputPort for CheckUseCase<S> {}
}
