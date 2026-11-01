//! Strategic markers that live on a type.
//!
//! A bounded context, its upstreams and partnerships are declared in the `[package.metadata.dca]`
//! table of the context's crates, not in code: Rust has no package annotation to carry them.

/// A published interface another bounded context may call: it sits in the context's api crate or in
/// an incoming adapter.
pub trait OpenHostService {}
