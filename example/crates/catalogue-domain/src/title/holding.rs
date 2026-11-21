use dca_building_blocks::ddd::tactical::Entity;
use kernel_domain::CopyId;

use super::ShelfMark;

/// A copy the library holds of a title, where it stands on the shelves.
#[derive(Debug, Clone)]
pub struct Holding {
    id: CopyId,
    shelf_mark: ShelfMark,
}

impl Holding {
    pub(crate) fn new(id: CopyId, shelf_mark: ShelfMark) -> Holding {
        Holding { id, shelf_mark }
    }

    /// Where it stands.
    pub fn shelf_mark(&self) -> &ShelfMark {
        &self.shelf_mark
    }
}

impl Entity for Holding {
    type Id = CopyId;
    fn id(&self) -> &CopyId {
        &self.id
    }
}
