//! The output ports more than one use case of the catalogue works with.

use core::fmt;
use std::future::Future;

use catalogue_domain::title::{Title, TitleName};
use dca_building_blocks::application::OutputPortFailure;
use dca_building_blocks::hexagonal::port_out::{PortError, Repository};

/// The titles of the catalogue.
pub trait TitleRepository: Repository<Title> {
    /// Adds a new title. A name the catalogue holds already is refused by the store - only the
    /// store sees every name.
    fn add(
        &mut self,
        title: &Title,
    ) -> impl Future<Output = Result<Result<(), TitleNameTaken>, PortError>> + Send;
}

/// Declared by [`TitleRepository::add`]: the catalogue holds a title of this name already.
#[derive(Debug, Clone)]
pub struct TitleNameTaken {
    pub name: TitleName,
}

impl fmt::Display for TitleNameTaken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the catalogue holds a title of this name already")
    }
}

impl std::error::Error for TitleNameTaken {}
impl OutputPortFailure for TitleNameTaken {}
