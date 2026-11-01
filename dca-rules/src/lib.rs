//! The Domain-Centric Architecture rule suite for Rust.
//!
//! The rules carry the same ids as the Java and .NET suites. What the compiler holds through the
//! crate graph and visibility needs no test; the rules here check what it does not: the crate graph
//! against the layers in `[package.metadata.dca]`, and the source with `syn`.
//!
//! An architecture test crate runs every implemented rule as its own test, named after its id:
//!
//! ```ignore
//! dca_rules::dca_tests!();
//! ```

#![forbid(unsafe_code)]

pub mod model;
pub mod rules;

use std::path::Path;
use std::sync::OnceLock;

pub use model::{Layer, Target, Workspace};

/// One finding of a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: &'static str,
    pub location: String,
    pub message: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.rule, self.location, self.message)
    }
}

/// A rule's entry in `rules.json`.
#[derive(Debug, Clone)]
pub struct CatalogueEntry {
    pub id: String,
    pub title: String,
    pub status: String,
    pub maturity: String,
}

const RULES_JSON: &str = include_str!("../rules.json");

/// Every DCA rule id with its Rust meaning, from `rules.json`.
pub fn catalogue() -> &'static [CatalogueEntry] {
    static CAT: OnceLock<Vec<CatalogueEntry>> = OnceLock::new();
    CAT.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(RULES_JSON).expect("rules.json is valid JSON");
        v["rules"]
            .as_array()
            .expect("rules.json has rules")
            .iter()
            .map(|r| CatalogueEntry {
                id: r["id"].as_str().unwrap_or_default().to_string(),
                title: r["title"].as_str().unwrap_or_default().to_string(),
                status: r["status"].as_str().unwrap_or("enforced").to_string(),
                maturity: r["maturity"].as_str().unwrap_or("released").to_string(),
            })
            .collect()
    })
}

pub fn entry(id: &str) -> Option<&'static CatalogueEntry> {
    catalogue().iter().find(|e| e.id == id)
}

/// What running one rule produced.
#[derive(Debug)]
pub enum Outcome {
    /// The rule ran; an informational rule lists its findings and never fails.
    Ran {
        violations: Vec<Violation>,
        informational: bool,
    },
    /// Experimental and not switched on in `[workspace.metadata.dca] experimental`.
    NotSwitchedOn,
    /// Listed in `[workspace.metadata.dca] disabled`.
    Disabled,
}

/// Runs one rule over a loaded workspace.
pub fn run(ws: &Workspace, id: &str) -> Outcome {
    let def = rules::find(id).unwrap_or_else(|| panic!("{id} is not implemented by dca-rules"));
    let entry = entry(id).unwrap_or_else(|| panic!("{id} is not in rules.json"));
    if ws.config.disabled.contains(id) {
        return Outcome::Disabled;
    }
    if entry.maturity == "experimental" {
        let on = match &ws.config.experimental {
            model::Experimental::All => true,
            model::Experimental::Only(ids) => ids.contains(id),
        };
        if !on {
            return Outcome::NotSwitchedOn;
        }
    }
    let mut violations = (def.check)(ws);
    violations.sort_by(|a, b| a.location.cmp(&b.location).then(a.message.cmp(&b.message)));
    violations.dedup();
    Outcome::Ran {
        violations,
        informational: entry.status == "informational",
    }
}

/// Loads the workspace around `dir` and fails with every violation of `id`, one line each.
pub fn assert_rule(dir: &str, id: &str) {
    let ws = Workspace::load(Path::new(dir))
        .unwrap_or_else(|e| panic!("[{id}] cannot load the workspace: {e}"));
    if !ws.problems.is_empty() {
        panic!(
            "[{id}] files the suite could not read:\n{}",
            ws.problems.join("\n")
        );
    }
    let title = entry(id).map(|e| e.title.as_str()).unwrap_or("");
    match run(&ws, id) {
        Outcome::Ran {
            violations,
            informational: true,
        } => {
            for v in &violations {
                println!("{v}");
            }
            println!(
                "[{id}] {title}: {} listed (informational, never fails)",
                violations.len()
            );
        }
        Outcome::Ran {
            violations,
            informational: false,
        } => {
            if !violations.is_empty() {
                let lines: Vec<String> = violations.iter().map(ToString::to_string).collect();
                panic!("[{id}] {title}\n{}\n", lines.join("\n"));
            }
        }
        Outcome::NotSwitchedOn => println!(
            "[{id}] experimental, not switched on in [workspace.metadata.dca] experimental"
        ),
        Outcome::Disabled => println!("[{id}] disabled in [workspace.metadata.dca] disabled"),
    }
}
