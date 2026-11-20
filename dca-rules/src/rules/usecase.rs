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

/// DCA-USE-021: a use case holds no state it changes - no `&mut self` method, no interior
/// mutability in a field.
pub fn use_021(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("InputPort")
        .filter(|d| !ws.is_building_block_decl(*d))
        .flat_map(|d| immutability(ws, "DCA-USE-021", d, "use case", false))
        .collect()
}

/// The success type a use case answers with: `T` of `Result<T, _>`, else the type itself.
fn success(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::Path(tp) = ty
        && let Some(last) = tp.path.segments.last()
        && last.ident == "Result"
        && let syn::PathArguments::AngleBracketed(args) = &last.arguments
        && let Some(syn::GenericArgument::Type(t)) = args.args.first()
    {
        return t;
    }
    ty
}

/// The primitive a type is, when it is `String`, `str` (through references), `bool`, `char` or a
/// numeric primitive.
fn primitive(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(r) => primitive(&r.elem),
        syn::Type::Paren(p) => primitive(&p.elem),
        syn::Type::Path(tp) if tp.qself.is_none() && tp.path.segments.len() == 1 => {
            let id = tp.path.segments[0].ident.to_string();
            const NAMES: &[&str] = &[
                "String", "str", "bool", "char", "i8", "i16", "i32", "i64", "i128", "isize", "u8",
                "u16", "u32", "u64", "u128", "usize", "f32", "f64",
            ];
            (NAMES.contains(&id.as_str()) && tp.path.segments[0].arguments.is_none()).then_some(id)
        }
        _ => None,
    }
}

/// DCA-USE-022: a use case answers with a result type, not a bare primitive - the `Output` of an
/// `impl UseCase<I>` block or of a trait's `UseCase<I, Output = ..>` bound, or the success type of
/// the `Result` it names.
pub fn use_022(ws: &Workspace) -> Vec<Violation> {
    let is_use_case = |t: &Option<Target>| {
        t.as_ref()
            .is_some_and(|t| ws.building_block(t).as_deref() == Some("UseCase"))
    };
    let mut out = vec![];
    let report = |m: usize, name: &str, line: usize, prim: String, out: &mut Vec<Violation>| {
        out.push(super::at_line(ws, "DCA-USE-022", m, name, line, format!(
            "`{name}` answers with the bare `{prim}`; answer with a result type that names what the use case found or did"
        )));
    };
    for imp in &ws.impls {
        if !is_use_case(&imp.trait_target) {
            continue;
        }
        let syn::Item::Impl(item) = &ws.modules[imp.module].items[imp.item] else {
            continue;
        };
        for ii in &item.items {
            if let syn::ImplItem::Type(t) = ii
                && t.ident == "Output"
                && let Some(prim) = primitive(success(&t.ty))
            {
                let name = imp
                    .self_decl
                    .map(|d| ws.decl_name(d).to_string())
                    .unwrap_or_default();
                report(
                    imp.module,
                    &name,
                    t.ident.span().start().line,
                    prim,
                    &mut out,
                );
            }
        }
    }
    for d in ws.trait_decls() {
        let syn::Item::Trait(t) = ws.item(d) else {
            continue;
        };
        let m = ws.decls[d].module;
        for bound in &t.supertraits {
            let syn::TypeParamBound::Trait(tb) = bound else {
                continue;
            };
            if !is_use_case(&ws.resolve_path(m, &tb.path)) {
                continue;
            }
            let Some(last) = tb.path.segments.last() else {
                continue;
            };
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                for arg in &args.args {
                    if let syn::GenericArgument::AssocType(b) = arg
                        && b.ident == "Output"
                        && let Some(prim) = primitive(success(&b.ty))
                    {
                        report(m, ws.decl_name(d), ws.decls[d].line, prim, &mut out);
                    }
                }
            }
        }
    }
    out
}

/// Whether an input-port trait takes a command: the input of a `UseCase<..>` bound, or a parameter
/// of one of its methods, is a type whose name ends with `Command`.
fn takes_command(ws: &Workspace, d: usize) -> bool {
    let syn::Item::Trait(t) = ws.item(d) else {
        return false;
    };
    let in_bound = t.supertraits.iter().any(|b| {
        let syn::TypeParamBound::Trait(tb) = b else {
            return false;
        };
        tb.path.segments.last().is_some_and(|s| {
            matches!(&s.arguments, syn::PathArguments::AngleBracketed(a)
                if a.args.iter().any(|g| matches!(g, syn::GenericArgument::Type(ty)
                    if type_idents(ty).iter().any(|i| i.ends_with("Command")))))
        })
    });
    in_bound
        || ws.trait_methods(d).iter().any(|m| {
            m.params()
                .iter()
                .any(|p| type_idents(p).iter().any(|i| i.ends_with("Command")))
        })
}

/// DCA-USE-018 (experimental): an incoming adapter's item drives the commands of one feature only.
/// A feature is the top-level module of an application crate that groups use cases
/// (`application::<feature>::<use_case>`); an input port declared at that depth or below belongs to
/// it, one declared directly in a top-level module to none.
pub fn use_018(ws: &Workspace) -> Vec<Violation> {
    let feature_of = |d: usize| -> Option<String> {
        let p = &ws.modules[ws.decls[d].module].path;
        (in_layer(ws, d, Layer::Application)
            && p.len() >= 2
            && p[0] != "shared"
            && ws.has_role(d, "InputPort")
            && takes_command(ws, d))
        .then(|| p[0].clone())
    };
    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_in_side(*m, "incoming"))
    {
        for item in &ws.modules[m].items {
            let mut c = crate::model::PathCollector::default();
            syn::visit::Visit::visit_item(&mut c, item);
            let features: BTreeSet<String> = c
                .paths
                .iter()
                .filter_map(|p| match ws.resolve_path(m, p) {
                    Some(Target::Decl(d)) if ws.decls[d].kind == DeclKind::Trait => feature_of(d),
                    _ => None,
                })
                .collect();
            if features.len() > 1 {
                let line = syn::spanned::Spanned::span(item).start().line;
                out.push(super::at_line(ws, "DCA-USE-018", m, "", line, format!(
                    "drives the commands of the features {} in one item - give each feature its own adapter",
                    features.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")
                )));
            }
        }
    }
    out
}
