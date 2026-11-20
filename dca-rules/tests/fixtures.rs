//! Every implemented rule fires on the fixtures, and the roles the rules select by are found.
//!
//! `fixtures/violations` holds at least one violation per rule; `fixtures/undeclared` is a workspace
//! without a context or a layer; `fixtures/conforming` follows every rule. None is ever built.

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
    for id in rules::implemented() {
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
    assert_eq!(lamp.len(), 3, "{lamp:?}");
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

/// DCA-ERR-010: `let _ =`, an `Err(_)` arm, `.is_ok()`, `if let Ok(..)` and `catch_unwind` around a port call are
/// reported; `?` after a `map_err` that uses the failure and an `Err(e)` arm are not.
#[test]
fn a_port_failure_is_answered_by_name() {
    let found = violations(&fixture("violations"), "DCA-ERR-010");
    let has = |text: &str| found.iter().any(|line| line.contains(text));
    assert_eq!(found.len(), 7, "{found:?}");
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
            "[DCA-MAP-013] alpha-domain (alpha-domain/Cargo.toml): alpha -> beta via events (acl)"
        ]
    );
}

/// DCA-CYC-002 slices the whole application crate, the shared module included; DCA-CYC-005 slices
/// the use cases and features only. DCA-CYC-001, 003 and 004 slice a domain crate's top-level
/// modules and the modules below the adapter sides.
#[test]
fn cycles_inside_a_crate_are_found_per_slice() {
    let ws = fixture("violations");
    let has = |id: &str, text: &str| violations(&ws, id).iter().any(|line| line.contains(text));
    assert!(has("DCA-CYC-002", "lookup -> shared -> lookup"));
    assert!(has("DCA-CYC-002", "do_thing -> other -> do_thing"));
    assert!(!has("DCA-CYC-005", "shared"));
    assert!(has("DCA-CYC-001", "left -> right -> left"));
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

/// The router presets: a GET handler that builds a command is reported, the POST handler beside it
/// and the conforming GET handler are not; a handler outside the incoming module is reported.
#[test]
fn handlers_are_found_through_the_router_presets() {
    let ws = fixture("violations");
    let hex_014 = violations(&ws, "DCA-HEX-014");
    assert_eq!(hex_014.len(), 1, "{hex_014:?}");
    assert!(hex_014[0].contains("`run` answers GET"), "{hex_014:?}");
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

/// DCA-STR-001 lists each context with its crates.
#[test]
fn the_contexts_are_listed_with_their_crates() {
    assert_eq!(
        violations(&fixture("conforming"), "DCA-STR-001"),
        vec![
            "[DCA-STR-001] workspace: context `alpha`: alpha-adapters (adapters), alpha-application (application), alpha-domain (domain)",
            "[DCA-STR-001] workspace: context `beta`: beta-api (api)",
        ]
    );
}
