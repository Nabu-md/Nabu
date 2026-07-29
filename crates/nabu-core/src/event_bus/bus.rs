use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

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
    handler: Box<dyn Fn(&Events) + Send + Sync>,
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
                handler: Box::new(handler),
            });

        Subscription {
            bus: Arc::downgrade(&self.inner),
            event_kind: event_kind.to_string(),
            id,
        }
    }

    /// Publish an event to all subscribers of the given kind.
    pub fn publish(&self, event_kind: &str, event: &Events) {
        let inner = self.inner.lock().unwrap();
        if let Some(subscribers) = inner.subscribers.get(event_kind) {
            for subscriber in subscribers {
                (subscriber.handler)(event);
            }
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
    bus: Weak<Mutex<BusInner<EventsGlobalDummy>>>,
    event_kind: String,
    id: SubscriberId,
}

// Dummy type because Weak needs a concrete type, but we only use unsubscribe via the bus
type EventsGlobalDummy = String;

impl Subscription {
    /// Unsubscribe this subscription from the event bus.
    pub fn unsubscribe(&self) {
        if let Some(bus) = self.bus.upgrade() {
            let mut inner = bus.lock().unwrap();
            if let Some(subscribers) = inner.subscribers.get_mut(&self.event_kind) {
                subscribers.retain(|s| s.id != self.id);
            }
        }
    }
}
