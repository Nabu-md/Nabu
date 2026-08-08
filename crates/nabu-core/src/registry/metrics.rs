//! # Metrics Aggregation — Runtime Metrics API for the Capability Platform
//!
//! This module defines the standardized contract for exposing runtime metrics
//! from the backend to the frontend and future monitoring tools.
//!
//! ## Architecture
//!
//! ```text
//! Frontend
//!     ↓  (`metrics` IPC)
//! ApplicationContext::metrics()
//!     ↓
//! MetricsAggregator implementations (registered services)
//!     ↓
//! RuntimeMetrics (timers, counters, gauges)
//!     ↓
//! Serialize → Frontend
//! ```
//!
//! ## Design Principles
//!
//! - **Lightweight**: Metrics collection is inexpensive — no blocking I/O,
//!   no expensive computation. Designed for frequent polling.
//! - **Provider-independent**: No dependency on a specific metrics backend
//!   (Prometheus, OpenTelemetry, etc.). The snapshot is a plain serializable
//!   struct.
//! - **Thread-safe**: All metric types use atomic operations or RwLock
//!   interior mutability.
//! - **Graceful degradation**: Unavailable or partially-initialized services
//!   do not prevent returning the remaining available metrics.
//! - **Forward-compatible**: All response fields use `#[serde(default)]` so
//!   new metric types or metadata can be added without breaking serialization.
//!
//! ## Extension Points
//!
//! Future services can expose additional metrics by implementing the
//! [`MetricsAggregator`] trait and registering the service in the
//! [`ServiceRegistry`]. The `ApplicationContext::metrics()` method will
//! automatically discover and aggregate them.
//!
//! [`ServiceRegistry`]: crate::registry::ServiceRegistry

use serde::{Deserialize, Serialize};
use std::sync::Mutex as StdMutex;
use std::sync::RwLock as StdRwLock;

// // ---------------------------------------------------------------------------
// // Metric types
// // ---------------------------------------------------------------------------

/// A single timer metric snapshot with statistics.
///
/// Timer metrics measure the duration of operations (e.g. indexing duration,
/// synchronization duration, startup time). Statistics are computed from a
/// sliding-window sample buffer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimerMetric {
    /// The metric key (e.g. `"capture.ingest"`, `"indexer.index"`).
    pub key: String,
    /// Number of recorded samples (total, not just window).
    pub count: u64,
    /// Number of samples in the current sliding window.
    pub window_count: u64,
    /// Minimum recorded duration in milliseconds.
    pub min_ms: f64,
    /// Maximum recorded duration in milliseconds.
    pub max_ms: f64,
    /// Average duration in milliseconds.
    pub avg_ms: f64,
    /// 50th percentile duration in milliseconds.
    pub p50_ms: f64,
    /// 90th percentile duration in milliseconds.
    pub p90_ms: f64,
    /// 99th percentile duration in milliseconds.
    pub p99_ms: f64,
    /// Sum of all recorded durations in milliseconds.
    pub sum_ms: f64,
}

/// A single counter metric snapshot.
///
/// Counter metrics are monotonically increasing counts (e.g. documents
/// processed, events published, IPC requests).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterMetric {
    /// The metric key (e.g. `"events.published"`, `"ipc.requests"`).
    pub key: String,
    /// The current counter value.
    pub value: u64,
}

/// A single gauge metric snapshot.
///
/// Gauge metrics represent point-in-time values that can go up or down
/// (e.g. active workers, queued tasks, connected services).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GaugeMetric {
    /// The metric key (e.g. `"workers.active"`, `"queue.depth"`).
    pub key: String,
    /// The current gauge value.
    pub value: i64,
}

/// Service-level metric entry — a named set of metric values.
///
/// Each service that implements [`MetricsAggregator`] can contribute
/// its own timers, counters, and gauges to the unified runtime metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ServiceMetrics {
    /// The service key (e.g. `"capture_engine"`, `"storage_manager"`).
    pub service: String,
    /// Timer metrics for this service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timers: Vec<TimerMetric>,
    /// Counter metrics for this service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<CounterMetric>,
    /// Gauge metrics for this service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gauges: Vec<GaugeMetric>,
}

/// The complete runtime metrics response.
///
/// This is the canonical response type for the `metrics` IPC endpoint.
/// It aggregates metrics from the [`PerformanceMonitor`] (instrumented
/// subsystem operations) and from every registered service that implements
/// [`MetricsAggregator`].
///
/// All fields use `#[serde(default)]` and `skip_serializing_if = "Vec::is_empty"`
/// so that future metric types or service sections can be added without
/// breaking existing deserialization on the frontend.
///
/// [`PerformanceMonitor`]: crate::diagnostics::PerformanceMonitor
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RuntimeMetrics {
    /// Aggregated timers from the PerformanceMonitor and service-specific
    /// metrics aggregators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timers: Vec<TimerMetric>,

    /// Aggregated counters from the PerformanceMonitor and service-specific
    /// metrics aggregators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<CounterMetric>,

    /// Aggregated gauges from the PerformanceMonitor and service-specific
    /// metrics aggregators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gauges: Vec<GaugeMetric>,

    /// Per-service metric snapshots. Each entry groups timers, counters,
    /// and gauges contributed by a single service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ServiceMetrics>,

    /// Number of services that were queried for metrics.
    #[serde(default)]
    pub service_count: usize,

    /// Number of errors encountered during metric collection.
    #[serde(default)]
    pub error_count: usize,

    /// Error messages from services that failed to report metrics.
    /// Empty when all services reported successfully.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl RuntimeMetrics {
    /// Creates an empty `RuntimeMetrics` with all fields initialized to defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges another `RuntimeMetrics` into this one.
    ///
    /// This is used by the aggregator to combine metrics from multiple sources
    /// into a single unified response. Vectors are concatenated; scalars are
    /// accumulated.
    pub fn merge(&mut self, other: RuntimeMetrics) {
        self.timers.extend(other.timers);
        self.counters.extend(other.counters);
        self.gauges.extend(other.gauges);
        self.services.extend(other.services);
        self.service_count += other.service_count;
        self.error_count += other.error_count;
        self.errors.extend(other.errors);
    }
}

// ---------------------------------------------------------------------------
// MetricsAggregator trait
// ---------------------------------------------------------------------------

/// Trait for services that expose runtime metrics.
///
/// Services implementing this trait can be discovered by the
/// [`ApplicationContext`] and queried for metric snapshots. The trait is
/// designed to be lightweight — `metrics()` returns a snapshot, not a live
/// reference, so implementations must clone any internal state.
///
/// ## Thread Safety
///
/// The method takes `&self` (not `&mut self`) and must be safe to call
/// concurrently. Implementations should use atomic operations or
/// `RwLock` interior mutability to ensure consistent snapshots.
///
/// ## Future Compatibility
///
/// Future metrics types (histograms, summaries) can be added to
/// [`ServiceMetrics`] without requiring changes to existing implementors
/// — the new fields will simply remain empty.
///
/// [`ApplicationContext`]: crate::registry::context::ApplicationContext

pub trait MetricsAggregator: Send + Sync {
    /// Returns a snapshot of this service's runtime metrics.
    ///
    /// Implementations should:
    /// - Return all timers, counters, and gauges the service tracks.
    /// - Be inexpensive — avoid blocking I/O or expensive computation.
    /// - Handle lock poisoning gracefully — return an empty snapshot
    ///   rather than panicking.
    fn metrics(&self) -> ServiceMetrics;
}

/// Blanket implementation for `std::sync::Mutex<T>` where `T: MetricsAggregator`.
///
/// This allows services wrapped in `Arc<Mutex<T>>` (e.g. `Indexer`) to be
/// registered as metrics aggregators without explicit wrapper impls at each
/// call site. Lock poisoning is handled gracefully — a poisoned lock returns
/// an empty metric snapshot.
impl<T: MetricsAggregator + ?Sized> MetricsAggregator for StdMutex<T> {
    fn metrics(&self) -> ServiceMetrics {
        match self.lock() {
            Ok(inner) => inner.metrics(),
            Err(poisoned) => {
                tracing::warn!("Metrics collection from Mutex<T> skipped: lock poisoned");
                let _ = poisoned;
                ServiceMetrics::default()
            }
        }
    }
}

/// Blanket implementation for `std::sync::RwLock<T>` where `T: MetricsAggregator`.
///
/// This allows services wrapped in `Arc<RwLock<T>>` (e.g. `VaultGraph`) to be
/// registered as metrics aggregators. Lock poisoning is handled gracefully —
/// a poisoned lock returns an empty metric snapshot.
impl<T: MetricsAggregator + ?Sized> MetricsAggregator for StdRwLock<T> {
    fn metrics(&self) -> ServiceMetrics {
        match self.read() {
            Ok(inner) => inner.metrics(),
            Err(poisoned) => {
                tracing::warn!("Metrics collection from RwLock<T> skipped: lock poisoned");
                let _ = poisoned;
                ServiceMetrics::default()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MetricsError
// ---------------------------------------------------------------------------

/// Structured error type for metrics collection failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsError {
    /// The service that failed to report metrics, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Human-readable error description.
    pub message: String,
    /// Whether this error prevents returning the remaining metrics
    /// (`true`) or can be recovered from (`false`).
    #[serde(default)]
    pub fatal: bool,
}

impl MetricsError {
    /// Creates a new non-fatal metrics error for a service.
    pub fn service_error(service: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            service: Some(service.into()),
            message: message.into(),
            fatal: false,
        }
    }

    /// Creates a new fatal metrics error.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            service: None,
            message: message.into(),
            fatal: true,
        }
    }
}

impl std::fmt::Display for MetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.service {
            Some(s) => write!(f, "metrics error for '{}': {}", s, self.message),
            None => write!(f, "metrics error: {}", self.message),
        }
    }
}

impl std::error::Error for MetricsError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_metric_serializes() {
        let m = TimerMetric {
            key: "test.timer".to_string(),
            count: 10,
            window_count: 5,
            min_ms: 1.0,
            max_ms: 100.0,
            avg_ms: 50.0,
            p50_ms: 50.0,
            p90_ms: 80.0,
            p99_ms: 99.0,
            sum_ms: 500.0,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: TimerMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, "test.timer");
        assert_eq!(back.count, 10);
        assert_eq!(back.window_count, 5);
        assert!((back.avg_ms - 50.0).abs() < 0.01);
    }

    #[test]
    fn counter_metric_serializes() {
        let m = CounterMetric {
            key: "test.counter".to_string(),
            value: 42,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: CounterMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, "test.counter");
        assert_eq!(back.value, 42);
    }

    #[test]
    fn gauge_metric_serializes() {
        let m = GaugeMetric {
            key: "test.gauge".to_string(),
            value: 7,
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: GaugeMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, "test.gauge");
        assert_eq!(back.value, 7);
    }

    #[test]
    fn service_metrics_default_is_empty() {
        let s = ServiceMetrics::default();
        assert!(s.timers.is_empty());
        assert!(s.counters.is_empty());
        assert!(s.gauges.is_empty());
        assert!(s.service.is_empty());
    }

    #[test]
    fn runtime_metrics_default_is_empty() {
        let m = RuntimeMetrics::default();
        assert!(m.timers.is_empty());
        assert!(m.counters.is_empty());
        assert!(m.gauges.is_empty());
        assert!(m.services.is_empty());
        assert_eq!(m.service_count, 0);
        assert_eq!(m.error_count, 0);
        assert!(m.errors.is_empty());
    }

    #[test]
    fn runtime_metrics_merge_concatenates() {
        let mut a = RuntimeMetrics {
            timers: vec![TimerMetric {
                key: "a".to_string(),
                count: 1,
                window_count: 1,
                min_ms: 1.0,
                max_ms: 1.0,
                avg_ms: 1.0,
                p50_ms: 1.0,
                p90_ms: 1.0,
                p99_ms: 1.0,
                sum_ms: 1.0,
            }],
            counters: vec![CounterMetric {
                key: "a".to_string(),
                value: 1,
            }],
            gauges: vec![GaugeMetric {
                key: "a".to_string(),
                value: 1,
            }],
            services: vec![],
            service_count: 1,
            error_count: 1,
            errors: vec!["err a".to_string()],
        };

        let b = RuntimeMetrics {
            timers: vec![TimerMetric {
                key: "b".to_string(),
                count: 1,
                window_count: 1,
                min_ms: 2.0,
                max_ms: 2.0,
                avg_ms: 2.0,
                p50_ms: 2.0,
                p90_ms: 2.0,
                p99_ms: 2.0,
                sum_ms: 2.0,
            }],
            counters: vec![CounterMetric {
                key: "b".to_string(),
                value: 2,
            }],
            gauges: vec![GaugeMetric {
                key: "b".to_string(),
                value: 2,
            }],
            services: vec![ServiceMetrics {
                service: "svc_b".to_string(),
                timers: vec![],
                counters: vec![],
                gauges: vec![GaugeMetric {
                    key: "b.gauge".to_string(),
                    value: 2,
                }],
            }],
            service_count: 1,
            error_count: 0,
            errors: vec![],
        };

        a.merge(b);
        assert_eq!(a.timers.len(), 2);
        assert_eq!(a.counters.len(), 2);
        assert_eq!(a.gauges.len(), 2);
        assert_eq!(a.services.len(), 1);
        assert_eq!(a.service_count, 2);
        assert_eq!(a.error_count, 1);
        assert_eq!(a.errors.len(), 1);
    }

    #[test]
    fn runtime_metrics_serializes_with_empty_fields() {
        let m = RuntimeMetrics::new();
        let json = serde_json::to_string(&m).unwrap();
        let back: RuntimeMetrics = serde_json::from_str(&json).unwrap();
        assert!(back.timers.is_empty());
        assert!(back.counters.is_empty());
        assert!(back.gauges.is_empty());
        assert_eq!(back.service_count, 0);
    }

    #[test]
    fn metrics_error_service_error() {
        let e = MetricsError::service_error("my_service", "lock poisoned");
        assert_eq!(e.service.as_deref(), Some("my_service"));
        assert!(!e.fatal);
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("my_service"));
        assert!(json.contains("lock poisoned"));
    }

    #[test]
    fn metrics_error_deserializes_missing_service() {
        let json = r#"{"message":"global failure","fatal":true}"#;
        let e: MetricsError = serde_json::from_str(json).unwrap();
        assert!(e.service.is_none());
        assert!(e.fatal);
        assert_eq!(e.message, "global failure");
    }

    #[test]
    fn metrics_error_display() {
        let e = MetricsError::service_error("svc", "timeout");
        assert_eq!(format!("{}", e), "metrics error for 'svc': timeout");

        let f = MetricsError::fatal("catastrophic");
        assert_eq!(format!("{}", f), "metrics error: catastrophic");
    }
}
