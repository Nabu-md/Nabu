//! Event bus implementation for publish/subscribe communication.
//!
//! This module provides a generic, thread-safe event bus that allows services
//! to publish events and subscribe to them without direct coupling.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A type-erased callback that accepts a reference to any event.
type BoxedCallback = Arc<dyn Fn(&(dyn Any + Send + Sync)) + Send + Sync>;

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
    subscribers: RwLock<HashMap<String, Vec<BoxedCallback>>>,
}

impl EventBus {
    /// Creates a new event bus with no subscribers.
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }

    /// Publishes an event to all subscribers of that event type.
    ///
    /// This method is synchronous; subscribers are invoked in the calling thread.
    /// Callbacks are invoked outside the lock to allow nested publishes.
    pub fn publish<T: Any + Send + Sync>(&self, event_type: &str, event: &T) {
        let callbacks = {
            let subscribers = match self.subscribers.read() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            subscribers.get(event_type).cloned()
        };

        if let Some(callbacks) = callbacks {
            for callback in callbacks {
                callback(event);
            }
        }
    }

    /// Subscribes to an event type.
    ///
    /// The callback is invoked synchronously whenever an event of the given
    /// type is published. The callback receives a reference to the event.
    pub fn subscribe<T: Any + Send + Sync, F: Fn(&T) + Send + Sync + 'static>(
        &self,
        event_type: &str,
        callback: F,
    ) {
        let wrapped = move |event: &(dyn Any + Send + Sync)| {
            if let Some(typed) = event.downcast_ref::<T>() {
                callback(typed);
            }
        };
        let boxed: BoxedCallback = Arc::new(wrapped);

        let mut subscribers = match self.subscribers.write() {
            Ok(s) => s,
            Err(poisoned) => poisoned.into_inner(),
        };

        subscribers
            .entry(event_type.to_string())
            .or_default()
            .push(boxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::ItemCaptured;

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
}
