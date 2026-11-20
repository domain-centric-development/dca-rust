//! Cycles between the slices of one crate. Cargo refuses cycles between crates; inside a crate
//! modules may name each other both ways, so each layer's crate is sliced by its modules.

use std::collections::{BTreeMap, BTreeSet};

use super::at_line;
use crate::Violation;
use crate::model::{Layer, Workspace};

/// Every dependency cycle between the slices of each crate of `layer`; `slice` names the slice a
/// module path of the crate belongs to, if any. One finding per cycle, at the first edge.
fn cycles(
    ws: &Workspace,
    rule: &'static str,
    layer: Layer,
    what: &str,
    slice: impl Fn(&[String]) -> Option<String>,
) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        if c.layer != Some(layer) {
            continue;
        }
        let slice_of = |m: usize| -> Option<String> {
            (ws.modules[m].krate == k).then(|| slice(&ws.modules[m].path))?
        };
        let mut edges: BTreeMap<String, BTreeMap<String, (usize, usize)>> = BTreeMap::new();
        for m in ws.project_modules() {
            let Some(from) = slice_of(m) else { continue };
            for (t, line, _) in ws.module_targets(m) {
                let Some(tm) = ws.target_module(&t) else {
                    continue;
                };
                if let Some(to) = slice_of(tm)
                    && to != from
                {
                    edges
                        .entry(from.clone())
                        .or_default()
                        .entry(to)
                        .or_insert((m, line));
                }
            }
        }
        let mut reported: BTreeSet<Vec<String>> = BTreeSet::new();
        for start in edges.keys() {
            // Depth-first search for a path back to `start`.
            let mut stack = vec![(start.clone(), vec![start.clone()])];
            while let Some((node, path)) = stack.pop() {
                for next in edges
                    .get(&node)
                    .map(|e| e.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
                {
                    if next == *start {
                        let mut cycle = path.clone();
                        let min = cycle
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, s)| (*s).clone())
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        cycle.rotate_left(min);
                        if reported.insert(cycle.clone()) {
                            let (m, line) = edges[&cycle[0]][&cycle[1 % cycle.len()]];
                            let mut shown = cycle.clone();
                            shown.push(cycle[0].clone());
                            out.push(at_line(
                                ws,
                                rule,
                                m,
                                "",
                                line,
                                format!("{what} form a cycle: {}", shown.join(" -> ")),
                            ));
                        }
                    } else if !path.contains(&next) {
                        let mut p = path.clone();
                        p.push(next.clone());
                        stack.push((next, p));
                    }
                }
            }
        }
    }
    out
}

/// The top-level module a path lies in.
fn top(p: &[String]) -> Option<String> {
    p.first().cloned()
}

/// The module directly below the adapter side `side` a path lies in.
fn below(side: &str) -> impl Fn(&[String]) -> Option<String> + '_ {
    move |p: &[String]| (p.len() >= 2 && p[0] == side).then(|| format!("{side}::{}", p[1]))
}

pub fn cyc_001(ws: &Workspace) -> Vec<Violation> {
    cycles(ws, "DCA-CYC-001", Layer::Domain, "domain modules", top)
}

pub fn cyc_002(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-002",
        Layer::Application,
        "application modules",
        top,
    )
}

pub fn cyc_003(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-003",
        Layer::Adapters,
        "outgoing adapter modules",
        below("outgoing"),
    )
}

pub fn cyc_004(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-004",
        Layer::Adapters,
        "incoming adapter modules",
        below("incoming"),
    )
}

pub fn cyc_005(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-005",
        Layer::Application,
        "use-case modules",
        |p: &[String]| top(p).filter(|s| s != "shared"),
    )
}
