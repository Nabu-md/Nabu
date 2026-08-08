//! # Metrics State — Centralized Frontend Metrics Pipeline
//!
//! Provides the single source of truth for runtime metrics on the frontend,
//! completing the end-to-end pipeline:
//!
//! ```text
//! Runtime Services → Metrics Aggregator → `metrics` IPC → Frontend IPC Client
//!   → Metrics Store (Signal<RuntimeMetrics>) → UI
//! ```
//!
//! ## Architecture
//!
//! * **Single source of truth** — one `MetricsContext` provided at the app
//!   root; all consumers (Statistics view, future dashboards) read from it.
//! * **IPC client** — `reload_metrics()` invokes the backend `metrics`
//!   Tauri command via the existing `crate::ipc` abstraction, deserializes the
//!   response using `serde-wasm-bindgen`, and updates the context signals.
//! * **Event-driven refresh** — subscribes to relevant platform events
//!   (`ItemStored`, `ItemProcessingCompleted`) so metrics update reactively
//!   after operations that mutate recorded data.
//! * **Periodic refresh** — an optional interval timer can be started for
//!   views that benefit from polling (e.g. active capture sessions).
//! * **Graceful degradation** — IPC failures, missing backend, or malformed
//!   responses set an error string in the context rather than crashing.
//!
//! ## Future Compatibility
//!
//! The `RuntimeMetrics` type is re-exported from `nabu-core` (not
//! duplicated), so any new metric types added on the backend are immediately
//! available on the frontend after a single recompile. The
//! `MetricsProvider` is designed to be a drop-in foundation for future
//! monitoring dashboards, diagnostics panels, Prometheus/OpenTelemetry
//! exporters, and cloud-monitoring adapters — none of which are implemented
//! here.

use dioxus::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

// Re-export the canonical backend types so the frontend has a single, typed
// view of the metrics schema. Do NOT duplicate the struct definitions here —
// `nabu-core` is the source of truth and uses `#[derive(Serialize, Deserialize)]`.
pub use nabu_core::registry::{
    CounterMetric, GaugeMetric, MetricsAggregator, MetricsError, RuntimeMetrics,
    ServiceMetrics, TimerMetric,
};

/// Shared metrics context — carries the current snapshot and load state.
///
/// Created once by [`MetricsProvider`] and stored in the Dioxus context tree.
/// `Copy` (it only holds `Signal` handles) so it can be freely passed into
/// closures and async tasks.
#[derive(Clone, Copy)]
pub struct MetricsContext {
    /// The latest metrics snapshot received from the backend.
    pub metrics: Signal<RuntimeMetrics>,
    /// Whether a metrics fetch is in progress.
    pub loading: Signal<bool>,
    /// `Some(error)` when the last fetch failed to deserialize or the IPC
    /// rejected. `None` when there is no error (including while loading).
    pub error: Signal<Option<String>>,
    /// Number of times a refresh has been requested (for cache-busting or
    /// display).
    pub refresh_count: Signal<u32>,
}

/// Retrieves the metrics context.
///
/// Panics if called outside a [`MetricsProvider`] subtree — same contract as
/// the other `use_*` accessors in this codebase.
pub fn use_metrics() -> MetricsContext {
    use_context::<MetricsContext>()
}

/// Provider component for centralized metrics state.
///
/// Wrap the application tree (typically at the root, alongside other providers)
/// so every component can call [`use_metrics`] to read the current snapshot,
/// trigger a refresh, or react to load/error state.
#[component]
pub fn MetricsProvider(children: Element) -> Element {
    provide_context(MetricsContext {
        metrics: use_signal(RuntimeMetrics::default),
        loading: use_signal(|| false),
        error: use_signal(|| None),
        refresh_count: use_signal(|| 0u32),
    });

    // One-time initial load.
    let ctx = use_metrics();
    let initialized = use_signal(|| false);
    if !*initialized.read() {
        *initialized.write_unchecked() = true;
        reload_metrics(ctx);
        subscribe_events(ctx);
    }

    rsx! { {children} }
}

/// Invokes the backend `metrics` IPC command, deserializes the response,
/// and updates the [`MetricsContext`] signals.
///
/// Uses `tauri_invoke_safe` so a missing or rejected command sets the error
/// signal rather than crashing the renderer. The response is deserialized via
/// `serde_wasm_bindgen::from_value` — the same path used by every other IPC
/// consumer in this crate — so `RuntimeMetrics` fields (timers, counters,
/// gauges, services) must round-trip through `serde_json::Value` exactly as
/// the backend serializes them.
pub fn reload_metrics(mut ctx: MetricsContext) {
    ctx.loading.set(true);
    ctx.error.set(None);
    let mut metrics = ctx.metrics;
    let mut loading = ctx.loading;
    let mut error = ctx.error;
    let mut refresh = ctx.refresh_count;
    spawn_local(async move {
        let empty =
            serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap_or(JsValue::UNDEFINED);
        let result = crate::ipc::tauri_invoke_safe("metrics", empty).await;
        match result {
            Some(val) => match serde_wasm_bindgen::from_value::<RuntimeMetrics>(val) {
                Ok(snap) => {
                    metrics.set(snap);
                    error.set(None);
                }
                Err(e) => {
                    error.set(Some(format!("metrics deserialization failed: {e}")));
                }
            },
            None => {
                error.set(Some(
                    "metrics IPC command unavailable — backend may not be running".to_string(),
                ));
            }
        }
        loading.set(false);
        let current = refresh.read();
        let next = *current + 1;
        drop(current);
        refresh.set(next);
    });
}

/// Subscribes to platform events that indicate metrics may have changed,
/// triggering an automatic refresh.
///
/// Listens for `ItemStored` and `ItemProcessingCompleted` — these cover the
/// common cases where timers and counters are updated. The subscription is
/// cleaned up automatically when the app unmounts (via `use_drop` in the
/// `use_event_listener` hook).
fn subscribe_events(ctx: MetricsContext) {
    use crate::events::{use_event_listener, FrontendEventKind};
    use crate::events::FrontendEvent;
    use nabu_core::event_bus::PipelineEvent;

    // ItemStored — a capture has completed and been persisted; timers/counters
    // in the capture and storage subsystems may have advanced.
    use_event_listener(FrontendEventKind::ItemStored, move |_ev: &FrontendEvent| {
        reload_metrics(ctx);
    });

    // ItemProcessingCompleted — a processing cycle finished; processing timer
    // and throughput counter have been updated.
    use_event_listener(
        FrontendEventKind::ItemProcessingCompleted,
        move |_ev: &FrontendEvent| {
            reload_metrics(ctx);
        },
    );

    // Pipeline migration completed — indexer timer/counter may have advanced.
    // Reuse the ItemProcessingCompleted event kind which also fires for
    // pipeline steps.
    let _ = PipelineEvent::ItemStored; // keep import alive for future kinds
}

/// Starts an interval that polls metrics every `interval_ms` milliseconds.
///
/// Returns a cleanup closure. Call inside `use_effect` or a component body:
///
/// ```no_run
/// # use dioxus::prelude::*;
/// # let ctx: crate::metrics::MetricsContext = unreachable!();
/// # let interval_ms = 5000u32;
/// let cleanup = start_metrics_interval(ctx, interval_ms);
/// use_drop(move || cleanup());
/// ```
///
/// The interval uses `window.setTimeout` (via
/// [`crate::components::ui::feedback::set_timeout`]) recursively rather than
/// `setInterval` so it integrates with the existing task-tracking and
/// cleanup model.
pub fn start_metrics_interval(ctx: MetricsContext, interval_ms: u32) -> impl FnOnce() {
    use crate::components::ui::feedback::set_timeout;
    use std::cell::RefCell;

    let active = std::rc::Rc::new(RefCell::new(true));
    let active_clone = active.clone();

    fn schedule(
        ctx: MetricsContext,
        ms: u32,
        active: std::rc::Rc<std::cell::RefCell<bool>>,
    ) {
        if !*active.borrow() {
            return;
        }
        set_timeout(move || {
            if *active.borrow() {
                reload_metrics(ctx);
                schedule(ctx, ms, active.clone());
            }
        }, ms);
    }

    schedule(ctx, interval_ms, active_clone.clone());

    move || {
        *active_clone.borrow_mut() = false;
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_context_is_copy() {
        // MetricsContext must be Copy so it can be captured by move closures
        // in async tasks and event callbacks without ownership issues.
        fn assert_copy<T: Copy>() {}
        assert_copy::<MetricsContext>();
    }

    #[test]
    fn runtime_metrics_has_three_axes() {
        let m = RuntimeMetrics::default();
        assert!(m.timers.is_empty());
        assert!(m.counters.is_empty());
        assert!(m.gauges.is_empty());
    }

    #[test]
    fn runtime_metrics_serialization_round_trips() {
        let original = RuntimeMetrics {
            timers: vec![TimerMetric {
                key: "capture.ingest".to_string(),
                count: 10,
                window_count: 10,
                min_ms: 1.0,
                max_ms: 50.0,
                avg_ms: 15.0,
                p50_ms: 12.0,
                p90_ms: 30.0,
                p99_ms: 45.0,
                sum_ms: 150.0,
            }],
            counters: vec![CounterMetric {
                key: "capture.count".to_string(),
                value: 42,
            }],
            gauges: vec![GaugeMetric {
                key: "queue.depth".to_string(),
                value: 7,
            }],
            services: vec![ServiceMetrics {
                service: "capture_engine".to_string(),
                timers: Vec::new(),
                counters: vec![CounterMetric {
                    key: "capture.ingest".to_string(),
                    value: 10,
                }],
                gauges: vec![GaugeMetric {
                    key: "capture.handler_count".to_string(),
                    value: 8,
                }],
            }],
            service_count: 1,
            error_count: 0,
            errors: Vec::new(),
        };

        let json = serde_json::to_string(&original).expect("serialize metrics");
        let restored: RuntimeMetrics =
            serde_json::from_str(&json).expect("deserialize metrics");
        assert_eq!(restored, original);
    }

    #[test]
    fn runtime_metrics_ignores_unknown_future_fields() {
        let json = r#"{
            "timers": [],
            "counters": [],
            "gauges": [],
            "services": [],
            "service_count": 1,
            "error_count": 0,
            "errors": [],
            "uptime_ms": 12345,
            "version": "0.1.0"
        }"#;
        let restored: RuntimeMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(restored.service_count, 1);
    }
}
