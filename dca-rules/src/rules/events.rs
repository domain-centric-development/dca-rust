//! Domain events, domain services, factories, specifications, and the framework metadata the
//! domain must not carry.

use syn::spanned::Spanned;

use super::{at, at_line, field_label, immutability, in_layer, target_has_role, type_idents};
use crate::Violation;
use crate::model::{Layer, Target, Workspace};

pub fn adv_001(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("DomainEvent")
        .flat_map(|d| immutability(ws, "DCA-ADV-001", d, "domain event", false))
        .collect()
}

fn outside_domain(
    ws: &Workspace,
    rule: &'static str,
    role: &'static str,
    what: &str,
) -> Vec<Violation> {
    ws.types_with_role(role)
        .filter(|d| !ws.is_building_block_decl(*d) && !in_layer(ws, *d, Layer::Domain))
        .map(|d| {
            at(
                ws,
                rule,
                d,
                format!("{what} `{}` sits outside the domain crate", ws.decl_name(d)),
            )
        })
        .collect()
}

pub fn adv_002(ws: &Workspace) -> Vec<Violation> {
    outside_domain(ws, "DCA-ADV-002", "DomainEvent", "domain event")
}

const VERSION_FIELDS: &[&str] = &["schema_version", "event_version", "contract_version"];

fn version_fields(ws: &Workspace, rule: &'static str, d: usize, what: &str) -> Vec<Violation> {
    ws.fields(d)
        .into_iter()
        .filter(|f| VERSION_FIELDS.contains(&f.name.as_str()))
        .map(|f| at(ws, rule, d, format!("{what} `{}` carries `{}` in its payload; the version is the type's metadata (IntegrationEvent::VERSION)", ws.decl_name(d), field_label(&f))))
        .collect()
}

pub fn adv_006(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("IntegrationEvent")
        .flat_map(|d| version_fields(ws, "DCA-ADV-006", d, "integration event"))
        .collect()
}

pub fn adv_007(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("DomainEvent")
        .filter(|d| !ws.has_role(*d, "IntegrationEvent"))
        .flat_map(|d| version_fields(ws, "DCA-ADV-007", d, "domain event"))
        .collect()
}

pub fn adv_008(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("DomainEvent") {
        let fields = ws.fields(d);
        let mut groups: std::collections::BTreeMap<Option<String>, bool> =
            std::collections::BTreeMap::new();
        if let syn::Item::Enum(e) = ws.item(d) {
            for v in &e.variants {
                groups.insert(Some(v.ident.to_string()), false);
            }
        } else {
            groups.insert(None, false);
        }
        let m = ws.decls[d].module;
        for f in &fields {
            let stamped = type_idents(f.ty)
                .iter()
                .any(|i| ws.config.timestamp_types.contains(i));
            // A variant that carries an event type of its own is checked on that type.
            let carries_event = f.variant.is_some()
                && ws
                    .type_decls_in(m, f.ty, &[])
                    .iter()
                    .any(|e| *e != d && ws.has_role(*e, "DomainEvent"));
            if stamped || carries_event {
                groups.insert(f.variant.clone(), true);
            }
        }
        for (variant, has) in groups {
            if !has {
                let what = variant
                    .map(|v| format!("variant `{v}` of "))
                    .unwrap_or_default();
                out.push(at(
                    ws,
                    "DCA-ADV-008",
                    d,
                    format!(
                        "{what}domain event `{}` has no timestamp field",
                        ws.decl_name(d)
                    ),
                ));
            }
        }
    }
    out
}

pub fn adv_010(ws: &Workspace) -> Vec<Violation> {
    outside_domain(ws, "DCA-ADV-010", "DomainService", "domain service")
}

pub fn adv_012(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("DomainService")
        .flat_map(|d| immutability(ws, "DCA-ADV-012", d, "domain service", false))
        .collect()
}

pub fn adv_013(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Factory")
        .filter(|d| !ws.decl_name(*d).ends_with("Factory"))
        .map(|d| {
            at(
                ws,
                "DCA-ADV-013",
                d,
                format!("factory `{}` is not named *Factory", ws.decl_name(d)),
            )
        })
        .collect()
}

pub fn adv_014(ws: &Workspace) -> Vec<Violation> {
    outside_domain(ws, "DCA-ADV-014", "Factory", "factory")
}

pub fn adv_016(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("Factory")
        .flat_map(|d| immutability(ws, "DCA-ADV-016", d, "factory", false))
        .collect()
}

fn specifications(ws: &Workspace) -> Vec<usize> {
    ws.type_decls()
        .filter(|d| ws.has_role(*d, "Specification") || ws.decl_name(*d).ends_with("Specification"))
        .collect()
}

pub fn adv_017(ws: &Workspace) -> Vec<Violation> {
    specifications(ws)
        .into_iter()
        .filter(|d| !in_layer(ws, *d, Layer::Domain))
        .map(|d| {
            at(
                ws,
                "DCA-ADV-017",
                d,
                format!(
                    "specification `{}` sits outside the domain crate",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn adv_019(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.type_decls().filter(|d| {
        ["AggregateRoot", "Entity", "Value"]
            .iter()
            .any(|r| ws.has_role(*d, r))
    }) {
        for m in ws.methods(d) {
            let Some(block) = m.block else { continue };
            let mut c = crate::model::PathCollector::default();
            syn::visit::Visit::visit_block(&mut c, block);
            let mut hit = c
                .paths
                .iter()
                .filter_map(|p| ws.resolve_path(m.module, p))
                .any(|t| target_has_role(ws, &t, "DomainService"));
            let generics = m.generics();
            let service_params: Vec<String> = m
                .sig
                .inputs
                .iter()
                .filter_map(|a| match a {
                    syn::FnArg::Typed(pt) => {
                        let ts = ws.type_targets(m.module, &pt.ty, &generics);
                        if ts.iter().any(|t| target_has_role(ws, t, "DomainService"))
                            && let syn::Pat::Ident(i) = &*pt.pat
                        {
                            return Some(i.ident.to_string());
                        }
                        None
                    }
                    _ => None,
                })
                .collect();
            if !service_params.is_empty() {
                struct Recv<'a> {
                    names: &'a [String],
                    hit: bool,
                }
                impl<'ast> syn::visit::Visit<'ast> for Recv<'_> {
                    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
                        if let syn::Expr::Path(p) = &*mc.receiver
                            && p.path.segments.len() == 1
                            && self.names.contains(&p.path.segments[0].ident.to_string())
                        {
                            self.hit = true;
                        }
                        syn::visit::visit_expr_method_call(self, mc);
                    }
                }
                let mut r = Recv {
                    names: &service_params,
                    hit: false,
                };
                syn::visit::Visit::visit_block(&mut r, block);
                hit |= r.hit;
            }
            if hit {
                out.push(at(ws, "DCA-ADV-019", d, format!("`{}` uses a domain service in `{}`; the service takes the aggregate, not the reverse", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

pub fn adv_020(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("DomainService") {
        for m in ws
            .methods(d)
            .into_iter()
            .filter(|m| m.inherent && m.public && m.has_receiver())
        {
            let generics = m.generics();
            let takes = m.params().iter().any(|p| {
                ws.type_targets(m.module, p, &generics).iter().any(|t| {
                    target_has_role(ws, t, "AggregateRoot") || target_has_role(ws, t, "Entity")
                })
            });
            if !takes {
                out.push(at(ws, "DCA-ADV-020", d, format!("`{}::{}` takes no aggregate or entity; a calculation on values belongs to the value", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

// --- framework metadata ------------------------------------------------------------------------

fn framework_attr(ws: &Workspace, m: usize, attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = vec![];
    let check = |p: &syn::Path, out: &mut Vec<String>| {
        let first = p
            .segments
            .first()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        let krate = match ws.resolve_path(m, p) {
            Some(Target::External { krate, .. }) => Some(krate),
            _ => None,
        };
        let hit = ws.config.framework_crates.contains(&first)
            || krate
                .as_ref()
                .is_some_and(|k| ws.config.framework_crates.contains(k));
        if hit {
            out.push(
                p.segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
        }
    };
    for a in attrs {
        if a.path().is_ident("derive") {
            if let Ok(list) = a.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ) {
                for p in &list {
                    check(p, &mut out);
                }
            }
        } else {
            check(a.path(), &mut out);
        }
    }
    out
}

fn metadata(ws: &Workspace, rule: &'static str, select: impl Fn(usize) -> bool) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .type_decls()
        .filter(|d| in_layer(ws, *d, Layer::Domain) && select(*d))
    {
        let m = ws.decls[d].module;
        let attrs: &[syn::Attribute] = match ws.item(d) {
            syn::Item::Struct(s) => &s.attrs,
            syn::Item::Enum(e) => &e.attrs,
            _ => &[],
        };
        for hit in framework_attr(ws, m, attrs) {
            out.push(at(
                ws,
                rule,
                d,
                format!("`{}` carries `{hit}` from a framework", ws.decl_name(d)),
            ));
        }
        let field_attrs: Vec<(String, Vec<syn::Attribute>)> = match ws.item(d) {
            syn::Item::Struct(s) => s
                .fields
                .iter()
                .map(|f| {
                    (
                        f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                        f.attrs.clone(),
                    )
                })
                .collect(),
            syn::Item::Enum(e) => e
                .variants
                .iter()
                .flat_map(|v| {
                    v.fields
                        .iter()
                        .map(|f| {
                            (
                                f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                                f.attrs.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            _ => vec![],
        };
        for (name, attrs) in field_attrs {
            for hit in framework_attr(ws, m, &attrs) {
                out.push(at(
                    ws,
                    rule,
                    d,
                    format!("field `{name}` carries `{hit}` from a framework"),
                ));
            }
        }
    }
    out
}

fn kind_of(ws: &Workspace, d: usize) -> &'static str {
    if ws.has_role(d, "DomainEvent") {
        "event"
    } else if ws.has_role(d, "DomainService") {
        "service"
    } else if ws.has_role(d, "Factory") {
        "factory"
    } else if ws.has_role(d, "Specification") || ws.decl_name(d).ends_with("Specification") {
        "specification"
    } else {
        "model"
    }
}

pub fn oni_003(ws: &Workspace) -> Vec<Violation> {
    metadata(ws, "DCA-ONI-003", |d| kind_of(ws, d) == "model")
}

pub fn adv_004(ws: &Workspace) -> Vec<Violation> {
    metadata(ws, "DCA-ADV-004", |d| kind_of(ws, d) == "event")
}

pub fn adv_011(ws: &Workspace) -> Vec<Violation> {
    metadata(ws, "DCA-ADV-011", |d| kind_of(ws, d) == "service")
}

pub fn adv_015(ws: &Workspace) -> Vec<Violation> {
    metadata(ws, "DCA-ADV-015", |d| kind_of(ws, d) == "factory")
}

pub fn adv_018(ws: &Workspace) -> Vec<Violation> {
    metadata(ws, "DCA-ADV-018", |d| kind_of(ws, d) == "specification")
}

/// DCA-ADV-021: a domain service is used only by the application and by other domain services.
/// Every function body outside the application crates and outside the impls of a domain service is
/// read: a path to a domain-service type (a struct literal, a unit value, an associated function), a
/// method call on a parameter, a local binding or a field of `self` that holds one, or on one built
/// in place. The composition root (an infrastructure or bootstrap crate) may construct one - a
/// struct literal, a unit value, an associated function without a receiver that returns `Self`.
pub fn adv_021(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in ws.project_modules() {
        let layer = ws.module_layer(m);
        if layer == Some(Layer::Application) {
            continue;
        }
        let root = matches!(layer, Some(Layer::Infrastructure | Layer::Bootstrap));
        for (idx, item) in ws.modules[m].items.iter().enumerate() {
            match item {
                syn::Item::Fn(f) => {
                    service_uses(ws, m, &f.sig, &f.block, None, root, &mut out);
                }
                syn::Item::Impl(block) => {
                    let self_decl = ws
                        .impls
                        .iter()
                        .find(|i| i.module == m && i.item == idx)
                        .and_then(|i| i.self_decl);
                    if self_decl.is_some_and(|d| ws.has_role(d, "DomainService")) {
                        continue;
                    }
                    for it in &block.items {
                        if let syn::ImplItem::Fn(f) = it {
                            service_uses(ws, m, &f.sig, &f.block, self_decl, root, &mut out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The domain-service decl a path names: the path itself, or its prefix for `Type::function`,
/// with that function's name.
fn service_of(ws: &Workspace, m: usize, path: &syn::Path) -> Option<(usize, Option<String>)> {
    let decl = |t: Option<Target>| match t {
        Some(Target::Decl(d)) if ws.has_role(d, "DomainService") => Some(d),
        _ => None,
    };
    let whole = decl(ws.resolve_path(m, path));
    if path.segments.len() < 2 {
        return whole.map(|d| (d, None));
    }
    let mut prefix = path.clone();
    let last = prefix.segments.pop()?.into_value().ident.to_string();
    if let Some(p) = prefix.segments.pop() {
        prefix.segments.push_value(p.into_value());
    }
    // `Type::function` may resolve to the type itself; the prefix tells it apart from a path
    // that names the type (`module::Type`).
    match decl(ws.resolve_path(m, &prefix)) {
        Some(d) => Some((d, Some(last))),
        None => whole.map(|d| (d, None)),
    }
}

/// Whether `function` of the service `d` builds one: no receiver, returns `Self`.
fn constructs(ws: &Workspace, d: usize, function: &str) -> bool {
    ws.methods(d)
        .iter()
        .any(|mm| mm.name == function && !mm.has_receiver() && mm.returns_self())
}

struct Uses<'a> {
    ws: &'a Workspace,
    m: usize,
    /// Parameters and local bindings that hold a domain service.
    names: Vec<String>,
    /// Fields of `self` that hold a domain service.
    fields: &'a [String],
    /// (line, what, whether it is a construction)
    found: Vec<(usize, String, bool)>,
}

impl Uses<'_> {
    fn is_service_type(&self, ty: &syn::Type) -> bool {
        let none: Vec<&syn::Generics> = vec![];
        self.ws
            .type_targets(self.m, ty, &none)
            .iter()
            .any(|t| target_has_role(self.ws, t, "DomainService"))
    }

    /// What an expression yields when it is a domain service: a binding or a field of `self` that
    /// holds one, or one constructed in place.
    fn yields_service(&self, e: &syn::Expr) -> Option<String> {
        match e {
            syn::Expr::Paren(p) => self.yields_service(&p.expr),
            syn::Expr::Reference(r) => self.yields_service(&r.expr),
            syn::Expr::Path(p) => {
                if p.path.segments.len() == 1 {
                    let n = p.path.segments[0].ident.to_string();
                    if self.names.contains(&n) {
                        return Some(n);
                    }
                }
                service_of(self.ws, self.m, &p.path)
                    .filter(|(_, f)| f.is_none())
                    .map(|(d, _)| self.ws.decl_name(d).to_string())
            }
            syn::Expr::Struct(s) => service_of(self.ws, self.m, &s.path)
                .filter(|(_, f)| f.is_none())
                .map(|(d, _)| self.ws.decl_name(d).to_string()),
            syn::Expr::Call(c) => match &*c.func {
                syn::Expr::Path(p) => service_of(self.ws, self.m, &p.path)
                    .filter(|(d, f)| f.as_ref().is_some_and(|f| constructs(self.ws, *d, f)))
                    .map(|(d, _)| self.ws.decl_name(d).to_string()),
                _ => None,
            },
            syn::Expr::Field(f) => match (&*f.base, &f.member) {
                (syn::Expr::Path(b), syn::Member::Named(n))
                    if b.path.is_ident("self") && self.fields.contains(&n.to_string()) =>
                {
                    Some(format!("self.{n}"))
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn path(&mut self, path: &syn::Path) {
        let Some((d, function)) = service_of(self.ws, self.m, path) else {
            return;
        };
        let name = self.ws.decl_name(d).to_string();
        let line = path.span().start().line;
        match function {
            None => self
                .found
                .push((line, format!("constructs `{name}`"), true)),
            Some(f) => {
                let built = constructs(self.ws, d, &f);
                let what = if built { "constructs" } else { "calls" };
                self.found
                    .push((line, format!("{what} `{name}::{f}`"), built));
            }
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Uses<'_> {
    fn visit_expr_struct(&mut self, e: &'ast syn::ExprStruct) {
        self.path(&e.path);
        syn::visit::visit_expr_struct(self, e);
    }

    fn visit_expr_path(&mut self, e: &'ast syn::ExprPath) {
        if e.qself.is_none() {
            self.path(&e.path);
        }
        syn::visit::visit_expr_path(self, e);
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        let (pat, ty) = match &l.pat {
            syn::Pat::Type(t) => (&*t.pat, Some(&*t.ty)),
            p => (p, None),
        };
        if let syn::Pat::Ident(i) = pat {
            let typed = ty.is_some_and(|ty| self.is_service_type(ty));
            let built = l
                .init
                .as_ref()
                .is_some_and(|init| self.yields_service(&init.expr).is_some());
            if typed || built {
                self.names.push(i.ident.to_string());
            }
        }
        syn::visit::visit_local(self, l);
    }

    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        if let Some(r) = self.yields_service(&mc.receiver) {
            self.found.push((
                mc.method.span().start().line,
                format!("calls `{r}.{}`", mc.method),
                false,
            ));
        }
        syn::visit::visit_expr_method_call(self, mc);
    }
}

fn service_uses(
    ws: &Workspace,
    m: usize,
    sig: &syn::Signature,
    block: &syn::Block,
    self_decl: Option<usize>,
    root: bool,
    out: &mut Vec<Violation>,
) {
    let mut uses = Uses {
        ws,
        m,
        names: vec![],
        fields: &[],
        found: vec![],
    };
    let params: Vec<String> = sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            syn::FnArg::Typed(pt) if uses.is_service_type(&pt.ty) => match &*pt.pat {
                syn::Pat::Ident(i) => Some(i.ident.to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let fields: Vec<String> = self_decl
        .map(|d| {
            ws.fields(d)
                .into_iter()
                .filter(|f| uses.is_service_type(f.ty))
                .map(|f| f.name.clone())
                .collect()
        })
        .unwrap_or_default();
    uses.names = params;
    uses.fields = &fields;
    syn::visit::Visit::visit_block(&mut uses, block);
    for (line, what, construction) in uses.found {
        if root && construction {
            continue;
        }
        out.push(at_line(
            ws,
            "DCA-ADV-021",
            m,
            "",
            line,
            format!(
                "`{}` {what} - a domain service is used by the application and by other domain services only; let the use case call it",
                sig.ident
            ),
        ));
    }
}
