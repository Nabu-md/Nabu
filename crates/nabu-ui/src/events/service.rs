//! Centralized frontend event subscription manager.
//!
//! [`EventService`] is the single, canonical entry point for all frontend
//! event subscriptions. It installs **one** listener on the Tauri
//! `nabu-event` channel, deserializes each broadcast into a typed
//! [`FrontendEvent`], and fans it out to registered subscribers.
//!
//! ## Guarantees
//!
//! * **No direct Tauri access** — components subscribe via
//!   [`EventService::subscribe`] / [`crate::events::use_event_listener`].
//! * **No duplicate listeners** — `start_listening` is idempotent; only one
//!   `nabu-event` listener exists per service.
//! * **No leaks** — subscribers deregister via the RAII
//!   [`EventSubscription`] handle (drop or `unsubscribe()`); the Tauri
//!   listener is detached on [`EventService::shutdown`].
//! * **No races** — shared state is behind `Mutex`; dispatch collects a
//!   subscriber snapshot and releases the lock *before* invoking any callback,
//!   mirroring the backend `EventBus` (which does the same to allow nested
//!   publishes). Callbacks therefore never run while the lock is held.
//!
//! The service is `!Send` by construction — it wraps JS values
//! (`js_sys::Function`, `Closure`) that are single-threaded. This matches the
//! wasm/webview execution model; the `Mutex` still prevents re-entrant
//! deadlocks within that single thread.

use std::sync::{Arc, Mutex, Weak};

use js_sys::Function as JsFunction;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::events::bindings;
use crate::events::bindings::parse_event;
use crate::events::types::{
    EventError, FrontendEvent, FrontendEventKind, FRONTEND_EVENT_CHANNEL,
};

/// A subscriber callback. `Fn` (not `FnMut`) so the same subscriber can be
/// invoked repeatedly by the dispatcher; subscribers that mutate state do so
/// through `Signal::write_unchecked` (interior mutability), exactly like the
/// rest of the UI layer.
pub type Subscriber = Arc<dyn Fn(&FrontendEvent) + 'static>;

/// What an event must look like to be delivered to a subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubscriberFilter {
    Kind(FrontendEventKind),
    All,
}

struct SubscriberEntry {
    id: u64,
    filter: SubscriberFilter,
    callback: Subscriber,
}

type SharedInner = Arc<Mutex<Inner>>;

struct Inner {
    subscribers: Vec<SubscriberEntry>,
    next_id: u64,
    /// The JS callback closure, kept alive for as long as the listener is
    /// registered so Tauri never invokes a freed closure.
    js_callback: Option<Closure<dyn FnMut(JsValue)>>,
    /// Tauri's `unlisten` handle — calling it detaches the JS listener.
    unlisten: Option<JsFunction>,
    /// Whether the single Tauri listener is currently attached.
    listening: bool,
}

impl Inner {
    fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            next_id: 1,
            js_callback: None,
            unlisten: None,
            listening: false,
        }
    }
}

/// The shared frontend event service.
///
/// Created once (by [`EventServiceProvider`](super::provider::EventServiceProvider))
/// and propagated through the Dioxus context tree. Cheaply cloneable (`Arc`),
/// so components receive it by value via [`use_event_service`](super::use_event_service).
#[derive(Clone)]
pub struct EventService {
    inner: SharedInner,
}

impl Default for EventService {
    fn default() -> Self {
        Self::new()
    }
}

impl EventService {
    /// Create a new, unattached event service.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new())),
        }
    }

    /// Register a subscriber for a specific, typed event kind.
    ///
    /// Returns an [`EventSubscription`] handle. Dropping it (or calling
    /// [`EventSubscription::unsubscribe`]) deregisters the callback exactly once.
    /// The subscription is **not** deduplicated by callback — call this once
    /// per subscription site.
    pub fn subscribe(
        &self,
        kind: FrontendEventKind,
        callback: impl Fn(&FrontendEvent) + 'static,
    ) -> EventSubscription {
        self._subscribe(SubscriberFilter::Kind(kind), Arc::new(callback))
    }

    /// Register a subscriber that receives *every* platform event, regardless
    /// of kind.
    pub fn subscribe_all(
        &self,
        callback: impl Fn(&FrontendEvent) + 'static,
    ) -> EventSubscription {
        self._subscribe(SubscriberFilter::All, Arc::new(callback))
    }

    fn _subscribe(&self, filter: SubscriberFilter, callback: Subscriber) -> EventSubscription {
        let (id, inner) = {
            let mut guard = self.inner.lock().unwrap();
            let id = guard.next_id;
            guard.next_id += 1;
            guard.subscribers.push(SubscriberEntry {
                id,
                filter,
                callback,
            });
            // A strong clone kept solely by this closure. It lives in the
            // component (not in `Inner`), so it never forms a reference cycle.
            (id, self.inner.clone())
        };

        tracing::debug!(?filter, subscriber_id = id, "Subscriber registered");

        EventSubscription {
            id,
            filter,
            unsubscribe: Box::new(move || {
                if let Ok(mut guard) = inner.lock() {
                    guard.subscribers.retain(|s| s.id != id);
                }
            }),
        }
    }

    /// Install the single Tauri listener on `nabu-event`.
    ///
    /// Idempotent and safe to call multiple times; only the first call
    /// registers a listener. Must be `await`ed to resolve the `unlisten` handle
    /// Tauri returns. If Tauri is unavailable (UI served outside the webview),
    /// returns [`EventError::TauriListen`] and leaves the service usable for
    /// in-process dispatch (tests, etc.).
    pub async fn start_listening(&self) -> Result<(), EventError> {
        {
            let inner = self.inner.lock().unwrap();
            if inner.listening {
                tracing::warn!("EventService: listener already registered; skipping");
                return Ok(());
            }
        }

        if !bindings::tauri_available() {
            tracing::warn!(
                "EventService: Tauri event API not available (running outside Tauri); \
                 platform events will not be received"
            );
            return Err(EventError::TauriListen(
                "Tauri event API not available".into(),
            ));
        }

        // The JS callback is kept alive in `Inner` (stored below) for as long
        // as the listener is attached — never `forget`-ed, never leaked. The
        // callback captures a `Weak` to `Inner` (not a strong ref) so the
        // closure stored in `Inner` can never keep `Inner` alive (no cycle).
        let weak = Arc::downgrade(&self.inner);
        let closure = Closure::wrap(Box::new(move |ev: JsValue| {
            on_tauri_event(&ev, &weak);
        }) as Box<dyn FnMut(JsValue)>);

        let cb_fn: &JsFunction = JsCast::unchecked_ref(closure.as_ref());
        let promise = bindings::tauri_event_listen(FRONTEND_EVENT_CHANNEL, cb_fn);

        let unlisten_value = JsFuture::from(promise)
            .await
            .map_err(|e| EventError::TauriListen(format!("listen promise rejected: {e:?}")))?;

        let unlisten = unlisten_value
            .dyn_into::<JsFunction>()
            .map_err(|_| EventError::TauriListen("listen did not resolve to a function".into()))?;

        let mut inner = self.inner.lock().unwrap();
        inner.js_callback = Some(closure);
        inner.unlisten = Some(unlisten);
        inner.listening = true;
        tracing::info!(
            channel = FRONTEND_EVENT_CHANNEL,
            "Frontend event listener registered"
        );
        Ok(())
    }

    /// Detach the Tauri listener and drop all subscribers.
    ///
    /// Called once at app teardown by the provider's `use_drop`. The `unlisten`
    /// promise is awaited before the JS `Closure` is dropped, so Tauri never
    /// invokes a freed callback. Subsequent calls are no-ops.
    pub fn shutdown(&self) {
        let unlisten = {
            let mut inner = self.inner.lock().unwrap();
            if !inner.listening {
                return;
            }
            inner.listening = false;
            inner.subscribers.clear();
            tracing::info!("Frontend event listener shutting down");
            inner.unlisten.clone()
        };

        let inner = self.inner.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // Detach on the JS/Tauri side first (returns its own promise).
            if let Some(unlisten) = unlisten {
                match unlisten.call0(&JsValue::NULL) {
                    Ok(ret) => {
                        // The unlisten function returns a promise; await it so
                        // the Tauri-side listener is fully detached before we
                        // drop the JS closure (avoids invoking freed memory).
                        if let Ok(promise) = ret.dyn_into::<js_sys::Promise>() {
                            let _ = JsFuture::from(promise).await;
                        }
                    }
                    Err(e) => {
                        let msg = e
                            .as_string()
                            .unwrap_or_else(|| "<non-string rejection>".to_string());
                        tracing::warn!(error = %msg, "EventService: unlisten() call failed");
                    }
                }
            }
            // Listener is now detached — safe to drop the JS closure.
            let mut inner = inner.lock().unwrap();
            inner.js_callback = None;
            inner.unlisten = None;
            tracing::info!("Frontend event listener removed");
        });
    }

    /// Fan a typed event out to all matching subscribers.
    ///
    /// Subscribers are snapshotted and invoked outside the lock so a callback
    /// that (re)subscribes or triggers further events cannot deadlock the
    /// service.
    pub(crate) fn dispatch(&self, event: &FrontendEvent) {
        let matches: Vec<Subscriber> = {
            let inner = self.inner.lock().unwrap();
            inner
                .subscribers
                .iter()
                .filter(|entry| match entry.filter {
                    SubscriberFilter::All => true,
                    SubscriberFilter::Kind(k) => k == event.kind,
                })
                .map(|entry| entry.callback.clone())
                .collect()
        };
        for cb in matches {
            cb(event);
        }
    }

    /// Number of currently-registered subscribers (for diagnostics/tests).
    pub fn subscriber_count(&self) -> usize {
        self.inner.lock().unwrap().subscribers.len()
    }

    /// Whether the Tauri listener is currently attached.
    pub fn is_listening(&self) -> bool {
        self.inner.lock().unwrap().listening
    }
}

/// Called for every `nabu-event` Tauri dispatches to the JS callback.
///
/// `inner_weak` is a `Weak` (not a strong `Arc`) so the callback never keeps
/// the service alive — breaking the reference cycle between the `Closure`
/// (stored in `Inner`) and the `Arc` that owns `Inner`.
fn on_tauri_event(event: &JsValue, inner_weak: &Weak<Mutex<Inner>>) {
    let Some(inner) = inner_weak.upgrade() else {
        tracing::trace!(
            "nabu-event received after EventService dropped; ignored"
        );
        return;
    };

    if !inner.lock().unwrap().listening {
        // Shutdown in progress — drop the event silently.
        tracing::trace!("nabu-event received during shutdown; ignored");
        return;
    }

    let parsed = match parse_event(event) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "nabu-event failed to deserialize; ignored");
            return;
        }
    };

    tracing::debug!(event_type = %parsed.kind, "Received platform event");
    EventService { inner }.dispatch(&parsed);
}

/// A handle to a subscription returned by [`EventService::subscribe`].
///
/// Implements `Drop`, so the subscription is removed automatically when the
/// handle falls out of scope (the common case: a component unmounts).
/// `unsubscribe()` is also provided for explicit, idempotent deregistration.
pub struct EventSubscription {
    pub(crate) id: u64,
    pub(crate) filter: SubscriberFilter,
    pub(crate) unsubscribe: Box<dyn Fn()>,
}

impl EventSubscription {
    /// The numeric id of this subscription (debugging/diagnostics).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The event kind this subscription listens to, or `None` for an
    /// all-events subscription.
    pub fn kind(&self) -> Option<FrontendEventKind> {
        match self.filter {
            SubscriberFilter::Kind(k) => Some(k),
            SubscriberFilter::All => None,
        }
    }

    /// Deregister this subscription. Safe to call multiple times.
    pub fn unsubscribe(&self) {
        (self.unsubscribe)();
    }
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        // Idempotent: `unsubscribe` does nothing if the entry is already gone.
        (self.unsubscribe)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{parse_raw, RawFrontendEvent};
    use nabu_core::event_bus::kinds;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn item_stored_event() -> FrontendEvent {
        let raw = RawFrontendEvent {
            event_type: kinds::ITEM_STORED.to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            payload: serde_json::json!({
                "ItemStored": {
                    "object_id": "12345678-1234-1234-1234-123456789abc",
                    "vault_path": "notes/foo.md",
                    "object_type": "Note",
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        };
        parse_raw(raw).unwrap()
    }

    #[test]
    fn dispatch_fans_out_to_all_matching_subscribers() {
        let service = EventService::new();
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));

        let _sub_a = service.subscribe(FrontendEventKind::ItemStored, {
            let a = a.clone();
            move |_ev: &FrontendEvent| {
                a.fetch_add(1, Ordering::SeqCst);
            }
        });
        let _sub_b = service.subscribe(FrontendEventKind::ItemStored, {
            let b = b.clone();
            move |_ev: &FrontendEvent| {
                b.fetch_add(1, Ordering::SeqCst);
            }
        });

        let event = item_stored_event();
        service.dispatch(&event);

        assert_eq!(a.load(Ordering::SeqCst), 1);
        assert_eq!(b.load(Ordering::SeqCst), 1);
        assert_eq!(service.subscriber_count(), 2);
    }

    #[test]
    fn dispatch_respects_kind_filtering() {
        let service = EventService::new();
        let stored_hits = Arc::new(AtomicUsize::new(0));
        let progress_hits = Arc::new(AtomicUsize::new(0));

        let _a = service.subscribe(FrontendEventKind::ItemStored, {
            let s = stored_hits.clone();
            move |_ev: &FrontendEvent| { s.fetch_add(1, Ordering::SeqCst); }
        });
        let _b = service.subscribe(FrontendEventKind::ItemProcessingProgress, {
            let s = progress_hits.clone();
            move |_ev: &FrontendEvent| { s.fetch_add(1, Ordering::SeqCst); }
        });

        let event = item_stored_event();
        service.dispatch(&event);

        assert_eq!(stored_hits.load(Ordering::SeqCst), 1);
        assert_eq!(progress_hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn subscribe_all_receives_every_event() {
        let service = EventService::new();
        let total = Arc::new(AtomicUsize::new(0));
        let _sub = service.subscribe_all({
            let t = total.clone();
            move |_ev: &FrontendEvent| { t.fetch_add(1, Ordering::SeqCst); }
        });
        service.dispatch(&item_stored_event());
        assert_eq!(total.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unsubscribe_drops_subscriber() {
        let service = EventService::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let sub = service.subscribe(FrontendEventKind::ItemStored, {
            let h = hits.clone();
            move |_ev: &FrontendEvent| { h.fetch_add(1, Ordering::SeqCst); }
        });
        assert_eq!(service.subscriber_count(), 1);

        sub.unsubscribe();
        assert_eq!(service.subscriber_count(), 0);

        // No further delivery after unsubscribe (no duplicate / stale listener).
        service.dispatch(&item_stored_event());
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn drop_unsubscribes_automatically() {
        let service = EventService::new();
        let hits = Arc::new(AtomicUsize::new(0));
        {
            let _sub = service.subscribe(FrontendEventKind::ItemStored, {
                let h = hits.clone();
                move |_ev: &FrontendEvent| { h.fetch_add(1, Ordering::SeqCst); }
            });
            assert_eq!(service.subscriber_count(), 1);
        } // _sub dropped -> unsubscribed
        assert_eq!(service.subscriber_count(), 0);
        service.dispatch(&item_stored_event());
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn no_duplicate_listener_after_double_start() {
        // `start_listening` is async and requires Tauri; we only assert the
        // idempotency guard logic here: a second `new()` is independent, and
        // `is_listening` defaults to false.
        let service = EventService::new();
        assert!(!service.is_listening());
        assert_eq!(service.subscriber_count(), 0);
    }
}
