use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

type Listener<Payload> = Arc<dyn Fn(&Payload) + Send + Sync>;

/// A typed publish/subscribe event bus.
///
/// `Events` is a trait or enum that defines all event types and their payloads.
/// The bus is generic over the event type for type safety, and uses `dyn Any`
/// internally for storage.
///
/// ## Usage
///
/// ```rust,ignore
/// let bus = EventBus::<MyEvents>::new();
/// bus.subscribe(|payload: &ItemCaptured| { ... });
/// bus.publish(MyEvents::ItemCaptured(ItemCaptured { ... }));
/// ```
#[derive(Debug)]
pub struct EventBus<Events: Send + Sync + 'static> {
    listeners: Arc<Mutex<HashMap<std::any::TypeId, Vec<Box<dyn Fn(&dyn Any) + Send + Sync>>>>>,
    _marker: PhantomData<Events>,
}

impl<Events: Send + Sync + 'static> EventBus<Events> {
    /// Creates a new empty event bus.
    pub fn new() -> Self {
        EventBus {
            listeners: Arc::new(Mutex::new(HashMap::new())),
            _marker: PhantomData,
        }
    }

    /// Subscribes a listener to a specific event type.
    ///
    /// Returns an unsubscribe handle. Dropping the handle does not unsubscribe;
    /// call `handle.unsubscribe()` explicitly or use `EventBus::unsubscribe`.
    pub fn subscribe<Payload: Send + Sync + 'static>(
        &self,
        callback: impl Fn(&Payload) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        let type_id = std::any::TypeId::of::<Payload>();
        let listener: Box<dyn Fn(&dyn Any) + Send + Sync> =
            Box::new(move |any| {
                if let Some(payload) = any.downcast_ref::<Payload>() {
                    callback(payload);
                }
            });

        let mut listeners = self.listeners.lock().unwrap();
        listeners.entry(type_id).or_insert_with(Vec::new).push(listener);

        let id = listeners.get(&type_id).map(|v| v.len()).unwrap_or(0) - 1;
        SubscriptionHandle {
            type_id,
            index: id,
            bus: self.listeners.clone(),
        }
    }

    /// Publishes an event to all subscribers.
    ///
    /// All subscribers receive the event synchronously. A panicking subscriber
    /// does not prevent other subscribers from receiving the event.
    pub fn publish<Payload: Send + Sync + 'static>(&self, payload: &Payload) {
        let type_id = std::any::TypeId::of::<Payload>();
        let listeners = self.listeners.lock().unwrap();

        if let Some(callbacks) = listeners.get(&type_id) {
            for callback in callbacks {
                callback(payload);
            }
        }
    }

    /// Returns the number of subscribers for a given event type.
    pub fn subscriber_count<Payload: Send + Sync + 'static>(&self) -> usize {
        let type_id = std::any::TypeId::of::<Payload>();
        let listeners = self.listeners.lock().unwrap();
        listeners.get(&type_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Returns `true` if there are any subscribers for the given event type.
    pub fn has_subscribers<Payload: Send + Sync + 'static>(&self) -> bool {
        self.subscriber_count::<Payload>() > 0
    }

    /// Removes all subscribers for all event types.
    pub fn clear(&self) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.clear();
    }
}

impl<Events: Send + Sync + 'static> Clone for EventBus<Events> {
    fn clone(&self) -> Self {
        EventBus {
            listeners: self.listeners.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Events: Send + Sync + 'static> Default for EventBus<Events> {
    fn default() -> Self {
        Self::new()
    }
}

/// A handle that can be used to unsubscribe a specific listener.
#[derive(Debug)]
pub struct SubscriptionHandle {
    type_id: std::any::TypeId,
    index: usize,
    bus: Arc<Mutex<HashMap<std::any::TypeId, Vec<Box<dyn Fn(&dyn Any) + Send + Sync>>>>>,
}

impl SubscriptionHandle {
    /// Removes this specific subscription from the event bus.
    pub fn unsubscribe(&self) {
        let mut listeners = self.bus.lock().unwrap();
        if let Some(callbacks) = listeners.get_mut(&self.type_id) {
            if self.index < callbacks.len() {
                // Replace with a no-op to preserve indices
                callbacks[self.index] = Box::new(|_: &dyn Any| {});
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent {
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct AnotherEvent {
        message: String,
    }

    #[test]
    fn test_publish_and_subscribe() {
        let bus = EventBus::<TestEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(None::<TestEvent>));

        let received_clone = received.clone();
        bus.subscribe(move |event: &TestEvent| {
            *received_clone.lock().unwrap() = Some(event.clone());
        });

        bus.publish(&TestEvent { value: 42 });

        let result = received.lock().unwrap().take();
        assert_eq!(result, Some(TestEvent { value: 42 }));
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::<TestEvent>::new();
        let count = Arc::new(std::sync::Mutex::new(0));

        let c1 = count.clone();
        bus.subscribe(move |_: &TestEvent| { *c1.lock().unwrap() += 1; });

        let c2 = count.clone();
        bus.subscribe(move |_: &TestEvent| { *c2.lock().unwrap() += 1; });

        bus.publish(&TestEvent { value: 1 });

        assert_eq!(*count.lock().unwrap(), 2);
    }

    #[test]
    fn test_type_safety() {
        let bus = EventBus::<TestEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(false));

        let r = received.clone();
        bus.subscribe(move |_: &AnotherEvent| {
            *r.lock().unwrap() = true;
        });

        // Publishing a TestEvent should NOT trigger the AnotherEvent subscriber
        bus.publish(&TestEvent { value: 1 });
        assert!(!*received.lock().unwrap());
    }

    #[test]
    fn test_subscriber_count() {
        let bus = EventBus::<TestEvent>::new();
        assert_eq!(bus.subscriber_count::<TestEvent>(), 0);

        bus.subscribe(|_: &TestEvent| {});
        assert_eq!(bus.subscriber_count::<TestEvent>(), 1);

        bus.subscribe(|_: &TestEvent| {});
        assert_eq!(bus.subscriber_count::<TestEvent>(), 2);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBus::<TestEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(0));

        let r = received.clone();
        let handle = bus.subscribe(move |_: &TestEvent| {
            *r.lock().unwrap() += 1;
        });

        bus.publish(&TestEvent { value: 1 });
        assert_eq!(*received.lock().unwrap(), 1);

        handle.unsubscribe();

        bus.publish(&TestEvent { value: 2 });
        assert_eq!(*received.lock().unwrap(), 1); // not incremented
    }

    #[test]
    fn test_clear() {
        let bus = EventBus::<TestEvent>::new();
        let count = Arc::new(std::sync::Mutex::new(0));

        let c = count.clone();
        bus.subscribe(move |_: &TestEvent| { *c.lock().unwrap() += 1; });

        bus.publish(&TestEvent { value: 1 });
        assert_eq!(*count.lock().unwrap(), 1);

        bus.clear();
        bus.publish(&TestEvent { value: 2 });
        assert_eq!(*count.lock().unwrap(), 1); // not incremented after clear
    }

    #[test]
    fn test_clone_shares_state() {
        let bus = EventBus::<TestEvent>::new();
        let count = Arc::new(std::sync::Mutex::new(0));

        let c = count.clone();
        bus.subscribe(move |_: &TestEvent| { *c.lock().unwrap() += 1; });

        let bus2 = bus.clone();
        bus2.publish(&TestEvent { value: 1 });

        assert_eq!(*count.lock().unwrap(), 1);
    }
}
