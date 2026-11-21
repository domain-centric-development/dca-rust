//! The system's clock and a source of identities.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use dca_building_blocks::ddd::tactical::{EventId, Timestamp};
use dca_building_blocks::hexagonal::port_out::OutputPort;
use kernel_application::{Clock, IdSource};

/// The operating system's clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl OutputPort for SystemClock {}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from(SystemTime::now())
    }
}

/// Identities counted up from one, shared by every clone: unique within the process, which is all
/// an in-memory store needs.
#[derive(Debug, Clone, Default)]
pub struct SequentialIds {
    last: Arc<AtomicU64>,
}

impl SequentialIds {
    fn next(&self) -> u128 {
        u128::from(self.last.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

impl OutputPort for SequentialIds {}

impl IdSource for SequentialIds {
    fn next_id(&self) -> u128 {
        self.next()
    }

    fn next_event_id(&self) -> EventId {
        EventId::from_u128(self.next())
    }
}
