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

#[test]
fn no_rule_finds_anything_in_the_conforming_fixture() {
    let ws = fixture("conforming");
    let found: Vec<String> = rules::implemented()
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

/// DCA-ERR-008 asks a closed result for a variant carrying a Refusal; one that has it passes.
#[test]
fn a_closed_result_needs_a_refusing_variant() {
    let ws = fixture("results");
    let listed = violations(&ws, "DCA-ERR-008");
    assert!(
        listed.iter().any(|line| line.contains("`CheckPartResult`")),
        "unit variants, none marked: {listed:?}"
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
    assert_eq!(lamp.len(), 2, "{lamp:?}");
    assert!(
        lamp.iter()
            .any(|line| line.contains("`switch` takes a bare `bool`")),
        "{lamp:?}"
    );
    assert!(
        lamp.iter()
            .any(|line| line.contains("is a public `bool` anyone may set")),
        "{lamp:?}"
    );
    assert!(!lamp.iter().any(|line| line.contains("lit`")), "{lamp:?}");
}
