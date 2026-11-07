//! Application building blocks: the transaction boundary, the use case's own failures and the
//! refusals a use case may answer with instead.

/// Runs a unit of work in one transaction: everything it saves and publishes is committed together
/// or not at all. A use case holds it; an adapter implements it; the domain never sees it.
///
/// A use case that answers its expected refusals as variants of a closed result returns them in
/// `Ok`; [`TransactionBoundary::in_transaction_closed`] rolls such a refusal back exactly as an
/// `Err` would be rolled back.
pub trait TransactionBoundary {
    /// Runs `work` in a transaction, committing on `Ok` and rolling back on `Err`.
    fn in_transaction<T, E>(&self, work: impl FnOnce() -> Result<T, E>) -> Result<T, E>;

    /// Runs `work` in a transaction whose `Ok` value is a closed result: commits on a success,
    /// rolls back on `Err` and on a refusal, and returns the refusal unchanged in `Ok`.
    ///
    /// The default implementation runs the work through [`TransactionBoundary::in_transaction`]
    /// and hands it an `Err` for a refusal, so every implementation of `in_transaction` rolls a
    /// refusal back without code of its own; the refusal comes back out as `Ok`.
    fn in_transaction_closed<R: ClosedResult, E>(
        &self,
        work: impl FnOnce() -> Result<R, E>,
    ) -> Result<R, E> {
        enum Undone<R, E> {
            Refused(R),
            Failed(E),
        }
        match self.in_transaction(|| match work() {
            Ok(result) if result.refusal().is_some() => Err(Undone::Refused(result)),
            Ok(result) => Ok(result),
            Err(failure) => Err(Undone::Failed(failure)),
        }) {
            Ok(result) => Ok(result),
            Err(Undone::Refused(result)) => Ok(result),
            Err(Undone::Failed(failure)) => Err(failure),
        }
    }
}

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
/// enum its caller matches over - in `Ok`. In the second style every refusing variant carries one
/// value whose type implements `Refusal`, and a variant that reports success never does:
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

/// A closed use-case result: an enum with at least one variant that refuses the request, and the
/// one place that says which variant does. The match in [`ClosedResult::refusal`] is checked by the
/// compiler twice over - it covers every variant, and it can only hand out a payload whose type is a
/// [`Refusal`]. A closed result without a refusing variant is a plain result.
pub trait ClosedResult {
    /// The refusal this result holds, or `None` for a success.
    fn refusal(&self) -> Option<&dyn Refusal>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Records how each transaction ended.
    #[derive(Default)]
    struct Recording {
        log: RefCell<Vec<&'static str>>,
    }

    impl TransactionBoundary for Recording {
        fn in_transaction<T, E>(&self, work: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
            let outcome = work();
            self.log.borrow_mut().push(if outcome.is_ok() {
                "commit"
            } else {
                "rollback"
            });
            outcome
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

    #[test]
    fn a_returned_refusal_rolls_back_and_comes_back_in_ok() {
        let boundary = Recording::default();
        let outcome: Result<Answer, ()> =
            boundary.in_transaction_closed(|| Ok(Answer::Taken(Taken)));
        assert!(matches!(outcome, Ok(Answer::Taken(_))));
        assert_eq!(*boundary.log.borrow(), ["rollback"]);
    }

    #[test]
    fn a_success_commits() {
        let boundary = Recording::default();
        let outcome: Result<Answer, ()> = boundary.in_transaction_closed(|| Ok(Answer::Done));
        assert!(matches!(outcome, Ok(Answer::Done)));
        assert_eq!(*boundary.log.borrow(), ["commit"]);
    }

    #[test]
    fn a_failure_still_rolls_back() {
        let boundary = Recording::default();
        let outcome: Result<Answer, &str> = boundary.in_transaction_closed(|| Err("broken"));
        assert!(matches!(outcome, Err("broken")));
        assert_eq!(*boundary.log.borrow(), ["rollback"]);
    }
}
