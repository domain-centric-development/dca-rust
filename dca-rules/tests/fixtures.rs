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
    role_of("Clock", "OutputPort");
}
