//! Failure rules. Rust has no exceptions: a failure is a type returned in `Err`, marked with
//! DomainFailure or UseCaseFailure.

use super::{at, in_layer};
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
        .types_with_role("UseCaseFailure")
        .filter(|d| !in_layer(ws, *d, Layer::Application))
    {
        out.push(at(
            ws,
            "DCA-ERR-002",
            d,
            format!(
                "use-case failure `{}` sits outside the application crate",
                ws.decl_name(d)
            ),
        ));
    }
    out
}

pub fn err_003(ws: &Workspace) -> Vec<Violation> {
    ws.type_decls()
        .filter(|d| in_layer(ws, *d, Layer::Application) && is_error_type(ws, *d))
        .filter(|d| !ws.has_role(*d, "DomainFailure") && !ws.has_role(*d, "UseCaseFailure"))
        .map(|d| {
            at(
                ws,
                "DCA-ERR-003",
                d,
                format!(
                    "failure `{}` in the application does not implement UseCaseFailure",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn err_005(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("DomainFailure")
        .chain(ws.types_with_role("UseCaseFailure"))
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
