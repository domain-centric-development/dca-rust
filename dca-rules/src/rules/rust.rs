//! Rules only Rust needs.

use super::{at, at_line, field_label, field_targets, target_has_role};
use crate::Violation;
use crate::model::{Layer, Target, Workspace};

const SHARED: &[&str] = &["Rc", "Arc", "Cell", "RefCell", "Mutex", "RwLock", "Weak"];

pub(super) fn rs_001(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .type_decls()
        .filter(|d| matches!(ws.layer_of_decl(*d), Some(Layer::Domain)))
    {
        for (f, targets) in field_targets(ws, d) {
            for t in targets {
                if let Target::External { path, .. } = &t
                    && path.last().is_some_and(|l| SHARED.contains(&l.as_str()))
                {
                    out.push(at(ws, "DCA-RS-001", d, format!("field `{}` holds `{}`; the domain owns its state through the aggregate", field_label(&f), ws.target_name(&t))));
                }
            }
        }
    }
    out
}

fn derives(item: &syn::Item) -> Vec<String> {
    let attrs = match item {
        syn::Item::Struct(s) => &s.attrs,
        syn::Item::Enum(e) => &e.attrs,
        _ => return vec![],
    };
    let mut out = vec![];
    for a in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        if let Ok(list) = a.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) {
            out.extend(
                list.iter()
                    .filter_map(|p| p.segments.last().map(|s| s.ident.to_string())),
            );
        }
    }
    out
}

pub(super) fn rs_002(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("Entity") {
        for name in derives(ws.item(d)) {
            if matches!(name.as_str(), "PartialEq" | "Eq" | "Hash") {
                out.push(at(ws, "DCA-RS-002", d, format!("entity `{}` derives {name}, which compares every field; implement PartialEq on the id", ws.decl_name(d))));
            }
        }
    }
    out
}

fn has_mut_ref(ty: &syn::Type) -> bool {
    struct V(bool);
    impl<'ast> syn::visit::Visit<'ast> for V {
        fn visit_type_reference(&mut self, r: &'ast syn::TypeReference) {
            if r.mutability.is_some() {
                self.0 = true;
            }
            syn::visit::visit_type_reference(self, r);
        }
    }
    let mut v = V(false);
    syn::visit::Visit::visit_type(&mut v, ty);
    v.0
}

pub(super) fn rs_003(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("AggregateRoot") {
        for m in ws.methods(d).into_iter().filter(|m| m.inherent && m.public) {
            if let syn::ReturnType::Type(_, ty) = &m.sig.output
                && has_mut_ref(ty)
            {
                out.push(at(ws, "DCA-RS-003", d, format!("`{}::{}` hands out a mutable reference; change parts through the root's operations", ws.decl_name(d), m.name)));
            }
        }
    }
    out
}

pub(super) fn rs_004(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) == Some(Layer::Domain))
    {
        struct V(Vec<(String, usize)>);
        impl<'ast> syn::visit::Visit<'ast> for V {
            fn visit_signature(&mut self, s: &'ast syn::Signature) {
                let future = match &s.output {
                    syn::ReturnType::Type(_, ty) => {
                        super::type_idents(ty).iter().any(|i| i == "Future")
                    }
                    _ => false,
                };
                if s.asyncness.is_some() || future {
                    self.0
                        .push((s.ident.to_string(), s.ident.span().start().line));
                }
                syn::visit::visit_signature(self, s);
            }
        }
        let mut v = V(vec![]);
        for (idx, item) in ws.modules[m].items.iter().enumerate() {
            if !super::may_be_async(ws, m, idx) {
                syn::visit::Visit::visit_item(&mut v, item);
            }
        }
        for (name, line) in v.0 {
            out.push(at_line(
                ws,
                "DCA-RS-004",
                m,
                &name,
                line,
                "the domain is synchronous outside its gateways and domain services; async belongs to those, the ports and the adapters",
            ));
        }
    }
    out
}

pub(super) fn rs_006(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.types_with_role("DomainEvent") {
        let syn::Item::Enum(e) = ws.item(d) else {
            continue;
        };
        let m = ws.decls[d].module;
        for v in &e.variants {
            let carries = match &v.fields {
                syn::Fields::Unnamed(f) if f.unnamed.len() == 1 => ws
                    .type_decls_in(m, &f.unnamed[0].ty, &[])
                    .iter()
                    .any(|t| *t != d && ws.has_role(*t, "DomainEvent")),
                _ => false,
            };
            if !carries {
                out.push(at(
                    ws,
                    "DCA-RS-006",
                    d,
                    format!(
                        "variant `{}` of `{}` carries no event type of its own; declare a struct that implements DomainEvent and wrap it",
                        v.ident,
                        ws.decl_name(d)
                    ),
                ));
            }
        }
    }
    out
}

/// The roles a transaction unit may hand out: what takes part in the use case's transaction.
const UNIT_PORTS: &[&str] = &["Repository", "Store", "DomainEventPublisher"];

/// Whether a resolved path is std's, core's or alloc's (`Send`, `Sync`).
fn is_std(target: &Target) -> bool {
    matches!(target, Target::External { krate, .. } if matches!(krate.as_str(), "std" | "core" | "alloc"))
}

/// DCA-RS-007 (experimental): a transaction unit is a port of its use case - declared in the use
/// case's module of an application crate - extends nothing but TransactionUnit and std's auto
/// traits, and hands out only repositories, stores and a domain-event publisher, each in
/// the one form: a generic associated type over a lifetime, handed out by value (`Self::Things<'_>`),
/// not by reference.
pub(super) fn rs_007(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.traits_with_role("TransactionUnit") {
        let path = &ws.modules[ws.decls[d].module].path;
        let placed = ws.layer_of_decl(d) == Some(Layer::Application)
            && !path.is_empty()
            && !ws.in_shared(path);
        if !placed {
            out.push(at(ws, "DCA-RS-007", d, format!(
                "transaction unit `{}` sits outside a use case's module of an application crate; declare it beside the use case that receives it",
                ws.decl_name(d)
            )));
        }
        let syn::Item::Trait(t) = ws.item(d) else {
            continue;
        };
        let m = ws.decls[d].module;
        for b in &t.supertraits {
            let syn::TypeParamBound::Trait(tb) = b else {
                continue;
            };
            let Some(target) = ws.resolve_path(m, &tb.path) else {
                continue;
            };
            if is_std(&target) || target_has_role(ws, &target, "TransactionUnit") {
                continue;
            }
            out.push(at(ws, "DCA-RS-007", d, format!(
                "transaction unit `{}` extends `{}`; a unit extends TransactionUnit only - what the work needs besides its ports, the use case holds",
                ws.decl_name(d),
                ws.target_name(&target)
            )));
        }
        let mut named: Vec<(syn::Path, String)> = vec![];
        for item in &t.items {
            match item {
                syn::TraitItem::Type(ty) => {
                    if ty.generics.lifetimes().next().is_none() {
                        out.push(at(ws, "DCA-RS-007", d, format!(
                            "transaction unit `{}` declares `type {}` without a lifetime; declare it as `type {}<'a>: .. where Self: 'a` so its port may borrow the unit",
                            ws.decl_name(d),
                            ty.ident,
                            ty.ident
                        )));
                    }
                    for b in &ty.bounds {
                        if let syn::TypeParamBound::Trait(tb) = b {
                            named.push((tb.path.clone(), format!("type {}", ty.ident)));
                        }
                    }
                }
                syn::TraitItem::Fn(f) => {
                    if let syn::ReturnType::Type(_, ty) = &f.sig.output
                        && matches!(&**ty, syn::Type::Reference(_))
                    {
                        out.push(at(ws, "DCA-RS-007", d, format!(
                            "transaction unit `{}` hands out a reference from `fn {}`; hand out `Self::<Port><'_>` by value - an adapter that owns the port binds it to `&'a mut` the adapter",
                            ws.decl_name(d),
                            f.sig.ident
                        )));
                    }
                    if let syn::ReturnType::Type(_, ty) = &f.sig.output {
                        let mut c = crate::model::PathCollector::default();
                        syn::visit::Visit::visit_type(&mut c, ty);
                        for p in c.paths {
                            if p.segments.first().is_some_and(|s| s.ident == "Self") {
                                continue;
                            }
                            named.push((p, format!("fn {}", f.sig.ident)));
                        }
                    }
                }
                _ => {}
            }
        }
        for (p, place) in named {
            let Some(target) = ws.resolve_path(m, &p) else {
                continue;
            };
            if is_std(&target) {
                continue;
            }
            if !UNIT_PORTS.iter().any(|r| target_has_role(ws, &target, r)) {
                out.push(at(ws, "DCA-RS-007", d, format!(
                    "transaction unit `{}` hands out `{}` in `{place}`, which is no repository, store or domain-event publisher",
                    ws.decl_name(d),
                    ws.target_name(&target)
                )));
            }
        }
    }
    out
}

/// Future types a port method may answer with: std's `Future` (through `impl Future`, `dyn
/// Future` or a `Pin<Box<..>>`) and the building blocks' boxed futures.
const FUTURES: &[&str] = &["Future", "BoxFuture", "ScopedBoxFuture"];

/// Items that drive a future to completion: a waker built by hand, or a known runtime's
/// `block_on` and runtime types. Read from resolved paths only.
fn drives_futures(krate: &str, path: &[String]) -> bool {
    match krate {
        "std" | "core" | "alloc" => {
            path.first().is_some_and(|p| p == "task")
                && path.iter().any(|p| {
                    matches!(
                        p.as_str(),
                        "Waker" | "RawWaker" | "Wake" | "RawWakerVTable" | "from_waker"
                    )
                })
        }
        "tokio" => path.first().is_some_and(|p| p == "runtime"),
        "futures" | "futures_executor" | "futures_lite" | "pollster" | "async_std" | "smol" => {
            path.iter().any(|p| p == "block_on" || p == "executor")
        }
        _ => false,
    }
}

/// The building-block roles of the ports that do I/O: a database, a file system, a queue behind them.
/// A plain output port - a clock, an id source - is an in-process provider and may stay synchronous.
const IO_PORTS: &[&str] = &[
    "Repository",
    "Store",
    "DomainEventPublisher",
    "IntegrationEventPublisher",
];

/// DCA-RS-008: the ports with I/O of an application crate answer asynchronously, and no application
/// crate drives a future itself.
pub(super) fn rs_008(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.traits_with_role("OutputPort") {
        if ws.layer_of_decl(d) != Some(Layer::Application)
            || !IO_PORTS.iter().any(|r| ws.has_role(d, r))
            || ws.has_role(d, "DomainGateway")
            || ws.has_role(d, "DomainService")
        {
            continue;
        }
        let syn::Item::Trait(t) = ws.item(d) else {
            continue;
        };
        let future_types: Vec<String> = t
            .items
            .iter()
            .filter_map(|i| match i {
                syn::TraitItem::Type(a)
                    if a.bounds.iter().any(|b| {
                        matches!(b, syn::TypeParamBound::Trait(tb)
                            if tb.path.segments.last().is_some_and(|s| FUTURES.contains(&s.ident.to_string().as_str())))
                    }) =>
                {
                    Some(a.ident.to_string())
                }
                _ => None,
            })
            .collect();
        for method in ws.trait_methods(d) {
            let answers_later = method.sig.asyncness.is_some()
                || match &method.sig.output {
                    syn::ReturnType::Type(_, ty) => {
                        let idents = super::type_idents(ty);
                        idents.iter().any(|i| FUTURES.contains(&i.as_str()))
                            || idents.iter().any(|i| future_types.contains(i))
                    }
                    syn::ReturnType::Default => false,
                };
            if !answers_later {
                out.push(at_line(
                    ws,
                    "DCA-RS-008",
                    method.module,
                    &format!("{}::{}", ws.decl_name(d), method.name),
                    method.line,
                    "the port does I/O and answers synchronously; its method is async fn or returns a future (impl Future, BoxFuture) - the program drives it from outside the application",
                ));
            }
        }
    }
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) == Some(Layer::Application))
    {
        for (t, line, text) in ws.module_targets(m) {
            if let Target::External { krate, path } = &t
                && drives_futures(krate, path)
            {
                out.push(at_line(
                    ws,
                    "DCA-RS-008",
                    m,
                    "",
                    line,
                    format!("`{text}` drives a future inside the application; the executor belongs to the composition root or an incoming adapter"),
                ));
            }
        }
    }
    out
}
