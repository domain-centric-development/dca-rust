//! The members of the library, as lending knows them.

use std::future::Future;

use dca_building_blocks::ddd::tactical::{DomainGateway, Id, Value};

/// A member of the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemberId(u128);

impl MemberId {
    /// The identity from its 128 bits.
    pub const fn from_u128(bits: u128) -> MemberId {
        MemberId(bits)
    }

    /// The 128 bits.
    pub const fn as_u128(&self) -> u128 {
        self.0
    }
}

impl Value for MemberId {}
impl Id for MemberId {}

/// Whether a member may borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Good,
    Suspended,
}

impl Value for Standing {}

/// The members' standing, kept by the membership office - a fact lending does not hold, asked
/// asynchronously: a domain gateway a domain service awaits.
pub trait MemberStandings: DomainGateway {
    /// The member's standing today.
    fn standing_of(&self, member: &MemberId) -> impl Future<Output = Standing> + Send;
}
