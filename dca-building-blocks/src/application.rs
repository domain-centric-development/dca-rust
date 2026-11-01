//! Application building blocks: the transaction boundary and the use case's own failures.

/// Runs a unit of work in one transaction: everything it saves and publishes is committed together
/// or not at all. A use case holds it; an adapter implements it; the domain never sees it.
pub trait TransactionBoundary {
    /// Runs `work` in a transaction, committing on `Ok` and rolling back on `Err`.
    fn in_transaction<T, E>(&self, work: impl FnOnce() -> Result<T, E>) -> Result<T, E>;
}

/// A failure a use case names that is not the domain's: a missing aggregate, a refused request.
pub trait UseCaseFailure: std::error::Error {}
