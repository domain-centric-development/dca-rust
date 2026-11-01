//! Use-case rules: commands, queries and results, the use case's shape, its writes and transaction.

use std::collections::BTreeSet;

use super::{
    at, calls, field_roles, field_targets, immutability, in_layer, target_has_role, type_idents,
    use_cases,
};
use crate::Violation;
use crate::model::{DeclKind, Layer, Target, Workspace};

pub fn use_001(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| ws.decl_name(*d) == "InputPort")
        .map(|d| {
            at(
                ws,
                "DCA-USE-001",
                d,
                "the InputPort contract is declared again; use the building block's",
            )
        })
        .collect()
}

fn named_outside(
    ws: &Workspace,
    rule: &'static str,
    suffix: &str,
    layer: Layer,
    what: &str,
) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| ws.decl_name(*d).ends_with(suffix) && !in_layer(ws, *d, layer))
        .map(|d| {
            at(
                ws,
                rule,
                d,
                format!("`{}` sits outside the {what}", ws.decl_name(d)),
            )
        })
        .collect()
}

pub fn use_002(ws: &Workspace) -> Vec<Violation> {
    named_outside(
        ws,
        "DCA-USE-002",
        "Command",
        Layer::Application,
        "application crate",
    )
}

pub fn use_003(ws: &Workspace) -> Vec<Violation> {
    named_outside(
        ws,
        "DCA-USE-003",
        "Query",
        Layer::Application,
        "application crate",
    )
}

fn immutable_named(ws: &Workspace, rule: &'static str, suffix: &str) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| {
            ws.decls[*d].kind == DeclKind::Struct
                && in_layer(ws, *d, Layer::Application)
                && ws.decl_name(*d).ends_with(suffix)
        })
        .flat_map(|d| immutability(ws, rule, d, &suffix.to_lowercase(), false))
        .collect()
}

pub fn use_004(ws: &Workspace) -> Vec<Violation> {
    immutable_named(ws, "DCA-USE-004", "Command")
}

pub fn use_005(ws: &Workspace) -> Vec<Violation> {
    immutable_named(ws, "DCA-USE-005", "Query")
}

pub fn use_006(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| {
            ws.decl_name(*d).ends_with("Result")
                && !ws.has_role(*d, "Value")
                && !in_layer(ws, *d, Layer::Application)
        })
        .filter(|d| !(ws.decls[*d].kind == DeclKind::Enum && in_layer(ws, *d, Layer::Domain)))
        .map(|d| {
            at(
                ws,
                "DCA-USE-006",
                d,
                format!(
                    "result `{}` sits outside the application crate",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn use_007(ws: &Workspace) -> Vec<Violation> {
    immutable_named(ws, "DCA-USE-007", "Result")
}

pub fn use_008(ws: &Workspace) -> Vec<Violation> {
    named_outside(
        ws,
        "DCA-USE-008",
        "Response",
        Layer::Adapters,
        "adapters crate",
    )
}

/// Method names of a use case whose body holds a call matching `pred`, closed over self-calls.
fn reaches(ws: &Workspace, d: usize, pred: &dyn Fn(&super::Call) -> bool) -> BTreeSet<String> {
    let methods = ws.methods(d);
    let mut hit: BTreeSet<String> = BTreeSet::new();
    for m in &methods {
        if let Some(b) = m.block
            && calls(b).iter().any(pred)
        {
            hit.insert(m.name.clone());
        }
    }
    loop {
        let before = hit.len();
        for m in &methods {
            if let Some(b) = m.block
                && calls(b)
                    .iter()
                    .any(|c| c.self_field.is_none() && hit.contains(&c.method))
            {
                hit.insert(m.name.clone());
            }
        }
        if hit.len() == before {
            return hit;
        }
    }
}

pub fn use_009(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in use_cases(ws) {
        let roles = field_roles(ws, d);
        let is = |c: &super::Call, role: &str| {
            c.self_field
                .as_ref()
                .is_some_and(|f| roles.get(f).is_some_and(|r| r.contains(role)))
        };
        let saves = reaches(ws, d, &|c| c.method == "save" && is(c, "Repository"));
        let publishes = reaches(ws, d, &|c| {
            c.method == "publish_and_clear_events" && is(c, "DomainEventPublisher")
        });
        for m in ws.methods(d).into_iter().filter(|m| m.public) {
            if saves.contains(&m.name) && !publishes.contains(&m.name) {
                out.push(at(
                    ws,
                    "DCA-USE-009",
                    d,
                    format!(
                        "`{}` saves an aggregate but never calls publish_and_clear_events",
                        m.name
                    ),
                ));
            }
        }
    }
    out
}

fn dto_in(ws: &Workspace, rule: &'static str, layer: Layer) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .type_decls()
        .filter(|d| in_layer(ws, *d, layer) && ws.decl_name(*d).ends_with("Dto"))
    {
        out.push(at(
            ws,
            rule,
            d,
            format!("`{}` is a DTO; DTOs belong to adapters", ws.decl_name(d)),
        ));
    }
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) == Some(layer))
    {
        for (t, line, text) in ws.module_targets(m) {
            if ws.target_name(&t).ends_with("Dto") {
                out.push(super::at_line(
                    ws,
                    rule,
                    m,
                    "",
                    line,
                    format!("names the DTO `{text}`"),
                ));
            }
        }
    }
    out
}

pub fn use_010(ws: &Workspace) -> Vec<Violation> {
    dto_in(ws, "DCA-USE-010", Layer::Domain)
}

pub fn use_011(ws: &Workspace) -> Vec<Violation> {
    dto_in(ws, "DCA-USE-011", Layer::Application)
}

pub fn use_012(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in use_cases(ws) {
        let roles = field_roles(ws, d);
        let is_write = |c: &super::Call| {
            let Some(f) = &c.self_field else { return false };
            let Some(r) = roles.get(f) else { return false };
            (r.contains("Repository") && matches!(c.method.as_str(), "save" | "delete_by_id"))
                || (r.contains("DomainEventPublisher")
                    && matches!(c.method.as_str(), "publish" | "publish_and_clear_events"))
        };
        let methods = ws.methods(d);
        // Helpers called inside a transaction closure, and helpers called outside one.
        let mut inside: BTreeSet<String> = BTreeSet::new();
        let mut outside: BTreeSet<String> = BTreeSet::new();
        for m in &methods {
            if let Some(b) = m.block {
                for c in calls(b).iter().filter(|c| c.self_field.is_none()) {
                    if c.in_transaction {
                        inside.insert(c.method.clone());
                    } else {
                        outside.insert(c.method.clone());
                    }
                }
            }
        }
        for m in &methods {
            let Some(b) = m.block else { continue };
            let covered_helper =
                !m.public && inside.contains(&m.name) && !outside.contains(&m.name);
            for c in calls(b).iter().filter(|c| is_write(c) && !c.in_transaction) {
                if !covered_helper {
                    out.push(at(ws, "DCA-USE-012", d, format!(
                        "`{}` calls `{}` on `{}` outside a transaction; run it inside TransactionBoundary::in_transaction",
                        m.name, c.method, c.self_field.clone().unwrap_or_default()
                    )));
                }
            }
        }
    }
    out
}

pub fn use_014(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (k, c) in ws.crates.iter().enumerate() {
        if c.layer != Some(Layer::Application) {
            continue;
        }
        let ucs: Vec<usize> = use_cases(ws)
            .into_iter()
            .filter(|d| ws.modules[ws.decls[*d].module].krate == k)
            .filter(|d| {
                ws.modules[ws.decls[*d].module]
                    .path
                    .first()
                    .map(String::as_str)
                    != Some("shared")
            })
            .collect();
        let depths: BTreeSet<usize> = ucs
            .iter()
            .map(|d| ws.modules[ws.decls[*d].module].path.len())
            .collect();
        for d in &ucs {
            let depth = ws.modules[ws.decls[*d].module].path.len();
            if depth == 0 || depth > 2 {
                out.push(at(ws, "DCA-USE-014", *d, format!("use case `{}` sits at module depth {depth}; use application::<use_case> or application::<feature>::<use_case>", ws.decl_name(*d))));
            }
        }
        if depths.contains(&1) && depths.contains(&2) {
            for d in &ucs {
                out.push(at(
                    ws,
                    "DCA-USE-014",
                    *d,
                    format!(
                        "use case `{}`: the crate mixes flat and grouped use-case modules",
                        ws.decl_name(*d)
                    ),
                ));
            }
        }
    }
    out
}

pub fn use_015(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.type_decls().filter(|d| {
        in_layer(ws, *d, Layer::Application)
            && ws.decl_name(*d).ends_with("Result")
            && !ws.has_role(*d, "Value")
    }) {
        let mut seen = BTreeSet::new();
        let mut stack = vec![d];
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur) {
                continue;
            }
            for (f, targets) in field_targets(ws, cur) {
                for t in &targets {
                    if target_has_role(ws, t, "AggregateRoot") || target_has_role(ws, t, "Entity") {
                        out.push(at(
                            ws,
                            "DCA-USE-015",
                            d,
                            format!(
                                "result `{}` exposes `{}` through `{}`; return values and ids",
                                ws.decl_name(d),
                                ws.target_name(t),
                                f.name
                            ),
                        ));
                    } else if let Target::Decl(n) = t
                        && in_layer(ws, *n, Layer::Application)
                        && matches!(ws.decls[*n].kind, DeclKind::Struct | DeclKind::Enum)
                    {
                        stack.push(*n);
                    }
                }
            }
        }
    }
    out
}

pub fn use_016(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in use_cases(ws) {
        let own: BTreeSet<usize> = ws.implements[d]
            .iter()
            .filter_map(|t| match t {
                Target::Decl(x) => Some(*x),
                _ => None,
            })
            .collect();
        for (f, targets) in field_targets(ws, d) {
            for t in targets {
                if !target_has_role(ws, &t, "InputPort") {
                    continue;
                }
                if let Target::Decl(x) = t
                    && (own.contains(&x) || x == d)
                {
                    continue;
                }
                if ws.building_block(&t).is_some() {
                    continue;
                }
                out.push(at(ws, "DCA-USE-016", d, format!("use case `{}` holds the input port `{}` in `{}`; a use case does not call another", ws.decl_name(d), ws.target_name(&t), f.name)));
            }
        }
    }
    out
}

pub fn use_017(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in use_cases(ws) {
        for m in ws.methods(d).into_iter().filter(|m| m.inherent && m.public) {
            if !m.has_receiver() && m.returns_self() {
                continue;
            }
            out.push(at(
                ws,
                "DCA-USE-017",
                d,
                format!(
                    "use case `{}` has the pub method `{}` outside its input port",
                    ws.decl_name(d),
                    m.name
                ),
            ));
        }
    }
    out
}

const WRITE_VERBS: &[&str] = &[
    "save", "delete", "remove", "add", "insert", "update", "upsert", "put", "store", "write",
    "create", "append", "archive", "purge", "clear", "set", "mark",
];

pub fn use_019(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in use_cases(ws) {
        let query = ws.methods(d).iter().any(|m| {
            m.public
                && m.params()
                    .iter()
                    .any(|p| type_idents(p).iter().any(|i| i.ends_with("Query")))
        });
        if !query {
            continue;
        }
        let roles = field_roles(ws, d);
        for m in ws.methods(d) {
            let Some(b) = m.block else { continue };
            for c in calls(b) {
                let Some(r) = c.self_field.as_ref().and_then(|f| roles.get(f)) else {
                    continue;
                };
                let write = (r.contains("Repository") || r.contains("Store"))
                    && WRITE_VERBS
                        .iter()
                        .any(|v| c.method == *v || c.method.starts_with(&format!("{v}_")));
                let publish =
                    r.contains("DomainEventPublisher") || r.contains("IntegrationEventPublisher");
                if write || publish {
                    out.push(at(
                        ws,
                        "DCA-USE-019",
                        d,
                        format!(
                            "query use case `{}` calls `{}` in `{}`; a query reads only",
                            ws.decl_name(d),
                            c.method,
                            m.name
                        ),
                    ));
                }
            }
        }
    }
    out
}
