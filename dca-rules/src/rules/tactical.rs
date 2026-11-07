//! Tactical rules: aggregates, entities, values, repositories and stores.

use std::collections::BTreeMap;

use super::{
    at, field_label, field_targets, immutability, in_layer, is_kind, render, target_has_role,
    type_idents,
};
use crate::Violation;
use crate::model::{DeclKind, Layer, Target, Workspace};

fn field_holds(
    ws: &Workspace,
    rule: &'static str,
    d: usize,
    roles: &[&str],
    what: &str,
) -> Vec<Violation> {
    let mut out = vec![];
    for (f, targets) in field_targets(ws, d) {
        for t in &targets {
            if let Some(role) = roles.iter().find(|r| target_has_role(ws, t, r)) {
                out.push(at(
                    ws,
                    rule,
                    d,
                    format!(
                        "field `{}` holds `{}`, which carries the role {role}; {what}",
                        field_label(&f),
                        ws.target_name(t)
                    ),
                ));
                break;
            }
        }
    }
    out
}

pub fn tac_001(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Domain)
                && ws.decl_name(*d).ends_with("AggregateRoot")
                && !ws.has_role(*d, "AggregateRoot")
        })
        .map(|d| {
            at(
                ws,
                "DCA-TAC-001",
                d,
                format!(
                    "`{}` is named an aggregate root but does not implement AggregateRoot",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn tac_002(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("AggregateRoot")
        .flat_map(|d| {
            field_holds(
                ws,
                "DCA-TAC-002",
                d,
                &["OutputPort"],
                "an aggregate does not reach its own persistence; the use case does",
            )
        })
        .collect()
}

pub fn tac_003(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("AggregateRoot")
        .flat_map(|d| {
            field_holds(
                ws,
                "DCA-TAC-003",
                d,
                &["AggregateRoot"],
                "reference another aggregate by its id",
            )
        })
        .collect()
}

pub fn tac_004(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("Entity") {
        if !is_kind(ws, d, DeclKind::Struct) {
            continue;
        }
        let has_id = field_targets(ws, d)
            .iter()
            .any(|(_, ts)| ts.iter().any(|t| target_has_role(ws, t, "Id")));
        if !has_id {
            out.push(at(
                ws,
                "DCA-TAC-004",
                d,
                format!(
                    "entity `{}` has no field of a type that implements Id",
                    ws.decl_name(d)
                ),
            ));
        }
    }
    out
}

pub fn tac_005(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .types_with_role("Entity")
        .filter(|d| !ws.has_role(*d, "AggregateRoot"))
    {
        for f in ws.fields(d) {
            if f.public {
                out.push(at(ws, "DCA-TAC-005", d, format!("entity `{}` has the pub field `{}`, so code outside the aggregate can build it; keep fields private", ws.decl_name(d), f.name)));
            }
        }
        for m in ws.methods(d) {
            if m.inherent && m.public && !m.has_receiver() && m.returns_self() {
                out.push(at(ws, "DCA-TAC-005", d, format!("entity `{}` is built by the pub function `{}`; make it pub(crate) so only the aggregate creates it", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

fn is_setter(m: &crate::model::MethodRef<'_>) -> bool {
    m.name.starts_with("set_")
        && m.receiver_mut()
        && m.params().len() == 1
        && matches!(m.sig.output, syn::ReturnType::Default)
}

pub fn tac_006(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("Entity") {
        for m in ws.methods(d) {
            if m.inherent && m.public && is_setter(&m) {
                out.push(at(ws, "DCA-TAC-006", d, format!("`{}` has the pub setter `{}`; name the operation after what it does in the domain", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

pub fn tac_007(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Entity")
        .filter(|d| !ws.has_role(*d, "AggregateRoot"))
        .flat_map(|d| {
            field_holds(
                ws,
                "DCA-TAC-007",
                d,
                &["AggregateRoot"],
                "reference the aggregate by its id",
            )
        })
        .collect()
}

pub fn tac_008(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Value")
        .flat_map(|d| {
            field_holds(
                ws,
                "DCA-TAC-008",
                d,
                &["AggregateRoot", "Entity"],
                "a value holds values and ids, not entities",
            )
        })
        .collect()
}

pub fn tac_010(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Value")
        .filter(|d| !ws.is_building_block_decl(*d))
        .flat_map(|d| immutability(ws, "DCA-TAC-010", d, "value", true))
        .collect()
}

pub fn tac_011(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .types_with_role("Value")
        .filter(|d| !ws.is_building_block_decl(*d))
    {
        for m in ws.methods(d) {
            if m.name.starts_with("set_") {
                out.push(at(
                    ws,
                    "DCA-TAC-011",
                    d,
                    format!("value `{}` has the setter `{}`", ws.decl_name(d), m.name),
                ));
            }
        }
    }
    out
}

pub fn tac_013(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Application)
                && ws.decl_name(*d).ends_with("Repository")
                && !ws.has_role(*d, "Repository")
        })
        .map(|d| {
            at(
                ws,
                "DCA-TAC-013",
                d,
                format!(
                    "trait `{}` is named a repository but does not extend Repository<T>",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn tac_014(ws: &Workspace) -> Vec<Violation> {
    ws.traits_with_role("Repository")
        .filter(|d| !in_layer(ws, *d, Layer::Application))
        .map(|d| {
            at(
                ws,
                "DCA-TAC-014",
                d,
                format!(
                    "repository `{}` is declared outside the application crate",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

fn outside_outgoing(
    ws: &Workspace,
    rule: &'static str,
    role: &'static str,
    what: &str,
) -> Vec<Violation> {
    ws.types_with_role(role)
        .filter(|d| !ws.is_building_block_decl(*d) && !ws.decl_in_side(*d, "outgoing"))
        .map(|d| {
            at(
                ws,
                rule,
                d,
                format!(
                    "`{}` implements a {what} outside the outgoing adapters",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn tac_015(ws: &Workspace) -> Vec<Violation> {
    outside_outgoing(ws, "DCA-TAC-015", "Repository", "repository")
}

/// The aggregate a repository trait binds through `Repository<T>`.
fn bound_aggregate(ws: &Workspace, d: usize) -> Option<(String, Option<usize>)> {
    let syn::Item::Trait(t) = ws.item(d) else {
        return None;
    };
    let m = ws.decls[d].module;
    for b in &t.supertraits {
        let syn::TypeParamBound::Trait(tb) = b else {
            continue;
        };
        let Some(target) = ws.resolve_path(m, &tb.path) else {
            continue;
        };
        if ws.building_block(&target).as_deref() != Some("Repository") {
            if let Target::Decl(sd) = target
                && ws.decls[sd].kind == DeclKind::Trait
                && sd != d
                && !ws.is_building_block_decl(sd)
                && let Some(found) = bound_aggregate(ws, sd)
            {
                return Some(found);
            }
            continue;
        }
        let last = tb.path.segments.last()?;
        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
            for a in &args.args {
                if let syn::GenericArgument::Type(ty) = a {
                    let decls = ws.type_decls_in(m, ty, &[&t.generics]);
                    let name = type_idents(ty).last().cloned().unwrap_or_default();
                    return Some((name, decls.first().copied()));
                }
            }
        }
    }
    None
}

pub fn tac_016(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .traits_with_role("Repository")
        .filter(|d| ws.decl_name(*d).ends_with("Repository"))
    {
        let Some((name, decl)) = bound_aggregate(ws, d) else {
            continue;
        };
        if let Some(a) = decl
            && !ws.has_role(a, "AggregateRoot")
        {
            out.push(at(
                ws,
                "DCA-TAC-016",
                d,
                format!(
                    "repository `{}` binds `{name}`, which is no aggregate root",
                    ws.decl_name(d)
                ),
            ));
            continue;
        }
        let expected = format!("{name}Repository");
        if ws.decl_name(d) != expected {
            out.push(at(
                ws,
                "DCA-TAC-016",
                d,
                format!(
                    "repository `{}` binds `{name}`; name it `{expected}`",
                    ws.decl_name(d)
                ),
            ));
        }
    }
    out
}

pub fn tac_017(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.traits_with_role("Repository") {
        let g = super::type_generics(ws, d);
        for m in ws.trait_methods(d) {
            let syn::ReturnType::Type(_, ty) = &m.sig.output else {
                continue;
            };
            for e in ws.type_decls_in(m.module, ty, &[g[0], &m.sig.generics]) {
                if ws.has_role(e, "Entity") && !ws.has_role(e, "AggregateRoot") {
                    out.push(at(
                        ws,
                        "DCA-TAC-017",
                        d,
                        format!(
                            "`{}` returns `{}`, an entity inside an aggregate; return the root",
                            m.name,
                            ws.decl_name(e)
                        ),
                    ));
                }
            }
        }
    }
    out
}

pub fn tac_018(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| {
            ws.decl_name(*d).ends_with("Store")
                && !matches!(
                    ws.layer_of_decl(*d),
                    Some(Layer::Adapters | Layer::Infrastructure)
                )
        })
        .filter(|d| !ws.has_role(*d, "Store") || ws.has_role(*d, "Repository"))
        .map(|d| {
            at(
                ws,
                "DCA-TAC-018",
                d,
                format!(
                    "store `{}` must extend Store and not Repository",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn tac_019(ws: &Workspace) -> Vec<Violation> {
    ws.traits_with_role("Store")
        .filter(|d| !in_layer(ws, *d, Layer::Application))
        .map(|d| {
            at(
                ws,
                "DCA-TAC-019",
                d,
                format!(
                    "store `{}` is declared outside the application crate",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn tac_020(ws: &Workspace) -> Vec<Violation> {
    outside_outgoing(ws, "DCA-TAC-020", "Store", "store")
}

pub fn tac_021(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.traits_with_role("Store") {
        for m in ws.trait_methods(d) {
            if matches!(m.name.as_str(), "save" | "delete" | "delete_by_id") {
                out.push(at(
                    ws,
                    "DCA-TAC-021",
                    d,
                    format!(
                        "store `{}` declares `{}`; a store reads and appends, a repository saves",
                        ws.decl_name(d),
                        m.name
                    ),
                ));
            }
        }
    }
    out
}

pub fn tac_023(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("AggregateRoot") {
        for f in ws.fields(d) {
            if f.public {
                out.push(at(ws, "DCA-TAC-023", d, format!("aggregate root `{}` has the pub field `{}`; a pub field opens the struct literal and every invariant", ws.decl_name(d), f.name)));
            }
        }
    }
    out
}

pub fn tac_024(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .types_with_role("Value")
        .filter(|d| !ws.is_building_block_decl(*d))
    {
        let factories: Vec<_> = ws
            .methods(d)
            .into_iter()
            .filter(|m| m.inherent && m.public && !m.has_receiver() && m.returns_self())
            .collect();
        let sig = |m: &crate::model::MethodRef<'_>| {
            m.params().iter().map(|t| render(t)).collect::<Vec<_>>()
        };
        for n in factories.iter().filter(|m| m.name == "new") {
            for o in factories.iter().filter(|m| m.name != "new") {
                if sig(n) == sig(o) {
                    out.push(at(ws, "DCA-TAC-024", d, format!("value `{}` has `new` beside `{}` with the same parameters; name each after where the input comes from", ws.decl_name(d), o.name)));
                }
            }
        }
    }
    out
}

const BARE: &[&str] = &[
    "String",
    "str",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "f32",
    "f64",
    "bool",
    "char",
    "Uuid",
    "SystemTime",
    "Instant",
    "NaiveDate",
    "NaiveDateTime",
    "NaiveTime",
    "OffsetDateTime",
    "PrimitiveDateTime",
    "Date",
    "Time",
];

fn bare_in(ty: &syn::Type) -> Option<String> {
    type_idents(ty)
        .into_iter()
        .find(|i| BARE.contains(&i.as_str()))
}

fn is_bool(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p) if p.path.is_ident("bool"))
}

pub fn tac_025(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.type_decls().filter(|d| {
        matches!(
            ws.layer_of_decl(*d),
            Some(Layer::Domain | Layer::SharedKernel)
        )
    }) {
        let wrapper = (ws.has_role(d, "Value") || ws.has_role(d, "Id")) && ws.fields(d).len() == 1;
        if wrapper || ws.has_role(d, "DomainFailure") || ws.has_role(d, "UseCaseFailure") {
            continue;
        }
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();
        for f in ws.fields(d) {
            // A single yes/no fact: the type is told to change it and answers it with a predicate.
            // A `pub` struct field is one anyone may set - a flag argument in another spelling.
            if is_bool(f.ty) {
                if f.public && f.variant.is_none() {
                    out.push(at(
                        ws,
                        "DCA-TAC-025",
                        d,
                        format!(
                            "field `{}` is a public `bool` anyone may set - a flag argument; keep it private and name the behaviour that changes it",
                            field_label(&f)
                        ),
                    ));
                }
                continue;
            }
            if let Some(b) = bare_in(f.ty) {
                let msg = format!(
                    "field `{}` is a bare `{b}`; give it a value type that says what it is",
                    field_label(&f)
                );
                if seen.insert(msg.clone(), ()).is_none() {
                    out.push(at(ws, "DCA-TAC-025", d, msg));
                }
            }
        }
        for m in ws.methods(d).into_iter().filter(|m| m.inherent) {
            for p in m.params() {
                if let Some(b) = bare_in(p) {
                    out.push(at(
                        ws,
                        "DCA-TAC-025",
                        d,
                        format!("`{}` takes a bare `{b}`", m.name),
                    ));
                }
            }
            if let syn::ReturnType::Type(_, ty) = &m.sig.output
                && !is_bool(ty)
                && let Some(b) = bare_in(ty)
            {
                out.push(at(
                    ws,
                    "DCA-TAC-025",
                    d,
                    format!("`{}` returns a bare `{b}`", m.name),
                ));
            }
        }
    }
    out
}
