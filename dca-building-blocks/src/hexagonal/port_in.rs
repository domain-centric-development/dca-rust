//! Input ports: the operations the application offers to incoming adapters.

/// Marks a trait as an input port. An incoming adapter depends on the port, never on the use case
/// that implements it.
pub trait InputPort {}

/// An input port with one operation that takes a command or a query and answers with a result.
pub trait UseCase<I>: InputPort {
    /// The answer, usually `Result<SomethingResult, SomethingFailure>`.
    type Output;

    /// Runs the use case.
    fn execute(&self, input: I) -> Self::Output;
}
