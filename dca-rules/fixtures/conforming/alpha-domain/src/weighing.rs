use std::future::Future;

use dca_building_blocks::ddd::tactical::{DomainService, Value};

use crate::thing::Thing;

/// A domain service declared as a trait, its implementation chosen by the composition root: it may
/// be asynchronous, as a gateway may.
pub trait Weighing: DomainService {
    fn weigh(&self, thing: &Thing) -> impl Future<Output = Weight> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weight(u64);
impl Value for Weight {}

/// Two actions a keeper takes, not a yes/no fact: `un` before a verb names its reverse action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Lock,
    Unlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loading {
    Load,
    Unload,
}
