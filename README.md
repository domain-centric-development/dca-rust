# dca-rust

*Written with AI assistance — drafted mainly by Claude, reviewed and directed by Christoph Bloemer since
2025. The architecture rules in this repository's tests are part of how that work is verified.*

Architecture rules for Domain-Centric Architecture in Rust: the same rule ids as the Java and .NET suites,
each held at the earliest level that can hold it — the crate graph, visibility, a trait bound, Clippy,
cargo-deny, or a test over the crate graph and the source.

| Crate | What it is |
|---|---|
| `dca-building-blocks` | the marker traits — `Value`, `Id`, `Entity`, `AggregateRoot` (its events kept in a `DomainEvents<E>` field), `Owned`, `DomainEvent`, `DomainService`, `DomainGateway`, `Repository`, `Store`, `UseCase`, `InputPort`, `OutputPort`, `DomainEventPublisher`, `TransactionBoundary` with the use case's `TransactionUnit`, `EventListener` and `TransactionalEventListener`, `PortError` with its `PortErrorKind`, `DomainFailure`, `UseCaseFailure`, `OutputPortFailure` (port failures, classified as `PortFailure`), `Refusal`, `ClosedResult`, … — no dependencies |
| `dca-rules` | the rule suite: `rules.json` (every DCA id with its Rust meaning), the rules, and `dca_tests!()` |

A project names each crate's role in its manifest and runs the suite from a test crate:

```toml
[package.metadata.dca]
context = "orders"
layer = "domain"        # domain | application | adapters | api | events | infrastructure | bootstrap
```

A context publishes in two crates, as Java keeps `{context}/api` and `{context}/events` side by side: the `api`
crate holds the operations other contexts call (an open host service), the `events` crate the integration events
they consume. A consumer of the events depends on the contract alone. Both are optional; another context's crates
depend on nothing else of it.

A shared kernel has crates like a context, one per layer, and names no context:

```toml
[package.metadata.dca]
shared-kernel = true
layer = "domain"        # domain | application | adapters | infrastructure
```

Every layer rule reads a kernel crate as a context's crate of that layer. Every context may depend on the kernel layer
by layer — its domain on the kernel's domain, its application on the kernel's domain and application, its adapters on
all of it — and the kernel depends on no context (DCA-STR-002). A shared outbox is the kernel's outgoing adapter.

**The application layer is asynchronous; the domain is synchronous, except where it asks the outside.** The input
ports, the output ports and the transaction boundary return futures that are `Send`.
There is one form of each port: an adapter over a database implements them as the database's driver allows, and an
adapter over memory or files implements the same ports — its work done inline when it is quick, behind the runtime's
blocking pool (`spawn_blocking`) when it is not. A project that swaps its store later swaps an adapter, nothing else.

In the domain, only two kinds of item may be asynchronous. A **domain gateway** — a trait the domain declares to
obtain a fact it does not hold (a tax rate, an availability) or to delegate a computation bound to a technology (a
password hash), implemented in an outgoing adapter, never writing to the outside — answers synchronously or
asynchronously; a **domain service** asks an asynchronous one and awaits the answer. Aggregates, entities, values,
events, failures, factories and specifications stay synchronous. A method of the model may take a gateway as a
parameter and call it — a synchronous one, then; it never holds one in a field, hands one out, or looks one up from a
registry, a static or by building it in place (DCA-TAC-027). The futures are std's; no async runtime enters a domain
crate (DCA-ONI-004, DCA-RS-004). Java and .NET keep the whole domain synchronous; the Rust reading of those rules says
so.

```rust
pub trait TaxRates: DomainGateway {                    // domain crate
    fn rate_for(&self, region: &Region) -> impl Future<Output = Option<Rate>> + Send;
}

impl Pricing {                                         // a DomainService
    pub async fn price<R: TaxRates + Sync>(&self, order: &Order, rates: &R) -> Price {
        let rate = rates.rate_for(order.region()).await.unwrap_or_default();
        order.total_with(rate)                         // the aggregate computes, synchronously
    }
}

impl User {                                            // an aggregate, a synchronous gateway as parameter
    pub fn change_password<H: PasswordHasher>(&mut self, hasher: &H, password: &Password) {
        self.digest = hasher.digest(password);
    }
}
```

A use case runs its writes through the transaction boundary, which hands the work the use case's own unit — the
repositories, stores, event publisher and inbox that take part in the open transaction. Each use case declares its
unit beside it, in one form (DCA-RS-007): every port a generic associated type over a lifetime, handed out by value
(`Self::Things<'_>`). A store's adapter hands out a type of its own that borrows the transaction's connection; an
adapter that owns its ports hands out `&'a mut` to one — a `&mut` to a port is the port, and a port trait of the
application's own is implemented for the `&mut` by the adapter. The work also borrows the
use case itself — its clock, its id source, its domain services:

```rust
use dca_building_blocks::application::{BoxFuture, ScopedFutureExt, TransactionBoundary, TransactionUnit};
use dca_building_blocks::hexagonal::port_in::{InputPort, UseCase};
use dca_building_blocks::hexagonal::port_out::{DomainEventPublisher, Repository};

/// What archiving a thing works with inside its transaction.
pub trait ArchiveThingUnit: TransactionUnit + Send {
    type Things<'a>: Repository<Thing> + Send where Self: 'a;
    type Events<'a>: DomainEventPublisher<ThingEvent> + Send where Self: 'a;
    fn things(&mut self) -> Self::Things<'_>;
    fn events(&mut self) -> Self::Events<'_>;
}

/// The input port; every use case that answers the command this way is it.
pub trait ArchiveThingInputPort: UseCase<ArchiveThingCommand, Output = Result<(), ArchiveThingError>> {}
impl<P> ArchiveThingInputPort for P where P: UseCase<ArchiveThingCommand, Output = Result<(), ArchiveThingError>> {}

pub struct ArchiveThingUseCase<T, C, U> {
    transactions: T,
    clock: C,
    unit: PhantomData<fn() -> U>,
}

impl<T, C, U> InputPort for ArchiveThingUseCase<T, C, U> {}

impl<T, C, U> UseCase<ArchiveThingCommand> for ArchiveThingUseCase<T, C, U>
where
    T: TransactionBoundary<U> + Sync,
    C: Clock + Sync,
    U: ArchiveThingUnit,
{
    type Output = Result<(), ArchiveThingError>;

    fn execute(&self, command: ArchiveThingCommand) -> BoxFuture<'_, Self::Output> {
        Box::pin(self.transactions.in_transaction(move |unit| {
            async move {
                let mut thing = unit.things().find_by_id(&command.thing).await?
                    .ok_or(ArchiveThingError::NotFound)?;
                thing.archive(self.clock.now());               // the work borrows the use case
                unit.things().save(&thing).await?;
                unit.events().publish_and_clear_events(&mut thing).await?;
                Ok(())
            }
            .scope_boxed()
        }))
    }
}
```

The port is a trait of the use case's own over `UseCase`, implemented by one blanket impl beside it, so the use
case's bounds are written once, on its `UseCase` impl; the use case writes out `impl InputPort`, which is where the
rule suite reads its role. The work is a closure that returns its future boxed with `.scope_boxed()`; the compiler
infers the rest. The boxed
`ScopedBoxFuture` carries two lifetimes — the unit's and the longer one of what the work borrows around it — which is
what lets the future borrow both and still be `Send` on stable Rust. A method that hands the boundary's future on
returns it as `impl Future<..> + Send` or boxed (`UseCase::execute` returns a `BoxFuture`, so an incoming adapter holds
the port as `dyn`); written as an `async fn`, the compiler cannot yet prove such a future `Send`.

The failure type of the work takes a `PortError` (`E: From<PortError>`): a transaction that cannot begin or commit ends
in the use case's "unavailable" case. A rollback that fails after the work failed is reported in place of the work's
failure (`PortError::rollback_failed`, of the kind `PortErrorKind::RollbackFailed`): what the store holds is then no
longer known, and that outweighs the failure that asked for the rollback. A `PortError` is the system's own
infrastructure failing; a failure the caller can answer — a conflict, a uniqueness only the store sees — is an
`OutputPortFailure` the port declares.

The aggregate and its events are stored in the same transaction; another transaction reads an event back and reacts
to it. The event publisher the unit hands out calls the listeners that react inside the transaction
(`EventListener<E, U>`, output ports only, which they reach through `U`, the ports of the open transaction the
publisher hands them) and records, in the same transaction, a delivery for each listener that drives a use case
(`TransactionalEventListener<E>`); the use case only calls `publish_and_clear_events` and never names a listener.
How that delivery survives a crash — an outbox table written in the publishing transaction, a relay that delivers
at least once, a record of what each listener received so a repeat is passed over — is not a building block: no rule
reads it, so it belongs to an adapter crate over the store. The example keeps it in its shared kernel, the record of
receipts as a `Store` its transaction units hand out.

A program without a runtime drives a use case with a small executor — `pollster::block_on`, or this one, std only,
for futures that never wait on I/O (memory, files done inline):

```rust
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::yield_now();
    }
}
```

The crate that declares a context — its domain crate, or its crate without a layer when it has no
domain crate — also carries the context map, the relationships the context-map rules (DCA-MAP-…) check
against the Cargo dependencies and the source:

```toml
description = "Orders and their lines."   # the context's description, on the crate that declares it

[[package.metadata.dca.upstreams]]       # a context of the workspace this one consumes
context = "billing"
via = ["api", "events"]                  # the channels: the api crate and the events crate
translation = "acl"                      # acl | conformist
status = "implemented"                   # implemented (default) | planned
rationale = "billing's tariff model stays out of ours"   # optional, listed by DCA-MAP-013

[[package.metadata.dca.external-upstreams]]   # a system outside the workspace
name = "Payment Provider"
interaction = "outbound"                 # outbound: this context calls it | inbound: it calls this context
translation = "acl"
contract-crates = ["provider-sdk"]       # the crates that carry its contract types, if any
protocol = "HTTPS/JSON"                  # optional: protocol, exchanges, rationale document the edge
exchanges = "payments, refunds"

[[package.metadata.dca.partnerships]]    # declared on both sides
context = "shipping"
rationale = "one team, one release train"
```

```rust
// tests/architecture.rs of a test crate with dca-rules as a dev-dependency
dca_rules::dca_tests!();   // one #[test] per rule id: dca_tac_003, dca_use_012, …
```

A violation fails its test with one line per finding, the rule id first:

```
[DCA-TAC-003] orders_domain::order::Order (crates/orders-domain/src/order/mod.rs:12): field `customer` holds `Customer`, which carries the role AggregateRoot; reference another aggregate by its id
```

Experimental rules run only where `[workspace.metadata.dca] experimental` names them (`"all"` for every one).
The same table configures what a rule selects by, framework crates only as presets: `routers` (`axum`,
`actix-web`, `rocket`; none by default — the handler rules then select nothing), `transaction-types`,
`persistence-crates`, `async-runtime-crates`, `authorization-extractors`, `authorization-layers`; each rule's
`selects` in `rules.json` names the key it reads and its default.

Layers are named per crate, so their names are free. The names inside a crate and the name suffixes are
renamed in the same table, as Java's `DcaLayout` renames packages and suffixes; each value is one identifier, and
a value that is none fails the load instead of switching a rule off:

```toml
[workspace.metadata.dca]
incoming-module = "incoming"           # the incoming adapters in an adapters crate
outgoing-module = "outgoing"           # the outgoing adapters
shared-module = "shared"               # the shared output ports of an application crate
web-module = "web"                     # server-rendered adapters below the incoming module
event-consumer-module = "event"        # event consumers below the incoming module
operation-containers = []              # application modules that only group operations
use-case-suffix = "UseCase"
input-port-suffix = "InputPort"
aggregate-root-suffix = "AggregateRoot"
repository-suffix = "Repository"
store-suffix = "Store"
factory-suffix = "Factory"
specification-suffix = "Specification"
```

Java's controller suffixes have no counterpart: a Rust handler is a function the router registers, found through
the `routers` preset, not by its name.
Not yet on crates.io; built and proven on a todo app. `example/` is a small compiled library application -
two contexts and a shared kernel - that uses every building block, the outbox, its relay and the inbox included, with
the rule suite green on it.

## Author

**Christoph Bloemer** — [@chbloemer](https://github.com/chbloemer)

## License

MIT — see [LICENSE](LICENSE).

Contributions are accepted under the MIT licence, and the copyright holder may additionally publish
them under other licences (for example a documentation licence for prose).
