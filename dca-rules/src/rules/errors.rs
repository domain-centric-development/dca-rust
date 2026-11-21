//! Failure rules. Rust has no exceptions: a failure is a type returned in `Err`, marked with
//! DomainFailure, UseCaseFailure or OutputPortFailure; the last two are port failures.

use std::collections::{BTreeMap, BTreeSet};

use syn::spanned::Spanned;

use super::{at, at_line, in_layer, target_has_role};
use crate::Violation;
use crate::model::{Layer, Target, Workspace};

/// Whether a type is an error type: it implements `std::error::Error`, or it carries a derive
/// `[workspace.metadata.dca] error-derives` lists (`thiserror::Error` by default).
pub(crate) fn is_error_type(ws: &Workspace, d: usize) -> bool {
    ws.implements[d].iter().any(|t| matches!(t, Target::External { krate, path } if matches!(krate.as_str(), "std" | "core") && path.ends_with(&["error".to_string(), "Error".to_string()])))
        || derives_error(ws, d)
}

/// Whether a type carries a configured error derive: the derive resolves to `crate::Name` of a
/// configured entry, or is written as one.
fn derives_error(ws: &Workspace, d: usize) -> bool {
    let attrs: &[syn::Attribute] = match ws.item(d) {
        syn::Item::Struct(s) => &s.attrs,
        syn::Item::Enum(e) => &e.attrs,
        _ => return false,
    };
    let m = ws.decls[d].module;
    let written: BTreeSet<String> = ws
        .config
        .error_derives
        .iter()
        .map(|c| c.replace('-', "_"))
        .collect();
    attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .filter_map(|a| {
            a.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .ok()
        })
        .flatten()
        .any(|p| {
            written.contains(&super::render_path(&p))
                || ws
                    .resolve_path(m, &p)
                    .is_some_and(|t| ws.is_configured(&t, &ws.config.error_derives))
        })
}

/// A failure type of the project: a type with the DomainFailure, UseCaseFailure or OutputPortFailure
/// role, or an error type in a domain or application crate.
fn is_failure(ws: &Workspace, d: usize) -> bool {
    !ws.is_building_block_decl(d)
        && (ws.has_role(d, "DomainFailure")
            || ws.has_role(d, "UseCaseFailure")
            || ws.has_role(d, "OutputPortFailure")
            || (matches!(
                ws.layer_of_decl(d),
                Some(Layer::Domain | Layer::Application)
            ) && is_error_type(ws, d)))
}

/// DCA-ERR-004: a failure carries no framework metadata - no derive or attribute of a configured
/// framework crate, and no impl of one of its traits (`impl IntoResponse for ...`): how the outside
/// answers a failure is the incoming adapter's decision.
pub(super) fn err_004(ws: &Workspace) -> Vec<Violation> {
    let frameworks: BTreeSet<String> = ws
        .config
        .framework_crates
        .iter()
        .map(|c| c.replace('-', "_"))
        .collect();
    let mut out = vec![];
    for d in ws.type_decls().filter(|d| is_failure(ws, *d)) {
        let attrs: &[syn::Attribute] = match ws.item(d) {
            syn::Item::Struct(s) => &s.attrs,
            syn::Item::Enum(e) => &e.attrs,
            _ => &[],
        };
        for hit in super::events::framework_attr(ws, ws.decls[d].module, attrs) {
            out.push(at(
                ws,
                "DCA-ERR-004",
                d,
                format!(
                    "failure `{}` carries `{hit}` from a framework; the incoming adapter decides how the outside answers it",
                    ws.decl_name(d)
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for t in &ws.implements[d] {
            if let Target::External { krate, .. } = t
                && frameworks.contains(krate)
                && seen.insert(ws.target_name(t))
            {
                out.push(at(
                    ws,
                    "DCA-ERR-004",
                    d,
                    format!(
                        "failure `{}` implements `{}` of a framework; the incoming adapter translates it",
                        ws.decl_name(d),
                        ws.target_name(t)
                    ),
                ));
            }
        }
    }
    out
}

pub(super) fn err_001(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| {
            in_layer(ws, *d, Layer::Domain)
                && is_error_type(ws, *d)
                && !ws.has_role(*d, "DomainFailure")
        })
        .map(|d| {
            at(
                ws,
                "DCA-ERR-001",
                d,
                format!(
                    "failure `{}` in the domain does not implement DomainFailure",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub(super) fn err_002(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws
        .types_with_role("DomainFailure")
        .filter(|d| !in_layer(ws, *d, Layer::Domain))
    {
        out.push(at(
            ws,
            "DCA-ERR-002",
            d,
            format!(
                "domain failure `{}` sits outside the domain crate",
                ws.decl_name(d)
            ),
        ));
    }
    for d in ws
        .types_with_role("PortFailure")
        .filter(|d| !in_layer(ws, *d, Layer::Application))
    {
        out.push(at(
            ws,
            "DCA-ERR-002",
            d,
            format!(
                "port failure `{}` - use-case or output-port - sits outside the application crate",
                ws.decl_name(d)
            ),
        ));
    }
    out
}

pub(super) fn err_003(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| in_layer(ws, *d, Layer::Application) && is_error_type(ws, *d))
        .filter(|d| {
            !ws.has_role(*d, "DomainFailure")
                && !ws.has_role(*d, "UseCaseFailure")
                && !ws.has_role(*d, "OutputPortFailure")
        })
        .map(|d| {
            at(
                ws,
                "DCA-ERR-003",
                d,
                format!(
                    "failure `{}` in the application implements neither UseCaseFailure nor \
                     OutputPortFailure",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

/// The enums an input port answers with, each with its variant names: an enum the port trait names in
/// an associated-type binding of a bound (`UseCase<C, Output = …>`) or in a method's return type,
/// through `Result`, `Option` and any other wrapper.
fn closed_results(ws: &Workspace) -> BTreeMap<usize, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for port in ws.traits_with_role("InputPort") {
        if ws.is_building_block_decl(port) {
            continue;
        }
        let syn::Item::Trait(t) = ws.item(port) else {
            continue;
        };
        let m = ws.decls[port].module;
        let mut answers: Vec<&syn::Type> = vec![];
        for bound in &t.supertraits {
            if let syn::TypeParamBound::Trait(tb) = bound {
                for seg in &tb.path.segments {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        for arg in &args.args {
                            if let syn::GenericArgument::AssocType(binding) = arg {
                                answers.push(&binding.ty);
                            }
                        }
                    }
                }
            }
        }
        for method in ws.trait_methods(port) {
            if let syn::ReturnType::Type(_, ty) = &method.sig.output {
                answers.push(ty);
            }
        }
        for ty in answers {
            for d in ws.type_decls_in(m, ty, &[&t.generics]) {
                if let syn::Item::Enum(e) = ws.item(d) {
                    out.entry(d).or_insert_with(|| {
                        e.variants.iter().map(|v| v.ident.to_string()).collect()
                    });
                }
            }
        }
    }
    out
}

pub(super) fn err_006(ws: &Workspace) -> Vec<Violation> {
    let failure = |t: &Target| {
        matches!(t, Target::Decl(d) if !ws.is_building_block_decl(*d)
            && (ws.has_role(*d, "DomainFailure") || ws.has_role(*d, "UseCaseFailure")))
    };
    let any_failure_type = (0..ws.decls.len()).any(|d| failure(&Target::Decl(d)));
    let results = closed_results(ws);
    if !any_failure_type && results.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_in_side(*m, "incoming"))
    {
        let targets = ws.module_targets(m);
        let mut bases: BTreeSet<String> = BTreeSet::new();
        for (t, _, _) in &targets {
            if let Some(bb) = ws.building_block(t)
                && matches!(
                    bb.as_str(),
                    "PortFailure" | "InputPortFailure" | "OutputPortFailure"
                )
            {
                bases.insert(bb);
            }
        }
        for base in bases {
            out.push(at_line(
                ws,
                "DCA-ERR-006",
                m,
                "",
                ws.modules[m].line,
                format!(
                    "names the port-failure classification {base} - match the named failures, each \
                     with its own answer"
                ),
            ));
        }
        let drives = targets
            .iter()
            .any(|(t, _, _)| target_has_role(ws, t, "InputPort"));
        if !drives {
            continue;
        }
        let translates = targets.iter().any(|(t, _, text)| {
            failure(t)
                || matches!(t, Target::Decl(d) if results.get(d).is_some_and(|variants| {
                    text.rsplit("::").next().is_some_and(|last| variants.contains(last))
                }))
        });
        if !translates {
            out.push(at_line(
                ws,
                "DCA-ERR-006",
                m,
                "",
                ws.modules[m].line,
                "drives an input port and names neither a failure type of the inner layers nor a \
                 variant of a closed result",
            ));
        }
    }
    out
}

/// Diagnostic: lists every closed result an input port answers with none of whose variants carries
/// a value whose type implements `Refusal`. A closed result may hold only outcomes that are not
/// refusals; whether a variant refuses is domain knowledge the type does not show, so the rule asks
/// rather than fails.
pub(super) fn err_008(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (d, variants) in closed_results(ws) {
        // The Err side of a Result is a failure, not a closed result.
        if ws.is_building_block_decl(d)
            || !in_layer(ws, d, Layer::Application)
            || is_error_type(ws, d)
            || ws.has_role(d, "DomainFailure")
            || ws.has_role(d, "PortFailure")
        {
            continue;
        }
        let syn::Item::Enum(e) = ws.item(d) else {
            continue;
        };
        let m = ws.decls[d].module;
        let refuses = e.variants.iter().any(|v| {
            v.fields.iter().any(|f| {
                ws.type_decls_in(m, &f.ty, &[&e.generics])
                    .into_iter()
                    .any(|t| ws.has_role(t, "Refusal"))
            })
        });
        if !refuses {
            out.push(at(
                ws,
                "DCA-ERR-008",
                d,
                format!(
                    "closed result `{}` answers with {:?} and none of its variants carries a \
                     Refusal - check: is none of these outcomes a refusal? If one is, give it a value \
                     whose type implements Refusal, or the transaction commits when it is returned",
                    ws.decl_name(d),
                    variants
                ),
            ));
        }
    }
    out
}

pub(super) fn err_005(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("DomainFailure")
        .chain(ws.types_with_role("PortFailure"))
        .filter(|d| {
            ["Http", "StatusCode", "ResponseStatus", "Response"]
                .iter()
                .any(|w| ws.decl_name(*d).contains(w))
        })
        .map(|d| {
            at(
                ws,
                "DCA-ERR-005",
                d,
                format!(
                    "failure `{}` is named in the language of the transport",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

/// A domain failure is constructed only in a domain crate: the model refuses, a use case never
/// invents a domain failure. A construction is an expression that makes a value of a type that
/// implements DomainFailure - a struct literal, a tuple-struct or variant call, a unit struct or
/// unit variant used as a value, a variant or tuple struct passed as a function, or a call of an
/// associated function that returns `Self` or the type itself.
pub(super) fn err_009(ws: &Workspace) -> Vec<Violation> {
    struct Constructions<'w> {
        ws: &'w Workspace,
        module: usize,
        found: Vec<(usize, usize)>,
    }
    impl Constructions<'_> {
        /// The domain failure a path in expression position constructs, if any.
        fn constructed(&self, path: &syn::Path) -> Option<usize> {
            let ws = self.ws;
            if path.segments.first().is_some_and(|s| s.ident == "Self") {
                return None;
            }
            let failure = |t: Option<Target>| match t {
                Some(Target::Decl(d))
                    if ws.has_role(d, "DomainFailure") && !ws.is_building_block_decl(d) =>
                {
                    Some(d)
                }
                _ => None,
            };
            let last = path.segments.last()?.ident.to_string();
            let d = match failure(ws.resolve_path(self.module, path)) {
                Some(d) => d,
                None if path.segments.len() > 1 => {
                    let mut owner = path.clone();
                    owner.segments.pop();
                    failure(ws.resolve_path(self.module, &owner))?
                }
                None => return None,
            };
            if matches!(ws.item(d), syn::Item::Struct(_)) && last == ws.decl_name(d) {
                return Some(d);
            }
            if let syn::Item::Enum(e) = ws.item(d)
                && e.variants.iter().any(|v| v.ident == last)
            {
                return Some(d);
            }
            let name = ws.decl_name(d).to_string();
            ws.methods(d)
                .iter()
                .any(|m| {
                    m.name == last
                        && !m.has_receiver()
                        && (m.returns_self()
                            || matches!(&m.sig.output, syn::ReturnType::Type(_, ty)
                                if super::type_idents(ty).contains(&name)))
                })
                .then_some(d)
        }

        fn note(&mut self, path: &syn::Path, line: usize) {
            if let Some(d) = self.constructed(path) {
                self.found.push((d, line));
            }
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Constructions<'_> {
        fn visit_expr_struct(&mut self, e: &'ast syn::ExprStruct) {
            self.note(&e.path, e.path.span().start().line);
            syn::visit::visit_expr_struct(self, e);
        }
        fn visit_expr_call(&mut self, e: &'ast syn::ExprCall) {
            if let syn::Expr::Path(p) = &*e.func {
                self.note(&p.path, p.path.span().start().line);
                for a in &e.args {
                    self.visit_expr(a);
                }
                return;
            }
            syn::visit::visit_expr_call(self, e);
        }
        fn visit_expr_path(&mut self, e: &'ast syn::ExprPath) {
            self.note(&e.path, e.path.span().start().line);
            syn::visit::visit_expr_path(self, e);
        }
        // A pattern names a variant to match it and constructs nothing; syn holds a path pattern
        // (`Failure::Unit =>`) as an expression path, so patterns are not entered.
        fn visit_pat(&mut self, _: &'ast syn::Pat) {}
    }

    let mut out = vec![];
    for m in ws
        .project_modules()
        .filter(|m| ws.module_layer(*m) != Some(Layer::Domain))
    {
        let mut v = Constructions {
            ws,
            module: m,
            found: vec![],
        };
        for item in &ws.modules[m].items {
            syn::visit::Visit::visit_item(&mut v, item);
        }
        for (d, line) in v.found {
            out.push(at_line(
                ws,
                "DCA-ERR-009",
                m,
                "",
                line,
                format!(
                    "constructs domain failure `{}` outside the domain crate - only the domain \
                     raises a domain failure; a failure the application finds is a variant of its \
                     result or a UseCaseFailure",
                    ws.decl_name(d)
                ),
            ));
        }
    }
    out
}

/// The expression under parentheses and `.await`.
fn bare(e: &syn::Expr) -> &syn::Expr {
    match e {
        syn::Expr::Paren(p) => bare(&p.expr),
        syn::Expr::Await(a) => bare(&a.base),
        _ => e,
    }
}

/// Whether a pattern matches anything without naming it: `_`, `..`, or a binding that covers
/// every value (`other`) - the latter only where `binding_counts`.
fn catches_all(p: &syn::Pat, binding_counts: bool) -> bool {
    match p {
        syn::Pat::Wild(_) | syn::Pat::Rest(_) => true,
        // `None`, `Empty`: a unit variant or a constant, not a binding.
        syn::Pat::Ident(i) => {
            binding_counts
                && i.subpat.is_none()
                && !i.ident.to_string().starts_with(char::is_uppercase)
        }
        syn::Pat::Paren(p) => catches_all(&p.pat, binding_counts),
        syn::Pat::Type(t) => catches_all(&t.pat, binding_counts),
        _ => false,
    }
}

/// The variant a tuple-struct pattern names (`Ok`, `Err`) with its one inner pattern.
fn variant_pat(p: &syn::Pat) -> Option<(String, Option<&syn::Pat>)> {
    match p {
        syn::Pat::TupleStruct(ts) => {
            Some((ts.path.segments.last()?.ident.to_string(), ts.elems.first()))
        }
        syn::Pat::Paren(p) => variant_pat(&p.pat),
        _ => None,
    }
}

/// The cases of a pattern, or-patterns split.
fn cases(p: &syn::Pat) -> Vec<&syn::Pat> {
    match p {
        syn::Pat::Or(o) => o.cases.iter().flat_map(cases).collect(),
        _ => vec![p],
    }
}

/// How a pattern over a port call's `Result` answers the failure without naming it, if it does:
/// `Err(_)`, or `_` / a bare binding that the failure reaches.
fn unnamed_err_arm(p: &syn::Pat, err_named_before: bool) -> Option<String> {
    for case in cases(p) {
        if let Some((v, inner)) = variant_pat(case)
            && v == "Err"
            && inner.is_none_or(|i| catches_all(i, false))
        {
            return Some("an `Err(_)` arm".into());
        }
        if !err_named_before && catches_all(case, true) {
            return Some("a catch-all arm".into());
        }
    }
    None
}

/// Whether a catch-all arm only cleans up and hands what it caught on unchanged: its pattern is a
/// bare binding (`other`) and every exit of its body - the tail, each `return` - is that binding
/// itself, not converted, wrapped or replaced. Such an arm does not answer the failure.
fn hands_on(p: &syn::Pat, body: &syn::Expr) -> bool {
    let name = match p {
        syn::Pat::Ident(i) if i.subpat.is_none() && i.by_ref.is_none() => i.ident.to_string(),
        syn::Pat::Paren(p) => return hands_on(&p.pat, body),
        syn::Pat::Type(t) => return hands_on(&t.pat, body),
        _ => return false,
    };
    let mut returns = Returns {
        name: &name,
        all_hand_on: true,
    };
    syn::visit::Visit::visit_expr(&mut returns, body);
    returns.all_hand_on && tail_hands_on(body, &name)
}

/// Whether an expression is the binding `name` itself, under parentheses.
fn is_binding(e: &syn::Expr, name: &str) -> bool {
    match e {
        syn::Expr::Paren(p) => is_binding(&p.expr, name),
        syn::Expr::Path(p) => p.qself.is_none() && p.path.is_ident(name),
        _ => false,
    }
}

/// Whether every way an expression ends hands on the binding `name`: the binding itself, a
/// `return` (checked by [`Returns`]), or a block, `if` with `else` or `match` whose branches do.
fn tail_hands_on(e: &syn::Expr, name: &str) -> bool {
    match e {
        syn::Expr::Return(_) => true,
        syn::Expr::Paren(p) => tail_hands_on(&p.expr, name),
        syn::Expr::Block(b) => block_hands_on(&b.block, name),
        syn::Expr::Unsafe(b) => block_hands_on(&b.block, name),
        syn::Expr::If(i) => {
            block_hands_on(&i.then_branch, name)
                && i.else_branch
                    .as_ref()
                    .is_some_and(|(_, e)| tail_hands_on(e, name))
        }
        syn::Expr::Match(m) => m.arms.iter().all(|a| tail_hands_on(&a.body, name)),
        _ => is_binding(e, name),
    }
}

fn block_hands_on(b: &syn::Block, name: &str) -> bool {
    match b.stmts.last() {
        Some(syn::Stmt::Expr(e, None)) => tail_hands_on(e, name),
        Some(syn::Stmt::Expr(e @ syn::Expr::Return(_), Some(_))) => tail_hands_on(e, name),
        _ => false,
    }
}

/// Whether every `return` of an arm's body - closures, async blocks and nested items left out,
/// their `return` is not the arm's - hands on the binding `name`.
struct Returns<'n> {
    name: &'n str,
    all_hand_on: bool,
}

impl<'ast> syn::visit::Visit<'ast> for Returns<'_> {
    fn visit_expr_return(&mut self, r: &'ast syn::ExprReturn) {
        if !r.expr.as_deref().is_some_and(|e| is_binding(e, self.name)) {
            self.all_hand_on = false;
        }
        syn::visit::visit_expr_return(self, r);
    }
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}
    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

/// Whether an arm's pattern takes the failure in hand: `Err(e)`.
fn names_err(p: &syn::Pat) -> bool {
    cases(p).into_iter().any(|c| {
        matches!(variant_pat(c), Some((v, Some(syn::Pat::Ident(i)))) if v == "Err" && i.subpat.is_none())
    })
}

/// Whether a closure's first parameter discards what it gets (`|_| ...`).
fn discards_arg(e: &syn::Expr) -> bool {
    match bare(e) {
        syn::Expr::Closure(c) => c.inputs.first().is_none_or(|p| catches_all(p, false)),
        _ => false,
    }
}

struct PortAnswers<'a> {
    ws: &'a Workspace,
    receivers: &'a super::Receivers,
    /// Locals bound to the result of a port call (`let r = port.run().await;`).
    results: super::Aliases,
    found: Vec<(usize, String, String)>,
    in_catch_unwind: usize,
}

/// `receiver.method` when an expression is a call of a method on a port that can fail: a call
/// whose method the receiver's type declares in the workspace (its trait, the traits that trait
/// extends, or its own impls) only with an answer other than a `Result` - an `Option`, a plain
/// value - is no failing call. A method the workspace does not declare (a building block's, a
/// receiver whose type is not written out) counts as failing.
fn port_call(ws: &Workspace, receivers: &super::Receivers, e: &syn::Expr) -> Option<String> {
    let syn::Expr::MethodCall(mc) = bare(e) else {
        return None;
    };
    let r = receivers.named_by(&mc.receiver)?;
    let method = mc.method.to_string();
    let answers = receivers
        .types
        .get(&r)
        .and_then(|targets| method_answers_result(ws, targets, &method));
    (answers != Some(false)).then(|| format!("{r}.{method}"))
}

/// Whether the method `name` of the types `targets` answers a `Result`, when the workspace declares
/// it on one of them.
fn method_answers_result(ws: &Workspace, targets: &[Target], name: &str) -> Option<bool> {
    let mut found = None;
    let mut seen = BTreeSet::new();
    let mut todo: Vec<usize> = targets
        .iter()
        .filter_map(|t| match t {
            Target::Decl(d) => Some(*d),
            _ => None,
        })
        .collect();
    while let Some(d) = todo.pop() {
        if !seen.insert(d) {
            continue;
        }
        let methods = if ws.decls[d].kind == crate::model::DeclKind::Trait {
            if let syn::Item::Trait(t) = ws.item(d) {
                for b in &t.supertraits {
                    if let syn::TypeParamBound::Trait(tb) = b
                        && let Some(Target::Decl(s)) = ws.resolve_path(ws.decls[d].module, &tb.path)
                    {
                        todo.push(s);
                    }
                }
            }
            ws.trait_methods(d)
        } else {
            ws.methods(d)
        };
        for m in methods.iter().filter(|m| m.name == name) {
            let fails = match &m.sig.output {
                syn::ReturnType::Type(_, ty) => answers_result(ty),
                syn::ReturnType::Default => false,
            };
            found = Some(found.unwrap_or(false) || fails);
        }
    }
    found
}

/// Whether a return type answers a `Result` (any type named `...Result`), looked through
/// `impl Future<Output = ..>`, `Pin<Box<dyn Future<Output = ..>>>` and `BoxFuture<'_, ..>`.
fn answers_result(ty: &syn::Type) -> bool {
    let future_output =
        |bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>| {
            bounds.iter().find_map(|b| match b {
                syn::TypeParamBound::Trait(tb) => {
                    let last = tb.path.segments.last()?;
                    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
                        return None;
                    };
                    args.args.iter().find_map(|a| match a {
                        syn::GenericArgument::AssocType(at) if at.ident == "Output" => {
                            Some(answers_result(&at.ty))
                        }
                        _ => None,
                    })
                }
                _ => None,
            })
        };
    match ty {
        syn::Type::ImplTrait(it) => future_output(&it.bounds).unwrap_or(false),
        syn::Type::TraitObject(to) => future_output(&to.bounds).unwrap_or(false),
        syn::Type::Paren(p) => answers_result(&p.elem),
        syn::Type::Group(g) => answers_result(&g.elem),
        syn::Type::Path(tp) => {
            let Some(last) = tp.path.segments.last() else {
                return false;
            };
            if last.ident.to_string().ends_with("Result") {
                return true;
            }
            let wraps = matches!(
                last.ident.to_string().as_str(),
                "Pin" | "Box" | "BoxFuture" | "LocalBoxFuture"
            );
            wraps
                && matches!(&last.arguments, syn::PathArguments::AngleBracketed(a)
                    if a.args.iter().any(|g| matches!(g, syn::GenericArgument::Type(t) if answers_result(t))))
        }
        _ => false,
    }
}

impl PortAnswers<'_> {
    /// `receiver.method` when an expression is a call of a method on a port, or a local bound to
    /// the result of one.
    fn port_call(&self, e: &syn::Expr) -> Option<String> {
        port_call(self.ws, self.receivers, e)
            .or_else(|| self.results.of(bare(e)).map(str::to_string))
    }

    fn note(&mut self, line: usize, call: String, how: impl Into<String>) {
        self.found.push((line, call, how.into()));
    }
}

impl<'ast> syn::visit::Visit<'ast> for PortAnswers<'_> {
    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        if let Some(call) = self.port_call(&mc.receiver) {
            let method = mc.method.to_string();
            let first = mc.args.first();
            let unnamed = match method.as_str() {
                "ok" | "unwrap_or" | "unwrap_or_default" | "or" | "is_ok" | "is_err" | "map_or" => {
                    true
                }
                "unwrap_or_else" | "map_err" | "or_else" | "map_or_else" => {
                    first.is_some_and(discards_arg)
                }
                _ => false,
            };
            if unnamed {
                self.note(
                    mc.method.span().start().line,
                    call,
                    format!("`.{method}(..)`"),
                );
            }
        } else if self.in_catch_unwind > 0
            && let Some(r) = self.receivers.named_by(&mc.receiver)
        {
            self.note(
                mc.method.span().start().line,
                format!("{r}.{}", mc.method),
                "`catch_unwind`",
            );
        }
        syn::visit::visit_expr_method_call(self, mc);
    }

    fn visit_expr_match(&mut self, e: &'ast syn::ExprMatch) {
        if let Some(call) = self.port_call(&e.expr) {
            let mut err_named = false;
            for arm in &e.arms {
                if let Some(how) = unnamed_err_arm(&arm.pat, err_named)
                    && !hands_on(&arm.pat, &arm.body)
                {
                    self.note(arm.pat.span().start().line, call.clone(), how);
                }
                err_named |= names_err(&arm.pat) && arm.guard.is_none();
            }
        }
        syn::visit::visit_expr_match(self, e);
    }

    fn visit_expr_let(&mut self, e: &'ast syn::ExprLet) {
        if let Some(call) = self.port_call(&e.expr) {
            let how = match variant_pat(&e.pat) {
                Some((v, _)) if v == "Ok" => Some("`if let Ok(..)`, which drops the failure"),
                _ if unnamed_err_arm(&e.pat, false).is_some() => Some("`if let Err(_)`"),
                _ => None,
            };
            if let Some(how) = how {
                self.note(e.pat.span().start().line, call, how);
            }
        }
        syn::visit::visit_expr_let(self, e);
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let Some(init) = &l.init
            && let Some(call) = self.port_call(&init.expr)
        {
            let how = if catches_all(&l.pat, false) {
                Some("`let _ =`, which drops the failure")
            } else if init.diverge.is_some()
                && matches!(variant_pat(&l.pat), Some((v, _)) if v == "Ok")
            {
                Some("`let Ok(..) = .. else`")
            } else {
                None
            };
            if let Some(how) = how {
                self.note(l.pat.span().start().line, call, how);
            }
        }
        syn::visit::visit_local(self, l);
    }

    fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
        let unwinds = matches!(&*c.func, syn::Expr::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "catch_unwind"));
        if unwinds {
            self.in_catch_unwind += 1;
        }
        syn::visit::visit_expr_call(self, c);
        if unwinds {
            self.in_catch_unwind -= 1;
        }
    }
}

/// DCA-ERR-010 (experimental). In the application and the incoming adapters, the failure of a call
/// on a port - a parameter, a written-out local or a field of `self` whose type carries the
/// input-port or output-port role - is propagated or matched by name, never answered without
/// naming it. A call of a method the port declares with an answer other than a `Result` is no
/// failing call.
pub(super) fn err_010(ws: &Workspace) -> Vec<Violation> {
    let is_port =
        |t: &Target| target_has_role(ws, t, "InputPort") || target_has_role(ws, t, "OutputPort");
    let mut out = vec![];
    for m in super::application_and_incoming(ws) {
        for f in super::functions(ws, m) {
            let receivers = super::Receivers::of(ws, m, &f, &is_port);
            if receivers.is_empty() {
                continue;
            }
            let results = super::aliases(f.block, &mut |e, _| port_call(ws, &receivers, e));
            let mut v = PortAnswers {
                ws,
                receivers: &receivers,
                results,
                found: vec![],
                in_catch_unwind: 0,
            };
            syn::visit::Visit::visit_block(&mut v, f.block);
            for (line, call, how) in v.found {
                out.push(at_line(
                    ws,
                    "DCA-ERR-010",
                    m,
                    &f.name(),
                    line,
                    format!(
                        "`{}` answers the failure of the port call `{call}` with {how} - every \
                         failure alike; propagate it with `?` or match the named failure",
                        f.name()
                    ),
                ));
            }
        }
    }
    out
}

/// Types a failure's field looks through, as Java looks through `java.util`: containers and smart
/// pointers of std.
const CONTAINERS: &[&str] = &[
    "Option",
    "Vec",
    "VecDeque",
    "LinkedList",
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "Box",
    "Rc",
    "Arc",
    "Cow",
];

const NUMBERS: &[&str] = &[
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
    "NonZeroU8",
    "NonZeroU16",
    "NonZeroU32",
    "NonZeroU64",
    "NonZeroU128",
    "NonZeroUsize",
    "NonZeroI8",
    "NonZeroI16",
    "NonZeroI32",
    "NonZeroI64",
    "NonZeroI128",
    "NonZeroIsize",
];

/// What a type a failure's field involves is, when it is listed: a plain string, an aggregate or
/// entity, or anything that is neither an id, a value, a number, an enum, a time type nor the
/// failure it wraps.
fn field_finding(
    ws: &Workspace,
    m: usize,
    generics: &[&syn::Generics],
    ty: &syn::Type,
) -> Vec<String> {
    let mut out = vec![];
    match ty {
        syn::Type::Reference(r) => return field_finding(ws, m, generics, &r.elem),
        syn::Type::Paren(p) => return field_finding(ws, m, generics, &p.elem),
        syn::Type::Group(g) => return field_finding(ws, m, generics, &g.elem),
        syn::Type::Slice(s) => return field_finding(ws, m, generics, &s.elem),
        syn::Type::Array(a) => return field_finding(ws, m, generics, &a.elem),
        syn::Type::Tuple(t) => {
            return t
                .elems
                .iter()
                .flat_map(|e| field_finding(ws, m, generics, e))
                .collect();
        }
        syn::Type::Path(tp) if tp.qself.is_none() => {
            let Some(last) = tp.path.segments.last() else {
                return out;
            };
            let name = last.ident.to_string();
            let args: Vec<&syn::Type> = match &last.arguments {
                syn::PathArguments::AngleBracketed(a) => a
                    .args
                    .iter()
                    .filter_map(|a| match a {
                        syn::GenericArgument::Type(t) => Some(t),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };
            for a in &args {
                out.extend(field_finding(ws, m, generics, a));
            }
            if name == "String" || name == "str" {
                out.push(format!("a plain string (`{name}`)"));
                return out;
            }
            // A wrapped cause (`std::io::Error`, `anyhow::Error`) and a span of time are fine.
            if CONTAINERS.contains(&name.as_str())
                || NUMBERS.contains(&name.as_str())
                || ws.config.timestamp_types.contains(&name)
                || name == "Error"
                || name == "Duration"
            {
                return out;
            }
            // The type itself, its arguments left out (read above); a generic parameter stands for
            // its bounds.
            let mut bare = tp.clone();
            if let Some(l) = bare.path.segments.last_mut() {
                l.arguments = syn::PathArguments::None;
            }
            let resolved = ws.type_targets(m, &syn::Type::Path(bare), generics);
            let mut model = None;
            let mut fine = false;
            for t in resolved {
                if target_has_role(ws, &t, "AggregateRoot") || target_has_role(ws, &t, "Entity") {
                    model = Some(ws.target_name(&t));
                } else if target_has_role(ws, &t, "Id")
                    || target_has_role(ws, &t, "Value")
                    // The cause a failure wraps, as Java's exception carries its cause.
                    || target_has_role(ws, &t, "DomainFailure")
                    || target_has_role(ws, &t, "PortFailure")
                    || matches!(t, Target::Decl(d) if is_error_type(ws, d))
                    || ws.building_block(&t).as_deref() == Some("PortError")
                    || matches!(t, Target::Decl(d) if ws.decls[d].kind == crate::model::DeclKind::Enum)
                {
                    fine = true;
                }
            }
            match model {
                Some(n) => out.push(format!("an aggregate or entity (`{n}`)")),
                None if fine => {}
                None => out.push(format!("neither an id, a value nor a number (`{name}`)")),
            }
        }
        // A wrapped cause: `dyn Error + Send + Sync`, `impl Error`.
        syn::Type::TraitObject(syn::TypeTraitObject { bounds, .. })
        | syn::Type::ImplTrait(syn::TypeImplTrait { bounds, .. })
            if bounds.iter().any(|b| {
                matches!(b, syn::TypeParamBound::Trait(tb)
                    if tb.path.segments.last().is_some_and(|s| s.ident == "Error"))
            }) => {}
        other => out.push(format!(
            "neither an id, a value nor a number (`{}`)",
            render_type(other)
        )),
    }
    out
}

/// A type as the finding names it: a trait object or `impl` by its bounds' paths.
fn render_type(ty: &syn::Type) -> String {
    let bounds = match ty {
        syn::Type::TraitObject(t) => Some(("dyn ", &t.bounds)),
        syn::Type::ImplTrait(t) => Some(("impl ", &t.bounds)),
        _ => None,
    };
    match bounds {
        Some((kw, bounds)) => {
            let parts: Vec<String> = bounds
                .iter()
                .filter_map(|b| match b {
                    syn::TypeParamBound::Trait(tb) => Some(super::render_path(&tb.path)),
                    syn::TypeParamBound::Lifetime(l) => Some(format!("'{}", l.ident)),
                    _ => None,
                })
                .collect();
            format!("{kw}{}", parts.join(" + "))
        }
        None => super::render(ty),
    }
}

/// DCA-ERR-007 (informational): a failure carries ids and figures, never display data or objects.
pub(super) fn err_007(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    let failures: BTreeSet<usize> = ws
        .types_with_role("DomainFailure")
        .chain(ws.types_with_role("PortFailure"))
        .filter(|d| !ws.is_building_block_decl(*d))
        .collect();
    for d in failures {
        let m = ws.decls[d].module;
        let generics = super::type_generics(ws, d);
        for f in ws.fields(d) {
            let mut findings = field_finding(ws, m, &generics, f.ty);
            findings.dedup();
            for what in findings {
                out.push(at(
                    ws,
                    "DCA-ERR-007",
                    d,
                    format!(
                        "failure `{}` carries {what} in `{}` - carry the id or the figure the rule compared",
                        ws.decl_name(d),
                        super::field_label(&f)
                    ),
                ));
            }
        }
    }
    out
}
