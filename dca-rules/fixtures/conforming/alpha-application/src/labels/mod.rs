//! A feature: it re-exports its operation, and the operation uses the feature's shared type - the
//! edge between a feature and an operation below it is no cycle (DCA-CYC-005).
pub mod relabel;
pub use relabel::RelabelUseCase;

/// What every operation on labels looks a batch up by.
#[derive(Debug, Clone, Copy)]
pub struct LabelBatch(pub u64);
