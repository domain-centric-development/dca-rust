//! Hexagonal rules: what the adapter sides may name, where ports and their implementations sit.

use super::{at, at_line, in_layer, target_has_role};
use crate::Violation;
use crate::model::{DeclKind, Layer, Target, Workspace};

/// Modules on an adapter side, the event-consumer submodule left out when `skip_consumers`.
fn side_modules(ws: &Workspace, side: &str, skip_consumers: bool) -> Vec<usize> {
    ws.project_modules()
        .filter(|m| ws.module_in_side(*m, side))
        .filter(|m| {
            !(skip_consumers
                && ws.modules[*m].path.get(1) == Some(&ws.config.event_consumer_module))
        })
        .collect()
}

/// Every target a set of modules names that `bad` refuses, once per module and target.
fn named(
    ws: &Workspace,
    rule: &'static str,
    modules: Vec<usize>,
    bad: impl Fn(&Target) -> Option<String>,
) -> Vec<Violation> {
    let mut out = vec![];
    for m in modules {
        let mut seen = std::collections::BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            if let Some(msg) = bad(&t)
                && seen.insert(text.clone())
            {
                out.push(at_line(
                    ws,
                    rule,
                    m,
                    "",
                    line,
                    format!("names `{text}`: {msg}"),
                ));
            }
        }
    }
    out
}

fn is_type(ws: &Workspace, t: &Target) -> bool {
    matches!(t, Target::Decl(d) if matches!(ws.decls[*d].kind, DeclKind::Struct | DeclKind::Enum))
}

pub fn hex_003(ws: &Workspace) -> Vec<Violation> {
    named(
        ws,
        "DCA-HEX-003",
        side_modules(ws, "incoming", false),
        |t| {
            target_has_role(ws, t, "Repository")
                .then(|| "an incoming adapter calls a use case, never a repository".into())
        },
    )
}

pub fn hex_004(ws: &Workspace) -> Vec<Violation> {
    named(
        ws,
        "DCA-HEX-004",
        side_modules(ws, "incoming", false),
        |t| {
            let m = ws.target_module(t)?;
            matches!(
                ws.module_layer(m),
                Some(Layer::Infrastructure | Layer::Bootstrap)
            )
            .then(|| {
                "an incoming adapter uses the application's input ports, not the infrastructure"
                    .into()
            })
        },
    )
}

pub fn hex_006(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in side_modules(ws, "incoming", true) {
        let krate = ws.modules[m].krate;
        let mut seen = std::collections::BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            let Some(tm) = ws.target_module(&t) else {
                continue;
            };
            if ws.modules[tm].krate == krate
                && ws.module_in_side(tm, "outgoing")
                && seen.insert(text.clone())
            {
                out.push(at_line(
                    ws,
                    "DCA-HEX-006",
                    m,
                    "",
                    line,
                    format!("names `{text}` in the outgoing adapters; go through the use case"),
                ));
            }
        }
    }
    out
}

pub fn hex_007(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in side_modules(ws, "incoming", true) {
        let ctx = ws.crates[ws.modules[m].krate].context.clone();
        let mut seen = std::collections::BTreeSet::new();
        for (t, line, text) in ws.module_targets(m) {
            let Some(tm) = ws.target_module(&t) else {
                continue;
            };
            let other = &ws.crates[ws.modules[tm].krate];
            if other.context.is_some() && other.context != ctx && seen.insert(text.clone()) {
                out.push(at_line(ws, "DCA-HEX-007", m, "", line, format!(
                    "names `{text}` of context `{}`; an incoming adapter depends on no other context, an event consumer only on the events it subscribes to",
                    other.context.clone().unwrap_or_default()
                )));
            }
        }
    }
    out
}

pub fn hex_008(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| ws.decl_name(*d).ends_with("Repository") && !ws.decl_in_side(*d, "outgoing"))
        .map(|d| {
            at(
                ws,
                "DCA-HEX-008",
                d,
                format!(
                    "`{}` is named a repository but sits outside the outgoing adapters",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn hex_009(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Application)
                && ws.modules[ws.decls[*d].module]
                    .path
                    .first()
                    .map(String::as_str)
                    == Some("shared")
        })
        .filter(|d| !ws.has_role(*d, "OutputPort"))
        .map(|d| {
            at(
                ws,
                "DCA-HEX-009",
                d,
                format!(
                    "trait `{}` in application::shared does not extend OutputPort",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn hex_010(ws: &Workspace) -> Vec<Violation> {
    ws.traits_with_role("OutputPort")
        .filter(|d| in_layer(ws, *d, Layer::Domain))
        .map(|d| {
            at(
                ws,
                "DCA-HEX-010",
                d,
                format!(
                    "output port `{}` is declared in the domain; declare it in the application",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn hex_011(ws: &Workspace) -> Vec<Violation> {
    named(
        ws,
        "DCA-HEX-011",
        side_modules(ws, "incoming", false),
        |t| {
            (is_type(ws, t) && target_has_role(ws, t, "InputPort"))
                .then(|| "a use case type; depend on its input-port trait".into())
        },
    )
}

pub fn hex_012(ws: &Workspace) -> Vec<Violation> {
    named(
        ws,
        "DCA-HEX-012",
        side_modules(ws, "incoming", false),
        |t| {
            target_has_role(ws, t, "DomainService")
                .then(|| "a domain service; an incoming adapter calls a use case".into())
        },
    )
}

pub fn hex_013(ws: &Workspace) -> Vec<Violation> {
    named(
        ws,
        "DCA-HEX-013",
        side_modules(ws, "outgoing", false),
        |t| {
            target_has_role(ws, t, "InputPort")
                .then(|| "an input port; an outgoing adapter implements output ports".into())
        },
    )
}

pub fn hex_019(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("OutputPort")
        .filter(|d| !ws.is_building_block_decl(*d) && !ws.decl_in_side(*d, "outgoing"))
        .filter(|d| {
            !matches!(
                ws.layer_of_decl(*d),
                Some(Layer::Infrastructure | Layer::Bootstrap)
            )
        })
        .map(|d| {
            at(
                ws,
                "DCA-HEX-019",
                d,
                format!(
                    "`{}` implements an output port outside the outgoing adapters",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

/// Whether a called name asks if the caller holds a role or an authority: `has_role`,
/// `has_any_role`, `has_authority`, `has_any_authority`, `is_in_role`, `is_user_in_role`.
fn asks_for_a_role(name: &str) -> bool {
    let rest = name
        .strip_prefix("has_any_")
        .or_else(|| name.strip_prefix("has_"));
    matches!(rest, Some("role" | "authority")) || name == "is_in_role" || name == "is_user_in_role"
}

/// The role questions a body asks: method calls and calls of a path, with their line.
#[derive(Default)]
struct RoleQuestions {
    found: Vec<(String, usize)>,
}

impl<'ast> syn::visit::Visit<'ast> for RoleQuestions {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        if asks_for_a_role(&name) {
            self.found.push((name, call.method.span().start().line));
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*call.func
            && let Some(last) = p.path.segments.last()
            && asks_for_a_role(&last.ident.to_string())
        {
            self.found
                .push((last.ident.to_string(), last.ident.span().start().line));
        }
        syn::visit::visit_expr_call(self, call);
    }
}

/// The names of the functions a router routes behind a configured authorization layer: every path
/// named in the receiver of a `.layer(...)` / `.route_layer(...)` call whose argument calls one of
/// the layer functions - the routes added before the layer, as the router applies it.
#[derive(Default)]
struct LayeredHandlers<'a> {
    layers: Option<&'a std::collections::BTreeSet<String>>,
    handlers: std::collections::BTreeSet<String>,
}

impl<'ast> syn::visit::Visit<'ast> for LayeredHandlers<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let layers = self.layers.expect("layers set");
        let is_layer = matches!(call.method.to_string().as_str(), "layer" | "route_layer");
        let calls_a_configured_layer = call.args.iter().any(|a| {
            let mut c = crate::model::PathCollector::default();
            syn::visit::Visit::visit_expr(&mut c, a);
            c.paths.iter().any(|p| {
                p.segments
                    .last()
                    .is_some_and(|s| layers.contains(&s.ident.to_string()))
            })
        });
        if is_layer && calls_a_configured_layer {
            let mut c = crate::model::PathCollector::default();
            syn::visit::Visit::visit_expr(&mut c, &call.receiver);
            for p in &c.paths {
                if let Some(s) = p.segments.last() {
                    self.handlers.insert(s.ident.to_string());
                }
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

/// DCA-HEX-020 (experimental, informational). A check that reads nothing but the caller's roles is
/// declared on the route - an extractor in the handler's signature or a layer on its route - not
/// decided by a branch in the handler's own body.
pub fn hex_020(ws: &Workspace) -> Vec<Violation> {
    let extractors = &ws.config.authorization_extractors;
    let layers = &ws.config.authorization_layers;
    if extractors.is_empty() && layers.is_empty() {
        return vec![];
    }
    let incoming = side_modules(ws, "incoming", false);
    let mut layered = LayeredHandlers {
        layers: Some(layers),
        ..Default::default()
    };
    if !layers.is_empty() {
        let crates: std::collections::BTreeSet<usize> =
            incoming.iter().map(|m| ws.modules[*m].krate).collect();
        for m in ws
            .project_modules()
            .filter(|m| crates.contains(&ws.modules[*m].krate))
        {
            for item in &ws.modules[m].items {
                syn::visit::Visit::visit_item(&mut layered, item);
            }
        }
    }
    let declared = |sig: &syn::Signature| {
        layered.handlers.contains(&sig.ident.to_string())
            || sig.inputs.iter().any(|a| match a {
                syn::FnArg::Typed(t) => super::type_idents(&t.ty)
                    .iter()
                    .any(|i| extractors.contains(i)),
                syn::FnArg::Receiver(_) => false,
            })
    };
    let declare = extractors
        .iter()
        .map(|e| format!("the extractor `{e}`"))
        .chain(layers.iter().map(|l| format!("the layer `{l}`")))
        .collect::<Vec<_>>()
        .join(" or ");
    let mut out = vec![];
    for m in incoming {
        let mut functions: Vec<(&syn::Signature, &syn::Block)> = vec![];
        for item in &ws.modules[m].items {
            match item {
                syn::Item::Fn(f) => functions.push((&f.sig, &f.block)),
                syn::Item::Impl(i) => {
                    for ii in &i.items {
                        if let syn::ImplItem::Fn(f) = ii {
                            functions.push((&f.sig, &f.block));
                        }
                    }
                }
                _ => {}
            }
        }
        for (sig, block) in functions {
            if declared(sig) {
                continue;
            }
            let mut questions = RoleQuestions::default();
            syn::visit::Visit::visit_block(&mut questions, block);
            for (name, line) in questions.found {
                out.push(at_line(
                    ws,
                    "DCA-HEX-020",
                    m,
                    &sig.ident.to_string(),
                    line,
                    format!(
                        "`{}` reads the caller's roles through `{name}` in its own body; declare a claims-only check with {declare}, and leave a check that needs the stored data to the use case",
                        sig.ident
                    ),
                ));
            }
        }
    }
    out
}
