//! # Metrics State — Centralized Frontend Metrics Pipeline
//!
//! Provides the single source of truth for runtime metrics on the frontend,
//! completing the end-to-end pipeline:
//!
//! ```text
//! Runtime Services → Metrics Aggregator → `metrics_get` IPC → Frontend IPC Client
//!   → Metrics Store (Signal<PerformanceSnapshot>) → UI
//! ```
//!
//! ## Architecture
//!
//! * **Single source of truth** — one `MetricsContext` provided at the app
//!   root; all consumers (Statistics view, future dashboards) read from it.
//! * **IPC client** — `reload_metrics()` invokes the backend `metrics_get`
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
//! The `PerformanceSnapshot` type is re-exported from `nabu-core` (not
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
pub use nabu_core::diagnostics::{
    CounterSnapshot, GaugeSnapshot, PerformanceSnapshot, PerformanceMonitor, Timer,
    TimerSnapshot, TimerStats,
};

/// Shared metrics context — carries the current snapshot and load state.
///
/// Created once by [`MetricsProvider`] and stored in the Dioxus context tree.
/// `Copy` (it only holds `Signal` handles) so it can be freely passed into
/// closures and async tasks.
#[derive(Clone, Copy)]
pub struct MetricsContext {
    /// The latest metrics snapshot received from the backend.
    pub metrics: Signal<PerformanceSnapshot>,
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
        metrics: use_signal(PerformanceSnapshot::default),
        loading: use_signal(|| false),
        error: use_signal(|| None),
        refresh_count: use_signal(|| 0u32),
    });

    // One-time initial load.
    let ctx = use_metrics();
    let mut initialized = use_signal(|| false);
    if !*initialized.read() {
        *initialized.write_unchecked() = true;
        reload_metrics(ctx);
        subscribe_events(ctx);
    }

    rsx! { {children} }
}

/// Invokes the backend `metrics_get` IPC command, deserializes the response,
/// and updates the [`MetricsContext`] signals.
///
/// Uses `tauri_invoke_safe` so a missing or rejected command sets the error
/// signal rather than crashing the renderer. The response is deserialized via
/// `serde_wasm_bindgen::from_value` — the same path used by every other IPC
/// consumer in this crate — so `PerformanceSnapshot` fields (timers, counters,
/// gauges) must round-trip through `serde_json::Value` exactly as the backend
/// serializes them.
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
        let result = crate::ipc::tauri_invoke_safe("metrics_get", empty).await;
        match result {
            Some(val) => match serde_wasm_bindgen::from_value::<PerformanceSnapshot>(val) {
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
                    "metrics_get IPC command unavailable — backend may not be running".to_string(),
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
    fn performance_snapshot_has_three_axes() {
        let snap = PerformanceSnapshot::default();
        assert!(snap.timers.is_empty());
        assert!(snap.counters.is_empty());
        assert!(snap.gauges.is_empty());
    }

    #[test]
    fn timer_stats_default_is_zeroed() {
        let stats = TimerStats::default();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.window_count, 0);
        assert_eq!(stats.avg_ms, 0.0);
        assert_eq!(stats.p90_ms, 0.0);
    }

    #[test]
    fn snapshot_serialization_round_trips() {
        let original = PerformanceSnapshot {
            timers: vec![TimerSnapshot {
                key: "capture.ingest".to_string(),
                stats: TimerStats {
                    count: 10,
                    window_count: 10,
                    min_ms: 1.0,
                    max_ms: 50.0,
                    avg_ms: 15.0,
                    p50_ms: 12.0,
                    p90_ms: 30.0,
                    p99_ms: 45.0,
                    sum_ms: 150.0,
                },
            }],
            counters: vec![CounterSnapshot {
                key: "capture.count".to_string(),
                value: 42,
            }],
            gauges: vec![GaugeSnapshot {
                key: "queue.depth".to_string(),
                value: 7,
            }],
        };

        let json = serde_json::to_string(&original).expect("serialize snapshot");
        let restored: PerformanceSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");
        assert_eq!(restored, original);
    }

    #[test]
    fn timer_snapshots_are_sorted_by_key() {
        let monitor = PerformanceMonitor::new();
        monitor.record("zeta", 1.0);
        monitor.record("alpha", 2.0);
        monitor.record("mid", 3.0);

        let snap = monitor.snapshot();
        let keys: Vec<&str> = snap.timers.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn counters_and_gauges_are_sorted_by_key() {
        let monitor = PerformanceMonitor::new();
        monitor.increment("zebra", 1);
        monitor.increment("apple", 2);
        monitor.set_gauge("mango", 5);
        monitor.set_gauge("banana", 10);

        let snap = monitor.snapshot();
        let counter_keys: Vec<&str> =
            snap.counters.iter().map(|c| c.key.as_str()).collect();
        let gauge_keys: Vec<&str> = snap.gauges.iter().map(|g| g.key.as_str()).collect();

        assert_eq!(counter_keys, vec!["apple", "zebra"]);
        assert_eq!(gauge_keys, vec!["banana", "mango"]);
    }
}
