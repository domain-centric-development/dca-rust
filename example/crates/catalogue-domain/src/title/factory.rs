use dca_building_blocks::ddd::tactical::{EventId, Factory, Timestamp};
use kernel_domain::TitleId;

use super::{Title, TitleName};

/// Enters a new title into the catalogue: the title starts without holdings and records its
/// registration.
#[derive(Debug, Clone, Copy, Default)]
pub struct TitleFactory;

impl Factory for TitleFactory {}

impl TitleFactory {
    /// A newly registered title.
    pub fn register(&self, id: TitleId, name: TitleName, event: EventId, at: Timestamp) -> Title {
        Title::registered(id, name, event, at)
    }
}
