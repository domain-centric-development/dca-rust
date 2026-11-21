//! Cycles between the slices of one crate. Cargo refuses cycles between crates; inside a crate
//! modules may name each other both ways, so each layer's crate is sliced by its modules.

use std::collections::{BTreeMap, BTreeSet};

use super::at_line;
use crate::Violation;
use crate::model::{Layer, Workspace};

/// Every dependency cycle between the slices of each crate of `layer`; `slice` names the slice a
/// module path of the crate belongs to, if any. One finding per cycle, at the first edge. An edge
/// between a slice and a slice nested in it (a feature and an operation below it) does not count:
/// a module re-exports what its submodules declare, and they use what it declares.
fn cycles(
    ws: &Workspace,
    rule: &'static str,
    layer: Layer,
    what: &str,
    slice: impl Fn(usize, &[String]) -> Option<String>,
) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        if c.layer != Some(layer) {
            continue;
        }
        let slice_of = |m: usize| -> Option<String> {
            (ws.modules[m].krate == k).then(|| slice(k, &ws.modules[m].path))?
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
                    && !nested(&from, &to)
                    && !nested(&to, &from)
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

/// Whether slice `inner` is nested in slice `outer`.
fn nested(outer: &str, inner: &str) -> bool {
    inner
        .strip_prefix(outer)
        .is_some_and(|rest| rest.starts_with("::"))
}

/// The top-level module a path lies in.
fn top(_: usize, p: &[String]) -> Option<String> {
    p.first().cloned()
}

/// The module directly below the adapter side `side` a path lies in.
fn below<'a>(ws: &'a Workspace, side: &str) -> impl Fn(usize, &[String]) -> Option<String> + 'a {
    let module = ws.side_module(side).to_string();
    move |_, p: &[String]| (p.len() >= 2 && p[0] == module).then(|| format!("{module}::{}", p[1]))
}

pub(super) fn cyc_001(ws: &Workspace) -> Vec<Violation> {
    cycles(ws, "DCA-CYC-001", Layer::Domain, "domain modules", top)
}

pub(super) fn cyc_002(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-002",
        Layer::Application,
        "application modules",
        top,
    )
}

pub(super) fn cyc_003(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-003",
        Layer::Adapters,
        "outgoing adapter modules",
        below(ws, "outgoing"),
    )
}

pub(super) fn cyc_004(ws: &Workspace) -> Vec<Violation> {
    cycles(
        ws,
        "DCA-CYC-004",
        Layer::Adapters,
        "incoming adapter modules",
        below(ws, "incoming"),
    )
}

/// DCA-CYC-005: one slice per operation root - the deepest module holding an operation (a struct or
/// enum of the application crate with the input-port role or named `...UseCase`) - with the
/// configured operation containers skipped in its name; a module below it joins it. A module on
/// the way to an operation that holds none itself is the feature slice of the first module below
/// the containers. The shared module and the crate root belong to no slice.
pub(super) fn cyc_005(ws: &Workspace) -> Vec<Violation> {
    let containers = &ws.config.operation_containers;
    let mut roots: BTreeMap<usize, BTreeSet<Vec<String>>> = BTreeMap::new();
    for d in ws.type_decls() {
        if ws.is_building_block_decl(d)
            || ws.layer_of_decl(d) != Some(Layer::Application)
            || !(ws.has_role(d, "InputPort")
                || ws.decl_name(d).ends_with(&ws.config.suffixes.use_case))
        {
            continue;
        }
        let m = ws.decls[d].module;
        roots
            .entry(ws.modules[m].krate)
            .or_default()
            .insert(ws.modules[m].path.clone());
    }
    let slice = |k: usize, p: &[String]| -> Option<String> {
        let index = p.iter().take_while(|s| containers.contains(*s)).count();
        if index == p.len() || p[index] == ws.config.modules.shared {
            return None;
        }
        let ops = roots.get(&k)?;
        let root = ops
            .iter()
            .filter(|r| !r.is_empty() && p.starts_with(r))
            .max_by_key(|r| r.len())
            .cloned()
            .or_else(|| {
                let feature = &p[..=index];
                ops.iter()
                    .any(|r| r.len() > feature.len() && r.starts_with(feature))
                    .then(|| feature.to_vec())
            })?;
        Some(
            root.iter()
                .filter(|s| !containers.contains(*s))
                .cloned()
                .collect::<Vec<_>>()
                .join("::"),
        )
    };
    cycles(
        ws,
        "DCA-CYC-005",
        Layer::Application,
        "use-case and feature modules",
        slice,
    )
}
