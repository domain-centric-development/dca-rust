//! Input ports: the operations the application offers to incoming adapters.

use crate::application::BoxFuture;

/// Marks a trait as an input port. An incoming adapter depends on the port, never on the use case
/// that implements it.
///
/// A use case implements it written out (`impl InputPort for RenameThingUseCase {}`): it is the
/// supertrait of [`UseCase`], and the rule suite reads the role from that impl.
pub trait InputPort {}

/// An input port with one operation that takes a command or a query and answers with a result.
///
/// The operation is asynchronous and returns a boxed future, so an incoming adapter can hold the
/// port as a trait object (`Arc<dyn PlaceOrderInputPort + Send + Sync>`).
///
/// A project names each port as a trait of its own over `UseCase`, and implements it for every
/// type that answers the command that way with one blanket impl beside it - so the use case's
/// bounds are written once, on its `UseCase` impl, and not repeated on an impl of the port:
///
/// ```
/// use dca_building_blocks::application::BoxFuture;
/// use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
///
/// pub struct RenameThingCommand { pub name: String }
/// #[derive(Debug, PartialEq)]
/// pub struct Renamed;
///
/// /// Renames a thing.
/// pub trait RenameThingInputPort: UseCase<RenameThingCommand, Output = Result<Renamed, ()>> {}
///
/// impl<P> RenameThingInputPort for P where P: UseCase<RenameThingCommand, Output = Result<Renamed, ()>> {}
///
/// pub struct RenameThingUseCase<R> { things: R }
///
/// impl<R> InputPort for RenameThingUseCase<R> {}
///
/// impl<R: Sync> UseCase<RenameThingCommand> for RenameThingUseCase<R> {
///     type Output = Result<Renamed, ()>;
///     fn execute(&self, _command: RenameThingCommand) -> BoxFuture<'_, Self::Output> {
///         Box::pin(async { Ok(Renamed) })
///     }
/// }
///
/// // The incoming adapter holds the port, not the use case.
/// let port: Box<dyn RenameThingInputPort + Send + Sync> =
///     Box::new(RenameThingUseCase { things: () });
/// # let _ = port;
/// ```
///
/// The rule suite reads the use case's input-port role from its written-out `impl InputPort`,
/// which the compiler asks for anyway; the blanket impl needs nothing of the suite.
pub trait UseCase<I>: InputPort {
    /// The answer, usually `Result<SomethingResult, SomethingFailure>`.
    type Output;

    /// Runs the use case.
    fn execute(&self, input: I) -> BoxFuture<'_, Self::Output>;
}
