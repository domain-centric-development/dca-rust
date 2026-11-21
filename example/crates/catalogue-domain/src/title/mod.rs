//! A title of the catalogue and its holdings - the copies the library acquired of it.

mod event;
mod factory;
mod failure;
mod holding;

pub use event::{CopyAcquired, TitleEvent, TitleRegistered};
pub use factory::TitleFactory;
pub use failure::CatalogueFailure;
pub use holding::Holding;

use dca_building_blocks::ddd::tactical::{
    AggregateRoot, DomainEvents, DomainGateway, Entity, EventId, Timestamp, Value,
};
use kernel_domain::{CopyId, TitleId};

/// The name a title is catalogued under, never blank.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TitleName(String);

impl TitleName {
    /// The name, trimmed; a blank one is refused.
    pub fn new(text: &str) -> Result<TitleName, CatalogueFailure> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(CatalogueFailure::BlankName)
        } else {
            Ok(TitleName(trimmed.to_string()))
        }
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Value for TitleName {}

/// Where a copy stands on the shelves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShelfMark(String);

impl ShelfMark {
    /// A shelf mark from its text.
    pub fn new(text: String) -> ShelfMark {
        ShelfMark(text)
    }

    /// The mark as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Value for ShelfMark {}

/// Assigns the shelf mark of a new copy - the library's classification scheme, computed in the
/// process: a synchronous domain gateway, which the title receives as a parameter.
pub trait ShelfMarks: DomainGateway {
    /// The mark of a copy of the title with this name.
    fn mark_for(&self, name: &TitleName, copy: &CopyId) -> ShelfMark;
}

/// A title of the catalogue: the aggregate root over its holdings.
#[derive(Debug, Clone)]
pub struct Title {
    id: TitleId,
    name: TitleName,
    holdings: Vec<Holding>,
    events: DomainEvents<TitleEvent>,
}

impl Title {
    pub(crate) fn registered(id: TitleId, name: TitleName, event: EventId, at: Timestamp) -> Title {
        let mut title = Title {
            id,
            name: name.clone(),
            holdings: Vec::new(),
            events: DomainEvents::new(),
        };
        title
            .events
            .record(TitleEvent::TitleRegistered(TitleRegistered {
                event,
                title: id,
                name,
                occurred_on: at,
            }));
        title
    }

    /// The name.
    pub fn name(&self) -> &TitleName {
        &self.name
    }

    /// The copies acquired of it.
    pub fn holdings(&self) -> &[Holding] {
        &self.holdings
    }

    /// Adds a copy the library acquired, shelved under the mark the classification gives it.
    pub fn acquire_copy<M: ShelfMarks>(
        &mut self,
        marks: &M,
        copy: CopyId,
        event: EventId,
        at: Timestamp,
    ) -> Result<ShelfMark, CatalogueFailure> {
        if self.holdings.iter().any(|h| h.id() == &copy) {
            return Err(CatalogueFailure::CopyHeldAlready);
        }
        let shelf_mark = marks.mark_for(&self.name, &copy);
        self.holdings.push(Holding::new(copy, shelf_mark.clone()));
        self.events.record(TitleEvent::CopyAcquired(CopyAcquired {
            event,
            title: self.id,
            copy,
            shelf_mark: shelf_mark.clone(),
            occurred_on: at,
        }));
        Ok(shelf_mark)
    }
}

impl Entity for Title {
    type Id = TitleId;
    fn id(&self) -> &TitleId {
        &self.id
    }
}

impl AggregateRoot for Title {
    type Event = TitleEvent;
    fn domain_events(&self) -> &[TitleEvent] {
        &self.events
    }
    fn take_domain_events(&mut self) -> Vec<TitleEvent> {
        self.events.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Numbered;
    impl DomainGateway for Numbered {}
    impl ShelfMarks for Numbered {
        fn mark_for(&self, name: &TitleName, copy: &CopyId) -> ShelfMark {
            ShelfMark::new(format!("{}-{}", name.as_str(), copy.as_u128()))
        }
    }

    fn title() -> Title {
        let name = TitleName::new(" Dune ").unwrap();
        TitleFactory.register(
            TitleId::from_u128(1),
            name,
            EventId::from_u128(1),
            Timestamp::from_unix_millis(0),
        )
    }

    #[test]
    fn a_registered_title_records_its_registration() {
        let mut title = title();
        assert_eq!(title.name().as_str(), "Dune");
        assert!(matches!(
            title.take_domain_events()[..],
            [TitleEvent::TitleRegistered(_)]
        ));
        assert!(title.domain_events().is_empty());
    }

    #[test]
    fn an_acquired_copy_is_shelved_under_its_mark() {
        let mut title = title();
        let copy = CopyId::from_u128(7);
        let mark = title.acquire_copy(
            &Numbered,
            copy,
            EventId::from_u128(2),
            Timestamp::from_unix_millis(1),
        );
        assert_eq!(mark.unwrap().as_str(), "Dune-7");
        assert_eq!(title.holdings().len(), 1);
        assert_eq!(title.domain_events().len(), 2);
        let again = title.acquire_copy(
            &Numbered,
            copy,
            EventId::from_u128(3),
            Timestamp::from_unix_millis(2),
        );
        assert_eq!(again.err(), Some(CatalogueFailure::CopyHeldAlready));
    }

    #[test]
    fn a_blank_name_is_refused() {
        assert_eq!(
            TitleName::new("  ").err(),
            Some(CatalogueFailure::BlankName)
        );
    }
}
