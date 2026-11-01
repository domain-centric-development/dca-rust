//! Strategic rules on types: open host services and integration events.

use super::{at, immutability, in_layer};
use crate::Violation;
use crate::model::{Layer, Workspace};

pub fn str_005(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("OpenHostService")
        .filter(|d| !in_layer(ws, *d, Layer::Api) && !ws.decl_in_side(*d, "incoming"))
        .map(|d| at(ws, "DCA-STR-005", d, format!("open host service `{}` is published neither in the api crate nor as an incoming adapter", ws.decl_name(d))))
        .collect()
}

fn in_events(ws: &Workspace, d: usize) -> bool {
    in_layer(ws, d, Layer::Api)
        && ws.modules[ws.decls[d].module]
            .path
            .first()
            .map(String::as_str)
            == Some("events")
}

pub fn str_007(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("IntegrationEvent")
        .filter(|d| !in_events(ws, *d))
        .map(|d| {
            at(
                ws,
                "DCA-STR-007",
                d,
                format!(
                    "integration event `{}` sits outside the events module of an api crate",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub fn str_008(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("IntegrationEvent")
        .flat_map(|d| immutability(ws, "DCA-STR-008", d, "integration event", false))
        .collect()
}

pub fn str_013(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| in_events(ws, *d) && !ws.has_role(*d, "IntegrationEvent"))
        .map(|d| {
            at(
                ws,
                "DCA-STR-013",
                d,
                format!(
                    "trait `{}` in the events module does not extend IntegrationEvent",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}
