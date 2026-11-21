//! The relay: delivers the committed outbox entries to their listeners, at least once - the
//! entries that share an ordering key one after the other, the rest in parallel.

use std::collections::HashMap;
use std::sync::Arc;

use dca_building_blocks::application::{BoxFuture, TransactionalEventListener};
use dca_building_blocks::hexagonal::port_out::PortError;
use kernel_application::ListenerId;
use tokio::task::JoinSet;

use super::memory::MemoryDatabase;
use super::outbox::{OutboxEntry, Payload};

type Delivery = Arc<dyn Fn(Payload) -> BoxFuture<'static, Result<(), PortError>> + Send + Sync>;

/// The relay of one store's outbox.
pub struct Relay<C> {
    database: MemoryDatabase<C>,
    routes: HashMap<ListenerId, Delivery>,
}

impl<C: Clone + Send + 'static> Relay<C> {
    /// A relay over the store's outbox, with no listener routed yet.
    pub fn new(database: &MemoryDatabase<C>) -> Self {
        Self {
            database: database.clone(),
            routes: HashMap::new(),
        }
    }

    /// Routes the entries registered for `id` to the listener.
    pub fn route<E, L>(mut self, id: ListenerId, listener: Arc<L>) -> Self
    where
        E: Send + Sync + 'static,
        L: TransactionalEventListener<E> + Send + Sync + 'static,
    {
        let delivery: Delivery = Arc::new(move |payload: Payload| {
            let listener = Arc::clone(&listener);
            Box::pin(async move {
                let event = payload.downcast::<E>().map_err(|payload| {
                    PortError::new(format!(
                        "the outbox holds another event type for this listener: {:?}",
                        payload.type_id()
                    ))
                })?;
                listener.on(&event, &mut ()).await
            })
        });
        self.routes.insert(id, delivery);
        self
    }

    /// Delivers every entry still to make and records each delivery that succeeded. Answers how
    /// many were delivered, or the first failure - the failed entry and those after it with the same
    /// key stay in the outbox for the next run.
    pub async fn deliver(&self) -> Result<usize, PortError> {
        let mut running = JoinSet::new();
        for group in in_order(self.database.undelivered().await) {
            let database = self.database.clone();
            let routes = self.routes.clone();
            running.spawn(async move {
                let mut delivered = 0;
                for entry in group {
                    let route = routes.get(&entry.listener).ok_or_else(|| {
                        PortError::new(format!(
                            "no listener routed for {}",
                            entry.listener.as_str()
                        ))
                    })?;
                    route(Arc::clone(&entry.payload)).await?;
                    database.mark_delivered(entry.sequence).await;
                    delivered += 1;
                }
                Ok::<usize, PortError>(delivered)
            });
        }
        let mut delivered = 0;
        let mut first_failure = None;
        while let Some(joined) = running.join_next().await {
            match joined {
                Ok(Ok(count)) => delivered += count,
                Ok(Err(failure)) => {
                    first_failure.get_or_insert(failure);
                }
                Err(panicked) => {
                    first_failure
                        .get_or_insert(PortError::caused_by("a delivery panicked", panicked));
                }
            }
        }
        match first_failure {
            None => Ok(delivered),
            Some(failure) => Err(failure),
        }
    }
}

/// The entries grouped for delivery: one group per ordering key, in registration order, and one
/// group per entry without a key.
fn in_order(entries: Vec<OutboxEntry>) -> Vec<Vec<OutboxEntry>> {
    let mut keyed: Vec<Vec<OutboxEntry>> = Vec::new();
    let mut groups = Vec::new();
    for entry in entries {
        match &entry.order {
            None => groups.push(vec![entry]),
            Some(key) => match keyed.iter_mut().find(|g| g[0].order.as_ref() == Some(key)) {
                Some(group) => group.push(entry),
                None => keyed.push(vec![entry]),
            },
        }
    }
    groups.extend(keyed);
    groups
}
