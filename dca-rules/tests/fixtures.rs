//! Every implemented rule fires on the fixtures, and the roles the rules select by are found.
//!
//! `fixtures/violations` holds at least one violation per rule; `fixtures/undeclared` is a workspace
//! without a context or a layer; `fixtures/empty` one without production code, `fixtures/entry-point`
//! one whose only code is a generated `fn main` (`fixtures/entry-point-plus` with a struct beside it);
//! `fixtures/conforming` follows every rule. None is ever built.

use std::path::PathBuf;

use dca_rules::{Outcome, Workspace, rules, run};

fn fixture(name: &str) -> Workspace {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    let ws = Workspace::load(&dir).expect("the fixture loads");
    assert!(ws.problems.is_empty(), "unread files: {:?}", ws.problems);
    ws
}

fn violations(ws: &Workspace, id: &str) -> Vec<String> {
    match run(ws, id) {
        Outcome::Ran { violations, .. } => violations.iter().map(ToString::to_string).collect(),
        other => panic!("{id} did not run: {other:?}"),
    }
}

#[test]
fn every_implemented_rule_fires_on_a_fixture() {
    let fixtures = [fixture("violations"), fixture("undeclared")];
    let mut silent = vec![];
    for id in rules::implemented().filter(|id| !DOCTRINE.contains(id)) {
        let found: Vec<String> = fixtures.iter().flat_map(|ws| violations(ws, id)).collect();
        if found.is_empty() {
            silent.push(id);
        }
        for line in &found {
            println!("{line}");
        }
    }
    assert!(
        silent.is_empty(),
        "rules that found nothing in the fixtures: {silent:?}"
    );
}

/// A workspace without production code has nothing to declare: the two guards against an empty
/// model pass there, while `fixtures/undeclared` - one struct, no declaration - keeps them red.
#[test]
fn a_workspace_without_production_code_passes_the_declaration_guards() {
    let empty = fixture("empty");
    let undeclared = fixture("undeclared");
    for id in ["DCA-STR-011", "DCA-STR-012"] {
        assert!(
            violations(&empty, id).is_empty(),
            "{id} fired on fixtures/empty"
        );
        assert!(
            !violations(&undeclared, id).is_empty(),
            "{id} silent on fixtures/undeclared"
        );
    }
}

/// The binary `cargo new` writes is a program entry point, no production code: the whole suite is
/// green on it. One struct beside `fn main` and both guards fire.
#[test]
fn a_generated_binary_is_no_production_code() {
    let ws = fixture("entry-point");
    let found: Vec<String> = rules::implemented()
        .flat_map(|id| violations(&ws, id))
        .collect();
    assert!(
        found.is_empty(),
        "findings on a generated binary:\n{}",
        found.join("\n")
    );
    let plus = fixture("entry-point-plus");
    for id in ["DCA-STR-011", "DCA-STR-012"] {
        assert!(
            !violations(&plus, id).is_empty(),
            "{id} silent beside a real type"
        );
    }
}

/// DCA-USE-023: the port of `install` that `check` uses is reported; the one only `install` uses,
/// and the conforming fixture's shared ports, are not.
#[test]
fn a_port_of_one_use_case_used_by_another_is_reported() {
    let found = violations(&fixture("violations"), "DCA-USE-023");
    assert!(
        found
            .iter()
            .any(|v| v.contains("install::Source") && v.contains("alpha_application::check")),
        "{found:?}"
    );
    assert!(!found.iter().any(|v| v.contains("OwnLog")), "{found:?}");
    assert!(violations(&fixture("conforming"), "DCA-USE-023").is_empty());
    assert!(violations(&fixture("empty"), "DCA-USE-023").is_empty());
}

/// DCA-RS-008: a store method that answers synchronously and a hand-built waker are reported; an
/// `async fn` and a `BoxFuture` pass, and so does a plain output port (no I/O role); switched off,
/// the rule does not run.
#[test]
fn a_synchronous_port_and_an_executor_in_the_application_are_reported() {
    let mut ws = fixture("violations");
    let found = violations(&ws, "DCA-RS-008");
    assert!(
        found.iter().any(|v| v.contains("Files::files")),
        "{found:?}"
    );
    assert!(
        !found
            .iter()
            .any(|v| v.contains("Files::later") || v.contains("Files::boxed")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|v| v.contains("Waker::noop") && v.contains("drives a future")),
        "{found:?}"
    );
    assert!(
        !found
            .iter()
            .any(|v| v.contains("Source::read") || v.contains("OwnLog::note")),
        "a plain output port is no port with I/O: {found:?}"
    );
    assert!(violations(&fixture("empty"), "DCA-RS-008").is_empty());
    ws.config.experimental = dca_rules::model::Experimental::Only(Default::default());
    assert!(matches!(run(&ws, "DCA-RS-008"), Outcome::NotSwitchedOn));
}

/// Rules of doctrine alone: they select nothing and so find nothing anywhere.
const DOCTRINE: &[&str] = &["DCA-LAY-001", "DCA-STR-010"];

/// Diagnostics that list what the workspace declares, not what is wrong with it: they list in a
/// conforming workspace too.
const INVENTORIES: &[&str] = &["DCA-MAP-013", "DCA-STR-001"];

#[test]
fn no_rule_finds_anything_in_the_conforming_fixture() {
    let ws = fixture("conforming");
    let found: Vec<String> = rules::implemented()
        .filter(|id| !INVENTORIES.contains(id))
        .flat_map(|id| violations(&ws, id))
        .collect();
    assert!(
        found.is_empty(),
        "findings in a workspace that follows every rule:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_conforming_fixture_carries_the_roles_its_rules_select_by() {
    let ws = fixture("conforming");
    let role_of = |name: &str, role: &str| {
        let d = (0..ws.decls.len())
            .find(|d| ws.decls[*d].name == name)
            .unwrap_or_else(|| panic!("{name} not found"));
        assert!(
            ws.has_role(d, role),
            "{name} should carry {role}, has {:?}",
            ws.roles[d]
        );
    };
    role_of("Thing", "AggregateRoot");
    role_of("Thing", "Entity");
    role_of("Part", "Entity");
    role_of("Label", "Value");
    role_of("ThingId", "Id");
    role_of("PartAdded", "DomainEvent");
    role_of("ThingEvent", "DomainEvent");
    role_of("ThingError", "DomainFailure");
    role_of("ThingRepository", "Repository");
    role_of("InMemoryThingRepository", "Repository");
    role_of("AddPartInputPort", "InputPort");
    role_of("AddPartUseCase", "InputPort");
    role_of("AddPartError", "UseCaseFailure");
    role_of("AddPartError", "InputPortFailure");
    role_of("AddPartError", "PortFailure");
    role_of("ThingNumberTaken", "OutputPortFailure");
    role_of("ThingNumberTaken", "PortFailure");
    role_of("Clock", "OutputPort");
}

/// DCA-ERR-006 counts a match over the variants of a closed result as translation; a module that
/// names neither a failure nor a variant, or only holds the result, is still listed.
#[test]
fn a_match_over_a_closed_result_counts_as_translation() {
    let ws = fixture("results");
    let listed = violations(&ws, "DCA-ERR-006");
    let names = |module: &str| {
        listed
            .iter()
            .any(|line| line.contains(&format!("alpha_adapters::incoming::{module} ")))
    };
    assert!(
        !names("page"),
        "a match through the enum's path: {listed:?}"
    );
    assert!(!names("web"), "a match over imported variants: {listed:?}");
    assert!(names("rest"), "neither a failure nor a variant: {listed:?}");
    assert!(
        names("feed"),
        "the result held, no variant named: {listed:?}"
    );
}

/// DCA-ERR-008 lists a closed result without a variant carrying a Refusal and never fails; one
/// that has such a variant is not listed.
#[test]
fn a_closed_result_without_a_refusing_variant_is_listed_not_failed() {
    let ws = fixture("results");
    match run(&ws, "DCA-ERR-008") {
        Outcome::Ran { informational, .. } => {
            assert!(informational, "DCA-ERR-008 is a diagnostic")
        }
        other => panic!("DCA-ERR-008 did not run: {other:?}"),
    }
    let listed = violations(&ws, "DCA-ERR-008");
    assert!(
        listed.iter().any(|line| line.contains("`CheckPartResult`")),
        "two successes, none marked: {listed:?}"
    );
    assert!(
        listed
            .iter()
            .any(|line| line.contains("is none of these outcomes a refusal")),
        "{listed:?}"
    );
    assert!(
        !listed.iter().any(|line| line.contains("`AddPartResult`")),
        "a variant carrying a Refusal: {listed:?}"
    );
    assert!(
        !listed.iter().any(|line| line.contains("RemovePartResult")),
        "a struct is no closed result: {listed:?}"
    );
}

/// The port failures: an output-port failure outside the application crate, a failure that
/// implements only the classification, an incoming module that names the base.
#[test]
fn port_failures_are_placed_based_and_named() {
    let ws = fixture("violations");
    let err_002 = violations(&ws, "DCA-ERR-002");
    assert!(
        err_002
            .iter()
            .any(|line| line.contains("`StoreRefusedThing`")),
        "{err_002:?}"
    );
    let err_003 = violations(&ws, "DCA-ERR-003");
    assert!(
        err_003.iter().any(|line| line.contains("`QuotaExceeded`")),
        "{err_003:?}"
    );
    let err_006 = violations(&ws, "DCA-ERR-006");
    assert!(
        err_006
            .iter()
            .any(|line| line.contains("names the port-failure classification PortFailure")),
        "{err_006:?}"
    );
}

/// DCA-ERR-009 reports a use case that constructs a domain failure, a variant call in a closure and
/// a unit variant alike; the domain crate constructing its own failures is not selected.
#[test]
fn a_domain_failure_is_constructed_only_in_the_domain() {
    let ws = fixture("violations");
    let listed = violations(&ws, "DCA-ERR-009");
    let in_use_case: Vec<&String> = listed
        .iter()
        .filter(|line| line.contains("alpha_application::remove_thing"))
        .collect();
    assert_eq!(in_use_case.len(), 2, "Unknown(id) and Gone: {listed:?}");
    assert!(
        in_use_case
            .iter()
            .all(|line| line.contains("`ThingRefused`")),
        "{listed:?}"
    );
    assert!(
        !listed.iter().any(|line| line.contains("alpha_domain")),
        "the domain raises its own failures: {listed:?}"
    );
}

/// A private `bool` field and a `bool` return pass; a `bool` parameter and a `pub` `bool` field are
/// reported as flag arguments.
#[test]
fn a_yes_no_fact_is_a_field_and_never_a_parameter() {
    let ws = fixture("violations");
    let lamp: Vec<String> = violations(&ws, "DCA-TAC-025")
        .into_iter()
        .filter(|line| line.contains("Lamp"))
        .collect();
    assert_eq!(lamp.len(), 4, "{lamp:?}");
    assert!(
        lamp.iter()
            .any(|line| line.contains("`dim_to` takes a bare `u8`")),
        "{lamp:?}"
    );
    assert!(
        lamp.iter()
            .any(|line| line.contains("`switch` takes a bare `bool`")),
        "{lamp:?}"
    );
    assert!(
        lamp.iter()
            .any(|line| line.contains("`restore` takes a bare `u32`")),
        "{lamp:?}"
    );
    assert!(
        !lamp
            .iter()
            .any(|line| line.contains("`restore` takes a bare `bool`")),
        "{lamp:?}"
    );
    assert!(
        lamp.iter()
            .any(|line| line.contains("is a public `bool` anyone may set")),
        "{lamp:?}"
    );
    assert!(!lamp.iter().any(|line| line.contains("lit`")), "{lamp:?}");
}

/// An owned aggregate whose repository loads it by id alone is reported; the conforming one, with a
/// lookup by its id and its keeper's id, is not.
#[test]
fn an_owned_aggregate_is_loaded_through_its_owner() {
    let ws = fixture("violations");
    let found = violations(&ws, "DCA-TAC-026");
    assert!(
        found.iter().any(|line| line.contains(
            "repository `WrongRepository` declares no lookup taking `ThingId` and `KeeperId` for the owned aggregate `Thing`"
        )),
        "{found:?}"
    );
    assert!(violations(&fixture("conforming"), "DCA-TAC-026").is_empty());
}

/// A handler that branches on the caller's role in its body is listed; one with the configured
/// extractor in its signature and one routed behind the configured layer are not.
#[test]
fn a_claims_only_check_is_declared_on_the_route() {
    let ws = fixture("violations");
    let found = violations(&ws, "DCA-HEX-020");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("`close_books` reads the caller's roles through `has_role`"),
        "{found:?}"
    );
    assert!(violations(&fixture("conforming"), "DCA-HEX-020").is_empty());
}

/// DCA-ADV-021: the model type, the outgoing adapter and the composition root's call are reported;
/// the composition root's construction is not, nor anything in the conforming fixture, whose use
/// case calls its domain service and whose bootstrap builds it.
#[test]
fn a_domain_service_is_used_by_the_application_only() {
    let ws = fixture("violations");
    let found = violations(&ws, "DCA-ADV-021");
    let has = |text: &str| found.iter().any(|line| line.contains(text));
    assert!(has("`rename` calls `service.calculate`"), "{found:?}");
    assert!(has("`export` calls `calculator.calculate`"), "{found:?}");
    assert!(has("`wire` calls `calculator.calculate`"), "{found:?}");
    assert!(!has("`wire` constructs"), "{found:?}");
    assert!(!has("Planner"), "{found:?}");
}

/// DCA-ERR-010: `let _ =`, an `Err(_)` arm, a catch-all binding arm, `.is_ok()`, `if let Ok(..)` and `catch_unwind` around a port call are
/// reported, a catch-all binding arm that hands on something else in one of its exits as well;
/// `?` after a `map_err` that uses the failure and an `Err(e)` arm are not, nor a catch-all
/// binding arm that hands what it caught on unchanged (the conforming fixture).
#[test]
fn a_port_failure_is_answered_by_name() {
    let found = violations(&fixture("violations"), "DCA-ERR-010");
    let has = |text: &str| found.iter().any(|line| line.contains(text));
    // Twelve here, three more through locals (`ports_and_results_are_followed_through_locals`).
    assert_eq!(found.len(), 15, "{found:?}");
    assert!(
        has(
            "`keep_or_wrap` answers the failure of the port call `self.things.save` with a catch-all arm"
        ),
        "{found:?}"
    );
    assert!(
        has(
            "`keep_or_pretend` answers the failure of the port call `self.things.save` with a catch-all arm"
        ),
        "{found:?}"
    );
    assert!(
        has(
            "`keep_or_drop` answers the failure of the port call `self.things.save` with a catch-all arm"
        ),
        "{found:?}"
    );
    assert!(
        has(
            "`purge` answers the failure of the port call `unit.things().delete_by_id` with `let _ =`"
        ),
        "{found:?}"
    );
    assert!(
        has("`store` answers the failure of the port call `unit.things().save` with `let _ =`"),
        "{found:?}"
    );
    assert!(
        has(
            "`ask_state` answers the failure of the port call `ports.other.run` with `.map_or(..)`"
        ),
        "{found:?}"
    );
    assert!(
        has("`ask_held` answers the failure of the port call `ports.other.run` with `.ok(..)`"),
        "{found:?}"
    );
    assert!(
        has("`ask` answers the failure of the port call `port.run` with `if let Ok(..)`"),
        "{found:?}"
    );
    assert!(
        has("`keep` answers the failure of the port call `self.things.save` with `let _ =`"),
        "{found:?}"
    );
    assert!(has("with an `Err(_)` arm"), "{found:?}");
    assert!(
        has("`try_keep` answers the failure of the port call `self.things.save` with `.is_ok(..)`"),
        "{found:?}"
    );
    assert!(
        has(
            "`keep_anyway` answers the failure of the port call `self.things.save` with `catch_unwind`"
        ),
        "{found:?}"
    );
    assert!(!has("keep_named"), "{found:?}");
}

/// DCA-MAP-013 lists the declared edges of a conforming workspace; the context-map rules find
/// nothing in it.
#[test]
fn the_declared_context_map_is_listed() {
    let ws = fixture("conforming");
    assert_eq!(
        violations(&ws, "DCA-MAP-013"),
        vec![
            "[DCA-MAP-013] alpha-domain (alpha-domain/Cargo.toml): alpha -> beta via events (acl): beta's shipping vocabulary stays out of alpha's model"
        ]
    );
}

/// DCA-CYC-002 slices the whole application crate, the shared module included; DCA-CYC-005 slices
/// at the operation roots, so a cycle inside one feature is found. DCA-CYC-001, 003 and 004 slice a domain crate's top-level
/// modules and the modules below the adapter sides.
#[test]
fn cycles_inside_a_crate_are_found_per_slice() {
    let ws = fixture("violations");
    let has = |id: &str, text: &str| violations(&ws, id).iter().any(|line| line.contains(text));
    assert!(has("DCA-CYC-002", "lookup -> shared -> lookup"));
    assert!(has("DCA-CYC-002", "do_thing -> other -> do_thing"));
    assert!(!has("DCA-CYC-005", "shared"));
    // Sliced at each operation root, the container `usecases` left out of the slice's name.
    assert!(has(
        "DCA-CYC-005",
        "sorting::sort_one -> sorting::sort_two -> sorting::sort_one"
    ));
    assert!(!has("DCA-CYC-005", "cycle: usecases"));
    assert!(has("DCA-CYC-001", "left -> right -> left"));
    // A `use` inside a function body and a path inside `assert_eq!` are read.
    assert!(has("DCA-CYC-001", "down -> up -> down"));
    assert!(has(
        "DCA-CYC-003",
        "outgoing::east -> outgoing::west -> outgoing::east"
    ));
    assert!(has(
        "DCA-CYC-004",
        "incoming::first -> incoming::second -> incoming::first"
    ));
}

/// DCA-LAY-004: the boundary named in the domain and a configured transaction type named in an
/// incoming adapter are reported; the application's boundary is not.
#[test]
fn transactions_are_run_by_the_application() {
    let found = violations(&fixture("violations"), "DCA-LAY-004");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().any(|l| l.contains("alpha_domain")
            && l.contains("the transaction boundary, in the domain crate")),
        "{found:?}"
    );
    assert!(found.iter().any(|l| l.contains("`vendor_sdk::Client`, a transaction type, in an incoming adapter")), "{found:?}");
}

/// The router presets: a GET handler that builds a command - itself or in a helper of its module it
/// calls - or calls a port that takes one is reported, the POST handler beside it and the
/// conforming GET handler are not; a handler outside the incoming module is reported.
#[test]
fn handlers_are_found_through_the_router_presets() {
    let ws = fixture("violations");
    let hex_014 = violations(&ws, "DCA-HEX-014");
    assert_eq!(hex_014.len(), 3, "{hex_014:?}");
    for expected in [
        "`run` answers GET and builds the command `RunCommand`",
        "`look` answers GET and calls `port.sweep` on an input port that takes a command",
        "`glance` answers GET and, through `prepare`, builds the command `RunCommand`",
    ] {
        assert!(
            hex_014.iter().any(|l| l.contains(expected)),
            "{expected}: {hex_014:?}"
        );
    }
    let hex_018 = violations(&ws, "DCA-HEX-018");
    assert_eq!(hex_018.len(), 1, "{hex_018:?}");
    assert!(hex_018[0].contains("`health` is registered"), "{hex_018:?}");
    let conforming = fixture("conforming");
    assert!(violations(&conforming, "DCA-HEX-014").is_empty());
    assert!(violations(&conforming, "DCA-HEX-018").is_empty());
}

/// DCA-HEX-016 lists `{:?}` (positional and named) and `.to_string()` on an inner enum, not `{}`.
#[test]
fn an_inner_enum_printed_by_an_adapter_is_listed() {
    let found = violations(&fixture("violations"), "DCA-HEX-016");
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        !found.iter().any(|l| l.contains("Phase::Open")),
        "{found:?}"
    );
}

/// DCA-STR-001 lists each context with its crates, and the shared kernel.
#[test]
fn the_contexts_are_listed_with_their_crates() {
    assert_eq!(
        violations(&fixture("conforming"), "DCA-STR-001"),
        vec![
            "[DCA-STR-001] workspace: context `alpha`: alpha-adapters (adapters), alpha-application (application), alpha-domain (domain)",
            "[DCA-STR-001] workspace: context `beta`: beta-events (events)",
            "[DCA-STR-001] workspace: shared kernel: kernel-adapters (adapters), kernel-application (application), kernel-domain (domain)",
        ]
    );
}

/// DCA-HEX-017: a listener that holds an input port and reacts in the transaction is reported - its
/// field read with the bounds of the impl block too; the conforming one, a transactional listener,
/// is not.
#[test]
fn a_listener_that_drives_a_use_case_runs_after_the_commit() {
    let found = violations(&fixture("violations"), "DCA-HEX-017");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .any(|l| l.contains("`RunOnEvent` drives an input port through the field `port`")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|l| l.contains("`RunLater` drives an input port through the field `next`")),
        "{found:?}"
    );
    assert!(violations(&fixture("conforming"), "DCA-HEX-017").is_empty());
}

/// DCA-RS-007: a unit outside a use case's module, units in the reference form, a unit that extends
/// a helper and units that hand out a helper or the outbox are reported; a use case's own unit in
/// the generic-associated-type form is not.
#[test]
fn a_transaction_unit_belongs_to_its_use_case() {
    let found = violations(&fixture("violations"), "DCA-RS-007");
    let has = |text: &str| found.iter().any(|l| l.contains(text));
    for expected in [
        "`SharedUnit` sits outside a use case's module",
        "`SharedUnit` declares `type Things` without a lifetime",
        "`SharedUnit` hands out a reference from `fn things`",
        "`SharedUnit` hands out `Helper` in `type Helper`",
        "`PurgeThingUnit` hands out `Helper` in `type Helper`",
        "`PurgeThingUnit` extends `Helper`",
        "`PurgeThingUnit` hands out `dca_building_blocks::hexagonal::port_out::OutputPort` in `type Pending`",
    ] {
        assert!(has(expected), "{expected}: {found:?}");
    }
    assert!(!has("StoreThingUnit"), "{found:?}");
    assert_eq!(found.len(), 9, "{found:?}");
}

/// A port the transaction unit hands out counts as the use case's port: a save through the unit
/// without handing the events on is reported.
#[test]
fn a_save_through_the_unit_counts() {
    let found = violations(&fixture("violations"), "DCA-USE-009");
    assert!(
        found
            .iter()
            .any(|l| l.contains("StoreThingUseCase") && l.contains("`store` saves")),
        "{found:?}"
    );
}

/// DCA-TAC-027: a gateway in a field, read from a static or from a field of a value handed in,
/// obtained from a locator or a trait's associated function, handed out by a method or constructed
/// in place is reported; one taken as a parameter and called is not - in the
/// violations' `price` nor in the conforming aggregate and value.
#[test]
fn the_model_receives_a_gateway_only_as_a_parameter() {
    let found = violations(&fixture("violations"), "DCA-TAC-027");
    let has = |text: &str| found.iter().any(|l| l.contains(text));
    assert!(
        has("value `Quote` holds the domain gateway `Rates` in `rates`"),
        "{found:?}"
    );
    assert!(
        has("`Thing` reads the gateway `FixedRates` from `RATES` in `price_from_static`"),
        "{found:?}"
    );
    assert!(
        has("`Thing` obtains the gateway `FixedRates` from `rates()` in `price_from_locator`"),
        "{found:?}"
    );
    assert!(
        has("`Thing` hands out the domain gateway `FixedRates` from `rate_source`"),
        "{found:?}"
    );
    assert!(
        has("`Quote` constructs the gateway implementation `FixedRates` in `fixed`"),
        "{found:?}"
    );
    assert!(
        has("`Thing` reads the gateway `Rates` from `ctx.rates` in `price_from_context`"),
        "{found:?}"
    );
    assert!(
        has(
            "`Thing` obtains the gateway `FixedRates` from `RateLocator::locate` in `price_from_trait`"
        ),
        "{found:?}"
    );
    assert!(
        has(
            "`Thing` obtains the gateway `FixedRates` from `RateLocator::locate` in `price_from_qualified`"
        ),
        "{found:?}"
    );
    assert!(!has("in `price`"), "{found:?}");
    assert!(violations(&fixture("conforming"), "DCA-TAC-027").is_empty());
}

#[test]
fn the_domain_is_async_only_in_its_gateways_and_services() {
    let violating = fixture("violations");
    let oni = violations(&violating, "DCA-ONI-004");
    assert!(oni.iter().any(|l| l.contains("`load` is async")), "{oni:?}");
    assert!(
        oni.iter()
            .any(|l| l.contains("of the async runtime `tokio`")),
        "{oni:?}"
    );
    assert!(!oni.iter().any(|l| l.contains("`rate`")), "{oni:?}");
    let conforming = fixture("conforming");
    assert!(violations(&conforming, "DCA-ONI-004").is_empty());
    assert!(violations(&conforming, "DCA-RS-004").is_empty());
}

/// DCA-TAC-025 beyond fields and inherent methods: a two-variant enum that names one fact and its
/// negation (`un` before an adjective-like name), and a domain trait's signatures - the output of a
/// returned future included. An enum whose two states have names of their own, two actions
/// (`Lock`/`Unlock`, `Load`/`Unload`) and a gateway that answers a bool pass.
#[test]
fn booleans_in_disguise_and_domain_traits_are_reported() {
    let found = violations(&fixture("violations"), "DCA-TAC-025");
    let has = |text: &str| found.iter().any(|l| l.contains(text));
    for expected in [
        "enum `Visibility` names one fact and its negation (`Visible`, `NotVisible`)",
        "enum `Power` names one fact and its negation (`On`, `Off`)",
        "enum `Latch` names one fact and its negation (`Locked`, `Unlocked`)",
        "`weigh` takes a bare `u32`",
        "`weigh` returns a bare `String`",
        "`tare` returns a bare `u64`",
    ] {
        assert!(has(expected), "{expected}: {found:?}");
    }
    assert!(violations(&fixture("conforming"), "DCA-TAC-025").is_empty());
}

/// Beyond inherent methods and structs: the methods a type takes from a trait of the workspace -
/// written in the impl or inherited as the trait's default -, the traits that carry a role or a
/// name, and the methods of an api crate's types and traits. A trait of std or of the building
/// blocks, a use case's own input port and the events crate are not read.
#[test]
fn trait_methods_and_traits_are_read_as_java_reads_interfaces() {
    let ws = fixture("violations");
    let expect = |id: &str, present: &[&str], absent: &[&str]| {
        let found = violations(&ws, id);
        for text in present {
            assert!(
                found.iter().any(|l| l.contains(text)),
                "{id} {text}: {found:?}"
            );
        }
        for text in absent {
            assert!(
                !found.iter().any(|l| l.contains(text)),
                "{id} not {text}: {found:?}"
            );
        }
    };
    expect(
        "DCA-TAC-006",
        &["setter `set_label`", "setter `set_title`"],
        &[],
    );
    expect("DCA-ADV-020", &["`Calculator::quote_for`"], &[]);
    expect(
        "DCA-USE-017",
        &[
            "`SortPartsUseCase` has the pub method `peek`",
            "`SortPartsUseCase` has the pub method `glance`",
        ],
        &["method `sort`", "method `clone`"],
    );
    expect("DCA-NAM-007", &["`PartDto`"], &[]);
    expect("DCA-NAM-008", &["`PartConverter`"], &[]);
    expect("DCA-NAM-010", &["`PartUtil`"], &[]);
    expect("DCA-STR-005", &["`Window`"], &[]);
    expect(
        "DCA-NAM-013",
        &["update_count", "set_count"],
        &["update_notice"],
    );
}

/// A shared-kernel crate is read by its layer: the rules on a domain crate read the kernel's domain
/// (the conforming fixture holds its events, services, factories and specifications in the kernel's
/// domain - no integration event, which is a context's published contract - its repository and store traits in the kernel's application, and the
/// outbox in the kernel's outgoing adapters). Contexts depend on it layer by layer; it depends on no
/// context.
#[test]
fn the_shared_kernel_is_read_by_its_layers() {
    let ws = fixture("violations");
    for (id, text) in [
        ("DCA-TAC-001", "kernel_domain::LedgerAggregateRoot"),
        ("DCA-ADV-004", "kernel_domain::Noted"),
        ("DCA-ADV-011", "kernel_domain::Sorting"),
        ("DCA-ADV-015", "kernel_domain::NotedFactory"),
        ("DCA-ADV-018", "kernel_domain::OpenSpecification"),
        (
            "DCA-STR-007",
            "integration event `Bulletin` sits outside a context's events crate",
        ),
        ("DCA-HEX-019", "`DomainLedger` implements an output port"),
        (
            "DCA-ONI-002",
            "kernel-domain (kernel-domain/Cargo.toml): the domain depends on `sqlx`",
        ),
    ] {
        let found = violations(&ws, id);
        assert!(
            found.iter().any(|l| l.contains(text)),
            "{id} {text}: {found:?}"
        );
    }
    assert_eq!(
        violations(&ws, "DCA-STR-002"),
        vec![
            "[DCA-STR-002] kernel-application (kernel-application/Cargo.toml): the shared kernel depends on `alpha-application` (application of alpha), a bounded context",
        ]
    );
    let str_003 = violations(&ws, "DCA-STR-003");
    assert!(
        !str_003.iter().any(|l| l.contains("kernel-")),
        "the kernel's dependency on a context is DCA-STR-002's: {str_003:?}"
    );
}

/// The single-rule gaps closed after the parity audit: what each rule now reads, and what still
/// passes beside it.
#[test]
fn single_rules_read_what_java_reads() {
    let ws = fixture("violations");
    let expect = |id: &str, present: &[&str], absent: &[&str]| {
        let found = violations(&ws, id);
        for text in present {
            assert!(
                found.iter().any(|l| l.contains(text)),
                "{id} {text}: {found:?}"
            );
        }
        for text in absent {
            assert!(
                !found.iter().any(|l| l.contains(text)),
                "{id} not {text}: {found:?}"
            );
        }
    };
    expect("DCA-ADV-004", &["variant `Away` carries `sqlx`"], &[]);
    expect(
        "DCA-ADV-011",
        &[
            "an impl block of `Mover` carries `sqlx::instrument`",
            "method `shift` carries `sqlx::trace`",
        ],
        &[],
    );
    expect(
        "DCA-ADV-019",
        &["`Bin` uses a domain service in `tidy`"],
        &[],
    );
    expect(
        "DCA-ONI-004",
        &[
            "the field `work` holds a Future",
            "`wait_for` takes a Future",
            "`later` returns a Future",
        ],
        &[],
    );
    expect("DCA-ERR-001", &["`Jammed`"], &[]);
    expect("DCA-ERR-003", &["`Blocked`"], &[]);
    expect(
        "DCA-ERR-004",
        &[
            "`Overdue` implements `axum::response::IntoResponse`",
            "`Spilled` carries `sqlx::Type`",
        ],
        &[],
    );
    expect(
        "DCA-HEX-019",
        &["`Relay` implements an output port"],
        &["Bridge"],
    );
    expect(
        "DCA-USE-016",
        &[
            "`ChainUseCase` holds the input port `OtherInputPort` in `next`",
            "`RelayUseCase` takes the input port `OtherInputPort` in `relay`",
        ],
        &[],
    );
    expect("DCA-NAM-012", &["`WhenDueUseCase`"], &[]);
    expect("DCA-NAM-013", &["`UpdateLimitUseCase`"], &[]);
    expect("DCA-MAP-010", &["names `feed::Entry`"], &[]);
    expect(
        "DCA-MAP-008",
        &[
            "context `alpha` translates the events channel of `beta` in an anti-corruption layer, but no module of the incoming side",
        ],
        &["the api channel of `alpha`"],
    );
    expect(
        "DCA-TAC-004",
        &[
            "`Pile` has no field of its id type `ThingId`",
            "`Slot` has no field of its id type `ThingId` in its variant `Empty`",
        ],
        &["variant `Filled`"],
    );
    expect("DCA-TAC-024", &["`Tag` has `new` beside `from_text`"], &[]);
    expect(
        "DCA-TAC-005",
        &[
            "entity `NoId` can be built from outside the aggregate through `#[derive(Default)]`",
            "entity `Part` can be built from outside the aggregate through `impl TryFrom<..>`",
            "entity `Slot` can be built from outside the aggregate through its variant `Filled`, as a pub enum",
            "entity `Slot` can be built from outside the aggregate through its variant `Empty`, as a pub enum",
        ],
        &[],
    );
    expect(
        "DCA-TAC-023",
        &[
            "aggregate root `Other` can be built through `impl Default`",
            "aggregate root `Other` can be built through `impl From<..>`",
        ],
        &["`Thing` can be built"],
    );
    let overweight: Vec<String> = violations(&ws, "DCA-ERR-007")
        .into_iter()
        .filter(|l| l.contains("`Overweight`"))
        .collect();
    assert_eq!(overweight.len(), 2, "{overweight:?}");
    assert!(
        overweight
            .iter()
            .any(|l| l.contains("neither an id, a value nor a number (`Holder`) in `extra`")),
        "{overweight:?}"
    );
    assert!(
        overweight
            .iter()
            .any(|l| l.contains("a plain string (`str`) in `label`")),
        "{overweight:?}"
    );
    assert!(violations(&fixture("conforming"), "DCA-HEX-019").is_empty());
}

/// X2: a local bound to a port - to a field of `self`, to a unit's accessor - counts as the port,
/// and a port call's result bound first and answered later is checked like the direct form; a
/// binding shadowed by a later `let` no longer counts. An aggregate rebuilt through its named
/// function into an unannotated local is typed.
#[test]
fn ports_and_results_are_followed_through_locals() {
    let ws = fixture("violations");
    let found = |id: &str| violations(&ws, id);
    let has = |id: &str, text: &str| found(id).iter().any(|l| l.contains(text));
    assert!(
        has("DCA-USE-009", "`stash` saves"),
        "{:?}",
        found("DCA-USE-009")
    );
    assert!(
        has("DCA-USE-009", "`tuck` saves"),
        "{:?}",
        found("DCA-USE-009")
    );
    assert!(
        has("DCA-USE-012", "`stash` calls `save` on `things`"),
        "{:?}",
        found("DCA-USE-012")
    );
    assert!(
        has(
            "DCA-USE-019",
            "query use case `AliasThingUseCase` calls `save` in `stash`"
        ),
        "{:?}",
        found("DCA-USE-019")
    );
    let err = found("DCA-ERR-010");
    for expected in [
        "`stash` answers the failure of the port call `things.save` with an `Err(_)` arm",
        "`peek` answers the failure of the port call `self.things.save` with `if let Ok(..)`",
        "`tuck` answers the failure of the port call `tucked.save` with `let _ =`",
    ] {
        assert!(
            err.iter().any(|l| l.contains(expected)),
            "{expected}: {err:?}"
        );
    }
    assert!(!err.iter().any(|l| l.contains("`settle`")), "{err:?}");
    assert_eq!(err.len(), 15, "{err:?}");
    assert!(
        has("DCA-HEX-021", "`rebuild` calls `rebuilt.parts_mut`"),
        "{:?}",
        found("DCA-HEX-021")
    );
}

/// A `use` inside a function body fills only names the module neither declares nor imports, and a
/// glob over an enum answers only for its variants: `tally::Error` stays the module's own failure,
/// `u128` in the module that globs `Seal::*` inside a function is no `Seal`.
#[test]
fn block_local_uses_neither_shadow_nor_answer_for_the_module() {
    let ws = fixture("conforming");
    let module = |path: &[&str]| {
        (0..ws.modules.len())
            .find(|m| {
                ws.crates[ws.modules[*m].krate].package == "alpha-domain"
                    && ws.modules[*m].path == path
            })
            .unwrap_or_else(|| panic!("{path:?} not found"))
    };
    // The module and the name of the decl a name resolves to in module `m`.
    let decl_of = |m: usize, name: &str| match ws.resolve(m, &[name.to_string()], 0) {
        Some(dca_rules::model::Target::Decl(d)) => Some((ws.decls[d].module, ws.decl_name(d))),
        _ => None,
    };
    let tally = module(&["tally"]);
    assert_eq!(decl_of(tally, "Error"), Some((tally, "Error")));
    let thing = module(&["thing"]);
    assert_eq!(decl_of(thing, "u128"), None);
    assert_eq!(decl_of(thing, "Open"), Some((thing, "Seal")));
}

/// Idiomatic code the suite once misread stays silent: an asynchronous domain service declared as a
/// trait (DCA-ONI-004, DCA-RS-004), two actions named `X`/`UnX` (DCA-TAC-025), a failure that wraps
/// its cause or a duration (DCA-ERR-007), a feature re-exporting its operation (DCA-CYC-005), an
/// `Option` a port answers matched with `Some(_)`/`None` and closure, loop, arm and `if let`
/// bindings named like a port call's result (DCA-ERR-010), a listener and its translator in sibling
/// modules (DCA-MAP-008).
#[test]
fn idiomatic_code_the_suite_once_misread_stays_silent() {
    let ws = fixture("conforming");
    for id in [
        "DCA-ONI-004",
        "DCA-RS-004",
        "DCA-TAC-008",
        "DCA-TAC-025",
        "DCA-ERR-001",
        "DCA-ERR-007",
        "DCA-ERR-010",
        "DCA-CYC-005",
        "DCA-MAP-008",
    ] {
        let found = violations(&ws, id);
        assert!(found.is_empty(), "{id}: {found:?}");
    }
}

/// DCA-STR-007, -013: a context publishes its integration events in an events crate beside its api
/// crate, as Java keeps `{context}/events` beside `{context}/api`. An integration event in a module of
/// the api crate is reported; a trait in the events crate is read as part of the contract.
#[test]
fn integration_events_live_in_their_own_crate() {
    let ws = fixture("violations");
    for (id, text) in [
        ("DCA-STR-007", "beta_api::events::LimitRaised"),
        ("DCA-STR-013", "beta_events::Notice"),
    ] {
        let found = violations(&ws, id);
        assert!(
            found.iter().any(|l| l.contains(text)),
            "{id} {text}: {found:?}"
        );
    }
    assert!(violations(&fixture("conforming"), "DCA-STR-007").is_empty());
}

/// Every module name and suffix the suite places or selects by can be renamed in
/// `[workspace.metadata.dca]`, as Java's `DcaLayout` renames packages and suffixes; the rules follow
/// the configured names and ignore the defaults.
#[test]
fn the_rules_follow_renamed_modules_and_suffixes() {
    let ws = fixture("renamed");
    let found: Vec<String> = rules::implemented()
        .filter(|id| !INVENTORIES.contains(id))
        .flat_map(|id| violations(&ws, id))
        .collect();
    let expected = [
        "[DCA-HEX-008] alpha_adapters::driving::StrayThingRepo ",
        "[DCA-HEX-009] alpha_application::common::Clock ",
        "[DCA-HEX-009] alpha_application::common::ThingRepoIndex ",
        "[DCA-TAC-001] alpha_domain::LedgerRoot ",
        "[DCA-ADV-013] alpha_domain::ThingFactory ",
        "[DCA-NAM-001] alpha_application::rename_part::RenamePartUseCase ",
        "[DCA-NAM-003] alpha_application::rename_part::RenamePartContract ",
        "[DCA-NAM-004] alpha_application::common::ThingRepoIndex ",
    ];
    let mut unexpected: Vec<&String> = found
        .iter()
        .filter(|l| !expected.iter().any(|e| l.starts_with(e)))
        .collect();
    unexpected.sort();
    let missing: Vec<&&str> = expected
        .iter()
        .filter(|e| !found.iter().any(|l| l.starts_with(**e)))
        .collect();
    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "unexpected: {unexpected:#?}\nmissing: {missing:#?}"
    );
    let shared = violations(&ws, "DCA-HEX-009");
    assert!(
        shared.iter().all(|l| l.contains("application::common")),
        "{shared:?}"
    );
    let naming = violations(&ws, "DCA-NAM-001");
    assert!(naming.iter().all(|l| l.contains("*Service")), "{naming:?}");
}

/// On a workspace that holds no production code yet the whole suite is green.
#[test]
fn no_rule_finds_anything_in_a_workspace_without_production_code() {
    let ws = fixture("empty");
    let found: Vec<String> = rules::implemented()
        .flat_map(|id| violations(&ws, id))
        .collect();
    assert!(
        found.is_empty(),
        "findings in a workspace without production code:\n{}",
        found.join("\n")
    );
}
