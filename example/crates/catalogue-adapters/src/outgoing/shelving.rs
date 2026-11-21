//! The library's shelf marks: the first letters of the name and the copy's number.

use catalogue_domain::title::{ShelfMark, ShelfMarks, TitleName};
use dca_building_blocks::ddd::tactical::DomainGateway;
use kernel_domain::CopyId;

/// Shelves a copy under the first three letters of its title's name.
#[derive(Debug, Clone, Copy, Default)]
pub struct InitialsShelfMarks;

impl DomainGateway for InitialsShelfMarks {}

impl ShelfMarks for InitialsShelfMarks {
    fn mark_for(&self, name: &TitleName, copy: &CopyId) -> ShelfMark {
        let initials: String = name
            .as_str()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(3)
            .collect();
        ShelfMark::new(format!("{}-{}", initials.to_uppercase(), copy.as_u128()))
    }
}
