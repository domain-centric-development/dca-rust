//! Rules only Rust needs.

use super::{at, at_line, field_label, field_targets};
use crate::Violation;
use crate::model::{Layer, Target, Workspace};

const SHARED: &[&str] = &["Rc", "Arc", "Cell", "RefCell", "Mutex", "RwLock", "Weak"];

pub fn rs_001(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for d in ws.type_decls().filter(|d| {
        matches!(
            ws.layer_of_decl(*d),
            Some(Layer::Domain | Layer::SharedKernel)
        )
    }) {
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

pub fn rs_002(ws: &Workspace) -> Vec<Violation> {
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

pub fn rs_003(ws: &Workspace) -> Vec<Violation> {
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

pub fn rs_004(ws: &Workspace) -> Vec<Violation> {
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
        for item in &ws.modules[m].items {
            syn::visit::Visit::visit_item(&mut v, item);
        }
        for (name, line) in v.0 {
            out.push(at_line(
                ws,
                "DCA-RS-004",
                m,
                &name,
                line,
                "the domain is synchronous; async belongs to ports and adapters",
            ));
        }
    }
    out
}

pub fn rs_006(ws: &Workspace) -> Vec<Violation> {
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
