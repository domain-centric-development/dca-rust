"""Writes rules.json for DCA-Rust: every DCA rule id with its meaning in Rust.

Usage: python3 map_rules.py <path to dca-java/rules.json> <output rules.json>

Titles and rationales come from the Java catalogue, so the id keeps one meaning; selects and checks
say how Rust holds the rule. Each entry carries a category and the enforcement levels that hold it:

  category     compiler | lint-test | reformulated | n/a
  enforcement  workspace    - the crate graph: a crate sees only what its Cargo.toml names
               visibility   - private fields and modules, pub(crate)
               trait-bound  - a supertrait or a generic bound the compiler checks
               clippy       - clippy.toml of the crate (disallowed-types, disallowed-methods)
               deny         - cargo-deny over the workspace
               graph-test   - a #[test] over cargo metadata: crates, dependencies, [package.metadata.dca]
               source-test  - a #[test] over the source with syn
"""

import json
import sys

# The layout the selections speak about. A crate states its role in [package.metadata.dca]:
#   context = "<bounded context>"          (absent on the shared kernel, bootstrap and the suite itself)
#   layer   = "domain" | "application" | "adapters" | "api" | "infrastructure" | "shared-kernel" | "bootstrap"
# Inside an adapters crate the modules `incoming` and `outgoing` are the two adapter directions; inside
# an application crate `shared` holds the shared output ports and every other top-level module is a use
# case or a feature. A building-block role is a trait of dca-building-blocks; a type carries it through
# `impl <Role> for <Type>`, a trait through a supertrait, transitively.
LAYOUT = {
    "metadata": "[package.metadata.dca] context, layer",
    "layers": ["domain", "application", "adapters", "api", "infrastructure", "shared-kernel", "bootstrap"],
    "adapterModules": {"incoming": "incoming", "outgoing": "outgoing"},
    "applicationShared": "shared",
    "apiModules": {"api": "api", "events": "events"},
}

C, LT, RF, NA = "compiler", "lint-test", "reformulated", "n/a"
W, V, T, CL, D, G, S = "workspace", "visibility", "trait-bound", "clippy", "deny", "graph-test", "source-test"

META_SEL = "Types in a domain crate: structs, enums and their fields."
META_CHK = ("No derive and no attribute from the configured framework crates (persistence, web, injection) on the "
            "type or its fields. A domain crate that does not depend on such a crate cannot name the derive at all "
            "(DCA-ONI-002); the source test holds the case where the project allows the crate in the domain.")


MAP_FORMAT = ("The declarations: [[package.metadata.dca.upstreams]] with context, via (a list of api and events), translation "
              "(acl or conformist) and status (implemented, the default, or planned); "
              "[[package.metadata.dca.external-upstreams]] with name, interaction (outbound or inbound), translation, "
              "contract-crates (a list of package names) and status; [[package.metadata.dca.partnerships]] with context.")


def m(category, enforcement, selects, checks, title=None, status=None, reason=None):
    return dict(category=category, enforcement=enforcement, selects=selects, checks=checks,
                title=title, status=status, reason=reason)


MAPPING = {
    # --- layered -----------------------------------------------------------------------------------------
    "DCA-LAY-001": m(LT, [], "Informational - selects nothing.",
                     "Informational - carries doctrine only. The dependency direction is held by the crate graph and the hexagonal rules."),
    "DCA-LAY-002": m(C, [W, G], "Every crate with layer = domain, the shared kernel's included.",
                     "Its Cargo dependencies name no crate with layer = infrastructure or bootstrap. The compiler then refuses every use of such a crate; the graph test refuses the Cargo.toml line."),
    "DCA-LAY-003": m(C, [W, G], "Every crate with layer = application.",
                     "Its Cargo dependencies name no crate with layer = infrastructure or bootstrap, its own context's included."),
    "DCA-LAY-004": m(RF, [S], "Paths in every module of every crate with a layer that name the building-block trait TransactionBoundary (wiring) or one of the types [workspace.metadata.dca] transaction-types lists (transaction use), written crate::Type - the crate and the type's name are compared, the modules between are not. The default preset: sqlx::Transaction, diesel::connection::AnsiTransactionManager, sea_orm::DatabaseTransaction, rusqlite::Transaction, tokio_postgres::Transaction, mongodb::ClientSession.",
                     "Transaction use sits in an application crate or in the outgoing module of an adapters crate; wiring sits there or in an infrastructure or bootstrap crate. A domain, shared-kernel or api crate, an incoming adapter or another module of an adapters crate that names one is reported, once per path. Rust has no declarative transaction attribute, so there is no annotation selection; a call that runs a transaction without naming the type (a method on a pool) is not seen."),
    "DCA-LAY-005": m(C, [W], "Items in the building blocks' port module.",
                     "A crate cannot add an item to another crate's module, so the building blocks' output-port module holds what the library declares and nothing a project adds. The rule holds by the language."),
    "DCA-LAY-007": m(LT, [S], "In a crate that carries a context but no layer (the two-crate form), every top-level module.",
                     "Informational - lists every top-level module that is none of domain, application, adapters, api, events and infrastructure, and never fails. In the crate-per-layer form every item sits in a layer by its crate and nothing is listed."),
    "DCA-LAY-008": m(LT, [G], "Every application crate and its normal Cargo dependencies.",
                     "None is a crate [workspace.metadata.dca] persistence-crates lists (package names; sqlx, diesel, sea-orm, rusqlite, tokio-postgres and mongodb by default). Without the dependency the compiler refuses every use of the crate."),
    # --- onion -------------------------------------------------------------------------------------------
    "DCA-ONI-001": m(C, [W, G], "Every crate with layer = domain.",
                     "Its Cargo dependencies name no crate with layer = application, its own context's included."),
    "DCA-ONI-002": m(LT, [W, G, D], "Every crate with layer = domain, the shared kernel's included.",
                     "Its normal dependencies are the building blocks, the shared kernel and the configured allowed crates only (by default none). Dev-dependencies are not counted. cargo-deny can refuse known framework crates workspace-wide; the graph test is the per-crate check."),
    "DCA-ONI-003": m(LT, [W, S], META_SEL, META_CHK, title="Domain models must not carry prohibited framework derives or attributes"),
    "DCA-ONI-004": m(LT, [W, S], "Every module of every domain crate.",
                     "No function is async or returns a type named Future, no expression awaits or builds an async block, and no path names a crate [workspace.metadata.dca] async-runtime-crates lists (tokio, async-std, futures and smol by default). DCA-RS-004 holds the signatures alone."),
    # --- hexagonal ---------------------------------------------------------------------------------------
    "DCA-HEX-001": m(C, [W, G], "Every crate with layer = domain.",
                     "Its Cargo dependencies name no crate with layer = adapters, of any context."),
    "DCA-HEX-002": m(C, [W, G], "Every crate with layer = application.",
                     "Its Cargo dependencies name no crate with layer = adapters, of any context."),
    "DCA-HEX-003": m(LT, [S], "Items in the incoming module of every adapters crate.",
                     "No item names a trait that carries the repository role or a type that implements one. Other output ports are not checked."),
    "DCA-HEX-004": m(LT, [W, G, S], "The incoming module of every adapters crate.",
                     "No item names a path into a crate with layer = infrastructure or bootstrap. The graph test refuses the dependency where the whole adapters crate must not have it; the source test refuses the use in the incoming module where the outgoing module may."),
    "DCA-HEX-005": m(C, [W, G], "Every crate with layer = adapters.",
                     "Its Cargo dependencies name no crate with layer = infrastructure of another context."),
    "DCA-HEX-006": m(LT, [S], "Items in the incoming module of every adapters crate, the configured event-consumer submodule excluded.",
                     "No item names a path into the outgoing module of the same crate. The compiler cannot hold this: both modules are one crate."),
    "DCA-HEX-007": m(LT, [W, G, S], "Every crate with layer = adapters and the items of its incoming module.",
                     "The crate depends on no crate of another context except that context's api crate; within the crate, the incoming module names no path into another context's crate, the configured event-consumer submodule excepted."),
    "DCA-HEX-008": m(LT, [S], "Structs and enums in every crate whose name ends with Repository.",
                     "Each sits in the outgoing module of an adapters crate."),
    "DCA-HEX-009": m(LT, [S], "Traits in the shared module of every application crate.",
                     "Each carries the output-port role, through OutputPort or one of its sub-traits (Repository, Store, the event publishers)."),
    "DCA-HEX-010": m(LT, [S], "Traits in every crate that carry the output-port role.",
                     "None sits in a domain crate. A domain crate sees the building blocks and could declare one; the compiler does not refuse it."),
    "DCA-HEX-011": m(LT, [S], "Items in the incoming module of every adapters crate, event consumers included.",
                     "No item names a type that implements an input port; naming the input-port trait is what the rule expects. The use case type stays pub because bootstrap constructs it, so visibility cannot hold this."),
    "DCA-HEX-012": m(LT, [S], "Items in the incoming module of every adapters crate.",
                     "No item names a type that carries the domain-service role."),
    "DCA-HEX-013": m(LT, [S], "Items in the outgoing module of every adapters crate.",
                     "No item names a trait that carries the input-port role or a type that implements one."),
    "DCA-HEX-014": m(RF, [S], "Handler functions in the incoming module of every adapters crate that a router preset named in [workspace.metadata.dca] routers registers for GET: axum - a free function passed to a routing function of axum (get(show)) or to get chained on one (post(create).get(show)); actix-web - one passed to .to(..) on web::get(), or one carrying #[get(..)] or #[route(..)] with GET; rocket - one carrying #[get(..)] or #[route(..)] with GET. Without a preset nothing is selected.",
                     "No such function names, in its signature or body, a type of an application crate whose name ends with Command - the input of a command port. Which port the handler calls is not resolved; a handler registered through a router another crate builds is found when that crate is in the workspace."),
    "DCA-HEX-016": m(RF, [S], "Function bodies in every adapters crate, and in them the enums declared in a domain or application crate: a parameter, a written-out local or a field of self of such a type, and a path to one of its variants.",
                     "Informational - lists every .to_string() on such an enum and every argument formatted with ? ({:?}, {:#?}, {0:?}, {name:?}, captured or named) by format!, write!, writeln!, print!, println!, eprint!, eprintln!, panic! or format_args!, and never fails. A serializer that writes the variant name, and an enum reached through an unannotated local, are not seen."),
    "DCA-HEX-017": m(RF, [S], "Functions registered as event listener by a configured listener preset.",
                     "A listener that calls an input port runs after the commit in its own transaction, as the preset expresses it. Not implemented: no Rust event library declares a listener's transaction phase, so there is no preset to select by; DCA-USE-012 holds that a use case writes inside its TransactionBoundary."),
    "DCA-HEX-018": m(RF, [S], "Handler functions the router presets named in [workspace.metadata.dca] routers register, anywhere in the workspace, read as DCA-HEX-014 reads them, for every verb. Without a preset nothing is selected.",
                     "Each is declared in the incoming module of an adapters crate. Rust handlers are functions, not classes; a method registered as a handler is not seen."),
    "DCA-HEX-019": m(LT, [T, S], "Every impl of a trait that carries the output-port role.",
                     "The implementing type sits in the outgoing module of an adapters crate. The orphan rule already keeps the impl out of the domain crate, which cannot see the application's port; a test double in the application crate is reported."),
    "DCA-HEX-020": m(LT, [S], "Functions and methods in the incoming module of every adapters crate, and the router expressions of those crates. Nothing is selected when [workspace.metadata.dca] names neither authorization-extractors (types whose presence in a handler's signature declares a claims-only check, such as a RequireRole<R> extractor) nor authorization-layers (functions whose call in a router's .layer(...) or .route_layer(...) declares it for the routes added before it); both are empty by default.",
                     "Informational - lists every call in a body, as a method or as a path, named has_role, has_any_role, has_authority, has_any_authority, is_in_role or is_user_in_role, and never fails. A function whose signature names a configured extractor, or whose name appears in the receiver of a .layer(...)/.route_layer(...) call that calls a configured layer function within the same crate, is not listed. Not seen: a handler routed behind a layer in a router another crate builds, a role compared by the handler itself (roles.contains(...)), and a helper of the project's own declared outside the incoming module. Rust has no route annotation; the extractor and the layer are its declarative forms."),
    "DCA-HEX-021": m(LT, [S], "Function bodies in the outgoing module of every adapters crate. Nothing is selected when no type implements AggregateRoot.",
                     "No body calls, on a parameter, a local binding whose type is written out or a field of self whose type names a type that implements AggregateRoot, a method of that type that takes &mut self - in Rust the receiver says which methods change the aggregate. A method of a building-block trait (clearing the recorded events after publishing) passes. A change written to storage directly, past the aggregate, and a call on an unannotated local are not seen."),
    # --- tactical ----------------------------------------------------------------------------------------
    "DCA-TAC-001": m(LT, [S], "Structs in every domain crate whose name ends with AggregateRoot.",
                     "The type implements AggregateRoot. Rust names rarely carry the suffix; selection by name stays as in Java."),
    "DCA-TAC-002": m(LT, [W, S], "Types that implement AggregateRoot.",
                     "No field type, generic arguments and Box<dyn ...> included, names a trait with the output-port role. The project's own ports live in the application crate, which the domain crate cannot see; the source test holds the building blocks' own port traits."),
    "DCA-TAC-003": m(LT, [S], "Types that implement AggregateRoot.",
                     "No field type, through Vec, Option, Box and other generic arguments, is a type that implements AggregateRoot. References by id pass."),
    "DCA-TAC-004": m(LT, [T, S], "Types that implement Entity, aggregate roots included.",
                     "The trait's associated type Id is bound to the Id marker, so the compiler holds the typed identity. The source test checks that a field of that type exists, so the id is state, not computed."),
    "DCA-TAC-005": m(LT, [V, S], "Types that implement Entity but not AggregateRoot.",
                     "The type has no pub field and no pub associated function that returns Self; pub(crate) and private ones pass. With private fields no code outside the aggregate's module can build one by struct literal - the compiler holds that part."),
    "DCA-TAC-006": m(LT, [S], "Types that implement Entity, aggregate roots included.",
                     "No pub method named set_<name> takes &mut self and exactly one further parameter and returns ()."),
    "DCA-TAC-007": m(LT, [S], "Types that implement Entity but not AggregateRoot.",
                     "No field type, through generic arguments, is a type that implements AggregateRoot."),
    "DCA-TAC-008": m(LT, [S], "Types that implement Value.",
                     "No field type, through generic arguments, is a type that implements Entity or AggregateRoot."),
    "DCA-TAC-009": m(NA, [], "", "", reason="Rust has no inheritance; a struct cannot be extended, so a value object is final by construction."),
    "DCA-TAC-010": m(RF, [S], "Types that implement Value.",
                     "No method takes &mut self, no field is pub, and no field type is an interior-mutability type (Cell, RefCell, Mutex, RwLock, atomics). Rust has no final field; immutability is the absence of a mutating API.",
                     title="Value Objects must not mutate their state"),
    "DCA-TAC-011": m(LT, [S], "Types that implement Value.",
                     "No method is named set_<name>, whatever its receiver."),
    "DCA-TAC-012": m(C, [T], "Types that implement Value.",
                     "The building-block trait Value has PartialEq as supertrait, so the compiler refuses a value object without equality. Whether the equality compares every attribute is not decidable; a derive is the expected form."),
    "DCA-TAC-013": m(LT, [S], "Traits in every application crate whose name ends with Repository.",
                     "The trait carries the repository role."),
    "DCA-TAC-014": m(LT, [S], "Traits that carry the repository role, the building block excluded.",
                     "Each sits in an application crate, the shared kernel's included."),
    "DCA-TAC-015": m(LT, [S], "Types that implement a trait with the repository role.",
                     "Each sits in the outgoing module of an adapters crate."),
    "DCA-TAC-016": m(LT, [T, S], "Traits with the repository role whose name ends with Repository.",
                     "The bound Repository<T: AggregateRoot> makes a repository for anything but a root unkompilable. The source test checks the name: the trait is named after the type it binds plus Repository."),
    "DCA-TAC-017": m(LT, [V, S], "Traits with the repository role.",
                     "No method of the trait returns a type, through generic arguments, that implements Entity but not AggregateRoot. A pub(crate) entity cannot be named by the application crate at all."),
    "DCA-TAC-018": m(LT, [S], "Traits outside adapters and infrastructure crates whose name ends with Store.",
                     "The trait carries the store role and not the repository role."),
    "DCA-TAC-019": m(LT, [S], "Traits that carry the store role, the building block excluded.",
                     "Each sits in an application crate."),
    "DCA-TAC-020": m(LT, [S], "Types that implement a trait with the store role.",
                     "Each sits in the outgoing module of an adapters crate."),
    "DCA-TAC-021": m(LT, [S], "Traits that carry the store role.",
                     "No method declared on the trait itself is named save, delete or delete_by_id."),
    "DCA-TAC-023": m(RF, [V, S], "Types that implement AggregateRoot.",
                     "No field is pub. Rust has no constructor; the struct literal is the constructor, and private fields keep it inside the aggregate's module. The root's associated functions (create, place, ...) are its factories and check invariants.",
                     title="Aggregate Roots must not expose a public struct literal"),
    "DCA-TAC-024": m(RF, [S], "Types that implement Value and their pub associated functions returning Self.",
                     "No function named new takes the same parameter types, in the same order, as another such function. Two named origins (from_input, from_stored) pass."),
    "DCA-TAC-025": m(LT, [S], "Types, fields and pub signatures in every domain crate, the shared kernel's included. Not selected: a single-field newtype that implements Value or Id, failures, and trait methods of a building block.",
                     "No field, parameter or return type, through generic arguments, is String, &str, a primitive, a time type (std::time::SystemTime, chrono, time) or a bare Uuid. A private field of type bool - a single yes/no fact - and a bool return - a predicate - pass; a bool parameter and a pub bool struct field, flag arguments, do not. A bool parameter of the factory that rebuilds a stored value - an associated function without a receiver that returns Self or the type, named as [workspace.metadata.dca] reconstituting-factories lists, reconstitute and restore by default, case ignored - passes; its other parameters are checked. The two-variant enum that names one fact and its negation, which the other languages report as a boolean in disguise, is not checked by the Rust suite yet."),
    "DCA-TAC-026": m(LT, [T, S], "Types that implement both Owned and AggregateRoot, each with the repository traits that bind it through Repository<T>. The owner id type is the type the impl Owned block binds to OwnerId (the trait requires it, and requires AggregateRoot as supertrait); the aggregate's id is the type its impl Entity binds to Id. An owned aggregate without a repository trait is not selected.",
                     "At least one method the repository trait declares itself takes a parameter whose type (through references) is the aggregate's id type and a further parameter whose type is the owner id type - either position, any name - and returns a type that names the aggregate, through Result, Option or another wrapper. A method that answers only yes or no does not count; the inherited find_by_id takes the id alone. An owner id type the suite cannot resolve (a type outside the workspace, a type alias) is reported. Whether a use case calls the scoped lookup is not checked; a lookup declared only in a generic intermediate trait is not seen. An empty selection passes."),
    # --- strategic ---------------------------------------------------------------------------------------
    "DCA-STR-001": m(LT, [G], "Every crate whose [package.metadata.dca] carries a context.",
                     "Informational - lists each context with its crates and their layers, and the shared-kernel crates, and never fails."),
    "DCA-STR-002": m(C, [W, G], "The crate with layer = shared-kernel.",
                     "Its Cargo dependencies name no crate that carries a context."),
    "DCA-STR-003": m(C, [W, G], "Every crate with layer = application.",
                     "Its Cargo dependencies name no crate of another context, that context's api crate included."),
    "DCA-STR-004": m(C, [W, G], "Every crate with layer = domain.",
                     "Its Cargo dependencies name no crate of another context, that context's api crate included."),
    "DCA-STR-005": m(LT, [S], "Types that implement the OpenHostService marker.",
                     "Each sits in an api crate or in the incoming module of an adapters crate."),
    "DCA-STR-006": m(C, [W, G], "Every crate with layer = adapters.",
                     "Its Cargo dependencies on another context name that context's api crate only. A foreign domain, application or adapters crate is refused by the graph test; anything the api crate does not export is refused by the compiler."),
    "DCA-STR-007": m(LT, [S], "Types that implement IntegrationEvent.",
                     "Each sits in the events module of an api crate."),
    "DCA-STR-008": m(RF, [S], "Types that implement IntegrationEvent.",
                     "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-STR-010": m(LT, [], "Informational - selects nothing.",
                     "Nothing is asserted. Whether a consumed integration event is translated before it reaches the domain is a review check."),
    "DCA-STR-011": m(LT, [G], "The workspace members.", "At least one crate carries a context in [package.metadata.dca]."),
    "DCA-STR-012": m(LT, [G], "The workspace members.", "At least one crate carries one of the layers in [package.metadata.dca]."),
    "DCA-STR-013": m(LT, [S], "Traits in the events module of every api crate.",
                     "Each carries the IntegrationEvent role."),
    # --- context map -------------------------------------------------------------------------------------
    "DCA-MAP-001": m(RF, [G], "Every crate's [package.metadata.dca], read for its upstreams, external-upstreams and partnerships arrays of tables. " + MAP_FORMAT,
                     "Only the crate that declares a context carries them: the context's domain crate, or, for a context without a domain crate, its crate that names no layer. A declaration on an application, adapters, api or infrastructure crate, or on a crate without a context, is reported; whether a declaration is well-formed is left to the other rules."),
    "DCA-MAP-002": m(RF, [G], "Every external-upstreams entry of every declaring crate, reading name, interaction, translation and status.",
                     "name is not blank and not the name of a context of the workspace; interaction is outbound or inbound, translation acl or conformist, status implemented or planned; and the pair (name, interaction) occurs at most once per context. The same external system declared by two contexts is not reported; contract-crates is not checked here."),
    "DCA-MAP-004": m(RF, [G], "Every upstreams entry of every declaring crate, reading context, translation and status. Planned entries are included.",
                     "context names a context some crate declares and not the declaring one; translation is acl or conformist and status implemented or planned. Whether any code depends on the target is not established here."),
    "DCA-MAP-005": m(RF, [G], "Every upstreams entry of every declaring crate, reading context and via. Planned entries are included.",
                     "via holds at least one channel, each one api or events, and the pair (context, channel) occurs at most once among the entries of one context; two entries towards one context on different channels pass."),
    "DCA-MAP-006": m(RF, [G], "Every declaring crate with its upstreams entries towards existing other contexts, planned ones included, and every normal Cargo dependency of the context's crates on another context's api crate.",
                     "The declared upstream contexts equal the contexts whose api crate a crate of the context depends on: a declared upstream no crate depends on is reported on the declaring crate, a dependency on an undeclared one on the depending crate. Cargo.toml is the module declaration in Rust; there is no second one to compare, and a dependency names a crate, not a channel - the channels are DCA-MAP-007's and DCA-MAP-011's."),
    "DCA-MAP-007": m(RF, [G, S], "Every upstreams entry with status implemented towards an existing other context, reading via. Planned entries and entries towards an unknown context are skipped.",
                     "For each channel in via, some module of some crate of the declaring context names a path into that channel of the target's api crate - the events module and below for events, the rest of the api crate for api. Which crate holds the use is not checked here."),
    "DCA-MAP-008": m(RF, [W, G, S], "Every upstreams entry with translation = acl towards an existing other context, reading via; the status is not consulted.",
                     "No crate of the declaring context other than its adapters crate has a normal dependency on the target's api crate; within the adapters crate, a path into the api channel sits only in the outgoing module and a path into the events channel only in the incoming module, for the channels the entry names. Whether the adapter translates is not established."),
    "DCA-MAP-009": m(C, [W, G], "Every upstreams entry with translation = conformist towards an existing other context; the status is not consulted.",
                     "The declaring context's domain crate has no normal dependency on the target's api crate; the graph test refuses the Cargo.toml line, and without it the compiler refuses every path into the crate. DCA-STR-004 refuses the same dependency for every context, declared or not."),
    "DCA-MAP-010": m(RF, [W, G, S], "Every external-upstreams entry that names contract-crates (package names; a crate outside the workspace counts by its name), reading translation and interaction; the status is not consulted. An entry without contract crates only documents the relationship.",
                     "With acl, no crate of the context but the adapters crate has a normal dependency on a contract crate, and within it only the matching module - outgoing for outbound, incoming for inbound - names a path into one. With conformist, the domain crate has no normal dependency on one. That the adapter translates is not established."),
    "DCA-MAP-011": m(RF, [G, S], "Every normal Cargo dependency of a context's crate on another context's api crate, and every path its modules name into such a crate, with the channel the path lies in (events module and below: events; the rest: api).",
                     "The declaring crate of the source context carries an upstreams entry for the target context - for a path, one whose via names that channel. Planned entries count as declared. Dependencies on a foreign context's other crates are not reported by this rule."),
    "DCA-MAP-012": m(RF, [G], "Every partnerships entry of every declaring crate, reading context.",
                     "It names a context some crate declares, not the declaring one, and the target's declaring crate carries a partnerships entry naming the declaring context in turn. A partnership grants no dependency."),
    "DCA-MAP-013": m(LT, [G], "Every upstreams, external-upstreams and partnerships entry of every declaring crate.",
                     "Informational - lists every declared edge with its channels or interaction and its translation, and never fails. The status is not printed."),
    "DCA-MAP-014": m(LT, [CL, G, S], "Every normal Cargo dependency of a context's crate on a crate of another context. A dependency on the api crate of a context the declaring crate names in a planned upstreams entry is skipped.",
                     "Some module of the crate names a path into the dependency. Paths inside macro invocations are not seen. The rustc lint unused_crate_dependencies holds the same per crate for every dependency."),
    # --- advanced ----------------------------------------------------------------------------------------
    "DCA-ADV-001": m(RF, [T, S], "Types that implement DomainEvent.",
                     "No method takes &mut self and no field type is an interior-mutability type. Implementing the trait is the selection itself."),
    "DCA-ADV-002": m(LT, [S], "Types that implement DomainEvent.", "Each sits in a domain crate."),
    "DCA-ADV-004": m(LT, [W, S], "Types that implement DomainEvent in a domain crate.", META_CHK),
    "DCA-ADV-005": m(C, [T], "Types that implement IntegrationEvent.",
                     "The trait requires the associated constants TYPE and VERSION, so the compiler refuses an integration event without its type metadata. Rust has no annotations; the constants are the metadata."),
    "DCA-ADV-006": m(LT, [S], "Types that implement IntegrationEvent.", "No field is named schema_version, event_version or contract_version."),
    "DCA-ADV-007": m(LT, [S], "Types that implement DomainEvent but not IntegrationEvent.", "No field is named schema_version, event_version or contract_version."),
    "DCA-ADV-008": m(LT, [T, S], "Types that implement DomainEvent.",
                     "The trait requires occurred_at(); the source test checks that a field of one of the configured timestamp types (by default the building blocks' Timestamp) backs it."),
    "DCA-ADV-010": m(LT, [S], "Types that implement DomainService.", "Each sits in a domain crate."),
    "DCA-ADV-011": m(LT, [W, S], "Types that implement DomainService in a domain crate.", META_CHK),
    "DCA-ADV-012": m(RF, [S], "Types that implement DomainService.",
                     "No method takes &mut self and no field type is an interior-mutability type. Rust has no final field; statelessness is the absence of mutation."),
    "DCA-ADV-013": m(LT, [S], "Types that implement Factory.", "The name ends with Factory."),
    "DCA-ADV-014": m(LT, [S], "Types that implement Factory.", "Each sits in a domain crate."),
    "DCA-ADV-015": m(LT, [W, S], "Types that implement Factory in a domain crate.", META_CHK),
    "DCA-ADV-016": m(RF, [S], "Types that implement Factory.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-ADV-017": m(LT, [S], "Types that implement Specification or whose name ends with Specification.", "Each sits in a domain crate."),
    "DCA-ADV-018": m(LT, [W, S], "Specifications in a domain crate.", META_CHK),
    "DCA-ADV-019": m(LT, [S], "Methods of types that implement AggregateRoot, Entity or Value.",
                     "No method body calls a function of, or constructs, a type that implements DomainService. A parameter of that type alone is not reported."),
    "DCA-ADV-020": m(LT, [S], "pub methods of types that implement DomainService.",
                     "At least one parameter is, by value or by reference, a type that implements AggregateRoot or Entity."),
    "DCA-ADV-021": m(LT, [S], "Every function body - free functions and the methods of impl blocks - in every crate except the application crates, the impls of a type that implements DomainService excluded. A domain service is a type that implements DomainService, as DCA-ADV-010 selects it: Rust marks no module, so a module of free functions is no domain service by this rule.",
                     "No body names a type that implements DomainService as an expression (a struct literal, a unit value, an associated function), nor calls a method on a parameter, a local binding or a field of self whose type implements it, nor on one built in place. In an infrastructure or bootstrap crate - the composition root - a construction (a struct literal, a unit value, an associated function without a receiver that returns Self) passes; a call there is reported. A parameter or a field of that type alone is not reported; a call through a generic bound or a trait object is not seen."),
    # --- use case ----------------------------------------------------------------------------------------
    "DCA-USE-001": m(LT, [S], "Traits named InputPort anywhere in the workspace.", "None outside the building blocks."),
    "DCA-USE-002": m(LT, [S], "Structs and enums whose name ends with Command.", "Each sits in an application crate."),
    "DCA-USE-003": m(LT, [S], "Structs and enums whose name ends with Query.", "Each sits in an application crate."),
    "DCA-USE-004": m(RF, [S], "Structs in application crates whose name ends with Command.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-005": m(RF, [S], "Structs in application crates whose name ends with Query.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-006": m(LT, [S], "Structs and enums whose name ends with Result and that do not implement Value. std's Result is not a declaration and is never selected.",
                     "Each sits in an application crate. An enum in a domain crate that a domain operation returns is a domain outcome and is exempt."),
    "DCA-USE-007": m(RF, [S], "Structs in application crates whose name ends with Result.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-008": m(LT, [S], "Structs and enums whose name ends with Response.", "Each sits in an adapters crate."),
    "DCA-USE-009": m(RF, [S], "Functions of types that implement an input port.",
                     "A function that calls save on a repository also takes the aggregate's events (take_events) and hands them to a DomainEventPublisher, directly or through a function of the same type."),
    "DCA-USE-010": m(LT, [W, S], "Items in every domain crate.", "No item declares or names a type whose name ends with Dto."),
    "DCA-USE-011": m(LT, [W, S], "Items in every application crate.", "No item declares or names a type whose name ends with Dto."),
    "DCA-USE-012": m(RF, [S], "Functions of types that implement an input port.",
                     "A function that calls save or delete on a repository or a DomainEventPublisher does so inside the closure it passes to TransactionBoundary (or a function of the same type that does). Rust has no transactional attribute; the explicit boundary is the only form."),
    "DCA-USE-013": m(NA, [], "", "", reason="Rust has no declarative transaction attribute; the boundary is the explicit TransactionBoundary port, which DCA-USE-012 reads, and the rule selects only declaratively transactional use cases."),
    "DCA-USE-014": m(LT, [S], "Types that implement an input port in every application crate, the shared module excluded.",
                     "All sit at one module depth: application::<use_case> or application::<feature>::<use_case>."),
    "DCA-USE-015": m(LT, [V, S], "Structs in application crates whose name ends with Result.",
                     "No field type, through generic arguments and into other application structs, implements AggregateRoot or Entity."),
    "DCA-USE-016": m(LT, [S], "Types that implement an input port.",
                     "No field, parameter or generic bound names another input-port trait or a type that implements one."),
    "DCA-USE-017": m(RF, [S], "Types that implement an input port.",
                     "No pub inherent method other than an associated function that returns Self (the constructor bootstrap calls). The operation lives in the input-port impl."),
    "DCA-USE-018": m(LT, [S], "Items in the incoming module of every adapters crate, and the input-port traits they name that take a command (the input of a UseCase<..> bound or a method parameter is a type whose name ends with Command) and are declared in a feature: two module levels deep in an application crate, application::<feature>::<use_case>, the shared module excluded.",
                     "The ports one item names belong to at most one feature. An application crate without feature modules selects nothing."),
    "DCA-USE-019": m(LT, [S], "Types that implement an input port whose input is a query.",
                     "No function calls a write (save, delete, remove, add, insert, update, ...) on a repository or store, nor an event publisher."),
    "DCA-USE-020": m(LT, [CL], "Every application crate.",
                     "clippy.toml lists uuid::Uuid::parse_str and uuid::Uuid::from_str under disallowed-methods. Parsing happens in the adapter or in the id's own constructor."),
    "DCA-USE-021": m(LT, [S], "Types that implement an input port (a trait extending InputPort or UseCase).",
                     "No method of their impls takes &mut self, and no field holds interior mutability (Cell, RefCell, Mutex, RwLock, an atomic)."),
    "DCA-USE-022": m(LT, [S], "impl UseCase<I> blocks, read for their Output type, and the traits that extend UseCase<I, Output = O>.",
                     "O, or the success type of a Result it names, is none of String, &str, bool, char or a numeric primitive. () passes - a command with nothing to answer -, and so does a container such as Vec<String>."),
    # --- naming ------------------------------------------------------------------------------------------
    "DCA-NAM-001": m(LT, [S], "Types in application crates that implement an input port.", "The name ends with UseCase."),
    "DCA-NAM-002": m(NA, [], "", "", reason="Rust has no injectable stereotype attribute; bootstrap constructs use cases in code, so there is nothing to list."),
    "DCA-NAM-003": m(LT, [S], "Traits in application crates that carry the input-port role, InputPort itself excluded.", "The name ends with InputPort."),
    "DCA-NAM-004": m(LT, [S], "Traits in application crates whose name contains Repository.", "The name ends with Repository."),
    "DCA-NAM-005": m(NA, [], "", "", reason="Rust web handlers are free functions, not controller classes; the module that holds them carries the name, and DCA-HEX-018 places it."),
    "DCA-NAM-006": m(NA, [], "", "", reason="Rust web handlers are free functions, not resource classes; the module that holds them carries the name, and DCA-HEX-018 places it."),
    "DCA-NAM-007": m(LT, [S], "Structs and enums whose name ends with Dto.", "Each sits in an adapters crate."),
    "DCA-NAM-008": m(LT, [S], "Structs and enums whose name ends with Converter.", "Each sits in an adapters crate."),
    "DCA-NAM-009": m(LT, [S], "Every module in the workspace's context crates.",
                     "No module is named entities, value_objects, helpers, util or utils."),
    "DCA-NAM-010": m(LT, [S], "Types in every domain crate.", "No name ends with Helper, Util, Utils, Impl or Implementation."),
    "DCA-NAM-011": m(LT, [S], "Structs whose name ends with ViewModel.", "Each sits in the incoming module of an adapters crate, in its configured web submodule."),
    "DCA-NAM-012": m(LT, [S], "Input-port traits and use-case types in application crates.",
                     "Informational - lists names that contain a trigger word (On, After, When, Upon, Before followed by a capital) and never fails."),
    "DCA-NAM-013": m(LT, [S], "Input-port traits and use-case types in application crates, and pub functions of api crates.",
                     "No type name starts with Set or Update followed by a capital, and no function name starts with set_ or update_."),
    # --- cycles ------------------------------------------------------------------------------------------
    "DCA-CYC-001": m(RF, [W, S], "One slice per top-level module of every domain crate; the items of the crate root belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Between the domain crates of two contexts Cargo refuses a cycle and DCA-STR-004 refuses any edge; inside one crate modules may name each other both ways, so the crate is sliced. Submodules of one top-level module form one slice."),
    "DCA-CYC-002": m(RF, [W, S], "One slice per top-level module of every application crate, the shared module included; the items of the crate root belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it. A cycle between use cases or features alone is also DCA-CYC-005's; one through the shared module is this rule's only."),
    "DCA-CYC-003": m(RF, [W, S], "One slice per module directly below the outgoing module of every adapters crate; items of the outgoing module itself belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it."),
    "DCA-CYC-004": m(RF, [W, S], "One slice per module directly below the incoming module of every adapters crate; items of the incoming module itself belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it. A path from the incoming into the outgoing module is DCA-HEX-006's."),
    "DCA-CYC-005": m(LT, [S], "One slice per use-case or feature module of every application crate, the shared module excluded.",
                     "The slices form no dependency cycle through use paths. Modules within one crate may depend on each other in both directions; the compiler allows it."),
    # --- errors ------------------------------------------------------------------------------------------
    "DCA-ERR-001": m(RF, [S], "Types in every domain crate that implement std::error::Error.",
                     "Each implements DomainFailure. Rust has no exceptions; a failure is a type returned in Err.",
                     title="Failures declared in the domain layer must implement DomainFailure"),
    "DCA-ERR-002": m(LT, [S], "Types that implement DomainFailure or a port failure (UseCaseFailure, OutputPortFailure, or the classification traits PortFailure and InputPortFailure).",
                     "A DomainFailure sits in a domain crate, a port failure in an application crate - an output port's failure beside the port, although the adapter that implements the port returns it."),
    "DCA-ERR-003": m(RF, [S], "Types in every application crate that implement std::error::Error and not DomainFailure.",
                     "Each implements UseCaseFailure or OutputPortFailure. Implementing only the classification traits PortFailure or InputPortFailure does not count: a type implements the leaf it is.",
                     title="Failures declared in the application layer must implement UseCaseFailure or OutputPortFailure"),
    "DCA-ERR-004": m(C, [W, G], "Types that implement DomainFailure or a port failure.",
                     "None implements a web framework's response trait (for example axum's IntoResponse): the domain and application crates do not depend on the framework, and the orphan rule refuses the impl in the adapters crate, where both trait and type are foreign. The adapter translates through its own type."),
    "DCA-ERR-005": m(RF, [S], "Types that implement DomainFailure or a port failure (UseCaseFailure, OutputPortFailure).",
                     "The name contains none of Http, StatusCode, ResponseStatus, Response. The suffix part of the Java rule does not apply: Rust names a failure ...Error by convention."),
    "DCA-ERR-006": m(LT, [S], "Modules in the incoming part of every adapters crate that name an input port.",
                     "Informational - lists a module when no item in it names a DomainFailure or UseCaseFailure type nor a variant of a closed result, and never fails. A closed result is an enum an input-port trait answers with - named in an associated-type binding of its bounds (Output = ...) or in a method's return type, through any wrapper such as Result. A variant counts when a path names it through its enum or through a use of it, so a match over the variants is seen; whether the match is exhaustive is left to the compiler. When the workspace has neither a failure type nor a closed result, nothing is listed. It also lists a module that names one of the classification traits PortFailure, InputPortFailure or OutputPortFailure itself - a bound or a trait object over the base is the generic handler that answers every outcome the same way."),
    "DCA-ERR-007": m(LT, [S], "Types that implement DomainFailure or a port failure, their fields and variant fields.",
                     "Informational - lists every field whose type names String or a type that implements AggregateRoot or Entity, through generic arguments, and never fails."),
    "DCA-ERR-008": m(LT, [S], "Closed results in every application crate: an enum an input-port trait answers with - named in an associated-type binding of its bounds (Output = ...) or in a method's return type, through any wrapper such as Result - as DCA-ERR-006 finds them; an enum that implements std::error::Error, DomainFailure or UseCaseFailure is the failure side and not selected.",
                     "Informational - lists every closed result none of whose variants carries a field whose type implements the building-block trait Refusal, and never fails. A unit variant carries nothing and cannot be the refusing one: a refusing variant wraps a value of a type that implements Refusal, and the result enum implements ClosedResult, whose refusal() match the compiler checks for coverage and for the Refusal bound - that is what TransactionBoundary::in_transaction_closed rolls back on. A listed result is not a finding by itself: one whose outcomes are all successes - found or nothing found - is correct as it is. A struct is no closed result."),
    "DCA-ERR-009": m(LT, [S], "Every expression in a function body of a module outside the domain crates - application, adapters, api, infrastructure, bootstrap and unplaced crates; #[cfg(test)] items and test targets are not read: a struct literal, a call of a tuple struct or an enum variant, a unit struct or unit variant used as a value, a variant or tuple struct passed as a function, and a call of an associated function without a receiver that returns Self or the type itself, whose type implements DomainFailure.",
                     "None of them names a domain failure - only a domain crate constructs one. Self is not followed: a domain failure declared outside the domain is DCA-ERR-002's finding. A failure the application only passes on (a variant wrapping it, ? or map_err with a conversion) is not a construction. Constructions inside macro invocations, through From/Into conversions or a generic constructor are not seen. An empty selection passes."),
    "DCA-ERR-010": m(RF, [S], "Every function body - free functions, methods of impl blocks, default methods of traits, closures included - in every application crate and in the incoming module of every adapters crate, and in it every call of a method on a port: a parameter, a local binding whose type is written out, or a field of self, whose type names a trait that carries the input-port or output-port role or a type that implements one, and a field of a parameter whose type is a struct of the workspace holding such a field (ports.add, State(ports): State<Ports> included - a one-field tuple-struct pattern binds its parameter). A field's type is read with the bounds of its struct and of the impl block. Rust has no try block; a Result is answered where it is consumed.",
                     "No port call has its failure answered without naming it: not an Err(_) arm, nor a _ or a bare binding arm the failure reaches, of a match over the call; not if let Ok(..) (which drops the failure) or if let Err(_) over it, nor let Ok(..) = .. else; not let _ = on it; not .ok(), .unwrap_or(..), .unwrap_or_default(), .or(..), .map_or(..), .is_ok() or .is_err() on it, nor .unwrap_or_else, .map_err, .or_else or .map_or_else with a closure that discards its (first) argument (|_| ..); and no port call inside std::panic::catch_unwind, the catch of every panic. The call is seen through parentheses and .await. Propagation with ?, an arm that matches a named failure or takes it in hand (Err(e)), and a map_err whose closure uses the failure pass. Not seen: a Result that is first bound to a name and answered later, a call through a helper of the type's own, and a call on a receiver whose type the suite cannot resolve (an unannotated local, a return value)."),
}

RUST_ONLY = [
    dict(id="DCA-RS-001", title="The domain holds no shared mutable state",
         rationale="Rc, Arc, Cell, RefCell, Mutex and RwLock give a value a second owner or a way around &mut. Inside an aggregate they let code outside the root change its parts without the root's invariant check, which is what ownership otherwise prevents.",
         category=LT, enforcement=[CL, S],
         selects="Every domain crate.",
         checks="clippy.toml lists std::rc::Rc, std::sync::Arc, std::cell::Cell, std::cell::RefCell, std::sync::Mutex and std::sync::RwLock under disallowed-types; the source test refuses the same types in field types, so a crate without the clippy.toml is held too."),
    dict(id="DCA-RS-002", title="Entities compare by identity",
         rationale="#[derive(PartialEq)] compares every field, which is value equality. Two states of the same entity must be equal and two entities with equal data must not.",
         category=LT, enforcement=[S],
         selects="Types that implement Entity, aggregate roots included.",
         checks="The type does not derive PartialEq, Eq or Hash. A manual PartialEq is the expected form; whether it compares only the id is not decidable."),
    dict(id="DCA-RS-003", title="Aggregate roots hand out no mutable reference to their parts",
         rationale="A pub method that returns &mut to an inner entity or collection lets the caller change the aggregate without passing the root's invariant check. Ownership does not prevent it; the signature does.",
         category=LT, enforcement=[S],
         selects="pub methods of types that implement AggregateRoot.",
         checks="No return type, through generic arguments, contains a &mut reference."),
    dict(id="DCA-RS-004", title="The domain stays synchronous",
         rationale="Async is an I/O concern of ports and adapters. A synchronous domain stays testable and deterministic and pulls no runtime into the model.",
         category=LT, enforcement=[W, S],
         selects="Functions and methods in every domain crate.",
         checks="None is async and none returns a type that implements Future. A domain crate without a runtime dependency cannot spawn a task either."),
    dict(id="DCA-RS-005", title="The domain reads no clock and no randomness",
         rationale="A domain that reads the time or draws a random number gives different results for the same input, so its tests depend on the moment they run. The value comes in from the application as a parameter.",
         category=LT, enforcement=[CL],
         selects="Every domain crate.",
         checks="clippy.toml lists std::time::SystemTime::now, std::time::Instant::now, chrono::Utc::now, chrono::Local::now and rand::random under disallowed-methods, and uuid::Uuid::new_v4 and uuid::Uuid::now_v7."),
    dict(id="DCA-RS-006", title="An aggregate's event enum carries one event type per variant",
         rationale="An event is a type of its own, in every DCA language: it can be named, passed, consumed and checked alone (DCA-ADV-001, DCA-ADV-008 per type). An enum gathers an aggregate's events the way a sealed interface lists its records; a variant with inline fields is an event without a type.",
         category=LT, enforcement=[S],
         selects="Enums that implement DomainEvent.",
         checks="Every variant has exactly one unnamed field whose type implements DomainEvent and is not the enum itself."),
]

# What the first suite implements; the rest stays planned. Rules held by the compiler are proven by a
# fixture that does not compile, not by a test.
IMPLEMENTED = set()  # filled by the suite's own list, see ../src/rules/mod.rs


def main():
    java_path, out_path = sys.argv[1], sys.argv[2]
    java = json.load(open(java_path))
    implemented = set(json.load(open(sys.argv[3]))) if len(sys.argv) > 3 else IMPLEMENTED
    rules = []
    missing = []
    for r in java["rules"]:
        mapping = MAPPING.get(r["id"])
        if mapping is None:
            missing.append(r["id"])
            continue
        if mapping["category"] == NA:
            rules.append(dict(set=r["set"], id=r["id"], status="n/a", reason=mapping["reason"]))
            continue
        entry = dict(set=r["set"], id=r["id"], title=mapping["title"] or r["title"], rationale=r["rationale"],
                     selects=mapping["selects"], checks=mapping["checks"], implementation="rust",
                     status=r.get("status", "enforced"), maturity=r.get("maturity", "released"),
                     category=mapping["category"], enforcement=mapping["enforcement"],
                     implemented=r["id"] in implemented)
        rules.append(entry)
    extra = sorted(set(MAPPING) - {r["id"] for r in java["rules"]})
    if missing or extra:
        sys.exit(f"mapping out of step with the Java catalogue: missing {missing}, unknown {extra}")
    for r in RUST_ONLY:
        rules.append(dict(set="rust", id=r["id"], title=r["title"], rationale=r["rationale"], selects=r["selects"],
                          checks=r["checks"], implementation="rust", status="enforced", maturity="experimental",
                          category=r["category"], enforcement=r["enforcement"], implemented=r["id"] in implemented))
    out = dict(library="dca-rust", version="0.0.0", layout=LAYOUT, retired=java.get("retired", []), rules=rules)
    with open(out_path, "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")


if __name__ == "__main__":
    main()
