# AGENTS.md

Guidance for AI coding agents working in `dca-rust` — the Rust variant of `dca-java` and `dca-dotnet`:
`dca-building-blocks` (the marker traits) and `dca-rules` (the rule suite, same rule ids, checked over the crate
graph with `cargo_metadata` and over the source with `syn`). Unpublished; planning WP-174. `README.md` explains
the crates.

## Principles that apply in every DCA repository

These hold for anyone working in any of the DCA repositories — human or agent — regardless of local tooling or memory.

1. **The samples exist to make the AI harness deterministic, not to ship features.** They are the experiment field
   for the harness: knowledge catalog, architecture rules, markers, plugins. Every architectural change in a sample
   answers three questions before it is done — does the catalog need a node (pitfall / decision / recipe /
   template)? could an ArchUnit / ArchUnitNET rule check it (same rule id in `dca-java` and `dca-dotnet`)? would one
   more *generic* marker in the building blocks make it checkable? Record the answer, "none" included, in the WP or
   ADR. Two agents building the same thing differently in the two samples is a determinism gap to close at the
   source (catalog, rule, marker), not with a code fix.
2. **Rules, markers and the catalog are general.** They are the foundation other production systems — any industry —
   build on with AI. Nothing in them may exist only because the e-commerce sample needs it: no shop vocabulary in
   rule or marker names or texts, no selection that only matches the sample's layout, no catalog node that presupposes
   a cart. The sample proves the general artifact; it is never its source of names or shapes.
3. **The core libraries are framework-neutral.** `dca-building-blocks` / `DomainCentric.BuildingBlocks` have zero
   dependencies; `dca-archunit` / `DomainCentric.ArchRules` reference frameworks only as configurable presets
   (`FrameworkAnnotations`, `FrameworkTypes`) and never on their class path. Framework-specific code goes into
   satellite artifacts (`dca-spring`, `dca-archunit-spring-modulith`) or into the sample. Spring is the default
   preset and the reference implementation's framework, not the vocabulary of the rules.
4. **Each reader artifact stands alone.** Guide, book and catalog bundle are independently readable; links never
   cross repository boundaries (the sample may cite the guide). `AGENTS.md` files are exempt — they carry the
   cross-project pointers and the sync duties.
## Working here

- **Build:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check` — the same three run in the `Dockerfile`.
- **The rule catalog:** `dca-rules/rules.json` holds every DCA id with its Rust meaning (category, enforcement
  levels, `selects`/`checks`, whether the suite implements it). It is written by
  `python3 dca-rules/mapping/map_rules.py ../dca-java/rules.json dca-rules/rules.json <implemented.json>` — titles
  and rationales come from the Java list, so an id keeps one meaning; the Rust readings live in the script.
  Regenerate it with every rule change; a mapping that misses a Java id fails. The knowledge catalog reads it
  (a `## Rust reading` per rule node): regenerate the catalog after a change here, as after one in `dca-java`.
- **A new rule** is a function in `dca-rules/src/rules/<set>.rs`, one line in `define_rules!` in
  `dca-rules/src/rules/mod.rs` (which also names its test in `dca_tests!()`), a violation in
  `dca-rules/fixtures/violations` and nothing that fires in `dca-rules/fixtures/conforming` — the fixture tests
  fail otherwise. Rust-only rules take the next `DCA-RS-nnn` and start `experimental`.
- **Fixtures** are never built, only read; they name contexts `alpha` and `beta` and types like `Thing` and
  `Part` — no vocabulary of a sample.
- **The building blocks have no dependencies.** `Timestamp` and `EventId` are their own values for that reason.
- **Macros:** the suite reads source, not expanded code. A trait impl it must see (a role) is written out.
- **Consumers:** the todo app the suite was proven on (outside this repository) depends on both crates by path. A change to a rule
  runs its architecture tests there too (`cargo nextest run -p arch-tests --profile ci`).

## Commit message format

Conventional commits, the scope naming the crate: `feat(dca-rules): …`, `fix(dca-building-blocks): …`.
