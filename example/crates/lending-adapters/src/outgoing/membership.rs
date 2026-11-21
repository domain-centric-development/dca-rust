//! The membership office, as lending asks it: a list of suspended members.

use std::collections::HashSet;

use dca_building_blocks::ddd::tactical::DomainGateway;
use lending_domain::member::{MemberId, MemberStandings, Standing};

/// The members the office suspended; every other member is in good standing.
#[derive(Debug, Clone, Default)]
pub struct SuspensionList {
    suspended: HashSet<MemberId>,
}

impl SuspensionList {
    /// The office with these members suspended.
    pub fn new(suspended: impl IntoIterator<Item = MemberId>) -> Self {
        Self {
            suspended: suspended.into_iter().collect(),
        }
    }
}

impl DomainGateway for SuspensionList {}

impl MemberStandings for SuspensionList {
    async fn standing_of(&self, member: &MemberId) -> Standing {
        if self.suspended.contains(member) {
            Standing::Suspended
        } else {
            Standing::Good
        }
    }
}
