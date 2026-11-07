# dca-rust

*Written with AI assistance — drafted mainly by Claude, reviewed and directed by Christoph Bloemer since
2025. The architecture rules in this repository's tests are part of how that work is verified.*

Architecture rules for Domain-Centric Architecture in Rust: the same rule ids as the Java and .NET suites,
each held at the earliest level that can hold it — the crate graph, visibility, a trait bound, Clippy,
cargo-deny, or a test over the crate graph and the source.

| Crate | What it is |
|---|---|
| `dca-building-blocks` | the marker traits — `Value`, `Id`, `Entity`, `AggregateRoot`, `DomainEvent`, `Repository`, `Store`, `UseCase`, `InputPort`, `OutputPort`, `TransactionBoundary`, `DomainFailure`, `UseCaseFailure`, `OutputPortFailure` (port failures, classified as `PortFailure`), `Refusal`, `ClosedResult`, … — no dependencies |
| `dca-rules` | the rule suite: `rules.json` (every DCA id with its Rust meaning), the rules, and `dca_tests!()` |

A project names each crate's role in its manifest and runs the suite from a test crate:

```toml
[package.metadata.dca]
context = "orders"
layer = "domain"        # domain | application | adapters | api | infrastructure | shared-kernel | bootstrap
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
Not yet on crates.io; built and proven on a todo app.

## Author

**Christoph Bloemer** — [@chbloemer](https://github.com/chbloemer)

## License

MIT — see [LICENSE](LICENSE).

Contributions are accepted under the MIT licence, and the copyright holder may additionally publish
them under other licences (for example a documentation licence for prose).
