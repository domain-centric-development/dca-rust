//! Strategic rules on types: open host services and integration events.

use super::{at, immutability, in_layer};
use crate::Violation;
use crate::model::{Layer, Workspace};

pub(super) fn str_005(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("OpenHostService")
        .chain(ws.traits_with_role("OpenHostService"))
        .filter(|d| !in_layer(ws, *d, Layer::Api) && !ws.decl_in_side(*d, "incoming"))
        .map(|d| at(ws, "DCA-STR-005", d, format!("open host service `{}` is published neither in the api crate nor as an incoming adapter", ws.decl_name(d))))
        .collect()
}

/// In a context's events crate: its published contract. The shared kernel holds no integration event.
fn in_events(ws: &Workspace, d: usize) -> bool {
    in_layer(ws, d, Layer::Events)
}

pub(super) fn str_007(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("IntegrationEvent")
        .filter(|d| !in_events(ws, *d))
        .map(|d| {
            at(
                ws,
                "DCA-STR-007",
                d,
                format!(
                    "integration event `{}` sits outside a context's events crate - it is a context's published contract, never the shared kernel's",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

pub(super) fn str_008(ws: &Workspace) -> Vec<Violation> {
    ws.types_with_role("IntegrationEvent")
        .flat_map(|d| immutability(ws, "DCA-STR-008", d, "integration event", false))
        .collect()
}

pub(super) fn str_013(ws: &Workspace) -> Vec<Violation> {
    ws.trait_decls()
        .filter(|d| in_events(ws, *d) && !ws.has_role(*d, "IntegrationEvent"))
        .map(|d| {
            at(
                ws,
                "DCA-STR-013",
                d,
                format!(
                    "trait `{}` in the events crate does not extend IntegrationEvent",
                    ws.decl_name(d)
                ),
            )
        })
        .collect()
}

/// DCA-STR-010 (informational): doctrine only - a consumed integration event is translated into the
/// consuming context's language before it reaches its domain. Whether it is, is a review check;
/// nothing is selected.
pub(super) fn str_010(_ws: &Workspace) -> Vec<Violation> {
    vec![]
}
