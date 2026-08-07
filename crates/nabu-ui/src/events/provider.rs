//! Dioxus integration for the frontend event layer.
//!
//! [`EventServiceProvider`] installs the single `nabu-event` listener once,
//! keeps it alive for the application lifetime, and exposes an [`EventService`]
//! through the Dioxus context so descendant components can subscribe without
//! ever touching the TaIuri listen API.
//!
//! Two ergonomic helpers are provided:
//!
//! * [`use_event_service`] — fetch the shared `EventService` (for advanced use).
//! * [`use_event_listener`] — subscribe to a typed event for the lifetime of the
//!   calling component, with automatic cleanup on unmount.

use std::sync::atomic::{AtomicBool, Ordering};

use dioxus::prelude::*;

use crate::events::service::{EventService, EventSubscription};
use crate::events::types::FrontendEventKind;
use crate::events::FrontendEvent;

/// Once flag guarding the global `tracing-wasm` subscriber installation.
///
/// `try_set_as_global_default` is itself idempotent (it returns `Err` if a
/// subscriber is already installed rather than panicking), but the guard avoids
/// repeatedly constructing the default layer config each render.
static TRACING_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install a `tracing-wasm` subscriber so `tracing::info!`/`warn!`/`error!`
/// calls in this module actually emit to the webview console.
///
/// Idempotent and panic-safe (`try_set_as_global_default` never panics). This
/// is the frontend analogue of `nabu_core::diagnostics::init` on the backend —
/// it exists so the event layer can emit the diagnostics required by its spec
/// ("Frontend listener registered", "Received ITEM_STORED", …) without the host
/// shell having to opt in.
pub fn init_logging() {
    if TRACING_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = tracing_wasm::try_set_as_global_default();
}

/// Provider that owns the lifetime of the frontend event listener.
///
/// Wrap the application tree (typically at the root, alongside the other
/// context providers) so every component can call [`use_event_service`] or
/// [`use_event_listener`]. The listener is started once on first mount and
/// detached on teardown via `use_drop`.
#[component]
pub fn EventServiceProvider(children: Element) -> Element {
    init_logging();

    // `use_signal` runs its init closure exactly once per scope, so the
    // `EventService` (an `Arc` clone) is stable across re-renders.
    let service = use_signal(|| EventService::new());
    let svc = service.peek().clone();
    provide_context(svc.clone());

    // Start the single Tauri listener exactly once.
    let started = use_signal(|| false);
    if !*started.read() {
        *started.write_unchecked() = true;
        let svc = svc.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = svc.start_listening().await {
                // Outside Tauri (e.g. the dev server) the listener simply
                // never attaches; the service stays usable for in-process
                // dispatch. Surface the reason for debugging.
                tracing::warn!(error = %e, "EventServiceProvider: listener not started");
            }
        });
    }

    // Lifecycle-aware teardown: detach + drop the JS callback + subscribers.
    // `use_drop` registers exactly one cleanup per scope (Dioxus hooks are
    // scope-cached), so this fires once at unmount / app exit.
    use_drop(move || {
        svc.shutdown();
    });

    rsx! { {children} }
}

/// Fetch the shared [`EventService`] provided by [`EventServiceProvider`].
///
/// Panics if called outside a provider subtree — same contract as the other
/// `use_*` context accessors in this codebase.
pub fn use_event_service() -> EventService {
    use_context::<EventService>()
}

/// Subscribe to a typed platform event for the lifetime of the calling
/// component.
///
/// The subscription is created once (on first mount) and automatically removed
/// when the component unmounts. `callback` receives a strongly-typed
/// [`crate::events::FrontendEvent`]; pattern-match on `.payload` (a
/// `PipelineEvent`) to access the typed data.
///
/// # Example
///
/// ```no_run
/// # use dioxus::prelude::*;
/// # use crate::events::{use_event_listener, FrontendEvent, FrontendEventKind};
/// # use nabu_core::event_bus::PipelineEvent;
/// // (inside a component body, under an EventServiceProvider)
/// use_event_listener(FrontendEventKind::ItemStored, move |ev: &FrontendEvent| {
///     if let PipelineEvent::ItemStored(stored) = &ev.payload {
///         tracing::info!(path = &stored.vault_path, "note stored");
///     }
/// });
/// ```
pub fn use_event_listener(
    kind: FrontendEventKind,
    callback: impl Fn(&FrontendEvent) + 'static,
) {
    let service = use_event_service();

    // The subscription handle is stored in a Signal so it survives re-renders.
    // `use_signal`'s init runs once; `peek`/`write_unchecked` read/write it.
    let handle = use_signal(|| None::<EventSubscription>);

    // Register exactly once. `peek` (non-subscribing read) checks whether a
    // subscription already exists before subscribing again.
    if handle.peek().is_none() {
        *handle.write_unchecked() = Some(service.subscribe(kind, callback));
    }

    // Remove the subscription when the component unmounts. `use_drop` is
    // unconditional (hooks-rule compliant) and fires once at scope teardown.
    use_drop(move || {
        if let Some(sub) = handle.write_unchecked().take() {
            sub.unsubscribe();
        }
    });
}
