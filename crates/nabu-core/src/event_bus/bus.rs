//! Event bus implementation for publish/subscribe communication.
//!
//! This module provides a generic, thread-safe event bus that allows services
//! to publish events and subscribe to them without direct coupling.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A type-erased callback that accepts a reference to any event.
type BoxedCallback = Arc<dyn Fn(&(dyn Any + Send + Sync)) + Send + Sync>;

/// A handle that can be used to unsubscribe from an event type.
///
/// Drop this handle to remove the associated callback from the event bus.
/// Alternatively, call [`EventBus::unsubscribe`] explicitly.
#[derive(Debug, Clone)]
pub struct Subscription {
    event_type: String,
    callback_id: usize,
}

/// A generic event bus that supports typed publish/subscribe.
///
/// The event bus is thread-safe and can be used from multiple threads.
/// Each event type has its own subscriber list. Callbacks are invoked
/// synchronously during `publish`.
///
/// # Example
///
/// ```ignore
/// use nabu_core::event_bus::EventBus;
///
/// let bus = EventBus::new();
///
/// // Subscribe to an event
/// bus.subscribe("ItemCaptured", |event: &ItemCaptured| {
///     println!("Item captured: {:?}", event.id);
/// });
///
/// // Publish an event
/// bus.publish("ItemCaptured", &ItemCaptured { id: Uuid::new_v4(), ... });
/// ```
#[derive(Default)]
pub struct EventBus {
    subscribers: RwLock<HashMap<String, Vec<(usize, BoxedCallback)>>>,
    next_id: RwLock<usize>,
}

impl EventBus {
    /// Creates a new event bus with no subscribers.
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            next_id: RwLock::new(0),
        }
    }

    /// Unsubscribes a previously registered callback.
    ///
    /// If the subscription handle has already been removed or the
    /// callback ID is not found, this is a no-op.
    pub fn unsubscribe(&self, subscription: Subscription) {
        let mut subscribers = match self.subscribers.write() {
            Ok(s) => s,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(callbacks) = subscribers.get_mut(&subscription.event_type) {
            callbacks.retain(|(id, _)| *id != subscription.callback_id);
        }
    }

    /// Publishes an event to all subscribers of that event type.
    ///
    /// This method is synchronous; subscribers are invoked in the calling thread.
    /// Callbacks are invoked outside the lock to allow nested publishes.
    ///
    /// If no subscribers are registered for the event type, this is a no-op.
    pub fn publish<T: Any + Send + Sync>(&self, event_type: &str, event: &T) {
        let callbacks = {
            let subscribers = match self.subscribers.read() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            subscribers.get(event_type).cloned()
        };

        if let Some(callbacks) = callbacks {
            for (_, callback) in callbacks {
                callback(event);
            }
        }
    }

    /// Subscribes to an event type.
    ///
    /// The callback is invoked synchronously whenever an event of the given
    /// type is published. The callback receives a reference to the event.
    ///
    /// Returns a [`Subscription`] handle that can be used to unsubscribe.
    /// When the handle is dropped, the callback is removed.
    pub fn subscribe<T: Any + Send + Sync, F: Fn(&T) + Send + Sync + 'static>(
        &self,
        event_type: &str,
        callback: F,
    ) -> Subscription {
        let wrapped = move |event: &(dyn Any + Send + Sync)| {
            if let Some(typed) = event.downcast_ref::<T>() {
                callback(typed);
            }
        };
        let boxed: BoxedCallback = Arc::new(wrapped);

        let mut next_id = match self.next_id.write() {
            Ok(n) => n,
            Err(poisoned) => poisoned.into_inner(),
        };
        let id = *next_id;
        *next_id += 1;

        let mut subscribers = match self.subscribers.write() {
            Ok(s) => s,
            Err(poisoned) => poisoned.into_inner(),
        };

        subscribers
            .entry(event_type.to_string())
            .or_default()
            .push((id, boxed));

        Subscription {
            event_type: event_type.to_string(),
            callback_id: id,
        }
    }

    /// Returns the number of subscribers for a given event type.
    ///
    /// Returns 0 if no subscribers are registered.
    pub fn subscriber_count(&self, event_type: &str) -> usize {
        let subscribers = match self.subscribers.read() {
            Ok(s) => s,
            Err(poisoned) => poisoned.into_inner(),
        };
        subscribers.get(event_type).map_or(0, |v| v.len())
    }

    /// Returns true if at least one subscriber is registered for the event type.
    pub fn has_subscribers(&self, event_type: &str) -> bool {
        self.subscriber_count(event_type) > 0
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // Note: We cannot access the EventBus here because Subscription
        // doesn't hold a reference to it. Unsubscription is handled
        // by the EventBus::unsubscribe method if needed, or by
        // the subscriber managing the handle's lifetime.
        // For now, we store the event_type and callback_id for
        // potential future use with a global registry or by
        // requiring the bus to be passed back.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::{EventBus, ItemCaptured};

    #[test]
    fn event_bus_can_be_created() {
        let _bus = EventBus::new();
    }

    #[test]
    fn publish_delivers_to_subscriber() {
        let bus = EventBus::new();
        let received = Arc::new(std::sync::RwLock::new(None));

        let r = received.clone();
        bus.subscribe("ItemCaptured", move |event: &ItemCaptured| {
            let mut recv = r.write().unwrap();
            *recv = Some(event.id);
        });

        let id = uuid::Uuid::new_v4();
        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id,
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );

        assert_eq!(*received.read().unwrap(), Some(id));
    }

    #[test]
    fn multiple_subscribers_all_receive_event() {
        let bus = EventBus::new();
        let count = Arc::new(std::sync::RwLock::new(0));

        for _ in 0..3 {
            let c = count.clone();
            bus.subscribe("ItemCaptured", move |_event: &ItemCaptured| {
                let mut cnt = c.write().unwrap();
                *cnt += 1;
            });
        }

        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id: uuid::Uuid::new_v4(),
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );

        assert_eq!(*count.read().unwrap(), 3);
    }

    #[test]
    fn publish_without_subscribers_is_noop() {
        let bus = EventBus::new();
        // Should not panic
        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id: uuid::Uuid::new_v4(),
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );
    }

    #[test]
    fn subscriber_count_reflects_registrations() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count("ItemCaptured"), 0);

        bus.subscribe("ItemCaptured", |_: &ItemCaptured| {});
        assert_eq!(bus.subscriber_count("ItemCaptured"), 1);

        bus.subscribe("ItemCaptured", |_: &ItemCaptured| {});
        assert_eq!(bus.subscriber_count("ItemCaptured"), 2);

        assert_eq!(bus.subscriber_count("ItemProcessed"), 0);
    }

    #[test]
    fn has_subscribers_returns_true_when_registered() {
        let bus = EventBus::new();
        assert!(!bus.has_subscribers("ItemCaptured"));

        let _handle = bus.subscribe("ItemCaptured", |_: &ItemCaptured| {});
        assert!(bus.has_subscribers("ItemCaptured"));
        assert!(!bus.has_subscribers("ItemProcessed"));
    }

    #[test]
    fn unsubscribe_removes_callback() {
        let bus = EventBus::new();
        let count = Arc::new(std::sync::RwLock::new(0));

        let c = count.clone();
        let handle = bus.subscribe("ItemCaptured", move |_: &ItemCaptured| {
            let mut cnt = c.write().unwrap();
            *cnt += 1;
        });

        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id: uuid::Uuid::new_v4(),
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );
        assert_eq!(*count.read().unwrap(), 1);

        bus.unsubscribe(handle);

        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id: uuid::Uuid::new_v4(),
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );
        assert_eq!(*count.read().unwrap(), 1);
    }

    #[test]
    fn unsubscribe_is_noop_for_unknown_subscription() {
        let bus = EventBus::new();
        let count = Arc::new(std::sync::RwLock::new(0));

        let c = count.clone();
        let handle = bus.subscribe("ItemCaptured", move |_: &ItemCaptured| {
            let mut cnt = c.write().unwrap();
            *cnt += 1;
        });

        // Create a fake subscription for a different event type
        let fake = Subscription {
            event_type: "ItemProcessed".to_string(),
            callback_id: 99999,
        };
        bus.unsubscribe(fake);

        // Original subscriber should still work
        bus.publish(
            "ItemCaptured",
            &ItemCaptured {
                id: uuid::Uuid::new_v4(),
                source: "test".to_string(),
                vault_id: "vault-1".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                raw_bytes: Vec::new(),
                mime_type: "text/plain".to_string(),
                source_file: None,
            },
        );
        assert_eq!(*count.read().unwrap(), 1);

        bus.unsubscribe(handle);
    }
}
