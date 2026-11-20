//! Failure rules. Rust has no exceptions: a failure is a type returned in `Err`, marked with
//! DomainFailure, UseCaseFailure or OutputPortFailure; the last two are port failures.

use std::collections::{BTreeMap, BTreeSet};

use syn::spanned::Spanned;

use super::{at, at_line, in_layer, target_has_role};
use crate::Violation;
use crate::model::{Layer, Target, Workspace};

fn is_error_type(ws: &Workspace, d: usize) -> bool {
    ws.implements[d].iter().any(|t| matches!(t, Target::External { krate, path } if matches!(krate.as_str(), "std" | "core") && path.ends_with(&["error".to_string(), "Error".to_string()])))
}

pub fn err_001(ws: &Workspace) -> Vec<Violation> {
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

pub fn err_002(ws: &Workspace) -> Vec<Violation> {
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

pub fn err_003(ws: &Workspace) -> Vec<Violation> {
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

pub fn err_006(ws: &Workspace) -> Vec<Violation> {
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
pub fn err_008(ws: &Workspace) -> Vec<Violation> {
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

pub fn err_005(ws: &Workspace) -> Vec<Violation> {
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
pub fn err_009(ws: &Workspace) -> Vec<Violation> {
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
        syn::Pat::Ident(i) => binding_counts && i.subpat.is_none(),
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
    receivers: &'a super::Receivers,
    found: Vec<(usize, String, String)>,
    in_catch_unwind: usize,
}

impl PortAnswers<'_> {
    /// `receiver.method` when an expression is a call of a method on a port.
    fn port_call(&self, e: &syn::Expr) -> Option<String> {
        match bare(e) {
            syn::Expr::MethodCall(mc) => self
                .receivers
                .named_by(&mc.receiver)
                .map(|r| format!("{r}.{}", mc.method)),
            _ => None,
        }
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
                if let Some(how) = unnamed_err_arm(&arm.pat, err_named) {
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
/// naming it.
pub fn err_010(ws: &Workspace) -> Vec<Violation> {
    let is_port =
        |t: &Target| target_has_role(ws, t, "InputPort") || target_has_role(ws, t, "OutputPort");
    let mut out = vec![];
    for m in super::application_and_incoming(ws) {
        for f in super::functions(ws, m) {
            let receivers = super::Receivers::of(ws, m, &f, &is_port);
            if receivers.is_empty() {
                continue;
            }
            let mut v = PortAnswers {
                receivers: &receivers,
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

/// DCA-ERR-007 (informational): a failure carries values, not prose and not the model's objects -
/// lists every field of a failure whose type is `String` or names an aggregate or an entity.
pub fn err_007(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    let failures: BTreeSet<usize> = ws
        .types_with_role("DomainFailure")
        .chain(ws.types_with_role("PortFailure"))
        .filter(|d| !ws.is_building_block_decl(*d))
        .collect();
    for d in failures {
        for (f, targets) in super::field_targets(ws, d) {
            let text = super::type_idents(f.ty).iter().any(|i| i == "String");
            let model = targets.iter().find(|t| {
                target_has_role(ws, t, "AggregateRoot") || target_has_role(ws, t, "Entity")
            });
            let what = match (text, model) {
                (_, Some(t)) => format!("the model object `{}`", ws.target_name(t)),
                (true, None) => "a `String`".to_string(),
                (false, None) => continue,
            };
            out.push(at(
                ws,
                "DCA-ERR-007",
                d,
                format!(
                    "failure `{}` carries {what} in `{}` - check: does it carry the values the \
                     caller needs to answer, by name?",
                    ws.decl_name(d),
                    super::field_label(&f)
                ),
            ));
        }
    }
    out
}
