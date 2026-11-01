//! Rules over the crate graph: which crate may name which in its Cargo.toml.
//!
//! The compiler refuses every use of a crate that is not a dependency; these rules refuse the
//! dependency itself, so adding a line to Cargo.toml does not open a layer.

use crate::Violation;
use crate::model::{Crate, Layer, Workspace};

fn crate_location(ws: &Workspace, c: &Crate) -> String {
    let manifest = c.manifest_dir.join("Cargo.toml");
    let rel = manifest.strip_prefix(&ws.root).unwrap_or(&manifest);
    format!("{} ({})", c.package, rel.display())
}

/// Every normal dependency of a crate in `from` on a workspace crate that `forbidden` refuses.
fn forbidden_deps(
    ws: &Workspace,
    rule: &'static str,
    from: impl Fn(&Crate) -> bool,
    forbidden: impl Fn(&Crate, &Crate) -> Option<String>,
) -> Vec<Violation> {
    let mut out = vec![];
    for c in ws.crates.iter().filter(|c| from(c)) {
        for dep in c.deps.iter().filter(|d| d.normal) {
            let Some(k) = ws.dep_crate(dep) else { continue };
            if let Some(msg) = forbidden(c, &ws.crates[k]) {
                out.push(Violation {
                    rule,
                    location: crate_location(ws, c),
                    message: msg,
                });
            }
        }
    }
    out
}

fn layer_is(c: &Crate, l: Layer) -> bool {
    c.layer == Some(l)
}

fn describe(c: &Crate) -> String {
    match (c.layer, &c.context) {
        (Some(l), Some(ctx)) => format!("`{}` ({} of {ctx})", c.package, l.name()),
        (Some(l), None) => format!("`{}` ({})", c.package, l.name()),
        _ => format!("`{}`", c.package),
    }
}

pub fn lay_002(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-LAY-002",
        |c| layer_is(c, Layer::Domain) || layer_is(c, Layer::SharedKernel),
        |_, to| {
            matches!(to.layer, Some(Layer::Infrastructure | Layer::Bootstrap)).then(|| {
                format!(
                    "the domain depends on {}; the domain sees no infrastructure",
                    describe(to)
                )
            })
        },
    )
}

pub fn lay_003(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-LAY-003",
        |c| layer_is(c, Layer::Application),
        |_, to| {
            matches!(to.layer, Some(Layer::Infrastructure | Layer::Bootstrap)).then(|| {
                format!(
                    "the application depends on {}; it uses output ports, which adapters implement",
                    describe(to)
                )
            })
        },
    )
}

pub fn oni_001(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-ONI-001",
        |c| layer_is(c, Layer::Domain),
        |_, to| {
            layer_is(to, Layer::Application).then(|| {
                format!(
                    "the domain depends on {}; the domain is the innermost layer",
                    describe(to)
                )
            })
        },
    )
}

pub fn oni_002(ws: &Workspace) -> Vec<Violation> {
    let mut out = vec![];
    for c in ws
        .crates
        .iter()
        .filter(|c| layer_is(c, Layer::Domain) || layer_is(c, Layer::SharedKernel))
    {
        for dep in c.deps.iter().filter(|d| d.normal) {
            let allowed = dep.package == ws.config.building_blocks
                || ws.config.domain_allowed_crates.contains(&dep.package)
                || ws
                    .dep_crate(dep)
                    .is_some_and(|k| layer_is(&ws.crates[k], Layer::SharedKernel));
            let domain_of_same = ws.dep_crate(dep).is_some_and(|k| {
                layer_is(&ws.crates[k], Layer::Domain) && ws.crates[k].context == c.context
            });
            if !allowed && !domain_of_same {
                out.push(Violation {
                    rule: "DCA-ONI-002",
                    location: crate_location(ws, c),
                    message: format!(
                        "the domain depends on `{}`; allowed are the building blocks, the shared kernel and [workspace.metadata.dca] domain-allowed-crates",
                        dep.package
                    ),
                });
            }
        }
    }
    out
}

pub fn hex_001(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-HEX-001",
        |c| layer_is(c, Layer::Domain),
        |_, to| {
            layer_is(to, Layer::Adapters).then(|| format!("the domain depends on {}", describe(to)))
        },
    )
}

pub fn hex_002(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-HEX-002",
        |c| layer_is(c, Layer::Application),
        |_, to| {
            layer_is(to, Layer::Adapters).then(|| format!("the application depends on {}; adapters depend on the application, not the reverse", describe(to)))
        },
    )
}

pub fn hex_005(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-HEX-005",
        |c| layer_is(c, Layer::Adapters),
        |from, to| {
            (layer_is(to, Layer::Infrastructure)
                && to.context.is_some()
                && to.context != from.context)
                .then(|| {
                    format!(
                        "the adapters depend on {}, another context's infrastructure",
                        describe(to)
                    )
                })
        },
    )
}

fn other_context(from: &Crate, to: &Crate) -> bool {
    to.context.is_some() && to.context != from.context
}

pub fn str_002(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-STR-002",
        |c| layer_is(c, Layer::SharedKernel),
        |_, to| {
            to.context.is_some().then(|| {
                format!(
                    "the shared kernel depends on {}, a bounded context",
                    describe(to)
                )
            })
        },
    )
}

pub fn str_003(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-STR-003",
        |c| layer_is(c, Layer::Application),
        |from, to| {
            other_context(from, to).then(|| format!("the application depends on {} of another context; reach it through an outgoing adapter", describe(to)))
        },
    )
}

pub fn str_004(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-STR-004",
        |c| layer_is(c, Layer::Domain),
        |from, to| {
            other_context(from, to)
                .then(|| format!("the domain depends on {} of another context", describe(to)))
        },
    )
}

pub fn str_006(ws: &Workspace) -> Vec<Violation> {
    forbidden_deps(
        ws,
        "DCA-STR-006",
        |c| layer_is(c, Layer::Adapters),
        |from, to| {
            (other_context(from, to) && !layer_is(to, Layer::Api))
            .then(|| format!("the adapters depend on {}; another context is reached through its api crate only", describe(to)))
        },
    )
}

pub fn str_011(ws: &Workspace) -> Vec<Violation> {
    if ws.crates.iter().any(|c| c.context.is_some()) {
        vec![]
    } else {
        vec![Violation {
            rule: "DCA-STR-011",
            location: "workspace".into(),
            message: "no crate declares a bounded context; set context in [package.metadata.dca]"
                .into(),
        }]
    }
}

pub fn str_012(ws: &Workspace) -> Vec<Violation> {
    if ws.crates.iter().any(|c| c.layer.is_some()) {
        vec![]
    } else {
        vec![Violation {
            rule: "DCA-STR-012",
            location: "workspace".into(),
            message: "no crate declares a layer; set layer in [package.metadata.dca]".into(),
        }]
    }
}
