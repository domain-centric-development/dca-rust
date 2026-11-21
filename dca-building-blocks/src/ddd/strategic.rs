//! Strategic markers that live on a type.
//!
//! A bounded context, its upstreams and partnerships are declared in the `[package.metadata.dca]`
//! table of the context's crates, not in code: Rust has no package annotation to carry them.

/// A published interface another bounded context may call: it sits in the context's api crate or in
/// an incoming adapter. The context it belongs to is its crate's `context`; what it offers, in the
/// context's language, is `DESCRIPTION`:
///
/// ```
/// use dca_building_blocks::ddd::strategic::OpenHostService;
///
/// pub struct CatalogueDesk;
/// impl OpenHostService for CatalogueDesk {
///     const DESCRIPTION: &'static str = "registers titles and acquires copies";
/// }
/// assert_eq!(<CatalogueDesk as OpenHostService>::DESCRIPTION, "registers titles and acquires copies");
/// ```
pub trait OpenHostService {
    /// What the service offers, in the context's language; empty when nothing is said.
    const DESCRIPTION: &'static str = "";
}
