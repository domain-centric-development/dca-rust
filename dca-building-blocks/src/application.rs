//! Application building blocks: the transaction boundary and the unit it hands the work, the event
//! listeners, the use case's own failures and the refusals a use case may answer with instead.
//!
//! The application layer is asynchronous: the boundary, the ports and the input ports return
//! futures that are `Send`. The domain stays synchronous.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use crate::hexagonal::port_out::PortError;

/// A boxed future that can move between threads: what an object-safe trait returns - an input
/// port an incoming adapter holds as `dyn`, an event listener a publisher holds in a list.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A future that borrows from two scopes: the transaction's unit, for `'unit`, and whatever the
/// work captured around it - the use case's `&self`, its clock, its id source - for `'scope`.
///
/// The bound parameter names the type `&'unit &'scope ()`, so wherever the trait object appears,
/// `'scope: 'unit` is implied: the work a [`TransactionBoundary`] lends the unit to, for any
/// `'unit`, may hold borrows that outlive the transaction. Every future that lives for `'unit` is
/// one; [`ScopedFutureExt::scope_boxed`] boxes it.
pub trait ScopedFuture<'scope, 'unit, Bound = PhantomData<&'unit &'scope ()>>: Future {}

impl<'scope: 'unit, 'unit, F: Future + 'unit> ScopedFuture<'scope, 'unit> for F {}

/// A [`ScopedFuture`] boxed and `Send`: what the work of a [`TransactionBoundary`] returns.
pub type ScopedBoxFuture<'scope, 'unit, T> =
    Pin<Box<dyn ScopedFuture<'scope, 'unit, Output = T> + Send + 'unit>>;

/// Boxes a future as the work of a [`TransactionBoundary`]: `async move { .. }.scope_boxed()`.
pub trait ScopedFutureExt: Future + Sized {
    /// The future, boxed as a [`ScopedBoxFuture`].
    fn scope_boxed<'scope, 'unit>(self) -> ScopedBoxFuture<'scope, 'unit, Self::Output>
    where
        Self: Send + 'unit,
        'scope: 'unit,
    {
        Box::pin(self)
    }
}

impl<F: Future> ScopedFutureExt for F {}

/// Runs a piece of work in one transaction: everything it saves and records is committed together
/// or not at all. A use case holds it; an adapter implements it; the domain never sees it.
///
/// The adapter opens the transaction and hands the work its unit `U` - the use case's
/// [`TransactionUnit`], through which the work reaches the repositories, stores, publisher and
/// inbox that take part in that transaction. Whatever the work saves through the unit is in the
/// transaction by construction; the use case never sees the store's own types. A boundary whose
/// ports join the transaction another way hands the unit `()`, the default.
///
/// The work borrows the unit for as long as its future runs, and may borrow what lives longer than
/// the call - the use case itself, its clock, its id source. It is a closure that returns its future
/// boxed with [`ScopedFutureExt::scope_boxed`]; nothing needs to be written out:
///
/// ```
/// use std::future::Future;
///
/// use dca_building_blocks::application::{ScopedFutureExt, TransactionBoundary};
/// use dca_building_blocks::hexagonal::port_out::PortError;
///
/// struct Rename<B> {
///     boundary: B,
///     suffix: String,
/// }
///
/// impl<B: TransactionBoundary<Vec<String>> + Sync> Rename<B> {
///     fn run(&self, name: String) -> impl Future<Output = Result<usize, PortError>> + Send + '_ {
///         self.boundary.in_transaction(move |names| {
///             async move {
///                 names.push(name + &self.suffix);
///                 Ok(names.len())
///             }
///             .scope_boxed()
///         })
///     }
/// }
/// ```
///
/// A method that returns the boundary's future returns it as `impl Future<..> + Send` (or boxed);
/// written as an `async fn`, the compiler cannot yet prove its future `Send` for every lifetime the
/// work borrows (rust-lang/rust#100013).
///
/// The returned future is `Send`, so a use case built on it runs on a multi-threaded runtime. When
/// it is dropped before it completes, the transaction is not committed: an implementation keeps the
/// open transaction in what the future owns, so dropping it rolls back.
///
/// The work's failure type takes a [`PortError`]: a transaction that cannot begin or commit ends in
/// the work's `Err` - the use case's "unavailable" case - like any other port failure. A rollback
/// that fails after the work failed ends in `E::from(PortError::rollback_failed(..))`, in place of
/// the work's failure: what the store holds is no longer known, and that outweighs the failure that
/// asked for the rollback.
///
/// A use case that answers its expected refusals as variants of a closed result returns them in
/// `Ok`; [`TransactionBoundary::in_transaction_closed`] rolls such a refusal back exactly as an
/// `Err` would be rolled back.
pub trait TransactionBoundary<U: Send = ()> {
    /// Runs `work` with the transaction's unit, committing on `Ok` and rolling back on `Err`. A
    /// failure to begin, commit or roll back is returned as `E::from(port_error)`.
    fn in_transaction<'scope, T, E, F>(&self, work: F) -> impl Future<Output = Result<T, E>> + Send
    where
        T: Send + 'scope,
        E: Send + From<PortError> + 'scope,
        F: for<'unit> FnOnce(&'unit mut U) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
            + Send
            + 'scope;

    /// Runs `work` with the transaction's unit, its `Ok` value a closed result: commits on a
    /// success, rolls back on `Err` and on a refusal, and returns the refusal unchanged in `Ok`.
    ///
    /// The default implementation runs the work through [`TransactionBoundary::in_transaction`]
    /// and hands it an `Err` for a refusal, so every implementation of `in_transaction` rolls a
    /// refusal back without code of its own; the refusal comes back out as `Ok`.
    fn in_transaction_closed<'scope, R, E, F>(
        &self,
        work: F,
    ) -> impl Future<Output = Result<R, E>> + Send
    where
        R: ClosedResult + Send + 'scope,
        E: Send + From<PortError> + 'scope,
        F: for<'unit> FnOnce(&'unit mut U) -> ScopedBoxFuture<'scope, 'unit, Result<R, E>>
            + Send
            + 'scope,
    {
        let running = self.in_transaction(move |unit| {
            let work = work(unit);
            async move { undo_refusal(work.await) }.scope_boxed()
        });
        async move {
            match running.await {
                Ok(result) => Ok(result),
                Err(undone) => undone.into_outcome(),
            }
        }
    }
}

/// One boundary serves every use case that shares it: a reference, a box or an `Arc` to a boundary
/// is the boundary.
macro_rules! forward_boundary {
    ($($wrapper:ty),*) => {$(
        impl<U: Send, B: TransactionBoundary<U> + ?Sized> TransactionBoundary<U> for $wrapper {
            fn in_transaction<'scope, T, E, F>(
                &self,
                work: F,
            ) -> impl Future<Output = Result<T, E>> + Send
            where
                T: Send + 'scope,
                E: Send + From<PortError> + 'scope,
                F: for<'unit> FnOnce(&'unit mut U) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
                    + Send
                    + 'scope,
            {
                (**self).in_transaction(work)
            }
        }
    )*};
}

forward_boundary!(&B, Box<B>, std::sync::Arc<B>);

/// Why a closed transaction was undone: the work refused, or it failed.
enum Undone<R, E> {
    Refused(R),
    Failed(E),
}

impl<R, E: From<PortError>> From<PortError> for Undone<R, E> {
    fn from(failure: PortError) -> Self {
        Undone::Failed(E::from(failure))
    }
}

impl<R, E> Undone<R, E> {
    /// The outcome the caller sees: a refusal comes back in `Ok`, a failure in `Err`.
    fn into_outcome(self) -> Result<R, E> {
        match self {
            Undone::Refused(result) => Ok(result),
            Undone::Failed(failure) => Err(failure),
        }
    }
}

/// Turns a refusal into an `Err`, so the boundary rolls it back.
fn undo_refusal<R: ClosedResult, E>(outcome: Result<R, E>) -> Result<R, Undone<R, E>> {
    match outcome {
        Ok(result) if result.refusal().is_some() => Err(Undone::Refused(result)),
        Ok(result) => Ok(result),
        Err(failure) => Err(Undone::Failed(failure)),
    }
}

/// Marks a trait as a use case's transaction unit: what a [`TransactionBoundary`] hands the work
/// when it opens the transaction. Each use case declares its own unit, in its own module of the
/// application crate, naming exactly the repositories, stores, event publisher and inbox it works
/// with. Its one form: each port is a generic associated type (`type Things<'a> where Self: 'a`)
/// and its accessor hands out `Self::Things<'_>`, a port that borrows the unit - so a store's
/// adapter hands out a type of its own over the transaction's connection, and an adapter that owns
/// its ports hands out `&'a mut` to one (a `&mut` to a port is the port). The outbox is the event
/// publisher's, never the unit's:
///
/// ```
/// use dca_building_blocks::application::TransactionUnit;
/// use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};
/// # use dca_building_blocks::ddd::tactical::*;
/// # #[derive(Clone, Debug, PartialEq, Eq, Hash)] pub struct ThingId(u128);
/// # impl Value for ThingId {} impl Id for ThingId {}
/// # #[derive(Clone, Debug)] pub struct ThingRenamed;
/// # impl DomainEvent for ThingRenamed {
/// #     fn event_id(&self) -> EventId { todo!() } fn occurred_on(&self) -> Timestamp { todo!() } }
/// # pub struct Thing { id: ThingId }
/// # impl Entity for Thing { type Id = ThingId; fn id(&self) -> &ThingId { &self.id } }
/// # impl AggregateRoot for Thing { type Event = ThingRenamed;
/// #     fn domain_events(&self) -> &[ThingRenamed] { &[] }
/// #     fn take_domain_events(&mut self) -> Vec<ThingRenamed> { vec![] } }
///
/// pub trait RenameThingUnit: TransactionUnit + Send {
///     type Things<'a>: Repository<Thing> + Send where Self: 'a;
///     type Events<'a>: DomainEventPublisher<ThingRenamed> + Send where Self: 'a;
///     fn things(&mut self) -> Self::Things<'_>;
///     fn events(&mut self) -> Self::Events<'_>;
/// }
/// ```
///
/// A unit is not an output port: the use case calls the ports it hands out, never the unit for work
/// of its own. Its implementation, built over the open transaction, sits in the outgoing adapters.
pub trait TransactionUnit {}

/// Reacts to an event inside the transaction that publishes it: a rollback undoes what it did. It
/// calls output ports only - capturing an integration event, counting a metric. A reaction that
/// drives a use case is a [`TransactionalEventListener`].
///
/// The publisher hands it `U`, the unit of the open transaction: the output ports the listener
/// reaches, over the transaction's own connection. The listener itself holds nothing of the
/// transaction - it is built once, in the composition root, and serves every transaction. A
/// listener that needs no port takes the default, `()`.
///
/// ```
/// use dca_building_blocks::application::{BoxFuture, EventListener};
/// use dca_building_blocks::hexagonal::port_out::PortError;
///
/// # #[derive(Clone, Debug)] pub struct ThingRenamed { pub name: String }
/// /// The output port the reaction writes to, as the transaction hands it out.
/// pub trait RenameLog: Send {
///     fn note(&mut self, name: &str);
/// }
///
/// /// Notes every rename in the log, in the renaming transaction.
/// pub struct NoteRename;
///
/// impl<U: RenameLog> EventListener<ThingRenamed, U> for NoteRename {
///     fn on<'a>(&'a self, event: &'a ThingRenamed, log: &'a mut U) -> BoxFuture<'a, Result<(), PortError>> {
///         Box::pin(async move {
///             log.note(&event.name);
///             Ok(())
///         })
///     }
/// }
///
/// // A publisher holds its listeners as trait objects.
/// let _listeners: Vec<Box<dyn EventListener<ThingRenamed, Vec<String>> + Send + Sync>> =
///     vec![Box::new(NoteRename)];
/// # impl RenameLog for Vec<String> { fn note(&mut self, name: &str) { self.push(name.into()) } }
/// ```
///
/// `on` returns a boxed future so a publisher can hold its listeners as trait objects.
pub trait EventListener<E, U = ()> {
    /// Reacts to one event, with the ports of the transaction that publishes it.
    fn on<'a>(&'a self, event: &'a E, unit: &'a mut U) -> BoxFuture<'a, Result<(), PortError>>;
}

/// Reacts to an event after the publishing transaction committed, in a transaction of its own. A
/// listener that drives an input port is of this kind (DCA-HEX-017); it reaches no transaction's
/// unit (its unit is `()`), since the use case it drives opens its own.
///
/// The trait is the declaration Rust has in place of a listener annotation: it adds no method. How
/// the delivery survives a crash between the commit and the reaction - an outbox written in the
/// publishing transaction, a relay that delivers at least once, a record of what each listener
/// received - is the adapters' concern; the record of receipts is a [`Store`] like any other.
///
/// ```
/// use dca_building_blocks::application::{BoxFuture, EventListener, TransactionalEventListener};
/// use dca_building_blocks::hexagonal::port_out::PortError;
///
/// # #[derive(Clone, Debug)] pub struct ThingCreated;
/// /// Shelves every thing created, after the creating transaction committed.
/// pub struct ShelveCreatedThing;
///
/// impl EventListener<ThingCreated> for ShelveCreatedThing {
///     fn on<'a>(&'a self, _: &'a ThingCreated, _: &'a mut ()) -> BoxFuture<'a, Result<(), PortError>> {
///         Box::pin(async move { Ok(()) }) // drives the shelving input port
///     }
/// }
///
/// impl TransactionalEventListener<ThingCreated> for ShelveCreatedThing {}
/// ```
///
/// [`Store`]: crate::hexagonal::port_out::Store
pub trait TransactionalEventListener<E>: EventListener<E> {}

/// A failure a port's contract declares - the base of the port failures, by classification.
///
/// Rust has no exceptions and no class hierarchy: a failure is a type returned in `Err`, and a type
/// implements the leaf it is - [`UseCaseFailure`] or [`OutputPortFailure`]. The rule suite derives
/// the classification from the leaf: a `UseCaseFailure` is an [`InputPortFailure`] and a
/// `PortFailure`, an `OutputPortFailure` is a `PortFailure`. The traits are not supertraits of the
/// leaves on purpose: nobody handles a port failure as such - a caller matches the named failure, each
/// with its own answer - so a bound on the base would only invite the generic handler the rules
/// list. A type implements one of the two leaves, never this trait.
pub trait PortFailure: std::error::Error {}

/// A failure an input port declares - by classification, see [`PortFailure`]: the use case returns
/// it, the incoming adapter that drives the port matches it and answers it. Its one leaf is
/// [`UseCaseFailure`]; a type implements that, never this trait.
pub trait InputPortFailure: std::error::Error {}

/// A failure a use case names that is not the domain's: a missing aggregate, a refused request. It is
/// an input port's failure (see [`InputPortFailure`]): the port declares it, only the use case
/// returns it, the incoming adapter matches it. An outgoing adapter never returns it - a failure an
/// output port declares is an [`OutputPortFailure`]. In a context that answers its refusals as
/// variants of a closed result, not-found and not-owned are variants, not failures; both styles are
/// valid, uniform per context.
pub trait UseCaseFailure: std::error::Error {}

/// A failure an output port declares in its contract, beside the port in the application crate: the
/// outgoing adapter that implements the port returns it because the contract asks for it, the use
/// case that calls the port matches it and decides the answer - a variant of its result, or its own
/// [`UseCaseFailure`]. For an expected failure the caller can answer - a uniqueness only the store
/// sees, or a foreign system behind the port that refused or did not answer (an external system on
/// the context map) - not for a failure of the system's own infrastructure (its database, its broker).
pub trait OutputPortFailure: std::error::Error {}

/// Marker for the payload of a variant of a closed use-case result that refuses the request.
///
/// A use case may answer an expected refusal in one of two styles, chosen per bounded context: it
/// returns a named failure in `Err`, or it returns the refusal as a variant of a closed result - an
/// enum its caller matches over - in `Ok`. The second is preferred, the first allowed. In the second
/// style every refusing variant carries one value whose type implements `Refusal`, and a variant that
/// reports success never does:
///
/// ```
/// use dca_building_blocks::application::{ClosedResult, Refusal};
///
/// pub struct Reserved { pub seat: u32 }
/// pub struct SeatTaken { pub seat: u32 }
/// impl Refusal for SeatTaken {}
///
/// pub enum ReserveSeatResult {
///     Reserved(Reserved),
///     SeatTaken(SeatTaken),
/// }
///
/// impl ClosedResult for ReserveSeatResult {
///     fn refusal(&self) -> Option<&dyn Refusal> {
///         match self {
///             Self::Reserved(_) => None,
///             Self::SeatTaken(refused) => Some(refused),
///         }
///     }
/// }
/// ```
///
/// A refusal rolls back like a failure: a use case that wrote something before it found out it has
/// to refuse runs through [`TransactionBoundary::in_transaction_closed`], which rolls the refusal
/// back. The rule for good code stays "decide, then write"; the roll-back is the net.
pub trait Refusal {}

/// A closed use-case result that refuses: an enum whose variants are the outcomes a use case can
/// answer with, some of them refusals, and the one place that says which variant does. The match in
/// [`ClosedResult::refusal`] is checked by the compiler twice over - it covers every variant, and it
/// can only hand out a payload whose type is a [`Refusal`]. A closed result may also hold only
/// outcomes that are not refusals - found or nothing found, sent or already sent; it is a closed
/// result all the same, carries no [`Refusal`] and needs no roll-back, so it need not implement
/// this trait.
pub trait ClosedResult {
    /// The refusal this result holds, or `None` for a success.
    fn refusal(&self) -> Option<&dyn Refusal>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    /// Runs a future to completion on the current thread; the futures here never wait.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
                return out;
            }
        }
    }

    /// A unit that buffers what the work writes; the boundary keeps it only on commit.
    #[derive(Default)]
    struct Unit {
        written: Vec<&'static str>,
    }

    /// Records how each transaction ended and what was committed.
    #[derive(Default)]
    struct Recording {
        log: Mutex<Vec<&'static str>>,
        committed: Mutex<Vec<&'static str>>,
    }

    impl TransactionBoundary<Unit> for Recording {
        async fn in_transaction<'scope, T, E, F>(&self, work: F) -> Result<T, E>
        where
            T: Send + 'scope,
            E: Send + From<PortError> + 'scope,
            F: for<'unit> FnOnce(&'unit mut Unit) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
                + Send
                + 'scope,
        {
            let mut unit = Unit::default();
            let outcome = work(&mut unit).await;
            if outcome.is_ok() {
                self.log.lock().unwrap().push("commit");
                self.committed.lock().unwrap().extend(unit.written);
            } else {
                self.log.lock().unwrap().push("rollback");
            }
            outcome
        }
    }

    /// A boundary that cannot begin a transaction, or whose rollback fails.
    enum Broken {
        CannotBegin,
        CannotRollBack,
    }

    impl TransactionBoundary<Unit> for Broken {
        async fn in_transaction<'scope, T, E, F>(&self, work: F) -> Result<T, E>
        where
            T: Send + 'scope,
            E: Send + From<PortError> + 'scope,
            F: for<'unit> FnOnce(&'unit mut Unit) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
                + Send
                + 'scope,
        {
            match self {
                Broken::CannotBegin => Err(E::from(PortError::new("the store is down"))),
                Broken::CannotRollBack => match work(&mut Unit::default()).await {
                    Ok(value) => Ok(value),
                    Err(_) => Err(E::from(PortError::rollback_failed(PortError::new(
                        "the connection broke",
                    )))),
                },
            }
        }
    }

    struct Taken;
    impl Refusal for Taken {}

    enum Answer {
        Done,
        Taken(Taken),
    }

    impl ClosedResult for Answer {
        fn refusal(&self) -> Option<&dyn Refusal> {
            match self {
                Self::Done => None,
                Self::Taken(refused) => Some(refused),
            }
        }
    }

    /// The work's failure in the tests: its own, or the transaction's.
    #[derive(Debug, PartialEq)]
    enum Failure {
        Broken,
        Unavailable(String),
    }

    impl From<PortError> for Failure {
        fn from(e: PortError) -> Self {
            Failure::Unavailable(e.to_string())
        }
    }

    #[test]
    fn the_work_writes_through_the_unit_and_a_success_commits_it() {
        let boundary = Recording::default();
        let outcome: Result<u8, Failure> = block_on(boundary.in_transaction(|unit| {
            async move {
                unit.written.push("part");
                Ok(1)
            }
            .scope_boxed()
        }));
        assert_eq!(outcome, Ok(1));
        assert_eq!(*boundary.log.lock().unwrap(), ["commit"]);
        assert_eq!(*boundary.committed.lock().unwrap(), ["part"]);
    }

    #[test]
    fn a_failure_rolls_back_what_the_unit_holds() {
        let boundary = Recording::default();
        let outcome: Result<u8, Failure> = block_on(boundary.in_transaction(|unit| {
            async move {
                unit.written.push("part");
                Err(Failure::Broken)
            }
            .scope_boxed()
        }));
        assert_eq!(outcome, Err(Failure::Broken));
        assert_eq!(*boundary.log.lock().unwrap(), ["rollback"]);
        assert!(boundary.committed.lock().unwrap().is_empty());
    }

    #[test]
    fn a_shared_boundary_is_the_boundary() {
        let boundary = std::sync::Arc::new(Recording::default());
        let shared = std::sync::Arc::clone(&boundary);
        let outcome: Result<u8, Failure> = block_on(shared.in_transaction(|unit| {
            async move {
                unit.written.push("part");
                Ok(1)
            }
            .scope_boxed()
        }));
        assert_eq!(outcome, Ok(1));
        fn rolls_back<B: TransactionBoundary<Unit>>(boundary: B) -> Result<u8, Failure> {
            block_on(boundary.in_transaction(|_| async { Err(Failure::Broken) }.scope_boxed()))
        }
        assert_eq!(rolls_back(&*boundary), Err(Failure::Broken));
        assert_eq!(*boundary.log.lock().unwrap(), ["commit", "rollback"]);
    }

    #[test]
    fn a_returned_refusal_rolls_back_and_comes_back_in_ok() {
        let boundary = Recording::default();
        let outcome: Result<Answer, Failure> = block_on(boundary.in_transaction_closed(|unit| {
            async move {
                unit.written.push("part");
                Ok(Answer::Taken(Taken))
            }
            .scope_boxed()
        }));
        assert!(matches!(outcome, Ok(Answer::Taken(_))));
        assert_eq!(*boundary.log.lock().unwrap(), ["rollback"]);
        assert!(boundary.committed.lock().unwrap().is_empty());
    }

    #[test]
    fn a_closed_success_commits() {
        let boundary = Recording::default();
        let outcome: Result<Answer, Failure> =
            block_on(boundary.in_transaction_closed(|_| async { Ok(Answer::Done) }.scope_boxed()));
        assert!(matches!(outcome, Ok(Answer::Done)));
        assert_eq!(*boundary.log.lock().unwrap(), ["commit"]);
    }

    #[test]
    fn a_closed_failure_still_rolls_back() {
        let boundary = Recording::default();
        let outcome: Result<Answer, Failure> = block_on(
            boundary.in_transaction_closed(|_| async { Err(Failure::Broken) }.scope_boxed()),
        );
        assert!(matches!(outcome, Err(Failure::Broken)));
        assert_eq!(*boundary.log.lock().unwrap(), ["rollback"]);
    }

    #[test]
    fn a_transaction_that_cannot_begin_ends_in_the_works_failure() {
        let outcome: Result<Answer, Failure> = block_on(
            Broken::CannotBegin.in_transaction_closed(|_| async { Ok(Answer::Done) }.scope_boxed()),
        );
        assert_eq!(
            outcome.err(),
            Some(Failure::Unavailable("the store is down".into()))
        );
    }

    #[test]
    fn a_failed_rollback_is_reported_in_place_of_the_works_failure() {
        let expected = Some(Failure::Unavailable(
            "the transaction failed and could not be rolled back".into(),
        ));
        let outcome: Result<u8, Failure> = block_on(
            Broken::CannotRollBack.in_transaction(|_| async { Err(Failure::Broken) }.scope_boxed()),
        );
        assert_eq!(outcome.err(), expected);
        let outcome: Result<Answer, Failure> = block_on(
            Broken::CannotRollBack
                .in_transaction_closed(|_| async { Ok(Answer::Taken(Taken)) }.scope_boxed()),
        );
        assert_eq!(outcome.err(), expected);
    }

    #[test]
    fn a_boundary_without_a_unit_hands_the_work_unit() {
        struct Plain;
        impl TransactionBoundary for Plain {
            async fn in_transaction<'scope, T, E, F>(&self, work: F) -> Result<T, E>
            where
                T: Send + 'scope,
                E: Send + From<PortError> + 'scope,
                F: for<'unit> FnOnce(&'unit mut ()) -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
                    + Send
                    + 'scope,
            {
                work(&mut ()).await
            }
        }
        let outcome =
            block_on(Plain.in_transaction(|_| async { Ok::<_, Failure>(2) }.scope_boxed()));
        assert_eq!(outcome, Ok(2));
    }

    /// A use case generic over its boundary and its unit whose work borrows the use case itself:
    /// the future it returns is `Send`, so a multi-threaded runtime can spawn it. The assertion is
    /// the compiler's.
    #[test]
    fn a_use_case_future_that_borrows_the_use_case_is_send() {
        trait CountUnit: TransactionUnit + Send {
            fn count(&mut self) -> &mut u32;
        }
        struct Step(u32);
        struct CountUseCase<B> {
            boundary: B,
            step: Step,
        }
        impl<B> CountUseCase<B> {
            fn execute<U>(&self, times: u32) -> impl Future<Output = Result<u32, PortError>> + Send
            where
                B: TransactionBoundary<U> + Sync,
                U: CountUnit,
            {
                self.boundary.in_transaction(move |unit: &mut U| {
                    async move {
                        *unit.count() += times * self.step.0;
                        Ok(*unit.count())
                    }
                    .scope_boxed()
                })
            }
        }
        fn assert_send<F: Future + Send>(future: F) -> F {
            future
        }

        #[derive(Default)]
        struct Counter(u32);
        impl TransactionUnit for Counter {}
        impl CountUnit for Counter {
            fn count(&mut self) -> &mut u32 {
                &mut self.0
            }
        }
        struct Fresh;
        impl TransactionBoundary<Counter> for Fresh {
            async fn in_transaction<'scope, T, E, F>(&self, work: F) -> Result<T, E>
            where
                T: Send + 'scope,
                E: Send + From<PortError> + 'scope,
                F: for<'unit> FnOnce(
                        &'unit mut Counter,
                    )
                        -> ScopedBoxFuture<'scope, 'unit, Result<T, E>>
                    + Send
                    + 'scope,
            {
                let mut unit = Counter::default();
                work(&mut unit).await
            }
        }
        let use_case = CountUseCase {
            boundary: Fresh,
            step: Step(2),
        };
        let outcome = block_on(assert_send(use_case.execute::<Counter>(3)));
        assert_eq!(outcome.ok(), Some(6));
    }
}
