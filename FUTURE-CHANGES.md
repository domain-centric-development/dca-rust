# Future changes

What the building blocks will gain once Rust or the method allows it, and what they never change. Written
2026-11-20 against stable Rust 1.100. Read it before changing a public signature of `dca-building-blocks`.

The rule for every entry: **add, never replace.** A new form arrives beside the old one; the old one stays until a
major version, so a project that builds today keeps building.

## Waiting for the language

### A transaction closure without boxing

Today the work a `TransactionBoundary` runs is a closure that boxes its future:

```rust
self.transactions.in_transaction(move |unit| {
    async move { /* … */ }.scope_boxed()
})
```

`ScopedBoxFuture` and `.scope_boxed()` exist for one reason: a generic function cannot yet promise that the future an
async closure returns is `Send`. Async closures are stable (1.85), but a bound on their future is not:

- Return type notation (`F(..): Send`, `T::method(..): Send`) is experimental — rust-lang/rust#109417.
- Naming the closure's future (`<F as AsyncFnOnce<..>>::CallOnceFuture: Send`) needs the unstable `async_fn_traits`.

Both were checked on stable 1.100 and refused.

**When it is possible**, add a second method — the name is open, `run` for instance:

```rust
fn run<'scope, T, E, F>(&self, work: F) -> impl Future<Output = Result<T, E>> + Send
where
    F: for<'unit> AsyncFnOnce(&'unit mut U) -> Result<T, E> + Send + 'scope,
    for<'unit> F(&'unit mut U): Send,   // RTN for async closures, the exact form per its RFC
    T: Send + 'scope,
    E: Send + From<PortError> + 'scope;
```

with a default implementation over `in_transaction`, a `run_closed` beside `in_transaction_closed`, the README and
the examples switched to it, and `in_transaction` documented as the older form. The rule suite reads both names
wherever it reads `in_transaction` (`runs_a_transaction` in `dca-rules/src/rules/mod.rs`, the mapping texts of
DCA-USE-009, -012, -019 and DCA-ERR-010).

**Where it stands** (searched 2026-11-20):

- RTN for trait methods is a 2026 project goal, together with type alias impl trait, aimed at late 2026 and blocked
  on the next-generation trait solver (default on nightly since August 2026). An earlier stabilization PR was closed.
  — <https://goals.rust-lang.org/2026/rtn.html>, <https://github.com/rust-lang/goals/issues/646>,
  <https://blog.rust-lang.org/2026/08/21/enabling-next-solver-on-nightly/>
- RTN for async closures — what `run` needs — first needs an RFC of its own; the implementation follows its
  acceptance. No date.
- The async roadmap notes that async closures still meet false `Send` errors in the compiler.
  — <https://goals.rust-lang.org/2026/roadmap-just-add-async.html>

To check again: compile the probe below on the newest stable; when it builds, the second step is open.

```rust
trait Repo { async fn find(&self) -> u32; }
fn needs_send<R: Repo<find(..): Send>>(_r: R) {}   // RTN for trait methods
fn main() {}
```

### Ports and gateways with `async fn`

Ports and gateways declare `fn …(&mut self, …) -> impl Future<Output = …> + Send`, because an `async fn` in a trait
cannot promise `Send`. With RTN for trait methods a trait may be written with `async fn` and the caller asks for
`Send` where it needs it. Do **not** rewrite the building-block traits then: the `impl Future + Send` form stays valid,
every implementation written as `async fn` already satisfies it, and changing the declaration would drop the `Send`
promise every use case relies on. Only the examples and the guidance for a project's own ports may move.

### Input ports without a box

`UseCase::execute` returns a `BoxFuture` so an incoming adapter can hold the port as `dyn`. When async functions in
traits become dyn-compatible (the 2026 goal "native async fn dynamic dispatch"), a boxless form may come beside it;
`execute` stays.

## Additive, whenever needed

None of these breaks a user; add them when a consumer asks.

- More `PortErrorKind`s (a timeout, for instance) when a caller has to tell one apart: the enum is
  `#[non_exhaustive]`. A conflict of the domain the caller answers — a uniqueness only the store sees — is not one:
  the port declares it as an `OutputPortFailure`.
- A satellite crate `dca-memory`: the transaction boundary, outbox, relay and inbox over memory that the example
  builds in its kernel adapters, for tests and small programs. Not in the building blocks: the relay needs an async
  runtime, and the building blocks stay without dependencies.
- Combinators for `Specification` (`and`, `or`, `not`) as provided methods.
- A `prelude` module.
- `#![no_std]` with `alloc`: `std::error::Error` and `core::error::Error` are the same trait.
- Blanket impls of `UseCase` for `Arc<P>`, `Box<P>` and `&P`, if generic incoming adapters ask for them. Check first
  that no consumer implements a port for such a wrapper itself — then the blanket would collide.

## Done

- `DomainEvents<E>` exists (2026-11-20): the counterpart of Java's `BaseAggregateRoot`, a field an aggregate root
  records its events in and delegates `domain_events` and `take_domain_events` to.
- `PortError::kind()` exists (2026-11-20), with the `#[non_exhaustive]` `PortErrorKind` — `Unavailable` (`new`,
  `caused_by`) and `RollbackFailed` (`rollback_failed`).
- `PortErrorKind::Conflict` exists (2026-11-22, `PortError::conflict`): a save refused because the aggregate changed
  since it was read — optimistic concurrency, the same technical event for every repository, so a kind of port error
  the caller reads; `Repository::save` keeps its signature. A conflict of the domain (a uniqueness) stays an
  `OutputPortFailure` the port declares.

## Fixed — changing these is a breaking change

Decided with the async application layer (2026-11-20); change them only in a major version, and only with a reason
that outweighs every project built on them.

- Every port future, the boundary's and the input port's are `Send`: a use case runs on a multi-threaded runtime.
  An adapter whose futures are not `Send` (single-threaded WASM) does not fit.
- Ports take `&mut self`; `&mut P` is a port again (the blanket impls), and so are `&B`, `Box<B>` and `Arc<B>` of a
  transaction boundary.
- A transaction unit has one form: each port a generic associated type, handed out by value (`Self::Things<'_>`);
  it hands out repositories, stores and a domain-event publisher only (DCA-RS-007).
- Event delivery after the commit - outbox, relay, inbox, ordering - is not a building block: no rule reads it, so it
  belongs to the adapters (the example keeps it in its shared kernel). The listener traits are the declaration
  DCA-HEX-017 reads; a record of the deliveries a listener received is a `Store`.
- `EventListener<E, U = ()>` takes the transaction's unit (`on(&self, &E, &mut U)`): an in-transaction listener
  reaches the output ports of the open transaction through it and holds nothing of the transaction itself.
  `TransactionalEventListener<E>` extends `EventListener<E>` with `U = ()` — it runs after the commit and drives a
  use case that opens its own; it adds no method. Both stay dyn-compatible.
- Public enums that may grow are `#[non_exhaustive]` (`PortErrorKind`).
- `DomainEvent` and `IntegrationEvent` are `Send + Sync + 'static`; an aggregate records one event type.
- The domain is synchronous except its gateways and domain services (DCA-ONI-004, DCA-RS-004).
- `EventId`, `Timestamp` and `PortError` keep their fields private.

## Guarding it

Stay on 0.x until the building blocks have carried a second consumer. From the first published version on,
`cargo semver-checks` against the last release belongs in the `Dockerfile` beside test, clippy and fmt, so a
breaking change is found by the build, not by a user: `cargo install cargo-semver-checks --locked`, then
`cargo semver-checks check-release --workspace`. Until there is a release there is nothing to compare against.

The `rust-version`s were checked on 2026-11-20 with the toolchains themselves: `dca-building-blocks` builds from
1.92 (1.91 refuses the default `in_transaction_closed`, the higher-ranked closure limitation rust-lang/rust#100013),
`dca-rules` from 1.88 (its own code; the newest `cargo-platform` asks for 1.91, the MSRV-aware resolver picks an
older one). Check again with `cargo +<version> check -p <crate>` when a signature changes.
