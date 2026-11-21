//! Layered rules on the source: where transactions are run and wired, and what a crate of the
//! two-crate form holds.

use std::collections::BTreeSet;

use super::at_line;
use crate::Violation;
use crate::model::{Layer, Workspace};

/// DCA-LAY-004: TransactionBoundary is named in the application, the outgoing adapters and the
/// composition root only; a configured transaction type in the application and the outgoing
/// adapters only.
pub(super) fn lay_004(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in ws.project_modules() {
        let Some(layer) = ws.module_layer(m) else {
            continue;
        };
        let runs = layer == Layer::Application || ws.module_in_side(m, "outgoing");
        let wires = runs || matches!(layer, Layer::Infrastructure | Layer::Bootstrap);
        if runs {
            continue;
        }
        let mut seen = BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            let boundary = ws.building_block(&t).as_deref() == Some("TransactionBoundary");
            let transaction = ws.is_configured(&t, &ws.config.transaction_types);
            let refused = (boundary && !wires) || (transaction && !runs);
            if refused && seen.insert(text.clone()) {
                let what = if boundary {
                    "the transaction boundary"
                } else {
                    "a transaction type"
                };
                out.push(at_line(ws, "DCA-LAY-004", m, "", line, format!(
                    "names `{text}`, {what}, in {} - a transaction is run by the application through TransactionBoundary and opened by an outgoing adapter",
                    place(ws, m, layer)
                )));
            }
        }
    }
    out
}

fn place(ws: &Workspace, m: usize, layer: Layer) -> String {
    if ws.module_in_side(m, "incoming") {
        "an incoming adapter".into()
    } else {
        format!("the {} crate", layer.name())
    }
}

/// The modules a crate of the two-crate form holds by name.
const LAYER_MODULES: &[&str] = &[
    "domain",
    "application",
    "adapters",
    "api",
    "events",
    "infrastructure",
];

/// DCA-LAY-007 (informational): in a crate that carries a context but no layer, every top-level
/// module that is none of the layer modules.
pub(super) fn lay_007(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for c in ws
        .crates
        .iter()
        .filter(|c| c.context.is_some() && c.layer.is_none())
    {
        let Some(root) = c.root else { continue };
        for (name, child) in &ws.modules[root].children {
            if !LAYER_MODULES.contains(&name.as_str()) {
                out.push(at_line(
                    ws,
                    "DCA-LAY-007",
                    *child,
                    "",
                    ws.modules[*child].line,
                    format!(
                        "module `{name}` of `{}` sits in no layer - move it into one of {}",
                        c.package,
                        LAYER_MODULES.join(", ")
                    ),
                ));
            }
        }
    }
    out
}

/// DCA-LAY-008: an application crate depends on no configured persistence or transaction crate.
pub(super) fn lay_008(ws: &Workspace) -> Vec<Violation> {
    let configured: BTreeSet<String> = ws
        .config
        .persistence_crates
        .iter()
        .map(|c| c.replace('-', "_"))
        .collect();
    let mut out = vec![];
    for c in ws
        .crates
        .iter()
        .filter(|c| c.layer == Some(Layer::Application))
    {
        for dep in c.deps.iter().filter(|d| d.normal) {
            if configured.contains(&dep.package.replace('-', "_")) {
                out.push(Violation {
                    rule: "DCA-LAY-008",
                    location: super::graph::crate_location(ws, c),
                    message: format!(
                        "the application depends on `{}`, a persistence crate; it reaches storage through output ports",
                        dep.package
                    ),
                });
            }
        }
    }
    out
}

/// DCA-ONI-004: the domain is synchronous outside its gateways and domain services - no async
/// function or block, no `.await`, no function that returns a `Future` elsewhere - and names no
/// configured async runtime crate anywhere.
pub(super) fn oni_004(ws: &Workspace) -> Vec<Violation> {
    #[derive(Default)]
    struct Async(Vec<(usize, String)>);
    /// Whether a type or a set of bounds names a future: `Future`, `BoxFuture`, `ScopedBoxFuture`.
    fn names_future(idents: &[String]) -> bool {
        idents
            .iter()
            .any(|i| matches!(i.as_str(), "Future" | "BoxFuture" | "ScopedBoxFuture"))
    }
    fn paths_in(f: impl FnOnce(&mut crate::model::PathCollector)) -> Vec<String> {
        let mut c = crate::model::PathCollector::default();
        f(&mut c);
        c.paths
            .iter()
            .filter_map(|p| p.segments.last().map(|s| s.ident.to_string()))
            .collect()
    }
    impl<'ast> syn::visit::Visit<'ast> for Async {
        fn visit_signature(&mut self, s: &'ast syn::Signature) {
            let line = s.ident.span().start().line;
            let future = matches!(&s.output, syn::ReturnType::Type(_, ty)
                if names_future(&super::type_idents(ty)));
            if s.asyncness.is_some() {
                self.0.push((line, format!("`{}` is async", s.ident)));
            } else if future {
                self.0
                    .push((line, format!("`{}` returns a Future", s.ident)));
            }
            let takes = s.inputs.iter().any(|a| match a {
                syn::FnArg::Typed(pt) => names_future(&super::type_idents(&pt.ty)),
                syn::FnArg::Receiver(_) => false,
            }) || names_future(&paths_in(|c| {
                syn::visit::Visit::visit_generics(c, &s.generics)
            }));
            if takes {
                self.0.push((line, format!("`{}` takes a Future", s.ident)));
            }
            syn::visit::visit_signature(self, s);
        }
        fn visit_field(&mut self, f: &'ast syn::Field) {
            if names_future(&super::type_idents(&f.ty)) {
                let name = f
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "a tuple field".to_string());
                self.0.push((
                    syn::spanned::Spanned::span(&f.ty).start().line,
                    format!("the field `{name}` holds a Future"),
                ));
            }
            syn::visit::visit_field(self, f);
        }
        fn visit_expr_await(&mut self, e: &'ast syn::ExprAwait) {
            self.0
                .push((e.await_token.span.start().line, "awaits".to_string()));
            syn::visit::visit_expr_await(self, e);
        }
        fn visit_expr_async(&mut self, e: &'ast syn::ExprAsync) {
            self.0.push((
                e.async_token.span.start().line,
                "builds an async block".to_string(),
            ));
            syn::visit::visit_expr_async(self, e);
        }
    }
    let runtimes: BTreeSet<String> = ws
        .config
        .async_runtime_crates
        .iter()
        .map(|c| c.replace('-', "_"))
        .collect();
    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) == Some(Layer::Domain))
    {
        let mut v = Async::default();
        for (idx, item) in ws.modules[m].items.iter().enumerate() {
            if super::may_be_async(ws, m, idx) {
                continue;
            }

            syn::visit::Visit::visit_item(&mut v, item);
        }
        let mut seen = BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            if let crate::Target::External { krate, .. } = &t
                && runtimes.contains(krate)
                && seen.insert(text.clone())
            {
                v.0.push((
                    line,
                    format!("names `{text}` of the async runtime `{krate}`"),
                ));
            }
        }
        for (line, what) in v.0 {
            out.push(at_line(
                ws,
                "DCA-ONI-004",
                m,
                "",
                line,
                format!("{what} - the domain is synchronous outside its gateways and domain services; async belongs to those, the ports and the adapters"),
            ));
        }
    }
    out
}

/// DCA-LAY-001 (informational): doctrine only - the layered architecture is followed. Nothing is
/// selected: the dependency direction is held by the crate graph and the hexagonal rules, and a
/// classic layered check would refuse the outgoing adapters that implement application ports.
pub(super) fn lay_001(_ws: &Workspace) -> Vec<Violation> {
    vec![]
}
