//! The inbox: which deliveries each listener has received, kept in the listener's transaction.

use std::collections::HashSet;

use dca_building_blocks::hexagonal::port_out::{OutputPort, PortError, Store};
use kernel_application::{Delivery, Inbox, Received};

/// The pairs (event, listener) received, as a transaction sees them.
#[derive(Debug, Clone, Default)]
pub struct InboxTable {
    received: HashSet<Received>,
}

impl InboxTable {
    /// The inbox port over this table, for the length of a transaction.
    pub fn port(&mut self) -> TransactionInbox<'_> {
        TransactionInbox { table: self }
    }

    /// How many deliveries were received.
    pub fn len(&self) -> usize {
        self.received.len()
    }

    /// Whether nothing was received yet.
    pub fn is_empty(&self) -> bool {
        self.received.is_empty()
    }
}

/// The inbox a transaction unit hands out: it borrows the transaction's table, so a pair is
/// recorded only if the reaction commits.
pub struct TransactionInbox<'a> {
    table: &'a mut InboxTable,
}

impl OutputPort for TransactionInbox<'_> {}
impl Store for TransactionInbox<'_> {}

impl Inbox for TransactionInbox<'_> {
    async fn receive(&mut self, received: Received) -> Result<Delivery, PortError> {
        if self.table.received.insert(received) {
            Ok(Delivery::First)
        } else {
            Ok(Delivery::Repeated)
        }
    }
}
