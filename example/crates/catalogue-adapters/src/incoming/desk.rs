//! The catalogue desk: the catalogue's published entry, for staff and for other systems.

use std::sync::Arc;

use catalogue_application::acquire_copy::{
    AcquireCopyCommand, AcquireCopyError, AcquireCopyInputPort,
};
use catalogue_application::register_title::{
    RegisterTitleCommand, RegisterTitleError, RegisterTitleInputPort,
};
use catalogue_domain::title::CatalogueFailure;
use dca_building_blocks::ddd::strategic::OpenHostService;
use kernel_domain::TitleId;

/// What the desk answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeskResponse {
    /// Entered: the new title's or copy's id, and for a copy its shelf mark.
    Created {
        id: u128,
        shelf_mark: Option<String>,
    },
    /// Refused, with the reason's code.
    Rejected { reason: &'static str },
    /// No such title.
    NotFound,
    /// The catalogue cannot answer now.
    Unavailable,
}

/// The catalogue's open host service: registers titles and acquires copies.
pub struct CatalogueDesk {
    register: Arc<dyn RegisterTitleInputPort + Send + Sync>,
    acquire: Arc<dyn AcquireCopyInputPort + Send + Sync>,
}

impl OpenHostService for CatalogueDesk {
    const DESCRIPTION: &'static str = "registers titles and acquires copies";
}

impl CatalogueDesk {
    /// The desk over the catalogue's input ports.
    pub fn new(
        register: Arc<dyn RegisterTitleInputPort + Send + Sync>,
        acquire: Arc<dyn AcquireCopyInputPort + Send + Sync>,
    ) -> Self {
        Self { register, acquire }
    }

    /// Enters a title.
    pub async fn register_title(&self, name: &str) -> DeskResponse {
        let command = RegisterTitleCommand {
            name: name.to_string(),
        };
        match self.register.execute(command).await {
            Ok(registered) => DeskResponse::Created {
                id: registered.title.as_u128(),
                shelf_mark: None,
            },
            Err(RegisterTitleError::Refused(refused)) => DeskResponse::Rejected {
                reason: reason(&refused),
            },
            Err(RegisterTitleError::NameTaken(_)) => DeskResponse::Rejected {
                reason: "name-taken",
            },
            Err(RegisterTitleError::Unavailable(_)) => DeskResponse::Unavailable,
        }
    }

    /// Acquires a copy of a title.
    pub async fn acquire_copy(&self, title: u128) -> DeskResponse {
        let command = AcquireCopyCommand {
            title: TitleId::from_u128(title),
        };
        match self.acquire.execute(command).await {
            Ok(acquired) => DeskResponse::Created {
                id: acquired.copy.as_u128(),
                shelf_mark: Some(acquired.shelf_mark.as_str().to_string()),
            },
            Err(AcquireCopyError::TitleNotFound) => DeskResponse::NotFound,
            Err(AcquireCopyError::Refused(refused)) => DeskResponse::Rejected {
                reason: reason(&refused),
            },
            Err(AcquireCopyError::Unavailable(_)) => DeskResponse::Unavailable,
        }
    }
}

fn reason(refused: &CatalogueFailure) -> &'static str {
    match refused {
        CatalogueFailure::BlankName => "blank-name",
        CatalogueFailure::CopyHeldAlready => "copy-held-already",
    }
}
