//! The implemented rules and what they share.

mod cycles;
mod errors;
mod events;
mod graph;
mod hexagonal;
mod naming;
mod rust;
mod strategic;
mod tactical;
mod usecase;

use std::collections::{BTreeMap, BTreeSet};

use quote::ToTokens;
use syn::visit::Visit;

use crate::Violation;
use crate::model::{DeclKind, FieldRef, Layer, Target, Workspace};

/// A rule the suite implements.
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
    dca_lay_002 => "DCA-LAY-002" => graph::lay_002,
    dca_lay_003 => "DCA-LAY-003" => graph::lay_003,
    dca_oni_001 => "DCA-ONI-001" => graph::oni_001,
    dca_oni_002 => "DCA-ONI-002" => graph::oni_002,
    dca_oni_003 => "DCA-ONI-003" => events::oni_003,
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
    dca_hex_019 => "DCA-HEX-019" => hexagonal::hex_019,
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
    dca_str_002 => "DCA-STR-002" => graph::str_002,
    dca_str_003 => "DCA-STR-003" => graph::str_003,
    dca_str_004 => "DCA-STR-004" => graph::str_004,
    dca_str_005 => "DCA-STR-005" => strategic::str_005,
    dca_str_006 => "DCA-STR-006" => graph::str_006,
    dca_str_007 => "DCA-STR-007" => strategic::str_007,
    dca_str_008 => "DCA-STR-008" => strategic::str_008,
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
    dca_use_019 => "DCA-USE-019" => usecase::use_019,
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
    dca_cyc_005 => "DCA-CYC-005" => cycles::cyc_005,
    dca_err_001 => "DCA-ERR-001" => errors::err_001,
    dca_err_002 => "DCA-ERR-002" => errors::err_002,
    dca_err_003 => "DCA-ERR-003" => errors::err_003,
    dca_err_005 => "DCA-ERR-005" => errors::err_005,
    dca_err_006 => "DCA-ERR-006" => errors::err_006,
    dca_err_008 => "DCA-ERR-008" => errors::err_008,
    dca_err_009 => "DCA-ERR-009" => errors::err_009,
    dca_rs_001 => "DCA-RS-001" => rust::rs_001,
    dca_rs_002 => "DCA-RS-002" => rust::rs_002,
    dca_rs_003 => "DCA-RS-003" => rust::rs_003,
    dca_rs_004 => "DCA-RS-004" => rust::rs_004,
    dca_rs_006 => "DCA-RS-006" => rust::rs_006,
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

pub(crate) fn render(ty: &syn::Type) -> String {
    ty.to_token_stream()
        .to_string()
        .replace(" < ", "<")
        .replace(" >", ">")
        .replace("& ", "&")
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
    /// Whether the call sits inside a closure passed to `in_transaction`.
    pub in_transaction: bool,
}

#[derive(Default)]
struct CallCollector {
    calls: Vec<Call>,
    tx_depth: usize,
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

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        let method = mc.method.to_string();
        self.calls.push(Call {
            method: method.clone(),
            self_field: self_field(&mc.receiver),
            in_transaction: self.tx_depth > 0,
        });
        self.visit_expr(&mc.receiver);
        let tx = method == "in_transaction";
        for a in &mc.args {
            if tx {
                self.tx_depth += 1;
            }
            self.visit_expr(a);
            if tx {
                self.tx_depth -= 1;
            }
        }
    }
}

pub(crate) fn calls(block: &syn::Block) -> Vec<Call> {
    let mut c = CallCollector::default();
    c.visit_block(block);
    c.calls
}

/// The roles of each field of a type, the generic bounds of its parameters included.
pub(crate) fn field_roles(ws: &Workspace, d: usize) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (f, targets) in field_targets(ws, d) {
        let entry = out.entry(f.name.clone()).or_default();
        for t in &targets {
            for role in [
                "Repository",
                "Store",
                "DomainEventPublisher",
                "IntegrationEventPublisher",
                "OutputPort",
                "TransactionBoundary",
                "InputPort",
                "DomainService",
            ] {
                if target_has_role(ws, t, role) {
                    entry.insert(role.to_string());
                }
            }
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
