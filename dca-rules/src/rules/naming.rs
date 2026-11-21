//! Naming rules.

use super::{at, in_layer, own_methods, use_cases};
use crate::Violation;
use crate::model::{Layer, Workspace};

pub(super) fn nam_001(ws: &Workspace) -> Vec<Violation> {
    use_cases(ws)
        .into_iter()
        .filter(|d| !ws.decl_name(*d).ends_with(&ws.config.suffixes.use_case))
        .map(|d| {
            at(
                ws,
                "DCA-NAM-001",
                d,
                format!(
                    "use case `{}` is not named *{}",
                    ws.decl_name(d),
                    ws.config.suffixes.use_case
                ),
            )
        })
        .collect()
}

fn input_port_traits(ws: &Workspace) -> Vec<usize> {
    ws.traits_with_role("InputPort")
        .filter(|d| in_layer(ws, *d, Layer::Application))
        .collect()
}

pub(super) fn nam_003(ws: &Workspace) -> Vec<Violation> {
    input_port_traits(ws)
        .into_iter()
        .filter(|d| {
            !matches!(ws.decl_name(*d), "InputPort" | "UseCase")
                && !ws.decl_name(*d).ends_with(&ws.config.suffixes.input_port)
        })
        .map(|d| {
            at(
                ws,
                "DCA-NAM-003",
                d,
                format!(
                    "input port `{}` is not named *{}",
                    ws.decl_name(d),
                    ws.config.suffixes.input_port
                ),
            )
        })
        .collect()
}

pub(super) fn nam_004(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| in_layer(ws, *d, Layer::Application))
        .filter(|d| {
            let suffix = &ws.config.suffixes.repository;
            ws.decl_name(*d).contains(suffix.as_str())
                && ws.decl_name(*d) != "Repository"
                && !ws.decl_name(*d).ends_with(suffix.as_str())
        })
        .map(|d| {
            at(
                ws,
                "DCA-NAM-004",
                d,
                format!(
                    "`{}` names a repository but does not end with {}",
                    ws.decl_name(d),
                    ws.config.suffixes.repository
                ),
            )
        })
        .collect()
}

fn suffix_outside_adapters(ws: &Workspace, rule: &'static str, suffix: &str) -> Vec<Violation> {
    ws.type_decls()
        .chain(ws.trait_decls())
        .filter(|d| ws.decl_name(*d).ends_with(suffix) && !in_layer(ws, *d, Layer::Adapters))
        .map(|d| {
            at(
                ws,
                rule,
                d,
                format!("`{}` sits outside the adapters crate", ws.decl_name(d)),
            )
        })
        .collect()
}

pub(super) fn nam_007(ws: &Workspace) -> Vec<Violation> {
    suffix_outside_adapters(ws, "DCA-NAM-007", "Dto")
}

pub(super) fn nam_008(ws: &Workspace) -> Vec<Violation> {
    suffix_outside_adapters(ws, "DCA-NAM-008", "Converter")
}

pub(super) fn nam_009(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for m in ws.project_modules() {
        let c = &ws.crates[ws.modules[m].krate];
        if c.context.is_none() && c.layer.is_none() {
            continue;
        }
        if let Some(name) = ws.modules[m].path.last()
            && matches!(
                name.as_str(),
                "entities" | "value_objects" | "valueobjects" | "helpers" | "util" | "utils"
            )
        {
            let parent = ws.modules[m].parent.unwrap_or(m);
            out.push(super::at_line(
                ws,
                "DCA-NAM-009",
                parent,
                name,
                ws.modules[m].line,
                format!("module `{name}` groups by technical kind; group by domain concept"),
            ));
        }
    }
    out
}

pub(super) fn nam_010(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .chain(ws.trait_decls())
        .filter(|d| in_layer(ws, *d, Layer::Domain))
        .filter(|d| {
            ["Helper", "Util", "Utils", "Impl", "Implementation"]
                .iter()
                .any(|s| ws.decl_name(*d).ends_with(s))
        })
        .map(|d| {
            at(
                ws,
                "DCA-NAM-010",
                d,
                format!(
                    "domain type `{}` carries a technical suffix",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub(super) fn nam_011(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| ws.decl_name(*d).ends_with("ViewModel"))
        .filter(|d| {
            !(ws.decl_in_side(*d, "incoming")
                && ws.modules[ws.decls[*d].module]
                    .path
                    .get(1)
                    .map(String::as_str)
                    == Some(ws.config.modules.web.as_str()))
        })
        .map(|d| {
            at(
                ws,
                "DCA-NAM-011",
                d,
                format!(
                    "view model `{}` sits outside adapters::incoming::web",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

fn operation_names(ws: &Workspace) -> Vec<(usize, String)> {
    let mut selected: Vec<usize> = input_port_traits(ws)
        .into_iter()
        .chain(use_cases(ws))
        .collect();
    // A type named as a use case without the input-port role is one by its name, as in Java.
    for d in ws.type_decls().filter(|d| {
        ws.layer_of_decl(*d) == Some(Layer::Application)
            && ws.decl_name(*d).ends_with(&ws.config.suffixes.use_case)
    }) {
        if !selected.contains(&d) {
            selected.push(d);
        }
    }
    selected
        .into_iter()
        .map(|d| {
            (
                d,
                ws.decl_name(d)
                    .trim_end_matches(ws.config.suffixes.use_case.as_str())
                    .trim_end_matches(ws.config.suffixes.input_port.as_str())
                    .to_string(),
            )
        })
        .collect()
}

pub(super) fn nam_012(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (d, name) in operation_names(ws) {
        let chars: Vec<char> = name.chars().collect();
        for word in ["On", "After", "When", "Upon", "Before"] {
            let w: Vec<char> = word.chars().collect();
            for i in 0..chars.len() {
                if chars[i..].starts_with(&w) {
                    let start_ok =
                        i == 0 || chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit();
                    let next_upper = chars.get(i + w.len()).is_some_and(|c| c.is_uppercase());
                    if start_ok && next_upper {
                        out.push(at(ws, "DCA-NAM-012", d, format!("`{}` is named after the trigger `{word}`; name it after its effect", ws.decl_name(d))));
                    }
                }
            }
        }
    }
    out
}

pub(super) fn nam_013(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for (d, name) in operation_names(ws) {
        for p in ["Set", "Update"] {
            if name.starts_with(p)
                && name[p.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_uppercase())
            {
                out.push(at(
                    ws,
                    "DCA-NAM-013",
                    d,
                    format!(
                        "`{}` is named after a field it writes; name the operation",
                        ws.decl_name(d)
                    ),
                ));
            }
        }
    }
    let field_named = |n: &str| n.starts_with("set_") || n.starts_with("update_");
    const MSG: &str = "a published operation named after a field it writes";
    // The api crate's modules; the events crate publishes records, not operations.
    let published = |m: usize| ws.module_layer(m) == Some(Layer::Api);
    for m in ws.project_modules().filter(|m| published(*m)) {
        for item in &ws.modules[m].items {
            if let syn::Item::Fn(f) = item {
                let n = f.sig.ident.to_string();
                if matches!(f.vis, syn::Visibility::Public(_)) && field_named(&n) {
                    out.push(super::at_line(
                        ws,
                        "DCA-NAM-013",
                        m,
                        &n,
                        f.sig.ident.span().start().line,
                        MSG,
                    ));
                }
            }
        }
    }
    // The methods of the api crate's types and traits.
    for d in ws
        .type_decls()
        .chain(ws.trait_decls())
        .filter(|d| published(ws.decls[*d].module))
    {
        let methods = if ws.decls[d].kind == crate::model::DeclKind::Trait {
            ws.trait_methods(d)
        } else {
            // A method of a published trait is reported where the trait declares it.
            own_methods(ws, d, false)
                .into_iter()
                .filter(|f| {
                    super::workspace_trait(ws, f).is_none_or(|t| !published(ws.decls[t].module))
                })
                .collect()
        };
        for f in methods
            .into_iter()
            .filter(|f| f.public && field_named(&f.name))
        {
            out.push(at(
                ws,
                "DCA-NAM-013",
                d,
                format!("`{}::{}` is {MSG}", ws.decl_name(d), f.name),
            ));
        }
    }
    out
}
