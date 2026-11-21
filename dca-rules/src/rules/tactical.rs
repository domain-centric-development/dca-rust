//! Tactical rules: aggregates, entities, values, repositories and stores.

use std::collections::BTreeMap;

use super::{
    at, field_label, field_targets, immutability, in_layer, is_kind, target_has_role, type_idents,
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

pub(super) fn tac_001(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Domain)
                && ws
                    .decl_name(*d)
                    .ends_with(&ws.config.suffixes.aggregate_root)
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

pub(super) fn tac_002(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_003(ws: &Workspace) -> Vec<Violation> {
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

/// The type an entity's impl of `Entity` binds to `Id`, with the module of that impl.
fn entity_id_type(ws: &Workspace, d: usize) -> Option<(usize, &syn::Type)> {
    ws.impls
        .iter()
        .filter(|i| i.self_decl == Some(d))
        .filter(|i| {
            i.trait_target
                .as_ref()
                .is_some_and(|t| ws.building_block(t).as_deref() == Some("Entity"))
        })
        .find_map(|i| {
            let syn::Item::Impl(item) = &ws.modules[i.module].items[i.item] else {
                return None;
            };
            item.items.iter().find_map(|it| match it {
                syn::ImplItem::Type(t) if t.ident == "Id" => Some((i.module, &t.ty)),
                _ => None,
            })
        })
}

pub(super) fn tac_004(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("Entity") {
        if ws.is_building_block_decl(d) || is_kind(ws, d, DeclKind::Trait) {
            continue;
        }
        let m = ws.decls[d].module;
        let id = entity_id_type(ws, d).map(|(im, ty)| super::type_key(ws, im, ty));
        // A field is the id when its type, references stripped, is the type bound to `Id`; without
        // a readable binding, any field whose type names a type with the Id role.
        let is_id = |f: &crate::model::FieldRef<'_>, targets: &[Target]| match &id {
            Some(key) => &super::type_key(ws, m, f.ty) == key,
            None => targets.iter().any(|t| target_has_role(ws, t, "Id")),
        };
        let fields = field_targets(ws, d);
        let what = match entity_id_type(ws, d) {
            Some((_, ty)) => format!("of its id type `{}`", super::render(ty)),
            None => "of a type that implements Id".to_string(),
        };
        match ws.item(d) {
            syn::Item::Enum(e) => {
                for v in &e.variants {
                    let name = v.ident.to_string();
                    // The id may sit in the state a tuple variant wraps: a struct of the
                    // workspace with a field of the id type.
                    let wrapped = |ts: &[Target]| {
                        ts.iter().any(|t| match t {
                            Target::Decl(inner)
                                if matches!(ws.item(*inner), syn::Item::Struct(_)) =>
                            {
                                let im = ws.decls[*inner].module;
                                ws.fields(*inner).iter().any(|f| match &id {
                                    Some(key) => &super::type_key(ws, im, f.ty) == key,
                                    None => false,
                                })
                            }
                            _ => false,
                        })
                    };
                    let has = fields.iter().any(|(f, ts)| {
                        f.variant.as_deref() == Some(name.as_str()) && (is_id(f, ts) || wrapped(ts))
                    });
                    if !has {
                        out.push(at(
                            ws,
                            "DCA-TAC-004",
                            d,
                            format!(
                                "entity `{}` has no field {what} in its variant `{name}`",
                                ws.decl_name(d)
                            ),
                        ));
                    }
                }
            }
            _ => {
                if !fields.iter().any(|(f, ts)| is_id(f, ts)) {
                    out.push(at(
                        ws,
                        "DCA-TAC-004",
                        d,
                        format!("entity `{}` has no field {what}", ws.decl_name(d)),
                    ));
                }
            }
        }
    }
    out
}

/// Whether a variant of a `pub` enum can be built wherever the enum is visible: a unit variant or
/// one with named fields always can; a tuple variant can unless every field it wraps is a struct of
/// the workspace whose fields are all private - a state that only its own module builds.
fn variant_open(ws: &Workspace, d: usize, v: &syn::Variant) -> bool {
    let syn::Fields::Unnamed(fields) = &v.fields else {
        return true;
    };
    let m = ws.decls[d].module;
    fields.unnamed.iter().any(|f| {
        let sealed = match &f.ty {
            syn::Type::Path(tp) if tp.qself.is_none() => match ws.resolve_path(m, &tp.path) {
                Some(Target::Decl(inner)) => {
                    matches!(ws.item(inner), syn::Item::Struct(_))
                        && !ws.fields(inner).is_empty()
                        && ws.fields(inner).iter().all(|f| !f.public)
                }
                _ => false,
            },
            _ => false,
        };
        !sealed
    })
}

/// The ways a type can be built wherever it is visible without a function of its own that names
/// the way: an impl of `Default`, `From<..>` or `TryFrom<..>` that is no trait of the workspace, a
/// `#[derive(Default)]`, and - for a `pub` enum - each variant that is open (see `variant_open`).
fn unnamed_ways(ws: &Workspace, d: usize) -> Vec<String> {
    const CONVERSIONS: &[&str] = &["Default", "From", "TryFrom"];
    let mut out = vec![];
    let attrs: &[syn::Attribute] = match ws.item(d) {
        syn::Item::Struct(s) => &s.attrs,
        syn::Item::Enum(e) => {
            if matches!(e.vis, syn::Visibility::Public(_)) {
                for v in e.variants.iter().filter(|v| variant_open(ws, d, v)) {
                    out.push(format!("its variant `{}`, as a pub enum", v.ident));
                }
            }
            &e.attrs
        }
        _ => return out,
    };
    let derives_default = attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .filter_map(|a| {
            a.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .ok()
        })
        .flatten()
        .any(|p| p.segments.last().is_some_and(|s| s.ident == "Default"));
    if derives_default {
        out.push("`#[derive(Default)]`".to_string());
    }
    for imp in ws.impls.iter().filter(|i| i.self_decl == Some(d)) {
        let syn::Item::Impl(item) = &ws.modules[imp.module].items[imp.item] else {
            continue;
        };
        let Some((None, path, _)) = &item.trait_ else {
            continue;
        };
        let Some(last) = path.segments.last() else {
            continue;
        };
        // The prelude's traits resolve to nothing, or - behind a glob import of another crate -
        // to that crate; a trait of the workspace with the same name is not std's.
        let std_trait = !matches!(&imp.trait_target, Some(Target::Decl(_) | Target::Module(_)));
        if std_trait && CONVERSIONS.contains(&last.ident.to_string().as_str()) {
            let args = if matches!(last.arguments, syn::PathArguments::None) {
                ""
            } else {
                "<..>"
            };
            out.push(format!("`impl {}{args}`", last.ident));
        }
    }
    out
}

pub(super) fn tac_005(ws: &Workspace) -> Vec<Violation> {
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
        for way in unnamed_ways(ws, d) {
            out.push(at(ws, "DCA-TAC-005", d, format!("entity `{}` can be built from outside the aggregate through {way}; let only the aggregate create it", ws.decl_name(d))));
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

pub(super) fn tac_006(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("Entity") {
        for m in super::own_methods(ws, d, true) {
            if m.public && is_setter(&m) {
                out.push(at(ws, "DCA-TAC-006", d, format!("`{}` has the pub setter `{}`; name the operation after what it does in the domain", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

pub(super) fn tac_007(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_008(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_010(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Value")
        .filter(|d| !ws.is_building_block_decl(*d))
        .flat_map(|d| immutability(ws, "DCA-TAC-010", d, "value", true))
        .collect()
}

pub(super) fn tac_011(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_013(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Application)
                && ws.decl_name(*d).ends_with(&ws.config.suffixes.repository)
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

pub(super) fn tac_014(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_015(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_016(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .traits_with_role("Repository")
        .filter(|d| ws.decl_name(*d).ends_with(&ws.config.suffixes.repository))
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
        let expected = format!("{name}{}", ws.config.suffixes.repository);
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

pub(super) fn tac_017(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_018(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| {
            ws.decl_name(*d).ends_with(&ws.config.suffixes.store)
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

pub(super) fn tac_019(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_020(ws: &Workspace) -> Vec<Violation> {
    outside_outgoing(ws, "DCA-TAC-020", "Store", "store")
}

pub(super) fn tac_021(ws: &Workspace) -> Vec<Violation> {
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

pub(super) fn tac_023(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("AggregateRoot") {
        for f in ws.fields(d) {
            if f.public {
                out.push(at(ws, "DCA-TAC-023", d, format!("aggregate root `{}` has the pub field `{}`; a pub field opens the struct literal and every invariant", ws.decl_name(d), f.name)));
            }
        }
        for way in unnamed_ways(ws, d) {
            out.push(at(ws, "DCA-TAC-023", d, format!("aggregate root `{}` can be built through {way}, an unnamed way past its creation and reconstitution functions; name the way in a function of its own", ws.decl_name(d))));
        }
    }
    out
}

pub(super) fn tac_024(ws: &Workspace) -> Vec<Violation> {
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
            m.params()
                .iter()
                .map(|t| super::type_key(ws, m.module, t))
                .collect::<Vec<_>>()
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

/// What a function answers: the return type, or the `Output` of a returned `impl Future`.
fn answered(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::ImplTrait(it) = ty {
        for b in &it.bounds {
            if let syn::TypeParamBound::Trait(tb) = b
                && let Some(last) = tb.path.segments.last()
                && last.ident == "Future"
                && let syn::PathArguments::AngleBracketed(args) = &last.arguments
            {
                for a in &args.args {
                    if let syn::GenericArgument::AssocType(at) = a
                        && at.ident == "Output"
                    {
                        return answered(&at.ty);
                    }
                }
            }
        }
    }
    ty
}

/// A variant name, lower-cased and without underscores, for comparing two of them.
fn plain(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether two names are one fact and its negation: a fixed pair, or a name and the name with a
/// negating prefix. `not`, `no` and `non` negate any name; `un` only an adjective-like one (ending
/// in -ed, -able, -ible, -ive, -ent, -ant, -al or -ful): `Locked`/`Unlocked` is a fact and its
/// negation, `Lock`/`Unlock` are two actions.
fn negation_pair(a: &str, b: &str) -> bool {
    const PAIRS: &[(&str, &str)] = &[
        ("yes", "no"),
        ("true", "false"),
        ("on", "off"),
        ("present", "absent"),
        ("enabled", "disabled"),
    ];
    const ADJECTIVE_ENDINGS: &[&str] = &["ed", "able", "ible", "ive", "ent", "ant", "al", "ful"];
    let (a, b) = (plain(a), plain(b));
    let negates = |x: &str, y: &str| {
        ["not", "no", "non"]
            .iter()
            .any(|p| y.strip_prefix(p) == Some(x))
            || (y.strip_prefix("un") == Some(x) && ADJECTIVE_ENDINGS.iter().any(|e| x.ends_with(e)))
    };
    PAIRS
        .iter()
        .any(|(x, y)| (a == *x && b == *y) || (a == *y && b == *x))
        || negates(&a, &b)
        || negates(&b, &a)
}

/// The two variant names when an enum is a boolean in disguise: exactly two fieldless variants that
/// name one fact and its negation.
fn boolean_in_disguise(ws: &Workspace, d: usize) -> Option<(String, String)> {
    let syn::Item::Enum(e) = ws.item(d) else {
        return None;
    };
    let names: Vec<String> = e
        .variants
        .iter()
        .filter(|v| matches!(v.fields, syn::Fields::Unit))
        .map(|v| v.ident.to_string())
        .collect();
    (e.variants.len() == 2 && names.len() == 2 && negation_pair(&names[0], &names[1]))
        .then(|| (names[0].clone(), names[1].clone()))
}

/// The bare values in a signature: each parameter and what it answers.
fn bare_signature(sig: &syn::Signature, rebuilds: bool) -> Vec<String> {
    let mut out = vec![];
    for input in &sig.inputs {
        let syn::FnArg::Typed(p) = input else {
            continue;
        };
        if rebuilds && is_bool(&p.ty) {
            continue;
        }
        if let Some(b) = bare_in(&p.ty) {
            out.push(format!("`{}` takes a bare `{b}`", sig.ident));
        }
    }
    if let syn::ReturnType::Type(_, ty) = &sig.output {
        let ty = answered(ty);
        if !is_bool(ty)
            && let Some(b) = bare_in(ty)
        {
            out.push(format!("`{}` returns a bare `{b}`", sig.ident));
        }
    }
    out
}

/// Whether a method's return type names the type itself, `Self` aside.
pub(super) fn returns_decl(m: &crate::model::MethodRef<'_>, name: &str) -> bool {
    match &m.sig.output {
        syn::ReturnType::Type(_, ty) => type_idents(ty).iter().any(|i| i == name),
        syn::ReturnType::Default => false,
    }
}

pub(super) fn tac_025(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .type_decls()
        .filter(|d| matches!(ws.layer_of_decl(*d), Some(Layer::Domain)))
    {
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
        if let Some((a, b)) = boolean_in_disguise(ws, d) {
            out.push(at(ws, "DCA-TAC-025", d, format!(
                "enum `{}` names one fact and its negation (`{a}`, `{b}`) - a boolean in disguise; keep the fact a private bool field, or name the two states for what they are",
                ws.decl_name(d)
            )));
        }
        for m in ws.methods(d).into_iter().filter(|m| m.inherent) {
            // The factory that rebuilds a stored value hands its yes/no fact back: its `bool` is no
            // flag argument. An associated function without a receiver that returns the type,
            // named as configured.
            let rebuilds = !m.has_receiver()
                && (m.returns_self() || returns_decl(&m, &ws.decls[d].name))
                && ws
                    .config
                    .reconstituting_factories
                    .contains(&m.name.to_lowercase());
            for msg in bare_signature(m.sig, rebuilds) {
                out.push(at(ws, "DCA-TAC-025", d, msg));
            }
        }
    }
    // The traits the domain declares - gateways, specifications of its own, any other - are checked
    // where they are declared; their implementations repeat the signature.
    for d in ws
        .trait_decls()
        .filter(|d| matches!(ws.layer_of_decl(*d), Some(Layer::Domain)))
    {
        for m in ws.trait_methods(d) {
            for msg in bare_signature(m.sig, false) {
                out.push(at(ws, "DCA-TAC-025", d, msg));
            }
        }
    }
    out
}

/// The type an associated type binds in the `impl <role> for <decl>` block, e.g. `type OwnerId = X`.
fn associated_type<'a>(
    ws: &'a Workspace,
    d: usize,
    role: &str,
    name: &str,
) -> Option<(usize, &'a syn::Type)> {
    for i in ws.impls.iter().filter(|i| i.self_decl == Some(d)) {
        let Some(target) = &i.trait_target else {
            continue;
        };
        if ws.building_block(target).as_deref() != Some(role) {
            continue;
        }
        let syn::Item::Impl(block) = &ws.modules[i.module].items[i.item] else {
            continue;
        };
        for item in &block.items {
            if let syn::ImplItem::Type(t) = item
                && t.ident == name
            {
                return Some((i.module, &t.ty));
            }
        }
    }
    None
}

/// The decl a type names at its outside, references and parentheses looked through.
fn outer_decl(ws: &Workspace, m: usize, ty: &syn::Type) -> Option<usize> {
    match ty {
        syn::Type::Reference(r) => outer_decl(ws, m, &r.elem),
        syn::Type::Paren(p) => outer_decl(ws, m, &p.elem),
        syn::Type::Path(tp) if tp.qself.is_none() => match ws.resolve_path(m, &tp.path)? {
            Target::Decl(d) => Some(d),
            _ => None,
        },
        _ => None,
    }
}

/// DCA-TAC-026 (experimental). An aggregate that belongs to someone is found through its owner: its
/// repository offers a lookup by the aggregate's id and the owner's id.
pub(super) fn tac_026(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    let repositories: Vec<usize> = ws.traits_with_role("Repository").collect();
    for d in ws
        .type_decls()
        .filter(|d| ws.has_role(*d, "Owned") && ws.has_role(*d, "AggregateRoot"))
    {
        let owner =
            associated_type(ws, d, "Owned", "OwnerId").and_then(|(m, ty)| outer_decl(ws, m, ty));
        let Some(owner) = owner else {
            out.push(at(
                ws,
                "DCA-TAC-026",
                d,
                format!(
                    "`{}` is owned but its `impl Owned` names no owner id type the suite can resolve; bind `type OwnerId` to the owner's id type",
                    ws.decl_name(d)
                ),
            ));
            continue;
        };
        let aggregate_id =
            associated_type(ws, d, "Entity", "Id").and_then(|(m, ty)| outer_decl(ws, m, ty));
        for r in repositories
            .iter()
            .copied()
            .filter(|r| matches!(bound_aggregate(ws, *r), Some((_, Some(a))) if a == d))
        {
            let scoped = ws.trait_methods(r).iter().any(|m| {
                let syn::ReturnType::Type(_, ret) = &m.sig.output else {
                    return false;
                };
                let g = m.generics();
                if !ws.type_decls_in(m.module, ret, &g).contains(&d) {
                    return false;
                }
                let params: Vec<Option<usize>> = m
                    .params()
                    .into_iter()
                    .map(|p| outer_decl(ws, m.module, p))
                    .collect();
                let Some(at_owner) = params.iter().position(|p| *p == Some(owner)) else {
                    return false;
                };
                params.iter().enumerate().any(|(i, p)| {
                    i != at_owner
                        && match (p, aggregate_id) {
                            (Some(p), Some(id)) => *p == id,
                            (Some(p), None) => ws.has_role(*p, "Id"),
                            _ => false,
                        }
                })
            });
            if !scoped {
                out.push(at(
                    ws,
                    "DCA-TAC-026",
                    r,
                    format!(
                        "repository `{}` declares no lookup taking `{}` and `{}` for the owned aggregate `{}`; add one such as find_by_id_for_owner(id, owner) and load the aggregate through it",
                        ws.decl_name(r),
                        aggregate_id.map_or("the aggregate's id", |id| ws.decl_name(id)),
                        ws.decl_name(owner),
                        ws.decl_name(d)
                    ),
                ));
            }
        }
    }
    out
}

/// The types of the model: aggregates, entities, values, factories and specifications. A type that
/// itself implements DomainGateway is none of them.
fn model_types(ws: &Workspace) -> Vec<(usize, &'static str)> {
    let mut out = vec![];
    for d in ws
        .type_decls()
        .filter(|d| !ws.has_role(*d, "DomainGateway"))
    {
        let kind = if ws.has_role(d, "AggregateRoot") {
            "aggregate"
        } else if ws.has_role(d, "Entity") {
            "entity"
        } else if ws.has_role(d, "Value") {
            "value"
        } else if ws.has_role(d, "Factory") {
            "factory"
        } else {
            continue;
        };
        out.push((d, kind));
    }
    for d in super::events::specifications(ws) {
        if !ws.has_role(d, "DomainGateway") && !out.iter().any(|(x, _)| *x == d) {
            out.push((d, "specification"));
        }
    }
    out
}

/// The paths a body uses as values - expression paths, called functions, struct literals - and the
/// names it binds (parameters, `let`, closure and match bindings), which shadow the module's items.
/// A qualified path `<T as Trait>::function` counts by its trait's path. Written-out locals and
/// typed parameters are kept with their types, and every field read (`ctx.rates`).
#[derive(Default)]
struct ExprPaths {
    paths: Vec<syn::Path>,
    locals: std::collections::BTreeSet<String>,
    typed: std::collections::BTreeMap<String, syn::Type>,
    fields: Vec<syn::ExprField>,
}

impl<'ast> syn::visit::Visit<'ast> for ExprPaths {
    fn visit_expr_path(&mut self, e: &'ast syn::ExprPath) {
        if e.qself.as_ref().is_none_or(|q| q.position > 0) {
            self.paths.push(e.path.clone());
        }
        syn::visit::visit_expr_path(self, e);
    }

    fn visit_pat_type(&mut self, p: &'ast syn::PatType) {
        if let Some(name) = super::pat_ident(&p.pat) {
            self.typed.insert(name, (*p.ty).clone());
        }
        syn::visit::visit_pat_type(self, p);
    }

    fn visit_expr_field(&mut self, e: &'ast syn::ExprField) {
        self.fields.push(e.clone());
        syn::visit::visit_expr_field(self, e);
    }

    fn visit_expr_struct(&mut self, e: &'ast syn::ExprStruct) {
        self.paths.push(e.path.clone());
        syn::visit::visit_expr_struct(self, e);
    }

    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        self.locals.insert(p.ident.to_string());
        syn::visit::visit_pat_ident(self, p);
    }
}

/// The gateway a type expression names, read in module `m`.
fn gateway_in(
    ws: &Workspace,
    m: usize,
    ty: &syn::Type,
    generics: &[&syn::Generics],
) -> Option<Target> {
    ws.type_targets(m, ty, generics)
        .into_iter()
        .find(|t| target_has_role(ws, t, "DomainGateway"))
}

/// The gateway a function returns, if its declared return type names one.
fn returns_gateway(
    ws: &Workspace,
    m: usize,
    sig: &syn::Signature,
    generics: &[&syn::Generics],
) -> Option<Target> {
    match &sig.output {
        syn::ReturnType::Type(_, ty) => gateway_in(ws, m, ty, generics),
        syn::ReturnType::Default => None,
    }
}

/// How a path in a model method's body obtains a gateway without receiving it, if it does: it
/// constructs an implementation, calls a function or an associated function that returns one (a
/// registry or locator) - of a type, of a trait, or of a generic parameter through its bounds - or
/// reads a static or a const of a gateway type.
fn hidden_gateway(
    ws: &Workspace,
    m: usize,
    path: &syn::Path,
    generics: &[&syn::Generics],
) -> Option<String> {
    if path.segments.len() > 1 {
        let last = path.segments.last()?.ident.to_string();
        let mut prefix = path.clone();
        prefix.segments.pop();
        prefix.segments.pop_punct();
        let ty = syn::Type::Path(syn::TypePath {
            qself: None,
            path: prefix,
        });
        for t in ws.type_targets(m, &ty, generics) {
            let Target::Decl(t) = t else { continue };
            if ws.decls[t].kind != DeclKind::Trait {
                continue;
            }
            for method in ws.trait_methods(t) {
                if method.name != last || method.has_receiver() {
                    continue;
                }
                let g = method.generics();
                if let Some(gw) = returns_gateway(ws, method.module, method.sig, &g) {
                    return Some(format!(
                        "obtains the gateway `{}` from `{}::{last}`",
                        ws.target_name(&gw),
                        ws.decl_name(t)
                    ));
                }
            }
        }
    }
    let Some(Target::Decl(d)) = ws.resolve_path(m, path) else {
        return None;
    };
    let dm = ws.decls[d].module;
    let last = path.segments.last()?.ident.to_string();
    match ws.decls[d].kind {
        DeclKind::Struct | DeclKind::Enum if ws.has_role(d, "DomainGateway") => Some(format!(
            "constructs the gateway implementation `{}`",
            ws.decl_name(d)
        )),
        DeclKind::Struct | DeclKind::Enum if path.segments.len() > 1 => {
            let method = ws
                .methods(d)
                .into_iter()
                .find(|mm| mm.name == last && !mm.has_receiver())?;
            let g = method.generics();
            returns_gateway(ws, method.module, method.sig, &g).map(|t| {
                format!(
                    "obtains the gateway `{}` from `{}::{last}`",
                    ws.target_name(&t),
                    ws.decl_name(d)
                )
            })
        }
        DeclKind::Fn => {
            let syn::Item::Fn(f) = ws.item(d) else {
                return None;
            };
            returns_gateway(ws, dm, &f.sig, &[&f.sig.generics]).map(|t| {
                format!(
                    "obtains the gateway `{}` from `{}()`",
                    ws.target_name(&t),
                    ws.decl_name(d)
                )
            })
        }
        DeclKind::Const | DeclKind::Static => {
            let ty = match ws.item(d) {
                syn::Item::Const(c) => &c.ty,
                syn::Item::Static(s) => &s.ty,
                _ => return None,
            };
            gateway_in(ws, dm, ty, &[]).map(|t| {
                format!(
                    "reads the gateway `{}` from `{}`",
                    ws.target_name(&t),
                    ws.decl_name(d)
                )
            })
        }
        _ => None,
    }
}

/// The gateway a field read hands out when the value it is read from has a known type - a
/// parameter or a local whose type is written out, or a field of `self` - and that type is a
/// struct of the workspace whose field holds a gateway: `ctx.rates`, `self.context.rates`. A
/// gateway field of `self` itself is the holding finding, not this one.
fn gateway_field_read(
    ws: &Workspace,
    d: usize,
    m: usize,
    read: &syn::ExprField,
    typed: &std::collections::BTreeMap<String, syn::Type>,
    generics: &[&syn::Generics],
) -> Option<(Target, String)> {
    let syn::Member::Named(member) = &read.member else {
        return None;
    };
    let (holders, base): (Vec<usize>, String) = match &*read.base {
        syn::Expr::Path(p) if !p.path.is_ident("self") => {
            let name = p.path.get_ident()?.to_string();
            let ty = typed.get(&name)?;
            (ws.type_decls_in(m, ty, generics), name)
        }
        syn::Expr::Field(f) => {
            let (syn::Expr::Path(p), syn::Member::Named(n)) = (&*f.base, &f.member) else {
                return None;
            };
            if !p.path.is_ident("self") {
                return None;
            }
            let field = ws.fields(d).into_iter().find(|fl| *n == fl.name)?;
            let g = super::type_and_impl_generics(ws, d);
            (
                ws.type_decls_in(ws.decls[d].module, field.ty, &g),
                format!("self.{n}"),
            )
        }
        _ => return None,
    };
    for h in holders {
        if ws.decls[h].kind != DeclKind::Struct {
            continue;
        }
        let Some(field) = ws.fields(h).into_iter().find(|fl| *member == fl.name) else {
            continue;
        };
        let g = super::type_generics(ws, h);
        if let Some(gw) = gateway_in(ws, ws.decls[h].module, field.ty, &g) {
            return Some((gw, format!("{base}.{member}")));
        }
    }
    None
}

/// DCA-TAC-027 (experimental): an aggregate, entity, value, factory or specification receives a
/// domain gateway only as a parameter. It holds none in a field, returns none from a method, and
/// obtains none in a method's body - by constructing an implementation, calling a registry or
/// locator, or reading a static or a const. A parameter of a gateway type and the calls made on it
/// pass.
pub(super) fn tac_027(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (d, kind) in model_types(ws) {
        let name = ws.decl_name(d).to_string();
        let dm = ws.decls[d].module;
        let type_generics = super::type_generics(ws, d);
        for f in ws.fields(d) {
            if let Some(t) = gateway_in(ws, dm, f.ty, &type_generics) {
                out.push(at(ws, "DCA-TAC-027", d, format!(
                    "{kind} `{name}` holds the domain gateway `{}` in `{}`; take it as a parameter of the method that needs it",
                    ws.target_name(&t),
                    field_label(&f)
                )));
            }
        }
        for method in ws.methods(d) {
            let mut generics = method.generics();
            generics.extend(type_generics.iter().copied());
            if let Some(t) = returns_gateway(ws, method.module, method.sig, &generics) {
                out.push(at(ws, "DCA-TAC-027", d, format!(
                    "{kind} `{name}` hands out the domain gateway `{}` from `{}`; the model receives a gateway, it never provides one",
                    ws.target_name(&t),
                    method.name
                )));
            }
            let Some(block) = method.block else { continue };
            let mut v = ExprPaths::default();
            for input in &method.sig.inputs {
                syn::visit::Visit::visit_fn_arg(&mut v, input);
            }
            syn::visit::Visit::visit_block(&mut v, block);
            let mut seen = std::collections::BTreeSet::new();
            let locals = &v.locals;
            for p in v.paths.iter().filter(|p| {
                !(p.segments.len() == 1 && locals.contains(&p.segments[0].ident.to_string()))
            }) {
                if let Some(how) = hidden_gateway(ws, method.module, p, &generics)
                    && seen.insert(how.clone())
                {
                    out.push(at(ws, "DCA-TAC-027", d, format!(
                        "{kind} `{name}` {how} in `{}`; take the gateway as a parameter instead",
                        method.name
                    )));
                }
            }
            for read in &v.fields {
                if let Some((gw, text)) =
                    gateway_field_read(ws, d, method.module, read, &v.typed, &generics)
                    && seen.insert(text.clone())
                {
                    out.push(at(ws, "DCA-TAC-027", d, format!(
                        "{kind} `{name}` reads the gateway `{}` from `{text}` in `{}`; take the gateway as a parameter instead",
                        ws.target_name(&gw),
                        method.name
                    )));
                }
            }
        }
    }
    out
}
