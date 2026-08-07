use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type SubscriberId = u64;

/// A generic, typed publish/subscribe event bus.
/// All event-driven systems in Nabu communicate through this single EventBus.
/// No duplicate event systems exist.
#[derive(Clone)]
pub struct EventBus<Events: Clone + Send + Sync + 'static> {
    inner: Arc<Mutex<BusInner<Events>>>,
}

struct BusInner<Events: Clone + Send + Sync + 'static> {
    subscribers: HashMap<String, Vec<SubscriberEntry<Events>>>,
    next_id: SubscriberId,
}

struct SubscriberEntry<Events: Clone + Send + Sync + 'static> {
    id: SubscriberId,
    handler: Arc<dyn Fn(&Events) + Send + Sync>,
}

impl<Events: Clone + Send + Sync + 'static> EventBus<Events> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BusInner {
                subscribers: HashMap::new(),
                next_id: 1,
            })),
        }
    }

    /// Subscribe to a specific event kind.
    /// Returns a subscription ID for unsubscription.
    pub fn subscribe<F>(&self, event_kind: &str, handler: F) -> Subscription
    where
        F: Fn(&Events) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;

        inner
            .subscribers
            .entry(event_kind.to_string())
            .or_default()
            .push(SubscriberEntry {
                id,
                handler: Arc::new(handler),
            });

        let inner_clone = self.inner.clone();
        let unsub_event_kind = event_kind.to_string();
        let unsub_id = id;
        Subscription {
            unsubscribe_fn: Box::new(move || {
                let mut inner = inner_clone.lock().unwrap();
                if let Some(subscribers) = inner.subscribers.get_mut(&unsub_event_kind) {
                    subscribers.retain(|s| s.id != unsub_id);
                }
            }),
            event_kind: event_kind.to_string(),
            id,
        }
    }

    /// Publish an event to all subscribers of the given kind.
    ///
    /// Subscribers are collected and the internal lock is released before
    /// any handler is invoked. This allows handlers to publish additional
    /// events (nested publish) without deadlocking on the non-reentrant
    /// Mutex, which is essential for the save pipeline:
    ///   StorageManager.save() -> ITEM_STORED -> indexer.index_object() -> INDEX_UPDATED
    pub fn publish(&self, event_kind: &str, event: &Events) {
        let handlers: Vec<Arc<dyn Fn(&Events) + Send + Sync>> = {
            let inner = self.inner.lock().unwrap();
            if let Some(subscribers) = inner.subscribers.get(event_kind) {
                subscribers.iter().map(|s| s.handler.clone()).collect()
            } else {
                Vec::new()
            }
        };
        for handler in &handlers {
            handler(event);
        }
    }

    /// Unsubscribe a specific subscription.
    pub fn unsubscribe(&self, subscription: &Subscription) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(subscribers) = inner.subscribers.get_mut(&subscription.event_kind) {
            subscribers.retain(|s| s.id != subscription.id);
        }
    }

    /// Number of subscribers for a given event kind.
    pub fn subscriber_count(&self, event_kind: &str) -> usize {
        let inner = self.inner.lock().unwrap();
        inner
            .subscribers
            .get(event_kind)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Total unique event kinds with subscribers.
    pub fn event_kind_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.subscribers.len()
    }

    /// Clear all subscribers (for testing / shutdown).
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.subscribers.clear();
    }
}

impl<Events: Clone + Send + Sync + 'static> Default for EventBus<Events> {
    fn default() -> Self {
        Self::new()
    }
}

/// A subscription handle returned by `subscribe()`.
/// Dropping the handle does NOT unsubscribe — call `unsubscribe()` explicitly.
pub struct Subscription {
    unsubscribe_fn: Box<dyn Fn() + Send + Sync>,
    event_kind: String,
    id: SubscriberId,
}

impl Subscription {
    /// Unsubscribe this subscription from the event bus.
    pub fn unsubscribe(&self) {
        (self.unsubscribe_fn)();
    }
}
