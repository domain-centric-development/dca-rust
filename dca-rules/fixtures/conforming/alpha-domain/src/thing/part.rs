use dca_building_blocks::ddd::tactical::Entity;

use super::{Label, PartId};

#[derive(Debug, Clone)]
pub struct Part {
    id: PartId,
    label: Label,
}

impl Part {
    pub(crate) fn new(id: PartId, label: Label) -> Part {
        Part { id, label }
    }

    pub fn label(&self) -> &Label {
        &self.label
    }
}

impl PartialEq for Part {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Entity for Part {
    type Id = PartId;
    fn id(&self) -> &PartId {
        &self.id
    }
}
