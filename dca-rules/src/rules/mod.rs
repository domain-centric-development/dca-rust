//! The implemented rules and what they share.

mod context_map;
mod cycles;
mod errors;
mod events;
mod graph;
mod hexagonal;
mod layered;
mod naming;
mod rust;
mod strategic;
mod tactical;
mod usecase;

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::visit::Visit;

use crate::Violation;
use crate::model::{DeclKind, FieldRef, Layer, MethodRef, Target, Workspace};

/// A rule the suite implements.
#[derive(Debug)]
pub struct RuleDef {
    pub id: &'static str,
    pub check: fn(&Workspace) -> Vec<Violation>,
}

pub fn find(id: &str) -> Option<&'static RuleDef> {
    RULES.iter().find(|r| r.id == id)
}

/// The ids the suite implements.
pub fn implemented() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|r| r.id)
}

macro_rules! define_rules {
    ($d:tt; $( $test:ident => $id:literal => $check:path ),* $(,)?) => {
        pub static RULES: &[RuleDef] = &[ $( RuleDef { id: $id, check: $check } ),* ];

        /// One `#[test]` per implemented rule, named after its id, over the workspace the calling
        /// crate belongs to.
        #[macro_export]
        macro_rules! dca_tests {
            () => {
                $(
                    #[test]
                    fn $test() {
                        $d crate::assert_rule(env!("CARGO_MANIFEST_DIR"), $id);
                    }
                )*
            };
        }
    };
}

define_rules! { $;
    dca_lay_001 => "DCA-LAY-001" => layered::lay_001,
    dca_lay_002 => "DCA-LAY-002" => graph::lay_002,
    dca_lay_003 => "DCA-LAY-003" => graph::lay_003,
    dca_lay_004 => "DCA-LAY-004" => layered::lay_004,
    dca_lay_007 => "DCA-LAY-007" => layered::lay_007,
    dca_lay_008 => "DCA-LAY-008" => layered::lay_008,
    dca_oni_001 => "DCA-ONI-001" => graph::oni_001,
    dca_oni_002 => "DCA-ONI-002" => graph::oni_002,
    dca_oni_003 => "DCA-ONI-003" => events::oni_003,
    dca_oni_004 => "DCA-ONI-004" => layered::oni_004,
    dca_hex_001 => "DCA-HEX-001" => graph::hex_001,
    dca_hex_002 => "DCA-HEX-002" => graph::hex_002,
    dca_hex_003 => "DCA-HEX-003" => hexagonal::hex_003,
    dca_hex_004 => "DCA-HEX-004" => hexagonal::hex_004,
    dca_hex_005 => "DCA-HEX-005" => graph::hex_005,
    dca_hex_006 => "DCA-HEX-006" => hexagonal::hex_006,
    dca_hex_007 => "DCA-HEX-007" => hexagonal::hex_007,
    dca_hex_008 => "DCA-HEX-008" => hexagonal::hex_008,
    dca_hex_009 => "DCA-HEX-009" => hexagonal::hex_009,
    dca_hex_010 => "DCA-HEX-010" => hexagonal::hex_010,
    dca_hex_011 => "DCA-HEX-011" => hexagonal::hex_011,
    dca_hex_012 => "DCA-HEX-012" => hexagonal::hex_012,
    dca_hex_013 => "DCA-HEX-013" => hexagonal::hex_013,
    dca_hex_014 => "DCA-HEX-014" => hexagonal::hex_014,
    dca_hex_016 => "DCA-HEX-016" => hexagonal::hex_016,
    dca_hex_017 => "DCA-HEX-017" => hexagonal::hex_017,
    dca_hex_018 => "DCA-HEX-018" => hexagonal::hex_018,
    dca_hex_019 => "DCA-HEX-019" => hexagonal::hex_019,
    dca_hex_020 => "DCA-HEX-020" => hexagonal::hex_020,
    dca_hex_021 => "DCA-HEX-021" => hexagonal::hex_021,
    dca_tac_001 => "DCA-TAC-001" => tactical::tac_001,
    dca_tac_002 => "DCA-TAC-002" => tactical::tac_002,
    dca_tac_003 => "DCA-TAC-003" => tactical::tac_003,
    dca_tac_004 => "DCA-TAC-004" => tactical::tac_004,
    dca_tac_005 => "DCA-TAC-005" => tactical::tac_005,
    dca_tac_006 => "DCA-TAC-006" => tactical::tac_006,
    dca_tac_007 => "DCA-TAC-007" => tactical::tac_007,
    dca_tac_008 => "DCA-TAC-008" => tactical::tac_008,
    dca_tac_010 => "DCA-TAC-010" => tactical::tac_010,
    dca_tac_011 => "DCA-TAC-011" => tactical::tac_011,
    dca_tac_013 => "DCA-TAC-013" => tactical::tac_013,
    dca_tac_014 => "DCA-TAC-014" => tactical::tac_014,
    dca_tac_015 => "DCA-TAC-015" => tactical::tac_015,
    dca_tac_016 => "DCA-TAC-016" => tactical::tac_016,
    dca_tac_017 => "DCA-TAC-017" => tactical::tac_017,
    dca_tac_018 => "DCA-TAC-018" => tactical::tac_018,
    dca_tac_019 => "DCA-TAC-019" => tactical::tac_019,
    dca_tac_020 => "DCA-TAC-020" => tactical::tac_020,
    dca_tac_021 => "DCA-TAC-021" => tactical::tac_021,
    dca_tac_023 => "DCA-TAC-023" => tactical::tac_023,
    dca_tac_024 => "DCA-TAC-024" => tactical::tac_024,
    dca_tac_025 => "DCA-TAC-025" => tactical::tac_025,
    dca_tac_026 => "DCA-TAC-026" => tactical::tac_026,
    dca_tac_027 => "DCA-TAC-027" => tactical::tac_027,
    dca_str_001 => "DCA-STR-001" => graph::str_001,
    dca_str_002 => "DCA-STR-002" => graph::str_002,
    dca_str_003 => "DCA-STR-003" => graph::str_003,
    dca_str_004 => "DCA-STR-004" => graph::str_004,
    dca_str_005 => "DCA-STR-005" => strategic::str_005,
    dca_str_006 => "DCA-STR-006" => graph::str_006,
    dca_str_007 => "DCA-STR-007" => strategic::str_007,
    dca_str_008 => "DCA-STR-008" => strategic::str_008,
    dca_str_010 => "DCA-STR-010" => strategic::str_010,
    dca_str_011 => "DCA-STR-011" => graph::str_011,
    dca_str_012 => "DCA-STR-012" => graph::str_012,
    dca_str_013 => "DCA-STR-013" => strategic::str_013,
    dca_adv_001 => "DCA-ADV-001" => events::adv_001,
    dca_adv_002 => "DCA-ADV-002" => events::adv_002,
    dca_adv_004 => "DCA-ADV-004" => events::adv_004,
    dca_adv_006 => "DCA-ADV-006" => events::adv_006,
    dca_adv_007 => "DCA-ADV-007" => events::adv_007,
    dca_adv_008 => "DCA-ADV-008" => events::adv_008,
    dca_adv_010 => "DCA-ADV-010" => events::adv_010,
    dca_adv_011 => "DCA-ADV-011" => events::adv_011,
    dca_adv_012 => "DCA-ADV-012" => events::adv_012,
    dca_adv_013 => "DCA-ADV-013" => events::adv_013,
    dca_adv_014 => "DCA-ADV-014" => events::adv_014,
    dca_adv_015 => "DCA-ADV-015" => events::adv_015,
    dca_adv_016 => "DCA-ADV-016" => events::adv_016,
    dca_adv_017 => "DCA-ADV-017" => events::adv_017,
    dca_adv_018 => "DCA-ADV-018" => events::adv_018,
    dca_adv_019 => "DCA-ADV-019" => events::adv_019,
    dca_adv_020 => "DCA-ADV-020" => events::adv_020,
    dca_adv_021 => "DCA-ADV-021" => events::adv_021,
    dca_use_001 => "DCA-USE-001" => usecase::use_001,
    dca_use_002 => "DCA-USE-002" => usecase::use_002,
    dca_use_003 => "DCA-USE-003" => usecase::use_003,
    dca_use_004 => "DCA-USE-004" => usecase::use_004,
    dca_use_005 => "DCA-USE-005" => usecase::use_005,
    dca_use_006 => "DCA-USE-006" => usecase::use_006,
    dca_use_007 => "DCA-USE-007" => usecase::use_007,
    dca_use_008 => "DCA-USE-008" => usecase::use_008,
    dca_use_009 => "DCA-USE-009" => usecase::use_009,
    dca_use_010 => "DCA-USE-010" => usecase::use_010,
    dca_use_011 => "DCA-USE-011" => usecase::use_011,
    dca_use_012 => "DCA-USE-012" => usecase::use_012,
    dca_use_014 => "DCA-USE-014" => usecase::use_014,
    dca_use_015 => "DCA-USE-015" => usecase::use_015,
    dca_use_016 => "DCA-USE-016" => usecase::use_016,
    dca_use_017 => "DCA-USE-017" => usecase::use_017,
    dca_use_018 => "DCA-USE-018" => usecase::use_018,
    dca_use_019 => "DCA-USE-019" => usecase::use_019,
    dca_use_021 => "DCA-USE-021" => usecase::use_021,
    dca_use_022 => "DCA-USE-022" => usecase::use_022,
    dca_use_023 => "DCA-USE-023" => usecase::use_023,
    dca_nam_001 => "DCA-NAM-001" => naming::nam_001,
    dca_nam_003 => "DCA-NAM-003" => naming::nam_003,
    dca_nam_004 => "DCA-NAM-004" => naming::nam_004,
    dca_nam_007 => "DCA-NAM-007" => naming::nam_007,
    dca_nam_008 => "DCA-NAM-008" => naming::nam_008,
    dca_nam_009 => "DCA-NAM-009" => naming::nam_009,
    dca_nam_010 => "DCA-NAM-010" => naming::nam_010,
    dca_nam_011 => "DCA-NAM-011" => naming::nam_011,
    dca_nam_012 => "DCA-NAM-012" => naming::nam_012,
    dca_nam_013 => "DCA-NAM-013" => naming::nam_013,
    dca_map_001 => "DCA-MAP-001" => context_map::map_001,
    dca_map_002 => "DCA-MAP-002" => context_map::map_002,
    dca_map_004 => "DCA-MAP-004" => context_map::map_004,
    dca_map_005 => "DCA-MAP-005" => context_map::map_005,
    dca_map_006 => "DCA-MAP-006" => context_map::map_006,
    dca_map_007 => "DCA-MAP-007" => context_map::map_007,
    dca_map_008 => "DCA-MAP-008" => context_map::map_008,
    dca_map_009 => "DCA-MAP-009" => context_map::map_009,
    dca_map_010 => "DCA-MAP-010" => context_map::map_010,
    dca_map_011 => "DCA-MAP-011" => context_map::map_011,
    dca_map_012 => "DCA-MAP-012" => context_map::map_012,
    dca_map_013 => "DCA-MAP-013" => context_map::map_013,
    dca_map_014 => "DCA-MAP-014" => context_map::map_014,
    dca_cyc_001 => "DCA-CYC-001" => cycles::cyc_001,
    dca_cyc_002 => "DCA-CYC-002" => cycles::cyc_002,
    dca_cyc_003 => "DCA-CYC-003" => cycles::cyc_003,
    dca_cyc_004 => "DCA-CYC-004" => cycles::cyc_004,
    dca_cyc_005 => "DCA-CYC-005" => cycles::cyc_005,
    dca_err_001 => "DCA-ERR-001" => errors::err_001,
    dca_err_002 => "DCA-ERR-002" => errors::err_002,
    dca_err_003 => "DCA-ERR-003" => errors::err_003,
    dca_err_004 => "DCA-ERR-004" => errors::err_004,
    dca_err_005 => "DCA-ERR-005" => errors::err_005,
    dca_err_006 => "DCA-ERR-006" => errors::err_006,
    dca_err_007 => "DCA-ERR-007" => errors::err_007,
    dca_err_008 => "DCA-ERR-008" => errors::err_008,
    dca_err_009 => "DCA-ERR-009" => errors::err_009,
    dca_err_010 => "DCA-ERR-010" => errors::err_010,
    dca_rs_001 => "DCA-RS-001" => rust::rs_001,
    dca_rs_002 => "DCA-RS-002" => rust::rs_002,
    dca_rs_003 => "DCA-RS-003" => rust::rs_003,
    dca_rs_004 => "DCA-RS-004" => rust::rs_004,
    dca_rs_006 => "DCA-RS-006" => rust::rs_006,
    dca_rs_007 => "DCA-RS-007" => rust::rs_007,
    dca_rs_008 => "DCA-RS-008" => rust::rs_008,
}

// --- shared helpers -------------------------------------------------------------------------------

pub(crate) fn at(
    ws: &Workspace,
    rule: &'static str,
    d: usize,
    message: impl Into<String>,
) -> Violation {
    Violation {
        rule,
        location: ws.where_decl(d),
        message: message.into(),
    }
}

pub(crate) fn at_line(
    ws: &Workspace,
    rule: &'static str,
    m: usize,
    name: &str,
    line: usize,
    message: impl Into<String>,
) -> Violation {
    Violation {
        rule,
        location: ws.where_in(m, name, line),
        message: message.into(),
    }
}

pub(crate) fn in_layer(ws: &Workspace, d: usize, layer: Layer) -> bool {
    ws.layer_of_decl(d) == Some(layer)
}

/// The trait of the workspace a method comes from: an impl of a trait the project declares - not
/// std, core, an external crate or the building blocks.
pub(crate) fn workspace_trait(ws: &Workspace, m: &MethodRef<'_>) -> Option<usize> {
    match &m.trait_target {
        Some(Target::Decl(t)) if !ws.is_building_block_decl(*t) => Some(*t),
        _ => None,
    }
}

/// The methods a type exposes as its own, as Java reads a class's methods: inherent ones and those
/// of its impls of workspace traits; with `defaults`, also a workspace trait's default methods the
/// impl does not write out (Java's inherited default methods).
pub(crate) fn own_methods(ws: &Workspace, d: usize, defaults: bool) -> Vec<MethodRef<'_>> {
    let mut out: Vec<MethodRef<'_>> = ws
        .methods(d)
        .into_iter()
        .filter(|m| m.inherent || workspace_trait(ws, m).is_some())
        .collect();
    if defaults {
        let traits: BTreeSet<usize> = out
            .iter()
            .filter_map(|m| workspace_trait(ws, m))
            .chain(ws.implements[d].iter().filter_map(|t| match t {
                Target::Decl(t) if !ws.is_building_block_decl(*t) => Some(*t),
                _ => None,
            }))
            .collect();
        for t in traits {
            let written: BTreeSet<String> = out
                .iter()
                .filter(|m| workspace_trait(ws, m) == Some(t))
                .map(|m| m.name.clone())
                .collect();
            for mut m in ws.trait_methods(t) {
                if m.block.is_some() && !written.contains(&m.name) {
                    m.trait_target = Some(Target::Decl(t));
                    out.push(m);
                }
            }
        }
    }
    out
}

/// Whether a target carries a role: a building block whose role closure holds it, or a decl with it.
pub(crate) fn target_has_role(ws: &Workspace, t: &Target, role: &str) -> bool {
    if let Some(bb) = ws.building_block(t) {
        if bb == role {
            return true;
        }
        if let Some((_, parents)) = crate::model::ROLES.iter().find(|(n, _)| *n == bb)
            && parents.contains(&role)
        {
            return true;
        }
    }
    matches!(t, Target::Decl(d) if ws.has_role(*d, role))
}

pub(crate) fn type_generics(ws: &Workspace, d: usize) -> Vec<&syn::Generics> {
    ws.generics_of(d).into_iter().collect()
}

/// Each field with the targets its type names.
pub(crate) fn field_targets<'a>(ws: &'a Workspace, d: usize) -> Vec<(FieldRef<'a>, Vec<Target>)> {
    let m = ws.decls[d].module;
    let g = type_generics(ws, d);
    ws.fields(d)
        .into_iter()
        .map(|f| {
            let t = ws.type_targets(m, f.ty, &g);
            (f, t)
        })
        .collect()
}

pub(crate) fn field_label(f: &FieldRef<'_>) -> String {
    match &f.variant {
        Some(v) => format!("{v}.{}", f.name),
        None => f.name.clone(),
    }
}

const INTERIOR: &[&str] = &[
    "Cell",
    "RefCell",
    "UnsafeCell",
    "OnceCell",
    "Mutex",
    "RwLock",
    "AtomicBool",
    "AtomicU8",
    "AtomicU16",
    "AtomicU32",
    "AtomicU64",
    "AtomicUsize",
    "AtomicI8",
    "AtomicI16",
    "AtomicI32",
    "AtomicI64",
    "AtomicIsize",
    "AtomicPtr",
];

pub(crate) fn is_interior(t: &Target) -> bool {
    matches!(t, Target::External { path, .. } if path.last().is_some_and(|l| INTERIOR.contains(&l.as_str())))
}

/// Findings for a type that must not change its state: `&mut self` methods, pub fields (when
/// `fields_private`), interior-mutability fields.
pub(crate) fn immutability(
    ws: &Workspace,
    rule: &'static str,
    d: usize,
    what: &str,
    fields_private: bool,
) -> Vec<Violation> {
    let mut out = vec![];
    for m in ws.methods(d) {
        if m.receiver_mut() {
            out.push(at(
                ws,
                rule,
                d,
                format!(
                    "{what} `{}` changes its state in `{}(&mut self)`; return a new value instead",
                    ws.decl_name(d),
                    m.name
                ),
            ));
        }
    }
    for (f, targets) in field_targets(ws, d) {
        if fields_private && f.public {
            out.push(at(
                ws,
                rule,
                d,
                format!(
                    "{what} `{}` has the pub field `{}`; keep fields private",
                    ws.decl_name(d),
                    field_label(&f)
                ),
            ));
        }
        if targets.iter().any(is_interior) {
            out.push(at(
                ws,
                rule,
                d,
                format!(
                    "{what} `{}` holds interior mutability in `{}`",
                    ws.decl_name(d),
                    field_label(&f)
                ),
            ));
        }
    }
    out
}

pub(crate) fn render_path(p: &syn::Path) -> String {
    p.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

pub(crate) fn render(ty: &syn::Type) -> String {
    ty.to_token_stream()
        .to_string()
        .replace(" < ", "<")
        .replace(" >", ">")
        .replace("& ", "&")
}

/// A type as the rules compare it: references, lifetimes, parentheses and groups stripped, a path
/// to a type of the workspace by that type, any other path by its last identifier (`String` and
/// `std::string::String` alike), generic arguments compared the same way.
pub(crate) fn type_key(ws: &Workspace, m: usize, ty: &syn::Type) -> String {
    match ty {
        syn::Type::Reference(r) => type_key(ws, m, &r.elem),
        syn::Type::Paren(p) => type_key(ws, m, &p.elem),
        syn::Type::Group(g) => type_key(ws, m, &g.elem),
        syn::Type::Path(tp) if tp.qself.is_none() => {
            let Some(last) = tp.path.segments.last() else {
                return String::new();
            };
            let head = match ws.resolve_path(m, &tp.path) {
                Some(Target::Decl(d)) => format!("#{d}"),
                _ => last.ident.to_string(),
            };
            let args: Vec<String> = match &last.arguments {
                syn::PathArguments::AngleBracketed(a) => a
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        syn::GenericArgument::Type(t) => Some(type_key(ws, m, t)),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };
            if args.is_empty() {
                head
            } else {
                format!("{head}<{}>", args.join(","))
            }
        }
        syn::Type::Slice(s) => format!("[{}]", type_key(ws, m, &s.elem)),
        syn::Type::Tuple(t) => format!(
            "({})",
            t.elems
                .iter()
                .map(|e| type_key(ws, m, e))
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => render(other),
    }
}

/// Last identifiers of every path in a type.
pub(crate) fn type_idents(ty: &syn::Type) -> Vec<String> {
    let mut c = crate::model::PathCollector::default();
    c.visit_type(ty);
    c.paths
        .iter()
        .filter_map(|p| p.segments.last().map(|s| s.ident.to_string()))
        .collect()
}

/// A method call found in a body.
pub(crate) struct Call {
    pub method: String,
    /// `Some(field)` when the receiver is `self.field`, through `.as_ref()` and the like.
    pub self_field: Option<String>,
    /// `Some(accessor)` when the receiver is `unit.accessor()` and `unit` is the transaction unit a
    /// closure passed to `in_transaction` receives.
    pub unit_accessor: Option<String>,
    /// Whether the call sits inside a closure passed to `in_transaction` or `in_transaction_closed`.
    pub in_transaction: bool,
}

impl Call {
    /// The roles of the receiver: of the field of `self`, or of what the unit's accessor returns.
    pub(crate) fn roles<'a>(
        &self,
        fields: &'a BTreeMap<String, BTreeSet<String>>,
        unit: &'a BTreeMap<String, BTreeSet<String>>,
    ) -> Option<&'a BTreeSet<String>> {
        match (&self.self_field, &self.unit_accessor) {
            (Some(f), _) => fields.get(f),
            (None, Some(a)) => unit.get(a),
            _ => None,
        }
    }
}

/// Whether a method runs its closure argument in a transaction.
pub(crate) fn runs_a_transaction(method: &str) -> bool {
    matches!(method, "in_transaction" | "in_transaction_closed")
}

/// The name the first parameter of a closure binds: the unit a transaction hands its work.
pub(crate) fn closure_unit(e: &syn::Expr) -> Option<String> {
    match e {
        syn::Expr::Closure(c) => c.inputs.first().and_then(binding),
        syn::Expr::Paren(p) => closure_unit(&p.expr),
        _ => None,
    }
}

#[derive(Default)]
struct CallCollector {
    calls: Vec<Call>,
    tx_depth: usize,
    units: Vec<String>,
    /// Per block, the locals bound so far: `Some` when a local stands for a field of `self` or a
    /// unit's accessor (`let things = unit.things();`), `None` when it shadows such a local.
    scopes: Vec<Vec<(String, Option<Receiver>)>>,
}

/// What a receiver stands for: a field of `self` or the accessor of a transaction unit.
#[derive(Clone)]
enum Receiver {
    Field(String),
    Accessor(String),
}

impl CallCollector {
    /// The receiver an expression names, a local bound to one included.
    fn receiver(&self, e: &syn::Expr) -> Option<Receiver> {
        if let Some(f) = self_field(e) {
            return Some(Receiver::Field(f));
        }
        if let Some(a) = unit_accessor(e, &self.units) {
            return Some(Receiver::Accessor(a));
        }
        match e {
            syn::Expr::Path(p) if p.qself.is_none() => {
                let name = p.path.get_ident()?.to_string();
                self.scopes
                    .iter()
                    .rev()
                    .flat_map(|s| s.iter().rev())
                    .find(|(n, _)| *n == name)
                    .and_then(|(_, r)| r.clone())
            }
            syn::Expr::Reference(r) => self.receiver(&r.expr),
            syn::Expr::Paren(p) => self.receiver(&p.expr),
            syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => self.receiver(&u.expr),
            syn::Expr::MethodCall(mc)
                if mc.args.is_empty()
                    && matches!(
                        mc.method.to_string().as_str(),
                        "as_ref"
                            | "as_deref"
                            | "as_mut"
                            | "borrow"
                            | "borrow_mut"
                            | "clone"
                            | "deref"
                    ) =>
            {
                self.receiver(&mc.receiver)
            }
            _ => None,
        }
    }
}

fn self_field(e: &syn::Expr) -> Option<String> {
    match e {
        syn::Expr::Field(f) => match &*f.base {
            syn::Expr::Path(p) if p.path.is_ident("self") => match &f.member {
                syn::Member::Named(i) => Some(i.to_string()),
                syn::Member::Unnamed(i) => Some(i.index.to_string()),
            },
            _ => None,
        },
        syn::Expr::MethodCall(mc) if mc.args.is_empty() => self_field(&mc.receiver),
        syn::Expr::Reference(r) => self_field(&r.expr),
        syn::Expr::Paren(p) => self_field(&p.expr),
        _ => None,
    }
}

/// `accessor` when an expression is `unit.accessor()` on one of the units.
fn unit_accessor(e: &syn::Expr, units: &[String]) -> Option<String> {
    match e {
        syn::Expr::MethodCall(mc) if mc.args.is_empty() => match &*mc.receiver {
            syn::Expr::Path(p)
                if p.path
                    .get_ident()
                    .is_some_and(|i| units.contains(&i.to_string())) =>
            {
                Some(mc.method.to_string())
            }
            _ => None,
        },
        syn::Expr::Reference(r) => unit_accessor(&r.expr, units),
        syn::Expr::Paren(p) => unit_accessor(&p.expr, units),
        _ => None,
    }
}

impl CallCollector {
    /// Visits `f` with the names the patterns bind shadowing any local of the same name.
    fn shadowed(&mut self, pats: &[&syn::Pat], f: impl FnOnce(&mut Self)) {
        let names = bound_names(pats);
        self.scopes
            .push(names.into_iter().map(|n| (n, None)).collect());
        f(self);
        self.scopes.pop();
    }
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.scopes.push(vec![]);
        syn::visit::visit_block(self, b);
        self.scopes.pop();
    }

    fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
        let pats: Vec<&syn::Pat> = c.inputs.iter().collect();
        self.shadowed(&pats, |v| v.visit_expr(&c.body));
    }

    fn visit_arm(&mut self, a: &'ast syn::Arm) {
        self.shadowed(&[&a.pat], |v| {
            if let Some((_, g)) = &a.guard {
                v.visit_expr(g);
            }
            v.visit_expr(&a.body);
        });
    }

    fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
        self.visit_expr(&f.expr);
        self.shadowed(&[&f.pat], |v| v.visit_block(&f.body));
    }

    fn visit_expr_if(&mut self, e: &'ast syn::ExprIf) {
        self.visit_expr(&e.cond);
        self.shadowed(&let_pats(&e.cond), |v| v.visit_block(&e.then_branch));
        if let Some((_, els)) = &e.else_branch {
            self.visit_expr(els);
        }
    }

    fn visit_expr_while(&mut self, e: &'ast syn::ExprWhile) {
        self.visit_expr(&e.cond);
        self.shadowed(&let_pats(&e.cond), |v| v.visit_block(&e.body));
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let Some(init) = &l.init {
            self.visit_expr(&init.expr);
            if let Some((_, e)) = &init.diverge {
                self.visit_expr(e);
            }
        }
        let alias = match (&l.init, pat_ident(&l.pat)) {
            (Some(init), Some(name)) if init.diverge.is_none() => {
                self.receiver(&init.expr).map(|r| (name, r))
            }
            _ => None,
        };
        let mut bound = BoundNames::default();
        bound.visit_pat(&l.pat);
        if let Some(scope) = self.scopes.last_mut() {
            for name in bound.0 {
                let r = alias
                    .as_ref()
                    .filter(|(n, _)| *n == name)
                    .map(|(_, r)| r.clone());
                scope.push((name, r));
            }
        }
    }

    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        let method = mc.method.to_string();
        let receiver = self.receiver(&mc.receiver);
        self.calls.push(Call {
            method: method.clone(),
            self_field: match &receiver {
                Some(Receiver::Field(f)) => Some(f.clone()),
                _ => None,
            },
            unit_accessor: match &receiver {
                Some(Receiver::Accessor(a)) => Some(a.clone()),
                _ => None,
            },
            in_transaction: self.tx_depth > 0,
        });
        self.visit_expr(&mc.receiver);
        let tx = runs_a_transaction(&method);
        for a in &mc.args {
            let unit = if tx { closure_unit(a) } else { None };
            if tx {
                self.tx_depth += 1;
            }
            if let Some(u) = &unit {
                self.units.push(u.clone());
            }
            self.visit_expr(a);
            if unit.is_some() {
                self.units.pop();
            }
            if tx {
                self.tx_depth -= 1;
            }
        }
    }
}

/// The names a pattern binds.
#[derive(Default)]
struct BoundNames(Vec<String>);

/// The names the patterns bind.
fn bound_names(pats: &[&syn::Pat]) -> Vec<String> {
    let mut bound = BoundNames::default();
    for p in pats {
        bound.visit_pat(p);
    }
    bound.0
}

/// The patterns of the `let`s of a condition (`if let`, `while let`, a `let` chain).
fn let_pats(cond: &syn::Expr) -> Vec<&syn::Pat> {
    match cond {
        syn::Expr::Let(l) => vec![&*l.pat],
        syn::Expr::Paren(p) => let_pats(&p.expr),
        syn::Expr::Binary(b) if matches!(b.op, syn::BinOp::And(_)) => {
            let mut out = let_pats(&b.left);
            out.extend(let_pats(&b.right));
            out
        }
        _ => vec![],
    }
}

impl<'ast> Visit<'ast> for BoundNames {
    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        self.0.push(p.ident.to_string());
        syn::visit::visit_pat_ident(self, p);
    }
}

type Pos = (usize, usize);

fn start(s: proc_macro2::Span) -> Pos {
    let l = s.start();
    (l.line, l.column)
}

fn end(s: proc_macro2::Span) -> Pos {
    let l = s.end();
    (l.line, l.column)
}

const OPEN: Pos = (usize::MAX, usize::MAX);

/// Locals bound by a straight-line `let` to something a rule follows - a port, a port call's
/// result - with the stretches of source where each binding holds: from the end of its `let` to the
/// end of its block, minus where a later `let` of the same name shadows it. Bindings of `if let`,
/// `while let`, `match` arms, closure parameters and `for` loops are not followed; they suspend an
/// alias of the same name inside their scope.
#[derive(Default, Debug, Clone)]
pub(crate) struct Aliases {
    spans: BTreeMap<String, Vec<(Pos, Pos, String)>>,
}

impl Aliases {
    /// What a one-identifier path stands for at its place in the source.
    pub(crate) fn of(&self, e: &syn::Expr) -> Option<&str> {
        let syn::Expr::Path(p) = e else { return None };
        if p.qself.is_some() {
            return None;
        }
        let ident = p.path.get_ident()?;
        let at = start(ident.span());
        self.spans
            .get(&ident.to_string())?
            .iter()
            .find(|(s, e, _)| *s <= at && at < *e)
            .map(|(_, _, o)| o.as_str())
    }

    fn open(&mut self, name: &str, from: Pos, origin: String) {
        self.spans
            .entry(name.to_string())
            .or_default()
            .push((from, OPEN, origin));
    }

    /// Ends the open stretch of `name` at `at`, handing back its origin.
    fn close(&mut self, name: &str, at: Pos) -> Option<String> {
        let open = self
            .spans
            .get_mut(name)?
            .iter_mut()
            .rev()
            .find(|(_, e, _)| *e == OPEN)?;
        open.1 = at;
        Some(open.2.clone())
    }
}

/// The aliases of a body: a local whose `let` initialiser `source` accepts - `source` sees the
/// aliases bound before it, so an alias of an alias counts.
pub(crate) fn aliases(
    block: &syn::Block,
    source: &mut dyn FnMut(&syn::Expr, &Aliases) -> Option<String>,
) -> Aliases {
    #[derive(Default)]
    struct Scope {
        /// Aliases this block suspended by shadowing one of an enclosing block, with the depth of
        /// the enclosing block.
        resume: Vec<(String, String, usize)>,
    }
    struct Binder<'s> {
        source: &'s mut dyn FnMut(&syn::Expr, &Aliases) -> Option<String>,
        out: Aliases,
        /// The alias open for each name: the depth of the block that bound it.
        open: BTreeMap<String, usize>,
        scopes: Vec<Scope>,
    }
    impl Binder<'_> {
        /// Visits `f` with the aliases of the names the patterns bind suspended from `from` to `to`.
        fn suspended(&mut self, pats: &[&syn::Pat], from: Pos, to: Pos, f: impl FnOnce(&mut Self)) {
            let mut held = vec![];
            for name in bound_names(pats) {
                if let Some(d) = self.open.remove(&name)
                    && let Some(origin) = self.out.close(&name, from)
                {
                    held.push((name, origin, d));
                }
            }
            f(self);
            for (name, origin, d) in held {
                self.out.open(&name, to, origin);
                self.open.insert(name, d);
            }
        }
    }
    impl<'ast> Visit<'ast> for Binder<'_> {
        fn visit_expr_closure(&mut self, c: &'ast syn::ExprClosure) {
            let pats: Vec<&syn::Pat> = c.inputs.iter().collect();
            let (from, to) = (
                start(c.or1_token.span),
                end(syn::spanned::Spanned::span(&c.body)),
            );
            self.suspended(&pats, from, to, |v| v.visit_expr(&c.body));
        }

        fn visit_arm(&mut self, a: &'ast syn::Arm) {
            let (from, to) = (
                start(syn::spanned::Spanned::span(&a.pat)),
                end(syn::spanned::Spanned::span(&a.body)),
            );
            self.suspended(&[&a.pat], from, to, |v| {
                if let Some((_, g)) = &a.guard {
                    v.visit_expr(g);
                }
                v.visit_expr(&a.body);
            });
        }

        fn visit_expr_for_loop(&mut self, f: &'ast syn::ExprForLoop) {
            self.visit_expr(&f.expr);
            let (from, to) = (
                start(syn::spanned::Spanned::span(&f.pat)),
                end(f.body.brace_token.span.close()),
            );
            self.suspended(&[&f.pat], from, to, |v| v.visit_block(&f.body));
        }

        fn visit_expr_if(&mut self, e: &'ast syn::ExprIf) {
            self.visit_expr(&e.cond);
            let b = &e.then_branch.brace_token.span;
            let (from, to) = (start(b.open()), end(b.close()));
            self.suspended(&let_pats(&e.cond), from, to, |v| {
                v.visit_block(&e.then_branch)
            });
            if let Some((_, els)) = &e.else_branch {
                self.visit_expr(els);
            }
        }

        fn visit_expr_while(&mut self, e: &'ast syn::ExprWhile) {
            self.visit_expr(&e.cond);
            let b = &e.body.brace_token.span;
            let (from, to) = (start(b.open()), end(b.close()));
            self.suspended(&let_pats(&e.cond), from, to, |v| v.visit_block(&e.body));
        }

        fn visit_block(&mut self, b: &'ast syn::Block) {
            self.scopes.push(Scope::default());
            syn::visit::visit_block(self, b);
            let depth = self.scopes.len();
            let at = start(b.brace_token.span.close());
            let ending: Vec<String> = self
                .open
                .iter()
                .filter(|(_, d)| **d == depth)
                .map(|(n, _)| n.clone())
                .collect();
            for name in ending {
                self.out.close(&name, at);
                self.open.remove(&name);
            }
            let scope = self.scopes.pop().unwrap_or_default();
            for (name, origin, d) in scope.resume {
                self.out.open(&name, at, origin);
                self.open.insert(name, d);
            }
        }

        fn visit_local(&mut self, l: &'ast syn::Local) {
            if let Some(init) = &l.init {
                self.visit_expr(&init.expr);
                if let Some((_, e)) = &init.diverge {
                    self.visit_expr(e);
                }
            }
            let alias = match (&l.init, pat_ident(&l.pat)) {
                (Some(init), Some(name)) if init.diverge.is_none() => {
                    (self.source)(&init.expr, &self.out).map(|o| (name, o))
                }
                _ => None,
            };
            let mut bound = BoundNames::default();
            bound.visit_pat(&l.pat);
            let depth = self.scopes.len();
            let at = start(l.let_token.span);
            let from = end(l.semi_token.span);
            for name in bound.0 {
                if let Some(d) = self.open.remove(&name)
                    && let Some(origin) = self.out.close(&name, at)
                    && d < depth
                    && let Some(scope) = self.scopes.last_mut()
                    && !scope.resume.iter().any(|(n, _, _)| *n == name)
                {
                    scope.resume.push((name.clone(), origin, d));
                }
                if let Some((n, origin)) = &alias
                    && *n == name
                {
                    self.out.open(&name, from, origin.clone());
                    self.open.insert(name, depth);
                }
            }
        }
    }
    let mut b = Binder {
        source,
        out: Aliases::default(),
        open: BTreeMap::new(),
        scopes: vec![],
    };
    b.visit_block(block);
    b.out
}

pub(crate) fn calls(block: &syn::Block) -> Vec<Call> {
    let mut c = CallCollector::default();
    c.visit_block(block);
    c.calls
}

/// The roles the rules read off a port's type.
const PORT_ROLES: &[&str] = &[
    "Repository",
    "Store",
    "DomainEventPublisher",
    "IntegrationEventPublisher",
    "OutputPort",
    "TransactionBoundary",
    "TransactionUnit",
    "InputPort",
    "DomainService",
];

fn roles_of(ws: &Workspace, targets: &[Target]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for t in targets {
        for role in PORT_ROLES {
            if target_has_role(ws, t, role) {
                out.insert(role.to_string());
            }
        }
    }
    out
}

/// The generics of a type and of every impl block for it.
pub(crate) fn type_and_impl_generics(ws: &Workspace, d: usize) -> Vec<&syn::Generics> {
    let mut generics: Vec<&syn::Generics> = type_generics(ws, d);
    for imp in ws.impls.iter().filter(|i| i.self_decl == Some(d)) {
        if let syn::Item::Impl(item) = &ws.modules[imp.module].items[imp.item] {
            generics.push(&item.generics);
        }
    }
    generics
}

/// The roles of each field of a type, the generic bounds of its parameters included - on the type
/// and on its impl blocks, by the parameter's name.
pub(crate) fn field_roles(ws: &Workspace, d: usize) -> BTreeMap<String, BTreeSet<String>> {
    let m = ws.decls[d].module;
    let generics = type_and_impl_generics(ws, d);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in ws.fields(d) {
        let targets = ws.type_targets(m, f.ty, &generics);
        out.entry(f.name.clone())
            .or_default()
            .extend(roles_of(ws, &targets));
    }
    out
}

/// The transaction-unit traits named in the bounds of `generics`, read in module `m`.
pub(crate) fn unit_traits(ws: &Workspace, m: usize, generics: &[&syn::Generics]) -> Vec<usize> {
    let mut paths: Vec<&syn::Path> = vec![];
    for g in generics {
        for p in g.type_params() {
            for b in &p.bounds {
                if let syn::TypeParamBound::Trait(tb) = b {
                    paths.push(&tb.path);
                }
            }
        }
        if let Some(w) = &g.where_clause {
            for pred in &w.predicates {
                if let syn::WherePredicate::Type(pt) = pred {
                    for b in &pt.bounds {
                        if let syn::TypeParamBound::Trait(tb) = b {
                            paths.push(&tb.path);
                        }
                    }
                }
            }
        }
    }
    let mut out = vec![];
    for p in paths {
        if let Some(Target::Decl(d)) = ws.resolve_path(m, p)
            && ws.decls[d].kind == DeclKind::Trait
            && ws.has_role(d, "TransactionUnit")
            && !out.contains(&d)
        {
            out.push(d);
        }
    }
    out
}

/// What each method of a unit trait hands out: the targets its return type names, an associated
/// type of the trait (`&mut Self::Things`) read through its bounds.
pub(crate) fn unit_accessors(ws: &Workspace, unit: usize) -> BTreeMap<String, Vec<Target>> {
    let syn::Item::Trait(t) = ws.item(unit) else {
        return BTreeMap::new();
    };
    let m = ws.decls[unit].module;
    let assoc = |name: &str| -> Vec<Target> {
        t.items
            .iter()
            .filter_map(|i| match i {
                syn::TraitItem::Type(ty) if ty.ident == name => Some(ty),
                _ => None,
            })
            .flat_map(|ty| ty.bounds.iter())
            .filter_map(|b| match b {
                syn::TypeParamBound::Trait(tb) => ws.resolve_path(m, &tb.path),
                _ => None,
            })
            .collect()
    };
    let mut out = BTreeMap::new();
    for method in ws.trait_methods(unit) {
        let syn::ReturnType::Type(_, ty) = &method.sig.output else {
            continue;
        };
        let mut c = crate::model::PathCollector::default();
        c.visit_type(ty);
        let mut targets = vec![];
        for p in &c.paths {
            let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
            if segs.len() == 2 && segs[0] == "Self" {
                targets.extend(assoc(&segs[1]));
            }
        }
        targets.extend(ws.type_targets(m, ty, &[&t.generics]));
        out.insert(method.name.clone(), targets);
    }
    out
}

/// The roles of what each accessor of the units a type's generics bound hands out.
pub(crate) fn unit_roles(ws: &Workspace, d: usize) -> BTreeMap<String, BTreeSet<String>> {
    let m = ws.decls[d].module;
    let generics = type_and_impl_generics(ws, d);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for unit in unit_traits(ws, m, &generics) {
        for (name, targets) in unit_accessors(ws, unit) {
            out.entry(name).or_default().extend(roles_of(ws, &targets));
        }
    }
    out
}

/// Use-case types: structs and enums in application crates that implement an input port.
pub(crate) fn use_cases(ws: &Workspace) -> Vec<usize> {
    ws.types_with_role("InputPort")
        .filter(|d| in_layer(ws, *d, Layer::Application))
        .collect()
}

pub(crate) fn is_kind(ws: &Workspace, d: usize, kind: DeclKind) -> bool {
    ws.decls[d].kind == kind
}

/// A function body of a module: a free function, a method of an impl block or a trait's default
/// method.
pub(crate) struct Function<'a> {
    pub sig: &'a syn::Signature,
    pub block: &'a syn::Block,
    /// The type an impl block is for, when the workspace declares it.
    pub self_decl: Option<usize>,
    /// The generics of the enclosing impl block or trait.
    pub outer_generics: Option<&'a syn::Generics>,
}

impl Function<'_> {
    pub(crate) fn name(&self) -> String {
        self.sig.ident.to_string()
    }

    pub(crate) fn generics(&self) -> Vec<&syn::Generics> {
        let mut g: Vec<&syn::Generics> = self.outer_generics.into_iter().collect();
        g.push(&self.sig.generics);
        g
    }
}

/// Every function body of module `m`.
pub(crate) fn functions(ws: &Workspace, m: usize) -> Vec<Function<'_>> {
    let mut out = vec![];
    for (idx, item) in ws.modules[m].items.iter().enumerate() {
        match item {
            syn::Item::Fn(f) => out.push(Function {
                sig: &f.sig,
                block: &f.block,
                self_decl: None,
                outer_generics: None,
            }),
            syn::Item::Impl(block) => {
                let self_decl = ws
                    .impls
                    .iter()
                    .find(|i| i.module == m && i.item == idx)
                    .and_then(|i| i.self_decl);
                for it in &block.items {
                    if let syn::ImplItem::Fn(f) = it {
                        out.push(Function {
                            sig: &f.sig,
                            block: &f.block,
                            self_decl,
                            outer_generics: Some(&block.generics),
                        });
                    }
                }
            }
            syn::Item::Trait(t) => {
                for it in &t.items {
                    if let syn::TraitItem::Fn(f) = it
                        && let Some(block) = &f.default
                    {
                        out.push(Function {
                            sig: &f.sig,
                            block,
                            self_decl: None,
                            outer_generics: Some(&t.generics),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The parameters and the fields of `self` of a function whose type names a target `pred` accepts.
/// A field's type is read with the generics of its struct and of the impl block, so a bound written
/// on the impl counts; a local binding counts when its type is written out. A parameter bound
/// through a one-field pattern (`State(ports): State<Ports>`) counts by the types its type names; a
/// parameter whose type is a struct of the workspace counts for the fields of that struct `pred`
/// accepts (`ports.add`). Inside a closure passed to `in_transaction`, `unit.accessor()` counts when
/// the accessor of a transaction unit the generics bound hands out a type `pred` accepts.
#[derive(Default, Debug)]
pub(crate) struct Receivers {
    pub names: BTreeSet<String>,
    pub fields: BTreeSet<String>,
    pub held: BTreeMap<String, BTreeSet<String>>,
    /// The names the transaction closures give their unit.
    pub units: BTreeSet<String>,
    /// The unit accessors that hand out an accepted type.
    pub accessors: BTreeSet<String>,
    /// Locals bound to one of these receivers (`let things = unit.things();`, `let r = &self.repo;`)
    /// or, without a written type, to `Type::function(..)` returning `Self` or the type.
    pub aliases: Aliases,
    /// The targets the type of a receiver names, by the text [`Receivers::named_by`] answers: for
    /// parameters, written-out locals, fields of `self` and held fields.
    pub types: BTreeMap<String, Vec<Target>>,
}

impl Receivers {
    pub(crate) fn of(
        ws: &Workspace,
        m: usize,
        f: &Function<'_>,
        pred: &dyn Fn(&Target) -> bool,
    ) -> Receivers {
        let generics = f.generics();
        let mut out = Receivers::default();
        for a in &f.sig.inputs {
            let syn::FnArg::Typed(pt) = a else { continue };
            let Some(name) = binding(&pt.pat) else {
                continue;
            };
            let targets = ws.type_targets(m, &pt.ty, &generics);
            if targets.iter().any(pred) {
                out.names.insert(name.clone());
                out.types.insert(name, targets);
                continue;
            }
            for t in &targets {
                if let Target::Decl(d) = t
                    && ws.decls[*d].kind == DeclKind::Struct
                {
                    let g = type_generics(ws, *d);
                    let dm = ws.decls[*d].module;
                    let mut held: BTreeSet<String> = BTreeSet::new();
                    for fl in ws.fields(*d) {
                        let ft = ws.type_targets(dm, fl.ty, &g);
                        if ft.iter().any(pred) {
                            out.types.insert(format!("{name}.{}", fl.name), ft);
                            held.insert(fl.name.clone());
                        }
                    }
                    if !held.is_empty() {
                        out.held.entry(name.clone()).or_default().extend(held);
                    }
                }
            }
        }
        let mut unit_generics = generics.clone();
        if let Some(d) = f.self_decl {
            unit_generics.extend(type_generics(ws, d));
        }
        for unit in unit_traits(ws, m, &unit_generics) {
            for (name, targets) in unit_accessors(ws, unit) {
                if targets.iter().any(pred) {
                    out.accessors.insert(name);
                }
            }
        }
        if !out.accessors.is_empty() {
            #[derive(Default)]
            struct Units(Vec<String>);
            impl<'ast> Visit<'ast> for Units {
                fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
                    if runs_a_transaction(&mc.method.to_string()) {
                        self.0.extend(mc.args.iter().filter_map(closure_unit));
                    }
                    syn::visit::visit_expr_method_call(self, mc);
                }
            }
            let mut units = Units::default();
            units.visit_block(f.block);
            out.units.extend(units.0);
        }
        if let Some(d) = f.self_decl {
            let mut g: Vec<&syn::Generics> = type_generics(ws, d);
            g.extend(f.outer_generics);
            let dm = ws.decls[d].module;
            for field in ws.fields(d) {
                let ft = ws.type_targets(dm, field.ty, &g);
                if field.variant.is_none() && ft.iter().any(pred) {
                    out.types.insert(format!("self.{}", field.name), ft);
                    out.fields.insert(field.name.clone());
                }
            }
        }
        struct Locals<'w, 'g> {
            ws: &'w Workspace,
            m: usize,
            generics: &'g [&'g syn::Generics],
            pred: &'g dyn Fn(&Target) -> bool,
            names: Vec<(String, Vec<Target>)>,
        }
        impl<'ast> Visit<'ast> for Locals<'_, '_> {
            fn visit_local(&mut self, l: &'ast syn::Local) {
                if let syn::Pat::Type(t) = &l.pat
                    && let Some(name) = pat_ident(&t.pat)
                {
                    let targets = self.ws.type_targets(self.m, &t.ty, self.generics);
                    if targets.iter().any(self.pred) {
                        self.names.push((name, targets));
                    }
                }
                syn::visit::visit_local(self, l);
            }
        }
        let mut locals = Locals {
            ws,
            m,
            generics: &generics,
            pred,
            names: vec![],
        };
        locals.visit_block(f.block);
        for (name, targets) in locals.names {
            out.names.insert(name.clone());
            out.types.entry(name).or_insert(targets);
        }
        let aliases = aliases(f.block, &mut |e, a| {
            out.named_in(e, a).or_else(|| {
                built_type(ws, m, e)
                    .filter(|t| pred(t))
                    .map(|t| ws.target_name(&t))
            })
        });
        out.aliases = aliases;
        out
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.names.is_empty()
            && self.fields.is_empty()
            && self.held.is_empty()
            && (self.units.is_empty() || self.accessors.is_empty())
            && self.aliases.spans.is_empty()
    }

    /// The receiver an expression names when it is one of these: `name`, `self.field` or `holder.field`, through
    /// references, parentheses, `.await` and argument-less calls such as `.as_ref()`.
    pub(crate) fn named_by(&self, e: &syn::Expr) -> Option<String> {
        self.named_in(e, &self.aliases)
    }

    fn named_in(&self, e: &syn::Expr, aliases: &Aliases) -> Option<String> {
        match e {
            syn::Expr::Path(p) if p.path.segments.len() == 1 && p.qself.is_none() => {
                let n = p.path.segments[0].ident.to_string();
                (self.names.contains(&n) || aliases.of(e).is_some()).then_some(n)
            }
            syn::Expr::Field(f) => match (&*f.base, &f.member) {
                (syn::Expr::Path(b), syn::Member::Named(n))
                    if b.path.is_ident("self") && self.fields.contains(&n.to_string()) =>
                {
                    Some(format!("self.{n}"))
                }
                (syn::Expr::Path(b), syn::Member::Named(n)) => {
                    let holder = b.path.get_ident()?.to_string();
                    let n = n.to_string();
                    self.held
                        .get(&holder)
                        .is_some_and(|fields| fields.contains(&n))
                        .then(|| format!("{holder}.{n}"))
                }
                _ => None,
            },
            syn::Expr::Reference(r) => self.named_in(&r.expr, aliases),
            syn::Expr::Paren(p) => self.named_in(&p.expr, aliases),
            syn::Expr::Await(a) => self.named_in(&a.base, aliases),
            syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
                self.named_in(&u.expr, aliases)
            }
            syn::Expr::MethodCall(mc)
                if mc.args.is_empty()
                    && self.accessors.contains(&mc.method.to_string())
                    && matches!(&*mc.receiver, syn::Expr::Path(p)
                        if p.path.get_ident().is_some_and(|i| self.units.contains(&i.to_string()))) =>
            {
                let syn::Expr::Path(p) = &*mc.receiver else {
                    return None;
                };
                Some(format!("{}.{}()", render_path(&p.path), mc.method))
            }
            syn::Expr::MethodCall(mc)
                if mc.args.is_empty()
                    && matches!(
                        mc.method.to_string().as_str(),
                        "as_ref" | "as_deref" | "borrow" | "clone" | "deref"
                    ) =>
            {
                self.named_in(&mc.receiver, aliases)
            }
            _ => None,
        }
    }
}

/// The type an associated-function call builds: `Type::function(..)` - through `?`, `.await` and
/// parentheses - where `function` of a workspace type returns `Self` or the type itself, or, after
/// `?`, a `Result` or `Option` of it.
pub(crate) fn built_type(ws: &Workspace, m: usize, e: &syn::Expr) -> Option<Target> {
    let mut e = e;
    let mut tried = false;
    loop {
        e = match e {
            syn::Expr::Try(t) => {
                tried = true;
                &t.expr
            }
            syn::Expr::Await(a) => &a.base,
            syn::Expr::Paren(p) => &p.expr,
            _ => break,
        };
    }
    let syn::Expr::Call(c) = e else { return None };
    let syn::Expr::Path(p) = &*c.func else {
        return None;
    };
    if p.qself.is_some() || p.path.segments.len() < 2 {
        return None;
    }
    let function = p.path.segments.last()?.ident.to_string();
    let mut ty = p.path.clone();
    ty.segments.pop();
    ty.segments.pop_punct();
    let Some(Target::Decl(d)) = ws.resolve_path(m, &ty) else {
        return None;
    };
    if !matches!(ws.decls[d].kind, DeclKind::Struct | DeclKind::Enum) {
        return None;
    }
    let method = ws
        .methods(d)
        .into_iter()
        .find(|mm| mm.name == function && !mm.has_receiver())?;
    let syn::ReturnType::Type(_, out) = &method.sig.output else {
        return None;
    };
    let is_it = |t: &syn::Type| match t {
        syn::Type::Path(tp) if tp.qself.is_none() => {
            tp.path.is_ident("Self")
                || ws.resolve_path(method.module, &tp.path) == Some(Target::Decl(d))
        }
        _ => false,
    };
    let built = if tried {
        match &**out {
            syn::Type::Path(tp) => tp.path.segments.last().is_some_and(|s| {
                matches!(&s.arguments, syn::PathArguments::AngleBracketed(a)
                    if matches!(a.args.first(), Some(syn::GenericArgument::Type(t)) if is_it(t)))
            }),
            _ => false,
        }
    } else {
        is_it(out)
    };
    built.then_some(Target::Decl(d))
}

/// The name a parameter pattern binds: a plain identifier, or the one identifier inside a
/// one-field tuple-struct pattern (`State(ports)`).
pub(crate) fn binding(p: &syn::Pat) -> Option<String> {
    match p {
        syn::Pat::TupleStruct(ts) if ts.elems.len() == 1 => pat_ident(&ts.elems[0]),
        syn::Pat::Type(t) => binding(&t.pat),
        _ => pat_ident(p),
    }
}

/// The name a pattern binds when it is a plain identifier (`x`, `mut x`, `ref x`).
pub(crate) fn pat_ident(p: &syn::Pat) -> Option<String> {
    match p {
        syn::Pat::Ident(i) if i.subpat.is_none() => Some(i.ident.to_string()),
        syn::Pat::Type(t) => pat_ident(&t.pat),
        _ => None,
    }
}

/// Modules of the application crates and of the incoming side of the adapters crates.
pub(crate) fn application_and_incoming(ws: &Workspace) -> Vec<usize> {
    ws.project_modules()
        .filter(|m| {
            ws.module_layer(*m) == Some(Layer::Application) || ws.module_in_side(*m, "incoming")
        })
        .collect()
}

/// Whether item `idx` of module `m` is a part of the domain that may be asynchronous: a trait that
/// carries the DomainGateway or the DomainService role, or an impl block for a type that implements
/// DomainService.
pub(crate) fn may_be_async(ws: &Workspace, m: usize, idx: usize) -> bool {
    match &ws.modules[m].items[idx] {
        syn::Item::Trait(_) => ws
            .decls
            .iter()
            .position(|d| d.module == m && d.item == idx)
            .is_some_and(|d| ws.has_role(d, "DomainGateway") || ws.has_role(d, "DomainService")),
        syn::Item::Impl(_) => ws
            .impls
            .iter()
            .find(|i| i.module == m && i.item == idx)
            .and_then(|i| i.self_decl)
            .is_some_and(|d| ws.has_role(d, "DomainService")),
        _ => false,
    }
}
