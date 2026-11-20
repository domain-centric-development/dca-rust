//! Hexagonal rules: what the adapter sides may name, where ports and their implementations sit.

use quote::ToTokens;
use syn::spanned::Spanned;

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

// --- handlers the router presets register ---------------------------------------------------------

/// The HTTP verbs a router registers a handler for.
const VERBS: &[&str] = &[
    "get", "post", "put", "delete", "patch", "head", "options", "trace", "any",
];

/// A function a configured router preset registers as a handler.
struct Handler {
    module: usize,
    name: String,
    line: usize,
    verbs: std::collections::BTreeSet<String>,
}

/// The verb a call registers through `krate`'s routing function (`get(..)`, `web::get()`), when the
/// callee resolves to a function of that name in that crate.
fn routing_verb(ws: &Workspace, m: usize, e: &syn::Expr, krate: &str) -> Option<String> {
    let syn::Expr::Call(c) = e else { return None };
    let syn::Expr::Path(p) = &*c.func else {
        return None;
    };
    let (k, name) = ws.crate_and_name(&ws.resolve_path(m, &p.path)?)?;
    (k == krate && VERBS.contains(&name.as_str())).then_some(name)
}

/// Collects the handlers a router expression registers: `axum` - a function passed to a routing
/// function (`get(show)`) or to a verb method chained on one (`get(show).post(create)`);
/// `actix-web` - a function passed to `.to(..)` on a routing function (`web::get().to(show)`).
struct Registrations<'w> {
    ws: &'w Workspace,
    m: usize,
    axum: bool,
    actix: bool,
    found: Vec<(syn::Path, String)>,
}

impl Registrations<'_> {
    /// The verb of an axum method router expression: a routing call or a verb chained on one.
    fn axum_router(&self, e: &syn::Expr) -> Option<String> {
        match e {
            syn::Expr::MethodCall(mc) if VERBS.contains(&mc.method.to_string().as_str()) => self
                .axum_router(&mc.receiver)
                .map(|_| mc.method.to_string()),
            _ => routing_verb(self.ws, self.m, e, "axum"),
        }
    }

    fn handler(&mut self, arg: Option<&syn::Expr>, verb: String) {
        if let Some(syn::Expr::Path(p)) = arg {
            self.found.push((p.path.clone(), verb));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for Registrations<'_> {
    fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
        if self.axum
            && let Some(verb) = routing_verb(self.ws, self.m, &syn::Expr::Call(c.clone()), "axum")
        {
            self.handler(c.args.first(), verb);
        }
        syn::visit::visit_expr_call(self, c);
    }

    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        let method = mc.method.to_string();
        if self.axum && VERBS.contains(&method.as_str()) && self.axum_router(&mc.receiver).is_some()
        {
            self.handler(mc.args.first(), method.clone());
        }
        if self.actix
            && method == "to"
            && let Some(verb) = routing_verb(self.ws, self.m, &mc.receiver, "actix_web")
        {
            self.handler(mc.args.first(), verb);
        }
        syn::visit::visit_expr_method_call(self, mc);
    }
}

/// Every handler the configured router presets register, anywhere in the workspace: a free function
/// passed to a registration (axum, actix-web) or one that carries a route attribute (`#[get(..)]`,
/// `#[route(..)]`; actix-web, rocket).
fn handlers(ws: &Workspace) -> Vec<Handler> {
    let routers = &ws.config.routers;
    let axum = routers.contains("axum");
    let actix = routers.contains("actix-web");
    let attributes = actix || routers.contains("rocket");
    let mut by_fn: std::collections::BTreeMap<(usize, String), Handler> = Default::default();
    let mut add = |module: usize, name: String, line: usize, verb: String| {
        by_fn
            .entry((module, name.clone()))
            .or_insert_with(|| Handler {
                module,
                name,
                line,
                verbs: Default::default(),
            })
            .verbs
            .insert(verb);
    };
    for m in ws.project_modules() {
        if axum || actix {
            let mut r = Registrations {
                ws,
                m,
                axum,
                actix,
                found: vec![],
            };
            for item in &ws.modules[m].items {
                syn::visit::Visit::visit_item(&mut r, item);
            }
            for (path, verb) in r.found {
                if let Some(Target::Decl(d)) = ws.resolve_path(m, &path)
                    && ws.decls[d].kind == DeclKind::Fn
                {
                    add(
                        ws.decls[d].module,
                        ws.decl_name(d).to_string(),
                        ws.decls[d].line,
                        verb,
                    );
                }
            }
        }
        if attributes {
            for item in &ws.modules[m].items {
                let syn::Item::Fn(f) = item else { continue };
                for a in &f.attrs {
                    let Some(last) = a.path().segments.last() else {
                        continue;
                    };
                    let name = last.ident.to_string();
                    let verb = if VERBS.contains(&name.as_str()) {
                        name
                    } else if name == "route" {
                        let tokens = a.meta.to_token_stream().to_string();
                        if tokens.contains("GET") {
                            "get"
                        } else {
                            "route"
                        }
                        .to_string()
                    } else {
                        continue;
                    };
                    add(
                        m,
                        f.sig.ident.to_string(),
                        f.sig.ident.span().start().line,
                        verb,
                    );
                }
            }
        }
    }
    by_fn.into_values().collect()
}

/// DCA-HEX-018 (experimental): a handler the router presets register sits in the incoming module
/// of an adapters crate.
pub fn hex_018(ws: &Workspace) -> Vec<Violation> {
    handlers(ws)
        .into_iter()
        .filter(|h| !ws.module_in_side(h.module, "incoming"))
        .map(|h| {
            at_line(ws, "DCA-HEX-018", h.module, &h.name, h.line, format!(
                "`{}` is registered as a handler outside the incoming module of an adapters crate",
                h.name
            ))
        })
        .collect()
}

/// DCA-HEX-014 (experimental): a handler registered for GET names no command - it reads, so it
/// drives no command port.
pub fn hex_014(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for h in handlers(ws)
        .into_iter()
        .filter(|h| h.verbs.contains("get") && ws.module_in_side(h.module, "incoming"))
    {
        let Some(item) = ws.modules[h.module]
            .items
            .iter()
            .find(|i| matches!(i, syn::Item::Fn(f) if f.sig.ident == h.name))
        else {
            continue;
        };
        let mut c = crate::model::PathCollector::default();
        syn::visit::Visit::visit_item(&mut c, item);
        let mut seen = std::collections::BTreeSet::new();
        for p in &c.paths {
            if let Some(Target::Decl(d)) = ws.resolve_path(h.module, p)
                && ws.decl_name(d).ends_with("Command")
                && in_layer(ws, d, Layer::Application)
                && seen.insert(d)
            {
                out.push(at_line(ws, "DCA-HEX-014", h.module, &h.name, p.span().start().line, format!(
                    "`{}` answers GET and builds the command `{}` - a GET reads; register the handler for POST, PUT, PATCH or DELETE",
                    h.name,
                    ws.decl_name(d)
                )));
            }
        }
    }
    out
}

// --- what outgoing adapters do to an aggregate -----------------------------------------------------

/// DCA-HEX-021 (experimental): an outgoing adapter calls no `&mut self` method of an aggregate - it
/// stores the aggregate's state, it does not change it. A method of a building-block trait passes.
pub fn hex_021(ws: &Workspace) -> Vec<Violation> {
    struct Changes<'a> {
        receivers: &'a super::Receivers,
        mutating: &'a std::collections::BTreeSet<String>,
        found: Vec<(usize, String)>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Changes<'_> {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            if self.mutating.contains(&mc.method.to_string())
                && let Some(r) = self.receivers.named_by(&mc.receiver)
            {
                self.found
                    .push((mc.method.span().start().line, format!("{r}.{}", mc.method)));
            }
            syn::visit::visit_expr_method_call(self, mc);
        }
    }
    let roots: Vec<usize> = ws
        .types_with_role("AggregateRoot")
        .filter(|d| !ws.is_building_block_decl(*d))
        .collect();
    let mut out = vec![];
    for m in side_modules(ws, "outgoing", false) {
        for f in super::functions(ws, m) {
            for d in &roots {
                let mutating: std::collections::BTreeSet<String> = ws
                    .methods(*d)
                    .into_iter()
                    .filter(|mm| {
                        mm.receiver_mut()
                            && mm
                                .trait_target
                                .as_ref()
                                .is_none_or(|t| ws.building_block(t).is_none())
                    })
                    .map(|mm| mm.name)
                    .collect();
                if mutating.is_empty() {
                    continue;
                }
                let receivers = super::Receivers::of(ws, m, &f, &|t| *t == Target::Decl(*d));
                if receivers.is_empty() {
                    continue;
                }
                let mut v = Changes {
                    receivers: &receivers,
                    mutating: &mutating,
                    found: vec![],
                };
                syn::visit::Visit::visit_block(&mut v, f.block);
                for (line, call) in v.found {
                    out.push(at_line(ws, "DCA-HEX-021", m, &f.name(), line, format!(
                        "`{}` calls `{call}`, which changes the aggregate `{}` - an outgoing adapter stores what the aggregate decided",
                        f.name(),
                        ws.decl_name(*d)
                    )));
                }
            }
        }
    }
    out
}

// --- enums formatted for the outside ---------------------------------------------------------------

const FORMAT_MACROS: &[&str] = &[
    "format",
    "format_args",
    "write",
    "writeln",
    "print",
    "println",
    "eprint",
    "eprintln",
    "panic",
];

/// The arguments a format string formats with `?`: positional indexes and names.
fn debug_arguments(fmt: &str) -> (Vec<usize>, Vec<String>) {
    let mut positions = vec![];
    let mut names = vec![];
    let mut next = 0;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
            let mut inner = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                inner.push(c);
            }
            let (arg, spec) = inner.split_once(':').unwrap_or((inner.as_str(), ""));
            let arg = arg.trim();
            let index = if arg.is_empty() {
                next += 1;
                Some(next - 1)
            } else {
                arg.parse::<usize>().ok()
            };
            if spec.contains('?') {
                match index {
                    Some(i) => positions.push(i),
                    None => names.push(arg.to_string()),
                }
            }
        }
    }
    (positions, names)
}

/// DCA-HEX-016 (experimental, informational): an adapter writes the outside's representation, not
/// the name of an inner enum's variant - lists `.to_string()` and `{:?}` applied to an enum declared
/// in a domain or application crate.
pub fn hex_016(ws: &Workspace) -> Vec<Violation> {
    struct Formats<'w> {
        ws: &'w Workspace,
        m: usize,
        receivers: &'w super::Receivers,
        found: Vec<(usize, String, &'static str)>,
    }
    impl Formats<'_> {
        /// The enum an expression is, by name: a receiver typed as one, or a path to one of its
        /// variants.
        fn inner_enum(&self, e: &syn::Expr) -> Option<String> {
            if let Some(r) = self.receivers.named_by(e) {
                return Some(r);
            }
            let syn::Expr::Path(p) = e else { return None };
            if let Some(Target::Decl(d)) = self.ws.resolve_path(self.m, &p.path)
                && is_inner_enum(self.ws, d)
            {
                return Some(super::render_path(&p.path));
            }
            None
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Formats<'_> {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            if mc.method == "to_string"
                && let Some(what) = self.inner_enum(&mc.receiver)
            {
                self.found
                    .push((mc.method.span().start().line, what, "`.to_string()`"));
            }
            syn::visit::visit_expr_method_call(self, mc);
        }

        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            let is_format = mac
                .path
                .segments
                .last()
                .is_some_and(|s| FORMAT_MACROS.contains(&s.ident.to_string().as_str()));
            if is_format
                && let Ok(args) = mac.parse_body_with(
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                )
            {
                let args: Vec<&syn::Expr> = args.iter().collect();
                if let Some(at) = args.iter().position(
                    |a| matches!(a, syn::Expr::Lit(l) if matches!(l.lit, syn::Lit::Str(_))),
                ) && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = args[at]
                {
                    let (positions, names) = debug_arguments(&s.value());
                    let rest = &args[at + 1..];
                    let positional: Vec<&syn::Expr> = rest
                        .iter()
                        .copied()
                        .filter(|a| !matches!(a, syn::Expr::Assign(_)))
                        .collect();
                    let mut formatted: Vec<syn::Expr> = positions
                        .iter()
                        .filter_map(|i| positional.get(*i).map(|e| (*e).clone()))
                        .collect();
                    for n in names {
                        let named = rest.iter().find_map(|a| match a {
                            syn::Expr::Assign(asg)
                                if matches!(&*asg.left, syn::Expr::Path(p) if p.path.is_ident(&n)) =>
                            {
                                Some((*asg.right).clone())
                            }
                            _ => None,
                        });
                        match named {
                            Some(e) => formatted.push(e),
                            None => {
                                if let Ok(e) = syn::parse_str::<syn::Expr>(&n) {
                                    formatted.push(e);
                                }
                            }
                        }
                    }
                    for e in formatted {
                        if let Some(what) = self.inner_enum(&e) {
                            self.found.push((s.span().start().line, what, "`{:?}`"));
                        }
                    }
                }
            }
            syn::visit::visit_macro(self, mac);
        }
    }
    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) == Some(Layer::Adapters))
    {
        for f in super::functions(ws, m) {
            let receivers = super::Receivers::of(
                ws,
                m,
                &f,
                &|t| matches!(t, Target::Decl(d) if is_inner_enum(ws, *d)),
            );
            let mut v = Formats {
                ws,
                m,
                receivers: &receivers,
                found: vec![],
            };
            syn::visit::Visit::visit_block(&mut v, f.block);
            for (line, what, how) in v.found {
                out.push(at_line(ws, "DCA-HEX-016", m, &f.name(), line, format!(
                    "`{}` writes `{what}` with {how} - check: is the variant's name the outside's representation, or should the adapter map it?",
                    f.name()
                )));
            }
        }
    }
    out
}

fn is_inner_enum(ws: &Workspace, d: usize) -> bool {
    ws.decls[d].kind == DeclKind::Enum
        && !ws.is_building_block_decl(d)
        && matches!(
            ws.layer_of_decl(d),
            Some(Layer::Domain | Layer::Application)
        )
}
