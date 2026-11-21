use dca_building_blocks::ddd::tactical::{DomainEvent, EventId};

/// The outbox of every context, an adapter of the event publishers: one entry per event and listener,
/// kept until the transaction commits.
#[derive(Default)]
pub struct InMemoryOutbox {
    entries: Vec<(EventId, &'static str)>,
}

impl InMemoryOutbox {
    /// Records that `event` is still to be delivered to `listener`.
    pub fn register<E: DomainEvent>(&mut self, event: &E, listener: &'static str) {
        self.entries.push((event.event_id(), listener));
    }
}
