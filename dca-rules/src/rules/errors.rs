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
