//! Cycles between use-case or feature modules inside one application crate. Cargo refuses cycles
//! between crates; inside a crate modules may name each other both ways.

use std::collections::{BTreeMap, BTreeSet};

use super::at_line;
use crate::Violation;
use crate::model::{Layer, Workspace};

pub fn cyc_005(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        if c.layer != Some(Layer::Application) {
            continue;
        }
        let slice = |m: usize| -> Option<String> {
            let p = &ws.modules[m].path;
            (ws.modules[m].krate == k && !p.is_empty() && p[0] != "shared").then(|| p[0].clone())
        };
        let mut edges: BTreeMap<String, BTreeMap<String, (usize, usize)>> = BTreeMap::new();
        for m in ws.project_modules() {
            let Some(from) = slice(m) else { continue };
            for (t, line, _) in ws.module_targets(m) {
                let Some(tm) = ws.target_module(&t) else {
                    continue;
                };
                if let Some(to) = slice(tm)
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
                            let (m, line) = edges[&cycle[0]][&cycle[(1) % cycle.len()]];
                            let mut shown = cycle.clone();
                            shown.push(cycle[0].clone());
                            out.push(at_line(
                                ws,
                                "DCA-CYC-005",
                                m,
                                "",
                                line,
                                format!("use-case modules form a cycle: {}", shown.join(" -> ")),
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
