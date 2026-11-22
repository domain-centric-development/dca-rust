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
#   layer   = "domain" | "application" | "adapters" | "api" | "infrastructure" | "bootstrap"
#   shared-kernel = true                    (a crate of the shared kernel: no context, and a layer of domain,
#                                            application, adapters or infrastructure)
# A shared kernel has crates like a context, one per layer. "A crate with layer = domain", "a domain crate" and the
# like include the shared kernel's crate of that layer: every layer rule reads it as a context's crate. A context
# depends on the kernel layer by layer (its domain on the kernel's domain, its application on the kernel's domain and
# application, its adapters on all of it); the kernel depends on no context.
# Inside an adapters crate the modules `incoming` and `outgoing` are the two adapter directions; inside
# an application crate `shared` holds the shared output ports and every other top-level module is a use
# case or a feature (each module name renamable, see LAYOUT["configurable"]). A context publishes in two crates, its operations in the api crate and its integration events in
# the events crate, as Java's {context}/api and {context}/events. A building-block role is a trait of dca-building-blocks; a type carries it through
# `impl <Role> for <Type>`, a trait through a supertrait, transitively. The application layer is asynchronous:
# the transaction boundary, the output ports and the input ports return futures; the domain is synchronous.
LAYOUT = {
    "metadata": "[package.metadata.dca] context, layer, shared-kernel",
    "layers": ["domain", "application", "adapters", "api", "events", "infrastructure", "bootstrap"],
    "sharedKernel": "shared-kernel = true with a layer of domain, application, adapters or infrastructure, no context",
    "adapterModules": {"incoming": "incoming", "outgoing": "outgoing"},
    "applicationShared": "shared",
    "publishedLayers": {"api": "api", "events": "events"},
    # Renamed in [workspace.metadata.dca], the default in parentheses (Java's DcaLayout with…Subpackage and
    # with…Suffix); a rule's text names the default.
    "configurable": {
        "modules": {"incoming-module": "incoming", "outgoing-module": "outgoing", "shared-module": "shared",
                    "web-module": "web", "event-consumer-module": "event"},
        "suffixes": {"use-case-suffix": "UseCase", "input-port-suffix": "InputPort",
                     "aggregate-root-suffix": "AggregateRoot", "repository-suffix": "Repository",
                     "store-suffix": "Store", "factory-suffix": "Factory",
                     "specification-suffix": "Specification"},
    },
}

C, LT, RF, NA = "compiler", "lint-test", "reformulated", "n/a"
W, V, T, CL, D, G, S = "workspace", "visibility", "trait-bound", "clippy", "deny", "graph-test", "source-test"

META_SEL = ("Types in a domain crate, the shared kernel's included: structs, enums, their "
            "fields and enum variants, the impl blocks for them - inherent and of traits - and the methods in those blocks.")
META_CHK = ("No derive and no attribute from the configured framework crates (persistence, web, injection) on the "
            "type, its fields, its variants, its impl blocks or their methods. A domain crate that does not depend on such a crate cannot name the derive at all "
            "(DCA-ONI-002); the source test holds the case where the project allows the crate in the domain.")


MAP_FORMAT = ("The declarations: [[package.metadata.dca.upstreams]] with context, via (a list of api and events), translation "
              "(acl or conformist) and status (implemented, the default, or planned); "
              "[[package.metadata.dca.external-upstreams]] with name, interaction (outbound or inbound), translation, "
              "contract-crates (a list of package names) and status; [[package.metadata.dca.partnerships]] with context.")


ALIAS_PORTS = (" A local bound by a straight-line let to a port (let things = unit.things();, let repo = &self.things;) "
               "counts as that port in its block and the blocks inside, until a later let of the same name shadows it; bindings "
               "of if let, while let, match arms, closure parameters and for loops are not followed, and inside their scope they "
               "suspend an alias of the same name.")

UNIT_PORTS = (" A port is a field of self, or what an accessor of the transaction unit hands out: unit.things() inside the "
              "closure passed to in_transaction or in_transaction_closed, the unit trait found among the bounds of the use "
              "case's generics and read through its associated types, generic ones (type Things<'a>) included. The "
              "closure returns its future boxed (async move { .. }.scope_boxed()); a call is seen through .await." + ALIAS_PORTS)


def m(category, enforcement, selects, checks, title=None, status=None, reason=None, maturity=None):
    """A Rust reading. `maturity` overrides Java's for a rule the Rust suite implements later than Java:
    a newly implemented check starts experimental, as every new rule does."""
    return dict(category=category, enforcement=enforcement, selects=selects, checks=checks,
                title=title, status=status, reason=reason, maturity=maturity)


MAPPING = {
    # --- layered -----------------------------------------------------------------------------------------
    "DCA-LAY-001": m(LT, [], "Informational - selects nothing.",
                     "Informational - carries doctrine only. The dependency direction is held by the crate graph and the hexagonal rules."),
    "DCA-LAY-002": m(C, [W, G], "Every crate with layer = domain, the shared kernel's domain crate included.",
                     "Its Cargo dependencies name no crate with layer = infrastructure or bootstrap. The compiler then refuses every use of such a crate; the graph test refuses the Cargo.toml line."),
    "DCA-LAY-003": m(C, [W, G], "Every crate with layer = application.",
                     "Its Cargo dependencies name no crate with layer = infrastructure or bootstrap, its own context's included."),
    "DCA-LAY-004": m(RF, [S], "Paths in every module of every crate with a layer that name the building-block trait TransactionBoundary (wiring) or one of the types [workspace.metadata.dca] transaction-types lists (transaction use), written crate::Type - the crate and the type's name are compared, the modules between are not. The default preset: sqlx::Transaction, diesel::connection::AnsiTransactionManager, sea_orm::DatabaseTransaction, rusqlite::Transaction, tokio_postgres::Transaction, mongodb::ClientSession.",
                     "Transaction use sits in an application crate or in the outgoing module of an adapters crate; wiring sits there or in an infrastructure or bootstrap crate. A domain, api or events crate (the shared kernel's domain crate included), an incoming adapter or another module of an adapters crate that names one is reported, once per path. Rust has no declarative transaction attribute, so there is no annotation selection; a call that runs a transaction without naming the type (a method on a pool) is not seen."),
    "DCA-LAY-005": m(C, [W], "Items in the building blocks' port module.",
                     "A crate cannot add an item to another crate's module, so the building blocks' output-port module holds what the library declares and nothing a project adds. The rule holds by the language."),
    "DCA-LAY-007": m(LT, [S], "In a crate that carries a context but no layer (the two-crate form), every top-level module.",
                     "Informational - lists every top-level module that is none of domain, application, adapters, api, events and infrastructure, and never fails. In the crate-per-layer form every item sits in a layer by its crate and nothing is listed."),
    "DCA-LAY-008": m(LT, [G], "Every application crate and its normal Cargo dependencies.",
                     "None is a crate [workspace.metadata.dca] persistence-crates lists (package names; sqlx, diesel, sea-orm, rusqlite, tokio-postgres and mongodb by default). Without the dependency the compiler refuses every use of the crate."),
    # --- onion -------------------------------------------------------------------------------------------
    "DCA-ONI-001": m(C, [W, G], "Every crate with layer = domain.",
                     "Its Cargo dependencies name no crate with layer = application, its own context's included."),
    "DCA-ONI-002": m(LT, [W, G, D], "Every crate with layer = domain, the shared kernel's domain crate included.",
                     "Its normal dependencies are the building blocks, the domain crates of its own context and of the shared kernel, and the configured allowed crates only (by default none). Dev-dependencies are not counted. cargo-deny can refuse known framework crates workspace-wide; the graph test is the per-crate check."),
    "DCA-ONI-003": m(LT, [W, S], META_SEL, META_CHK, title="Domain models must not carry prohibited framework derives or attributes"),
    "DCA-ONI-004": m(RF, [W, S], "Every module of every domain crate. Exempt from the async part: the traits that carry the DomainGateway or the DomainService role, with their methods, and the impl blocks of types that implement DomainService.",
                     "Outside the exempt items no function is async, returns or takes a type named Future, BoxFuture or ScopedBoxFuture (a parameter's type, impl Future included, or a bound of the function's generics), no field holds one (Pin<Box<dyn Future>>), and no expression awaits or builds an async block; nowhere in the domain crate does a path name a crate [workspace.metadata.dca] async-runtime-crates lists (tokio, async-std, futures and smol by default). This diverges from Java and .NET, whose domain is synchronous throughout: in Rust a domain gateway answers asynchronously, so the gateways and the domain services that await them are async, std's Future and nothing more; aggregates, entities, values, events, failures, factories and specifications stay synchronous. DCA-RS-004 holds the signatures alone."),
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
    "DCA-HEX-007": m(LT, [W, S], "The incoming module of every crate with layer = adapters (one crate per layer), and every module below it.",
                     "No module there names a path into a crate of another context, the configured event-consumer submodule (event-consumer-module) excepted. The crate's own dependencies on another context are DCA-STR-006's graph check."),
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
                     "No such function names, in its signature or body, a type of an application crate whose name ends with Command - the input of a command port - and none calls a method on an input port that takes a command (a trait with the input-port role whose UseCase<..> input or a method parameter is a ...Command type, or a type that implements one): a parameter, a local binding whose type is written out, a field of a parameter's struct, or a local bound to one of these by a straight-line let. The same holds for the free functions of the handler's module it calls, one level deep. Not seen: a helper of another module or a method, a call through more than one helper, and a receiver whose type the suite cannot resolve; a handler registered through a router another crate builds is found when that crate is in the workspace."),
    "DCA-HEX-016": m(RF, [S], "Function bodies in every adapters crate, and in them the enums declared in a domain or application crate: a parameter, a written-out local or a field of self of such a type, and a path to one of its variants.",
                     "Informational - lists every .to_string() on such an enum and every argument formatted with ? ({:?}, {:#?}, {0:?}, {name:?}, captured or named) by format!, write!, writeln!, print!, println!, eprint!, eprintln!, panic! or format_args!, and never fails. A serializer that writes the variant name, and an enum reached through an unannotated local, are not seen."),
    "DCA-HEX-017": m(RF, [T, S], "Types that implement the building-block trait EventListener<E, U> - a listener that reacts inside the publishing transaction, reaching its output ports through the unit U - and not TransactionalEventListener<E>. What reaches an input port: a field whose type, through generic arguments and the bounds of the struct and of its impl blocks, names a trait that carries the input-port role or a type that implements one; in the body of its on method, a parameter, a written-out local, a field of self of such a type, a field of a parameter whose type is a struct of the workspace holding one, and any path that names one.",
                     "No such listener reaches an input port: a listener that drives a use case implements TransactionalEventListener<E>, so it is delivered after the commit, in a transaction of its own. Rust has no listener annotation; the two building-block traits are the declaration. A call through a helper of the listener's own is not followed."),
    "DCA-HEX-018": m(RF, [S], "Handler functions the router presets named in [workspace.metadata.dca] routers register, anywhere in the workspace, read as DCA-HEX-014 reads them, for every verb. Without a preset nothing is selected.",
                     "Each is declared in the incoming module of an adapters crate. Rust handlers are functions, not classes; a method registered as a handler is not seen."),
    "DCA-HEX-019": m(LT, [T, S], "Every impl of a trait that carries the output-port role.",
                     "The implementing type sits in the outgoing module of an adapters crate, or it is the bridge in an infrastructure or bootstrap crate: a type that names an input port - a field whose type, through generic arguments and the bounds of the type and its impl blocks, names a trait that carries the input-port role or a type that implements one, or such a bound itself - and so answers the output port by driving a use case. The orphan rule already keeps the impl out of the domain crate, which cannot see the application's port; a test double in the application crate is reported."),
    "DCA-HEX-020": m(LT, [S], "Functions and methods in the incoming module of every adapters crate, and the router expressions of those crates. Nothing is selected when [workspace.metadata.dca] names neither authorization-extractors (types whose presence in a handler's signature declares a claims-only check, such as a RequireRole<R> extractor) nor authorization-layers (functions whose call in a router's .layer(...) or .route_layer(...) declares it for the routes added before it); both are empty by default.",
                     "Informational - lists every call in a body, as a method or as a path, named has_role, has_any_role, has_authority, has_any_authority, is_in_role or is_user_in_role, and never fails. A function whose signature names a configured extractor, or whose name appears in the receiver of a .layer(...)/.route_layer(...) call that calls a configured layer function within the same crate, is not listed. Not seen: a handler routed behind a layer in a router another crate builds, a role compared by the handler itself (roles.contains(...)), and a helper of the project's own declared outside the incoming module. Rust has no route annotation; the extractor and the layer are its declarative forms."),
    "DCA-HEX-021": m(LT, [S], "Function bodies in the outgoing module of every adapters crate. Nothing is selected when no type implements AggregateRoot.",
                     "No body calls, on a parameter, a local binding whose type is written out or a field of self whose type names a type that implements AggregateRoot - or on a local bound without a written type to Type::function(..) of that type returning Self or the type (after ? or .await, a Result or Option of it), or to one of these by a straight-line let, until a later let shadows the name - a method of that type that takes &mut self - in Rust the receiver says which methods change the aggregate. A method of a building-block trait (clearing the recorded events after publishing) passes. A change written to storage directly, past the aggregate, and a call on an unannotated local initialised otherwise (a method's return value) are not seen."),
    # --- tactical ----------------------------------------------------------------------------------------
    "DCA-TAC-001": m(LT, [S], "Structs and enums in every domain crate, the shared kernel's included, whose name ends with AggregateRoot.",
                     "The type implements AggregateRoot. Rust names rarely carry the suffix; selection by name stays as in Java."),
    "DCA-TAC-002": m(LT, [W, S], "Types that implement AggregateRoot.",
                     "No field type, generic arguments and Box<dyn ...> included, names a trait with the output-port role. The project's own ports live in the application crate, which the domain crate cannot see; the source test holds the building blocks' own port traits."),
    "DCA-TAC-003": m(LT, [S], "Types that implement AggregateRoot.",
                     "No field type, through Vec, Option, Box and other generic arguments, is a type that implements AggregateRoot. References by id pass."),
    "DCA-TAC-004": m(LT, [T, S], "Types that implement Entity, aggregate roots included.",
                     "The trait's associated type Id is bound to the Id marker, so the compiler holds the typed identity. The source test checks that a field of exactly that type exists - the type the impl Entity binds to Id, compared resolved, references stripped; a collection of ids (Vec<PartId>) is no id - so the id is state, not computed. An enum entity holds it in every variant, directly or in the workspace struct the variant wraps. When the binding cannot be read, any field whose type names a type that implements Id counts."),
    "DCA-TAC-005": m(LT, [V, S], "Types that implement Entity but not AggregateRoot.",
                     "The type has no pub field and no pub associated function that returns Self; pub(crate) and private ones pass. Nor can it be built wherever it is visible without a function of its own: no impl of Default, From<..> or TryFrom<..> (a trait of the workspace with that name excepted), no #[derive(Default)], and a pub enum has no open variant - a unit variant and one with named fields are constructors wherever the enum is visible, a tuple variant is one unless every field it wraps is a workspace struct whose fields are all private (a state only its own module builds), so a state machine of sealed states passes. With private fields no code outside the aggregate's module can build one by struct literal - the compiler holds that part. Java checks who calls the constructor; Rust checks the visibility of every way in, so a pub(crate) way used outside the aggregate passes."),
    "DCA-TAC-006": m(LT, [S], "Types that implement Entity, aggregate roots included, with their methods. A type's methods are, as Java reads a class's: its inherent methods and the methods of its impls of workspace traits, a workspace trait's default methods the impl does not write out included; impls of std, core, external and building-block traits are not read.",
                     "No pub method named set_<name> takes &mut self and exactly one further parameter and returns ()."),
    "DCA-TAC-007": m(LT, [S], "Types that implement Entity but not AggregateRoot.",
                     "No field type, through generic arguments, is a type that implements AggregateRoot."),
    "DCA-TAC-008": m(LT, [S], "Types that implement Value. Names resolve as the compiler resolves them: a use inside a function body counts for the module only where the module neither declares nor imports the name, and a glob over an enum (use Seal::*;) brings in its variants only.",
                     "No field type, through generic arguments, is a type that implements Entity or AggregateRoot."),
    "DCA-TAC-009": m(NA, [], "", "", reason="Rust has no inheritance; a struct cannot be extended, so a value object is final by construction."),
    "DCA-TAC-010": m(RF, [S], "Types that implement Value.",
                     "No method takes &mut self, no field is pub, and no field type is an interior-mutability type (Cell, RefCell, Mutex, RwLock, atomics). Rust has no final field; immutability is the absence of a mutating API.",
                     title="Value Objects must not mutate their state"),
    "DCA-TAC-011": m(LT, [S], "Types that implement Value.",
                     "No method is named set_<name>, whatever its receiver."),
    "DCA-TAC-012": m(C, [T], "Types that implement Value.",
                     "The building-block trait Value has PartialEq as supertrait, so the compiler refuses a value object without equality. Whether the equality compares every attribute is not decidable; a derive is the expected form. Immutability is DCA-TAC-010's."),
    "DCA-TAC-013": m(LT, [S], "Traits in every application crate whose name ends with Repository.",
                     "The trait carries the repository role."),
    "DCA-TAC-014": m(LT, [S], "Traits that carry the repository role, the building block excluded.",
                     "Each sits in an application crate, the shared kernel's application crate included."),
    "DCA-TAC-015": m(LT, [S], "Types that implement a trait with the repository role.",
                     "Each sits in the outgoing module of an adapters crate."),
    "DCA-TAC-016": m(LT, [T, S], "Traits with the repository role (through Repository<T>) whose name ends with Repository.",
                     "The bound Repository<T: AggregateRoot> makes a repository for anything but a root unkompilable. The source test checks the name: the trait is named after the type it binds plus Repository."),
    "DCA-TAC-017": m(LT, [V, S], "Traits with the repository role.",
                     "No method of the trait returns a type, through generic arguments, that implements Entity but not AggregateRoot. A pub(crate) entity cannot be named by the application crate at all."),
    "DCA-TAC-018": m(LT, [S], "Traits outside adapters and infrastructure crates whose name ends with Store.",
                     "The trait carries the store role and not the repository role."),
    "DCA-TAC-019": m(LT, [S], "Traits that carry the store role, the building block excluded.",
                     "Each sits in an application crate, the shared kernel's application crate included."),
    "DCA-TAC-020": m(LT, [S], "Types that implement a trait with the store role.",
                     "Each sits in the outgoing module of an adapters crate."),
    "DCA-TAC-021": m(LT, [S], "Traits that carry the store role.",
                     "No method declared on the trait itself is named save, delete or delete_by_id."),
    "DCA-TAC-023": m(RF, [V, S], "Types that implement AggregateRoot.",
                     "No field is pub, and no unnamed way builds it: no impl of Default, From<..> or TryFrom<..> (a trait of the workspace with that name excepted), no #[derive(Default)], and a pub enum has no open variant - a unit variant and one with named fields are constructors wherever the enum is visible, a tuple variant is one unless every field it wraps is a workspace struct whose fields are all private (a state only its own module builds), so a state machine of sealed states passes. Rust has no constructor; the struct literal is the constructor, and private fields keep it inside the aggregate's module. The root's associated functions (create, place, ...) are its named factories and check invariants.",
                     title="Aggregate Roots must not expose a public struct literal"),
    "DCA-TAC-024": m(RF, [S], "Types that implement Value and their pub associated functions returning Self.",
                     "No function named new takes the same parameter types, in the same order, as another such function. Types are compared resolved, references and lifetimes stripped: &'a str and &str, String and std::string::String are the same; a type of the workspace compares by the type a path resolves to. Two named origins (from_input, from_stored) pass."),
    "DCA-TAC-025": m(LT, [S], "Structs, enums and traits in every domain crate, the shared kernel's included: the fields of structs and enum variants, the signatures of inherent methods - private ones included - and of the methods a trait declares; an impl of a trait is read where the trait is declared. Not selected: a single-field newtype that implements Value or Id, failures, and trait methods of a building block.",
                     "No field, parameter or return type, through generic arguments, is String, &str, a primitive, a time type (std::time::SystemTime, chrono, time) or a bare Uuid. A private field of type bool - a single yes/no fact - and a bool return - a predicate - pass; a bool parameter and a pub bool struct field, flag arguments, do not. A bool parameter of the factory that rebuilds a stored value - an associated function without a receiver that returns Self or the type, named as [workspace.metadata.dca] reconstituting-factories lists, reconstitute and restore by default, case ignored - passes; its other parameters are checked. A returned impl Future is read by its Output. A domain enum with exactly two fieldless variants that name one fact and its negation is a boolean in disguise and is reported where it is declared: the pairs Yes/No, True/False, On/Off, Present/Absent, Enabled/Disabled, and any X with NotX, NoX or NonX, and UnX where X is adjective-like - it ends in -ed, -able, -ible, -ive, -ent, -ant, -al or -ful (Locked/Unlocked) - (case and underscores ignored). Java takes UnX for any X; Rust reads UnX before a verb as its reverse action, so Lock/Unlock, Load/Unload, Follow/Unfollow and Do/Undo pass. Two variants with names of their own (Open/Closed) pass."),
    "DCA-TAC-026": m(LT, [T, S], "Types that implement both Owned and AggregateRoot, each with the repository traits that bind it through Repository<T>. The owner id type is the type the impl Owned block binds to OwnerId (the trait requires it, and requires AggregateRoot as supertrait); the aggregate's id is the type its impl Entity binds to Id. An owned aggregate without a repository trait is not selected.",
                     "At least one method the repository trait declares itself takes a parameter whose type (through references) is the aggregate's id type and a further parameter whose type is the owner id type - either position, any name - and returns a type that names the aggregate, through Result, Option or another wrapper. A method that answers only yes or no does not count; the inherited find_by_id takes the id alone. An owner id type the suite cannot resolve (a type outside the workspace, a type alias) is reported. Whether a use case calls the scoped lookup is not checked; a lookup declared only in a generic intermediate trait is not seen. An empty selection passes."),
    "DCA-TAC-027": m(LT, [S], "Types that implement AggregateRoot, Entity, Value or Factory, and specifications (types that implement Specification or whose name ends with Specification); a type that itself implements DomainGateway is not selected. Read: their fields, through generic arguments and the type's bounds, the return types of their methods, and the paths their method bodies use as values - expression paths, called functions, struct literals - a name the body binds (a parameter, a let, a closure or match binding) shadowing an item of the same name.",
                     "A type that names a trait carrying the DomainGateway role, or a type that implements one, appears in none of these places: a field; the return type of a method; a function or an associated function the body calls whose declared return type names a gateway - a registry or locator - an associated function of a trait included, called through the trait's path, <T as Trait>::f or a generic parameter bounded by the trait; a field the body reads from another value (ctx.rates, self.context.rates) whose type is known - a parameter or a local with its type written out, or a field of self - when that value's struct holds a gateway in the field; a static or a const the body reads whose type names a gateway; an implementation the body constructs (a struct literal, a unit or tuple struct, an associated function of the implementation). A gateway taken as a method parameter and the calls made on it pass. A domain service and the application may use the gateway freely. The model is synchronous (DCA-ONI-004): it can call only a gateway whose methods are synchronous - a hasher, a converter; an asynchronous gateway is awaited by a domain service, which hands the model the answer. Not seen: a method called on a value that returns a gateway, a field read from a value whose type is not written out (an unannotated local, a return value, a closure parameter) or from a generic parameter, a trait function whose return type names the gateway only through an associated type, and a gateway hidden behind a type alias."),
    # --- strategic ---------------------------------------------------------------------------------------
    "DCA-STR-001": m(LT, [G], "Every crate whose [package.metadata.dca] carries a context or shared-kernel = true.",
                     "Informational - lists each context with its crates and their layers, and the shared kernel's crates with their layers, and never fails."),
    "DCA-STR-002": m(C, [W, G], "Every crate with shared-kernel = true, whatever its layer.",
                     "Its Cargo dependencies name no crate that carries a context."),
    "DCA-STR-003": m(C, [W, G], "Every crate with layer = application.",
                     "Its Cargo dependencies name no crate of another context, that context's api and events crates included. Another context is a crate whose context is set and differs: a crate with a layer and no context (the shared kernel, bootstrap) is never another context, so every context may depend on the shared kernel; a shared-kernel crate is not selected - its dependency on a context is DCA-STR-002's."),
    "DCA-STR-004": m(C, [W, G], "Every crate with layer = domain.",
                     "Its Cargo dependencies name no crate of another context, that context's api and events crates included. Another context is a crate whose context is set and differs: a crate with a layer and no context (the shared kernel, bootstrap) is never another context, so every context may depend on the shared kernel; a shared-kernel crate is not selected - its dependency on a context is DCA-STR-002's."),
    "DCA-STR-005": m(LT, [S], "Types that implement the OpenHostService marker and traits that carry it.",
                     "Each sits in an api crate or in the incoming module of an adapters crate."),
    "DCA-STR-006": m(C, [W, G], "Every crate with layer = adapters.",
                     "Its Cargo dependencies on another context name that context's api and events crates only. A foreign domain, application or adapters crate is refused by the graph test; anything those crates do not export is refused by the compiler. Another context is a crate whose context is set and differs: a crate with a layer and no context (the shared kernel, bootstrap) is never another context, so every context may depend on the shared kernel; a shared-kernel crate is not selected - its dependency on a context is DCA-STR-002's."),
    "DCA-STR-007": m(LT, [S], "Types that implement IntegrationEvent.",
                     "Each sits in a context's events crate (layer = events) - its published contract, a crate beside its api crate as Java's {context}/events beside {context}/api; a module named events inside the api crate is not the place. Never in the shared kernel, which declares no events layer: an integration event is one context's contract, not shared domain; translator and publisher sit in the adapters. Java's ..events.. of a module root also matches the shared kernel; the Rust reading follows the decision that the kernel holds none."),
    "DCA-STR-008": m(RF, [S], "Types that implement IntegrationEvent.",
                     "No method takes &mut self and no field type is an interior-mutability type. Its fields may be pub - a deliberate Rust reading: an integration event is the published record another context reads and builds, its fields are its contract, and a value held by another crate cannot be changed through a shared reference."),
    "DCA-STR-010": m(LT, [], "Informational - selects nothing.",
                     "Nothing is asserted. Whether a consumed integration event is translated before it reaches the domain is a review check."),
    "DCA-STR-011": m(LT, [G], "The workspace members.", "At least one crate carries a context in [package.metadata.dca]. A workspace that holds no production code yet passes: no crate has an item outside #[cfg(test)] in the module tree of its library or binary target, a program entry point aside - test-only crates, a library root that holds nothing but comments and test modules, and a binary crate (no library target) whose root declares no module and whose only item is fn main, are no production code. Once one such item exists, the rule fails until a context is declared, so a workspace whose crates miss the declaration still fails."),
    "DCA-STR-012": m(LT, [G], "The workspace members.", "At least one crate carries one of the layers in [package.metadata.dca]. A workspace that holds no production code yet passes: no crate has an item outside #[cfg(test)] in the module tree of its library or binary target, a program entry point aside - test-only crates, a library root that holds nothing but comments and test modules, and a binary crate (no library target) whose root declares no module and whose only item is fn main, are no production code. Once one such item exists, the rule fails until a layer is declared, so a workspace whose crates miss the declaration still fails."),
    "DCA-STR-013": m(LT, [S], "Traits in every events crate.",
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
    "DCA-MAP-006": m(RF, [G], "Every declaring crate with its upstreams entries towards existing other contexts, planned ones included, and every normal Cargo dependency of the context's crates on another context's api or events crate.",
                     "The declared upstream contexts equal the contexts whose api or events crate a crate of the context depends on: a declared upstream no crate depends on is reported on the declaring crate, a dependency on an undeclared one on the depending crate. Cargo.toml is the module declaration in Rust; there is no second one to compare, and a dependency names a crate, not a channel - the channels are DCA-MAP-007's and DCA-MAP-011's."),
    "DCA-MAP-007": m(RF, [G, S], "Every upstreams entry with status implemented towards an existing other context, reading via. Planned entries and entries towards an unknown context are skipped.",
                     "For each channel in via, some module of some crate of the declaring context names a path into that channel of the target - its events crate for events, its api crate for api. Which crate holds the use is not checked here."),
    "DCA-MAP-008": m(RF, [W, G, S], "Every upstreams entry with translation = acl towards an existing other context, reading via and, for the translation site, the status.",
                     "No crate of the declaring context other than its adapters crate has a normal dependency on the target's api or events crate; within the adapters crate, a path into the api channel sits only in the outgoing module and a path into the events channel only in the incoming module, for the channels the entry names. For an implemented entry, each channel the target publishes as a crate also has its translation site: some module on the matching side (outgoing for api, incoming for events) of an adapters crate of the context names both a path into that channel and a path into a crate of its own context with layer domain or application - the module stands for Java's class -, or two modules of that side share it: one names the channel, the other its own domain or application, and one of the two names the other (a listener and its translator). Whether the adapter translates beyond naming both is not established."),
    "DCA-MAP-009": m(C, [W, G], "Every upstreams entry with translation = conformist towards an existing other context; the status is not consulted.",
                     "The declaring context's domain crate has no normal dependency on the target's api or events crate; the graph test refuses the Cargo.toml line, and without it the compiler refuses every path into the crate. DCA-STR-004 refuses the same dependency for every context, declared or not."),
    "DCA-MAP-010": m(RF, [W, G, S], "Every external-upstreams entry that names contract-crates (package names; a crate outside the workspace counts by its name), reading translation and interaction; the status is not consulted. An entry without contract crates only documents the relationship.",
                     "With acl, no crate of the context but the adapters crate has a normal dependency on a contract crate, and within it only the matching module - outgoing for outbound, incoming for inbound - names a path into one, by its package name or by the name a renamed dependency gives it (feed = { package = \"vendor-feed\" }). With conformist, the domain crate has no normal dependency on one. That the adapter translates is not established."),
    "DCA-MAP-011": m(RF, [G, S], "Every normal Cargo dependency of a context's crate on another context's api or events crate, and every path its modules name into such a crate, with the channel the path lies in (the events crate: events; the api crate: api).",
                     "The declaring crate of the source context carries an upstreams entry for the target context - for a path, one whose via names that channel. Planned entries count as declared. Dependencies on a foreign context's other crates are not reported by this rule."),
    "DCA-MAP-012": m(RF, [G], "Every partnerships entry of every declaring crate, reading context.",
                     "It names a context some crate declares, not the declaring one, and the target's declaring crate carries a partnerships entry naming the declaring context in turn. A partnership grants no dependency."),
    "DCA-MAP-013": m(LT, [G], "Every upstreams, external-upstreams and partnerships entry of every declaring crate.",
                     "Informational - lists every declared edge with its channels or interaction and its translation, and the protocol, exchanges and rationale where declared, and never fails. The status is not printed."),
    "DCA-MAP-014": m(LT, [CL, G, S], "Every normal Cargo dependency of a context's crate on a crate of another context. A dependency on the api or events crate of a context the declaring crate names in a planned upstreams entry is skipped.",
                     "Some module of the crate names a path into the dependency. The rustc lint unused_crate_dependencies holds the same per crate for every dependency."),
    # --- advanced ----------------------------------------------------------------------------------------
    "DCA-ADV-001": m(RF, [T, S], "Types that implement DomainEvent.",
                     "No method takes &mut self and no field type is an interior-mutability type. Implementing the trait is the selection itself."),
    "DCA-ADV-002": m(LT, [S], "Types that implement DomainEvent.", "Each sits in a domain crate, the shared kernel's included."),
    "DCA-ADV-004": m(LT, [W, S], "Types that implement DomainEvent in a domain crate, the shared kernel's included.", META_CHK),
    "DCA-ADV-005": m(C, [T], "Types that implement IntegrationEvent.",
                     "The trait requires the associated constants TYPE and VERSION, so the compiler refuses an integration event without its type metadata. Rust has no annotations; the constants are the metadata."),
    "DCA-ADV-006": m(LT, [S], "Types that implement IntegrationEvent.", "No field is named schema_version, event_version or contract_version."),
    "DCA-ADV-007": m(LT, [S], "Types that implement DomainEvent but not IntegrationEvent.", "No field is named schema_version, event_version or contract_version."),
    "DCA-ADV-008": m(LT, [T, S], "Types that implement DomainEvent.",
                     "The trait requires occurred_on(); the source test checks that a field whose type names one of the configured timestamp types ([workspace.metadata.dca] timestamp-types; Timestamp, SystemTime, DateTime and OffsetDateTime by default), generic arguments included (Option<Timestamp>), backs it - on an enum, in every variant or through a variant that carries an event type of its own."),
    "DCA-ADV-010": m(LT, [S], "Types that implement DomainService.", "Each sits in a domain crate, the shared kernel's included."),
    "DCA-ADV-011": m(LT, [W, S], "Types that implement DomainService in a domain crate, the shared kernel's included.", META_CHK),
    "DCA-ADV-012": m(RF, [S], "Types that implement DomainService.",
                     "No method takes &mut self and no field type is an interior-mutability type. Rust has no final field; statelessness is the absence of mutation."),
    "DCA-ADV-013": m(LT, [S], "Types that implement Factory.", "The name ends with Factory."),
    "DCA-ADV-014": m(LT, [S], "Types that implement Factory.", "Each sits in a domain crate, the shared kernel's included."),
    "DCA-ADV-015": m(LT, [W, S], "Types that implement Factory in a domain crate, the shared kernel's included.", META_CHK),
    "DCA-ADV-016": m(RF, [S], "Types that implement Factory.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-ADV-017": m(LT, [S], "Types that implement Specification or whose name ends with Specification.", "Each sits in a domain crate, the shared kernel's included."),
    "DCA-ADV-018": m(LT, [W, S], "Specifications in a domain crate, the shared kernel's included.", META_CHK),
    "DCA-ADV-019": m(LT, [S], "Methods of types that implement AggregateRoot, Entity or Value.",
                     "No method body calls a function of, or constructs, a type that implements DomainService, nor calls a method on one held in a parameter (through its generic bounds), a field of self, or a local binding whose type is written out or that is initialised with one. A parameter of that type alone is not reported."),
    "DCA-ADV-020": m(LT, [S], "pub methods with a receiver of types that implement DomainService. A type's methods are, as Java reads a class's: its inherent methods and the methods of its impls of workspace traits; impls of std, core, external and building-block traits are not read.",
                     "At least one parameter is, by value or by reference, a type that implements AggregateRoot or Entity."),
    "DCA-ADV-021": m(LT, [S], "Every function body - free functions and the methods of impl blocks - in every crate except the application crates, the impls of a type that implements DomainService excluded. A domain service is a type that implements DomainService, as DCA-ADV-010 selects it: Rust marks no module, so a module of free functions is no domain service by this rule.",
                     "No body names a type that implements DomainService as an expression (a struct literal, a unit value, an associated function), nor calls a method on a parameter, a local binding or a field of self whose type implements it, nor on one built in place. In an infrastructure or bootstrap crate - the composition root - a construction (a struct literal, a unit value, an associated function without a receiver that returns Self or the type by its name) passes; a call there is reported. A parameter or a field of that type alone is not reported; a call through a generic bound or a trait object is not seen."),
    # --- use case ----------------------------------------------------------------------------------------
    "DCA-USE-001": m(LT, [S], "Traits named InputPort anywhere in the workspace.", "None outside the building blocks."),
    "DCA-USE-002": m(LT, [S], "Structs and enums whose name ends with Command.", "Each sits in an application crate."),
    "DCA-USE-003": m(LT, [S], "Structs and enums whose name ends with Query.", "Each sits in an application crate."),
    "DCA-USE-004": m(RF, [S], "Structs in application crates whose name ends with Command.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-005": m(RF, [S], "Structs in application crates whose name ends with Query.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-006": m(LT, [S], "Structs and enums whose name ends with Result and that do not implement Value. std's Result is not a declaration and is never selected.",
                     "Each sits in an application crate. Every enum in a domain crate is exempt, as in Java: an enum there is a domain outcome."),
    "DCA-USE-007": m(RF, [S], "Structs in application crates whose name ends with Result.", "No method takes &mut self and no field type is an interior-mutability type."),
    "DCA-USE-008": m(LT, [S], "Structs and enums whose name ends with Response.", "Each sits in an adapters crate."),
    "DCA-USE-009": m(RF, [S], "Functions of types that implement an input port." + UNIT_PORTS,
                     "A function that calls save on a repository also hands the aggregate's events on - publish_and_clear_events on a DomainEventPublisher - directly or through a function of the same type. The outbox is the publisher's adapter's: a use case never registers an entry itself."),
    "DCA-USE-010": m(LT, [W, S], "Items in every domain crate.", "No item declares or names a type whose name ends with Dto."),
    "DCA-USE-011": m(LT, [W, S], "Items in every application crate.", "No item declares or names a type whose name ends with Dto."),
    "DCA-USE-012": m(RF, [S], "Functions of types that implement an input port." + UNIT_PORTS,
                     "A function that calls save or delete on a repository, or publish on a DomainEventPublisher, does so inside the closure it passes to in_transaction or in_transaction_closed of a TransactionBoundary (or a function of the same type that does). A port the transaction unit hands out is inside by construction. Rust has no transactional attribute; the explicit boundary is the only form."),
    "DCA-USE-013": m(NA, [], "", "", reason="Rust has no declarative transaction attribute; the boundary is the explicit TransactionBoundary port, which DCA-USE-012 reads, and the rule selects only declaratively transactional use cases."),
    "DCA-USE-014": m(LT, [S], "Types that implement an input port in every application crate, the shared module excluded.",
                     "All sit at one module depth: application::<use_case> or application::<feature>::<use_case>."),
    "DCA-USE-015": m(LT, [V, S], "Structs in application crates whose name ends with Result.",
                     "No field type, through generic arguments and into other application structs, implements AggregateRoot or Entity."),
    "DCA-USE-016": m(LT, [S], "Types that implement an input port.",
                     "No field and no parameter of its methods names another input-port trait or a type that implements one, read with the generics of the type, of its impl blocks and of the method (a generic field or parameter stands for its bounds); the building blocks' role traits are not counted."),
    "DCA-USE-017": m(RF, [S], "Types that implement an input port.",
                     "No pub method other than the methods of the input-port traits it implements and an inherent associated function that returns Self (the constructor bootstrap calls). A method of another workspace trait - written in the impl or inherited as the trait's default - is reported, as Java reports an unrelated interface's method; impls of std, core, external and building-block traits are not read. The operation lives in the input-port impl."),
    "DCA-USE-018": m(LT, [S], "Items in the incoming module of every adapters crate, and the input-port traits they name that take a command (the input of a UseCase<..> bound or a method parameter is a type whose name ends with Command) and are declared in a feature: two module levels deep in an application crate, application::<feature>::<use_case>, the shared module excluded.",
                     "The ports one item names belong to at most one feature. An application crate without feature modules selects nothing."),
    "DCA-USE-019": m(LT, [S], "Types that implement an input port whose input is a query." + UNIT_PORTS,
                     "No function calls a write (save, delete, remove, add, insert, update, ...) on a repository or store, nor an event publisher."),
    "DCA-USE-020": m(LT, [CL], "Every application crate.",
                     "clippy.toml lists uuid::Uuid::parse_str and uuid::Uuid::from_str under disallowed-methods. Parsing happens in the adapter or in the id's own constructor."),
    "DCA-USE-021": m(LT, [S], "Types that implement an input port (a trait extending InputPort or UseCase).",
                     "No method of their impls takes &mut self, and no field holds interior mutability (Cell, RefCell, Mutex, RwLock, an atomic)."),
    "DCA-USE-022": m(LT, [S], "impl UseCase<I> blocks, read for their Output type, and the traits that extend UseCase<I, Output = O>.",
                     "O, or the success type of a Result it names, is none of String, &str, bool, char or a numeric primitive. () passes - a command with nothing to answer -, and so does a container such as Vec<String>."),
    "DCA-USE-023": m(LT, [S], "Traits with the OutputPort role declared in a use case module: the module of a struct or enum of an application crate with the input-port role or a name ending in UseCase, and every module below it - a module belongs to the deepest one. Traits in the shared module, in a feature module or at the crate root are not selected.",
                     "No module of another use case module, in any crate, names the trait - a use, a path in a signature, a bound or a body. Adapters, the shared module and modules outside a use case module are not counted. An empty selection passes."),
    # --- naming ------------------------------------------------------------------------------------------
    "DCA-NAM-001": m(LT, [S], "Types in application crates that implement an input port.", "The name ends with UseCase."),
    "DCA-NAM-002": m(NA, [], "", "", reason="Rust has no injectable stereotype attribute; bootstrap constructs use cases in code, so there is nothing to list."),
    "DCA-NAM-003": m(LT, [S], "Traits in application crates that carry the input-port role, InputPort itself excluded.", "The name ends with InputPort."),
    "DCA-NAM-004": m(LT, [S], "Traits in application crates whose name contains Repository.", "The name ends with Repository."),
    "DCA-NAM-005": m(NA, [], "", "", reason="Rust web handlers are free functions, not controller classes; the module that holds them carries the name, and DCA-HEX-018 places it."),
    "DCA-NAM-006": m(NA, [], "", "", reason="Rust web handlers are free functions, not resource classes; the module that holds them carries the name, and DCA-HEX-018 places it."),
    "DCA-NAM-007": m(LT, [S], "Structs, enums and traits whose name ends with Dto.", "Each sits in an adapters crate."),
    "DCA-NAM-008": m(LT, [S], "Structs, enums and traits whose name ends with Converter.", "Each sits in an adapters crate."),
    "DCA-NAM-009": m(LT, [S], "Every module of every crate that carries a context or a layer.",
                     "No module is named entities, value_objects, valueobjects, helpers, util or utils. Module names only; type names are DCA-NAM-010's."),
    "DCA-NAM-010": m(LT, [S], "Structs, enums and traits in every domain crate, the shared kernel's included.", "No name ends with Helper, Util, Utils, Impl or Implementation."),
    "DCA-NAM-011": m(LT, [S], "Structs and enums whose name ends with ViewModel.", "Each sits in the incoming module of an adapters crate, in its submodule web (web-module)."),
    "DCA-NAM-012": m(LT, [S], "Input-port traits and use-case types in application crates, and structs and enums in application crates whose name ends with UseCase, with or without the input-port role.",
                     "Informational - lists names that contain a trigger word (On, After, When, Upon, Before followed by a capital) and never fails."),
    "DCA-NAM-013": m(LT, [S], "Input-port traits and use-case types in application crates, and structs and enums there whose name ends with UseCase, with or without the input-port role; in api crates (the events crate publishes records, not operations, and is not read), the pub free functions, the methods of traits and the pub methods of types (inherent, and of impls of workspace traits declared outside the api crate).",
                     "No type name starts with Set or Update followed by a capital, and no function or method name starts with set_ or update_."),
    # --- cycles ------------------------------------------------------------------------------------------
    "DCA-CYC-001": m(RF, [W, S], "One slice per top-level module of every domain crate; the items of the crate root belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Between the domain crates of two contexts Cargo refuses a cycle and DCA-STR-004 refuses any edge; inside one crate modules may name each other both ways, so the crate is sliced. Submodules of one top-level module form one slice."),
    "DCA-CYC-002": m(RF, [W, S], "One slice per top-level module of every application crate, the shared module included; the items of the crate root belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it. A cycle between use cases or features alone is also DCA-CYC-005's; one through the shared module is this rule's only."),
    "DCA-CYC-003": m(RF, [W, S], "One slice per module directly below the outgoing module of every adapters crate; items of the outgoing module itself belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it."),
    "DCA-CYC-004": m(RF, [W, S], "One slice per module directly below the incoming module of every adapters crate; items of the incoming module itself belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name. Cargo refuses a cycle between crates; inside one crate the compiler allows it. A path from the incoming into the outgoing module is DCA-HEX-006's."),
    "DCA-CYC-005": m(LT, [S], "One slice per operation root of every application crate: the deepest module holding an operation - a struct or enum of the crate with the input-port role or a name ending in UseCase; a module below it joins it. A module that leads to operations without holding one is the feature slice of the first module below the containers. Modules [workspace.metadata.dca] operation-containers lists (a list, empty by default; Java's withOperationContainers) only group operations: they are skipped at the front and left out of the slice's name. The shared module, the crate root and modules outside every operation root belong to no slice.",
                     "The slices form no dependency cycle through the paths their modules name: two features, or two use cases of one feature, that name each other, directly or through further slices, are reported. Modules within one crate may depend on each other in both directions; the compiler allows it. An edge between a feature slice and an operation slice nested in it does not count: a Rust module re-exports what its submodules declare (pub use place::PlaceUseCase;) and they use what it declares (use super::Shared;) - Java's feature package holds no such re-export."),
    # --- errors ------------------------------------------------------------------------------------------
    "DCA-ERR-001": m(RF, [S], "Types in every domain crate that implement std::error::Error or carry a derive [workspace.metadata.dca] error-derives lists (written crate::Derive, the crate and the derive's name compared; thiserror::Error by default).",
                     "Each implements DomainFailure. Rust has no exceptions; a failure is a type returned in Err.",
                     title="Failures declared in the domain layer must implement DomainFailure"),
    "DCA-ERR-002": m(LT, [S], "Types that implement DomainFailure or a port failure (UseCaseFailure, OutputPortFailure, or the classification traits PortFailure and InputPortFailure).",
                     "A DomainFailure sits in a domain crate, a port failure in an application crate - an output port's failure beside the port, although the adapter that implements the port returns it."),
    "DCA-ERR-003": m(RF, [S], "Types in every application crate that implement std::error::Error or carry a derive [workspace.metadata.dca] error-derives lists (thiserror::Error by default), and not DomainFailure.",
                     "Each implements UseCaseFailure or OutputPortFailure. Implementing only the classification traits PortFailure or InputPortFailure does not count: a type implements the leaf it is.",
                     title="Failures declared in the application layer must implement UseCaseFailure or OutputPortFailure"),
    "DCA-ERR-004": m(LT, [W, S], "Types that implement DomainFailure, UseCaseFailure or OutputPortFailure, and error types (std::error::Error or a configured error derive) in domain and application crates; the building blocks excluded.",
                     "None carries a derive or an attribute of a crate [workspace.metadata.dca] framework-crates lists, and none implements a trait of one (for example axum's IntoResponse). The orphan rule refuses such an impl in the adapters crate, where both trait and type are foreign, so the source test holds the domain and application crates that depend on the framework. The adapter translates through its own type.", maturity="experimental"),
    "DCA-ERR-005": m(RF, [S], "Types that implement DomainFailure or a port failure (UseCaseFailure, OutputPortFailure).",
                     "The name contains none of Http, StatusCode, ResponseStatus, Response. The suffix part of the Java rule does not apply: Rust names a failure ...Error by convention."),
    "DCA-ERR-006": m(LT, [S], "Modules in the incoming part of every adapters crate that name an input port.",
                     "Informational - lists a module when no item in it names a DomainFailure or UseCaseFailure type nor a variant of a closed result, and never fails. A closed result is an enum an input-port trait answers with - named in an associated-type binding of its bounds (Output = ...) or in a method's return type, through any wrapper such as Result. A variant counts when a path names it through its enum or through a use of it, so a match over the variants is seen; whether the match is exhaustive is left to the compiler. When the workspace has neither a failure type nor a closed result, nothing is listed. It also lists a module that names one of the classification traits PortFailure, InputPortFailure or OutputPortFailure itself - a bound or a trait object over the base is the generic handler that answers every outcome the same way."),
    "DCA-ERR-007": m(LT, [S], "Types that implement DomainFailure or a port failure, their fields and variant fields.",
                     "Informational - lists every field that involves - through its own type or a generic argument, references, slices, arrays, tuples and std's containers and smart pointers (Option, Vec, VecDeque, LinkedList, HashMap, BTreeMap, HashSet, BTreeSet, Box, Rc, Arc, Cow) looked through - a plain string (String, str), a type that implements AggregateRoot or Entity, or any other type that is neither an Id or Value, a number, bool or char, an enum, a time type [workspace.metadata.dca] timestamp-types lists, nor the cause it wraps (a type with a failure role, an error type, a type named Error such as std::io::Error, a trait object or impl bounded by Error such as Box<dyn std::error::Error + Send + Sync>, the building blocks' PortError), nor a Duration - and never fails. A Vec<u8> passes as Java's list of numbers does; a bare Uuid and any other trait object are listed."),
    "DCA-ERR-008": m(LT, [S], "Closed results in every application crate: an enum an input-port trait answers with - named in an associated-type binding of its bounds (Output = ...) or in a method's return type, through any wrapper such as Result - as DCA-ERR-006 finds them; an enum that implements std::error::Error, DomainFailure or UseCaseFailure is the failure side and not selected.",
                     "Informational - lists every closed result none of whose variants carries a field whose type implements the building-block trait Refusal, and never fails. A unit variant carries nothing and cannot be the refusing one: a refusing variant wraps a value of a type that implements Refusal, and the result enum implements ClosedResult, whose refusal() match the compiler checks for coverage and for the Refusal bound - that is what TransactionBoundary::in_transaction_closed rolls back on. A listed result is not a finding by itself: one whose outcomes are all successes - found or nothing found - is correct as it is. A struct is no closed result."),
    "DCA-ERR-009": m(LT, [S], "Every expression in a function body of a module outside the domain crates - application, adapters, api, infrastructure, bootstrap and unplaced crates; #[cfg(test)] items and test targets are not read: a struct literal, a call of a tuple struct or an enum variant, a unit struct or unit variant used as a value, a variant or tuple struct passed as a function, and a call of an associated function without a receiver that returns Self or the type itself, whose type implements DomainFailure.",
                     "None of them names a domain failure - only a domain crate constructs one. Self is not followed: a domain failure declared outside the domain is DCA-ERR-002's finding. A failure the application only passes on (a variant wrapping it, ? or map_err with a conversion) is not a construction. Constructions inside macro invocations, through From/Into conversions or a generic constructor are not seen. An empty selection passes."),
    "DCA-ERR-010": m(RF, [S], "Every function body - free functions, methods of impl blocks, default methods of traits, closures included - in every application crate and in the incoming module of every adapters crate, and in it every call of a method on a port: a parameter, a local binding whose type is written out, or a field of self, whose type names a trait that carries the input-port or output-port role or a type that implements one, and a field of a parameter whose type is a struct of the workspace holding such a field (ports.add, State(ports): State<Ports> included - a one-field tuple-struct pattern binds its parameter), and what an accessor of a transaction unit hands out inside the closure passed to in_transaction (unit.things()). A field's type is read with the bounds of its struct and of the impl block. A call whose method the receiver's type declares in the workspace - its trait, the traits that trait extends, its own impls - only with an answer other than a Result (a type named ...Result, looked through impl Future, Pin<Box<dyn Future>> and BoxFuture) is no failing call and is not selected (an Option a port answers); a method the workspace does not declare, a building block's included, is selected." + ALIAS_PORTS + " Rust has no try block; a Result is answered where it is consumed.",
                     "No port call has its failure answered without naming it: not an Err(_) arm, nor a _ or a bare lower-case binding arm the failure reaches (an upper-case identifier is a unit variant or a constant: None), of a match over the call; not if let Ok(..) (which drops the failure) or if let Err(_) over it, nor let Ok(..) = .. else; not let _ = on it; not .ok(), .unwrap_or(..), .unwrap_or_default(), .or(..), .map_or(..), .is_ok() or .is_err() on it, nor .unwrap_or_else, .map_err, .or_else or .map_or_else with a closure that discards its (first) argument (|_| ..); and no port call inside std::panic::catch_unwind, the catch of every panic. The call is seen through parentheses and .await. Propagation with ?, an arm that matches a named failure or takes it in hand (Err(e)), and a map_err whose closure uses the failure pass. A bare binding arm that only cleans up and passes the caught failure on unchanged does not answer it and passes: every exit of its body - the tail and each return, closures and async blocks left out - is the binding itself (other => { undo(); return other; }); one exit that converts, wraps or replaces it (other.map_err(..), .into(), Ok(..)) is reported. A Result bound by a straight-line let to a name (let r = port.run().await;) and answered later - matched, if let, let _ =, or one of the methods above - is checked like the direct form, until a later let shadows the name. Not seen: a Result moved on into another name (let s = r;), a call through a helper of the type's own, and a call on a receiver whose type the suite cannot resolve (an unannotated local initialised otherwise, a return value)."),
}


# The source rules that place or select by layer read the layer from the crate: one crate per layer. The two-crate
# form (a context crate without a layer whose layers are top-level modules, which DCA-LAY-007 and the graph rules
# accept) is not mapped onto layers yet (WP-174), so these rules say so.
CRATE_PER_LAYER = (" Layers are read from crates with a layer (one crate per layer); in the two-crate form - a context crate "
                   "without a layer whose layers are top-level modules - this rule reads no layer.")
BY_CRATE_LAYER = [
    "DCA-LAY-004", "DCA-ONI-004", "DCA-HEX-003", "DCA-HEX-004", "DCA-HEX-006", "DCA-HEX-008",
    "DCA-HEX-009", "DCA-HEX-010", "DCA-HEX-011", "DCA-HEX-012", "DCA-HEX-013", "DCA-HEX-014",
    "DCA-HEX-016", "DCA-HEX-018", "DCA-HEX-019", "DCA-HEX-020", "DCA-HEX-021", "DCA-USE-002",
    "DCA-USE-003", "DCA-USE-004", "DCA-USE-005", "DCA-USE-006", "DCA-USE-007", "DCA-USE-008",
    "DCA-USE-010", "DCA-USE-011", "DCA-USE-014", "DCA-USE-015", "DCA-USE-018", "DCA-ERR-001",
    "DCA-ERR-002", "DCA-ERR-003", "DCA-ERR-004", "DCA-ERR-006", "DCA-ERR-008", "DCA-ERR-009", "DCA-ERR-010",
]
for _id in BY_CRATE_LAYER:
    MAPPING[_id]["selects"] += CRATE_PER_LAYER

# The rules that read the paths a module names: use declarations and the paths of every other item.
PATHS_READ = (" Paths are read from use declarations - those inside function bodies and blocks too, whose scope is "
              "approximated by the whole module - and from every other item, the arguments of vec!, matches!, assert!, "
              "assert_eq!, assert_ne!, debug_assert!, debug_assert_eq!, debug_assert_ne!, format!, write!, writeln!, print!, "
              "println!, eprint!, eprintln! and panic! parsed as expressions (and the pattern of matches!). Not seen: paths "
              "inside any other macro invocation, and arguments of those macros that do not parse as expressions.")
BY_PATHS = [
    "DCA-MAP-007", "DCA-MAP-008", "DCA-MAP-010", "DCA-MAP-011", "DCA-MAP-014", "DCA-CYC-001", "DCA-CYC-002",
    "DCA-CYC-003", "DCA-CYC-004", "DCA-CYC-005", "DCA-LAY-004", "DCA-ONI-004", "DCA-HEX-003", "DCA-HEX-004",
    "DCA-HEX-006", "DCA-HEX-007", "DCA-HEX-011", "DCA-HEX-012", "DCA-HEX-013", "DCA-HEX-014", "DCA-USE-010",
    "DCA-USE-011", "DCA-ERR-006",
]
for _id in BY_PATHS:
    MAPPING[_id]["checks"] += PATHS_READ

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
    dict(id="DCA-RS-004", title="The domain stays synchronous outside its gateways and domain services",
         rationale="Async is an I/O concern. The domain reaches I/O only through a domain gateway, and only a domain service asks one; everything else in the model stays synchronous, testable and deterministic, and no runtime enters the model.",
         category=RF, enforcement=[W, S],
         selects="Functions and methods in every domain crate, except the methods of traits that carry the DomainGateway or the DomainService role and of the impl blocks of types that implement DomainService.",
         checks="None is async and none returns a type that implements Future. Java and .NET keep the whole domain synchronous; Rust lets a gateway's methods and a domain service's methods be async, with std's Future only. A domain crate without a runtime dependency cannot spawn a task either."),
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
    dict(id="DCA-RS-007", title="A transaction unit belongs to its use case and hands out only what takes part in the transaction",
         rationale="A transaction boundary hands its work a unit - the repositories, stores and event publisher that take part in the open transaction. Declared per use case, beside it, the unit says exactly what the use case changes in one transaction; a unit shared between use cases, or one that hands out a clock or a foreign client, hides what the transaction covers and lets the work reach past it. One form - a generic associated type per port, handed out by value - lets a store's adapter hand out a port that borrows the transaction's connection, and keeps two implementations of the same use case alike.",
         category=LT, enforcement=[T, S],
         selects="Traits that carry the building-block role TransactionUnit, the building block excluded: the bounds of their associated types, generic ones (type Things<'a>: ... where Self: 'a) included, and the paths in the return types of their methods, Self::<associated type> read through the type's bounds.",
         checks="Each sits in a use case's module of an application crate - a top-level module other than shared, or below one; extends no trait but TransactionUnit and std's and core's (Send, Sync); declares every associated type with a lifetime parameter (type Things<'a> where Self: 'a) and returns no reference from a method (Self::Things<'_>, not &mut Self::Things); and every trait it binds or returns, std's and core's excepted, carries the role Repository, Store or DomainEventPublisher - a record of the events a listener received is a Store. Whether the unit names exactly what its use case uses is not checked."),
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
                     status=r.get("status", "enforced"), maturity=mapping["maturity"] or r.get("maturity", "released"),
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
