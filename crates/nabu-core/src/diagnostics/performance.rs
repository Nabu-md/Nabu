//! # PerformanceMonitor — Local Performance Instrumentation
//!
//! The central metrics aggregator for Nabu's performance instrumentation.
//!
//! Every subsystem reports timing and count data through this single monitor.
//! No subsystem calculates timings independently.
//!
//! ## Design
//!
//! - All metrics are **local-only** — nothing leaves the machine.
//! - All metrics are **thread-safe** via RwLock interior mutability.
//! - Metrics use a sliding window (default 1000 samples) for recent history.
//! - Statistics (min, max, avg, p50, p90, p99) computed on-demand.
//! - Data is discardable between sessions unless explicitly persisted.
//!
//! ## Usage
//!
//! ```rust
//! use std::sync::Arc;
//! use nabu_core::diagnostics::PerformanceMonitor;
//!
//! let monitor = PerformanceMonitor::new();
//!
//! // Record a duration for a subsystem operation
//! monitor.record_capture("ingest", 42.0);
//!
//! // Record a queue latency
//! monitor.record_queue("enqueue", 5.0);
//!
//! // Record a processor duration
//! monitor.record_processor("ocr", 1500.0);
//!
//! // Query stats
//! let stats = monitor.capture_stats("ingest");
//! println!("Capture ingest: {}", stats);
//!
//! // Generate a full report
//! let report = monitor.report();
//! println!("{}", report);
//! ```

use crate::diagnostics::metrics::{
    Counter, Gauge, PerformanceSnapshot, Timer, TimerStats,
};
use crate::registry::metrics::{
    CounterMetric, GaugeMetric, MetricsAggregator, ServiceMetrics, TimerMetric,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

// ---------------------------------------------------------------------------
// Global monitor (convenience singleton)
// ---------------------------------------------------------------------------

static GLOBAL_MONITOR: OnceLock<Arc<PerformanceMonitor>> = OnceLock::new();

/// Get or initialize the global PerformanceMonitor singleton.
///
/// This is a convenience for subsystems that don't want to manage their own
/// monitor reference. Use `global_monitor()` from anywhere:
///
/// ```rust
/// use nabu_core::diagnostics::performance::global_monitor;
/// global_monitor().record("my.operation", 42.0);
/// ```
pub fn global_monitor() -> Arc<PerformanceMonitor> {
    GLOBAL_MONITOR
        .get_or_init(|| Arc::new(PerformanceMonitor::new()))
        .clone()
}

/// Reset the global monitor (for testing).
#[cfg(test)]
pub fn reset_global_monitor() {
    global_monitor().reset();
}

// ---------------------------------------------------------------------------
// Metric key constants
// ---------------------------------------------------------------------------

/// Capture subsystem metric keys.
pub mod capture_keys {
    pub const INGEST: &str = "capture.ingest";
    pub const ROUTE: &str = "capture.route";
    pub const ENQUEUE: &str = "capture.enqueue";
    pub const HANDLER: &str = "capture.handler";
}

/// Queue subsystem metric keys.
pub mod queue_keys {
    pub const ENQUEUE: &str = "queue.enqueue";
    pub const DEQUEUE: &str = "queue.dequeue";
    pub const LATENCY: &str = "queue.latency";
    pub const CANCEL: &str = "queue.cancel";
    pub const RETRY: &str = "queue.retry";
    pub const MARK_COMPLETED: &str = "queue.mark_completed";
    pub const MARK_FAILED: &str = "queue.mark_failed";
    pub const DEPTH: &str = "queue.depth";
}

/// Worker subsystem metric keys.
pub mod worker_keys {
    pub const RUN_CYCLE: &str = "worker.run_cycle";
    pub const PICKUP: &str = "worker.pickup";
    pub const EXECUTE: &str = "worker.execute";
    pub const UTILIZATION: &str = "worker.utilization";
    pub const ACTIVE_WORKERS: &str = "worker.active";
    pub const IDLE_WORKERS: &str = "worker.idle";
}

/// Processing pipeline metric keys.
pub mod processing_keys {
    pub const PIPELINE_RUN: &str = "processing.pipeline.run";
    pub const PROCESSOR_RUN: &str = "processing.processor.run";
    pub const THROUGHPUT: &str = "processing.throughput";
    pub const SKIPPED: &str = "processing.skipped";
    pub const CANCELLED: &str = "processing.cancelled";
}

/// Storage subsystem metric keys.
pub mod storage_keys {
    pub const SAVE: &str = "storage.save";
    pub const LOAD: &str = "storage.load";
    pub const DELETE: &str = "storage.delete";
}

/// Indexer subsystem metric keys.
pub mod indexer_keys {
    pub const INDEX: &str = "indexer.index";
    pub const REMOVE: &str = "indexer.remove";
    pub const SEARCH: &str = "indexer.search";
}

/// Graph subsystem metric keys.
pub mod graph_keys {
    pub const ADD_NODE: &str = "graph.add_node";
    pub const REMOVE_NODE: &str = "graph.remove_node";
    pub const ADD_EDGE: &str = "graph.add_edge";
    pub const PERSIST: &str = "graph.persist";
    pub const REBUILD: &str = "graph.rebuild";
    pub const LOAD: &str = "graph.load";
}

/// Event bus metric keys.
pub mod event_bus_keys {
    pub const PUBLISH: &str = "event_bus.publish";
    pub const SUBSCRIBE: &str = "event_bus.subscribe";
    pub const UNSUBSCRIBE: &str = "event_bus.unsubscribe";
}

/// Pipeline migration metric keys.
pub mod pipeline_keys {
    pub const EXECUTE: &str = "pipeline_migration.execute";
    pub const COMPLETED: &str = "pipeline_migration.completed";
    pub const FAILED: &str = "pipeline_migration.failed";
}

/// Export subsystem metric keys.
pub mod export_keys {
    pub const EXPORT: &str = "export.export";
    pub const HTML: &str = "export.html";
    pub const MARKDOWN: &str = "export.markdown";
}

// ---------------------------------------------------------------------------
// PerformanceMonitor
// ---------------------------------------------------------------------------

/// The central metrics aggregator for all Nabu subsystems.
///
/// Thread-safe, local-only, sliding-window performance instrumentation.
pub struct PerformanceMonitor {
    timers: RwLock<HashMap<String, Timer>>,
    counters: RwLock<HashMap<String, Counter>>,
    gauges: RwLock<HashMap<String, Gauge>>,
}

impl PerformanceMonitor {
    /// Create a new PerformanceMonitor with no metrics.
    pub fn new() -> Self {
        Self {
            timers: RwLock::new(HashMap::new()),
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
        }
    }

    // ------------------------------------------------------------------
    // Timer recording
    // ------------------------------------------------------------------

    /// Record a duration for a named metric.
    pub fn record(&self, key: &str, duration_ms: f64) {
        if let Ok(mut timers) = self.timers.write() {
            timers
                .entry(key.to_string())
                .or_insert_with(Timer::new)
                .record_ms(duration_ms);
        }
    }

    /// Record a capture subsystem operation duration.
    pub fn record_capture(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("capture.{}", operation), duration_ms);
    }

    /// Record a queue subsystem operation duration.
    pub fn record_queue(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("queue.{}", operation), duration_ms);
    }

    /// Record a worker subsystem operation duration.
    pub fn record_worker(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("worker.{}", operation), duration_ms);
    }

    /// Record a processing subsystem operation duration.
    pub fn record_processing(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("processing.{}", operation), duration_ms);
    }

    /// Record a specific processor duration.
    pub fn record_processor(&self, processor_name: &str, duration_ms: f64) {
        self.record(
            &format!("processing.processor.{}", processor_name),
            duration_ms,
        );
    }

    /// Record a storage subsystem operation duration.
    pub fn record_storage(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("storage.{}", operation), duration_ms);
    }

    /// Record an indexer operation duration.
    pub fn record_indexer(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("indexer.{}", operation), duration_ms);
    }

    /// Record a graph operation duration.
    pub fn record_graph(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("graph.{}", operation), duration_ms);
    }

    /// Record an event bus operation duration.
    pub fn record_event_bus(&self, operation: &str, duration_ms: f64) {
        self.record(&format!("event_bus.{}", operation), duration_ms);
    }

    // ------------------------------------------------------------------
    // Counter operations
    // ------------------------------------------------------------------

    /// Increment a named counter.
    pub fn increment(&self, key: &str, delta: u64) -> u64 {
        if let Ok(mut counters) = self.counters.write() {
            counters
                .entry(key.to_string())
                .or_insert_with(Counter::new)
                .add(delta)
        } else {
            0
        }
    }

    /// Get a counter value.
    pub fn counter(&self, key: &str) -> u64 {
        if let Ok(counters) = self.counters.read() {
            counters.get(key).map(|c| c.value()).unwrap_or(0)
        } else {
            0
        }
    }

    // ------------------------------------------------------------------
    // Gauge operations
    // ------------------------------------------------------------------

    /// Set a named gauge.
    pub fn set_gauge(&self, key: &str, value: i64) {
        if let Ok(mut gauges) = self.gauges.write() {
            gauges
                .entry(key.to_string())
                .or_insert_with(Gauge::new)
                .set(value);
        }
    }

    /// Increment a gauge.
    pub fn increment_gauge(&self, key: &str, delta: i64) -> i64 {
        if let Ok(mut gauges) = self.gauges.write() {
            gauges
                .entry(key.to_string())
                .or_insert_with(Gauge::new)
                .add(delta)
        } else {
            0
        }
    }

    /// Get a gauge value.
    pub fn gauge(&self, key: &str) -> i64 {
        if let Ok(gauges) = self.gauges.read() {
            gauges.get(key).map(|g| g.value()).unwrap_or(0)
        } else {
            0
        }
    }

    // ------------------------------------------------------------------
    // Query
    // ------------------------------------------------------------------

    /// Get stats for a named timer.
    pub fn stats(&self, key: &str) -> TimerStats {
        if let Ok(timers) = self.timers.read() {
            timers.get(key).map(|t| t.stats()).unwrap_or_default()
        } else {
            TimerStats::default()
        }
    }

    /// Get stats for a capture operation.
    pub fn capture_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("capture.{}", operation))
    }

    /// Get stats for a queue operation.
    pub fn queue_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("queue.{}", operation))
    }

    /// Get stats for a worker operation.
    pub fn worker_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("worker.{}", operation))
    }

    /// Get stats for a processing operation.
    pub fn processing_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("processing.{}", operation))
    }

    /// Get stats for a specific processor.
    pub fn processor_stats(&self, processor_name: &str) -> TimerStats {
        self.stats(&format!("processing.processor.{}", processor_name))
    }

    /// Get stats for a storage operation.
    pub fn storage_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("storage.{}", operation))
    }

    /// Get stats for an indexer operation.
    pub fn indexer_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("indexer.{}", operation))
    }

    /// Get stats for a graph operation.
    pub fn graph_stats(&self, operation: &str) -> TimerStats {
        self.stats(&format!("graph.{}", operation))
    }

    // ------------------------------------------------------------------
    // Categories (bulk queries)
    // ------------------------------------------------------------------

    /// Get all timer stats matching a prefix (e.g., "capture", "queue").
    pub fn stats_by_prefix(&self, prefix: &str) -> Vec<(String, TimerStats)> {
        if let Ok(timers) = self.timers.read() {
            timers
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, t)| (k.clone(), t.stats()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// All capture-related metrics.
    pub fn capture_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("capture")
    }

    /// All queue-related metrics.
    pub fn queue_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("queue")
    }

    /// All worker-related metrics.
    pub fn worker_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("worker")
    }

    /// All processing-related metrics.
    pub fn processing_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("processing")
    }

    /// All storage-related metrics.
    pub fn storage_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("storage")
    }

    /// All indexer-related metrics.
    pub fn indexer_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("indexer")
    }

    /// All graph-related metrics.
    pub fn graph_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("graph")
    }

    /// All event bus metrics.
    pub fn event_bus_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("event_bus")
    }

    /// All pipeline migration metrics.
    pub fn pipeline_metrics(&self) -> Vec<(String, TimerStats)> {
        self.stats_by_prefix("pipeline_migration")
    }

    // ------------------------------------------------------------------
    // Report
    // ------------------------------------------------------------------

    /// Generate a complete performance report as a formatted string.
    ///
    /// Includes all subsystem metrics with their current statistics.
    /// The report is local-only and never leaves the machine.
    pub fn report(&self) -> String {
        let mut lines = Vec::new();
        lines.push("══════════════════════════════════════════════".to_string());
        lines.push("  Nabu Performance Report (local-only)".to_string());
        lines.push("══════════════════════════════════════════════".to_string());

        let categories: Vec<(&str, Vec<(String, TimerStats)>)> = vec![
            ("Capture", self.capture_metrics()),
            ("Queue", self.queue_metrics()),
            ("Worker", self.worker_metrics()),
            ("Processing", self.processing_metrics()),
            ("Storage", self.storage_metrics()),
            ("Indexer", self.indexer_metrics()),
            ("Graph", self.graph_metrics()),
            ("Event Bus", self.event_bus_metrics()),
            ("Pipeline Migration", self.pipeline_metrics()),
        ];

        for (name, metrics) in &categories {
            if metrics.is_empty() {
                continue;
            }
            lines.push(format!("\n── {} ──", name));
            for (key, stats) in metrics {
                if stats.count > 0 {
                    lines.push(format!("  {}  {}", key, stats));
                }
            }
        }

        // Add counters
        if let Ok(counters) = self.counters.read() {
            if !counters.is_empty() {
                lines.push("\n── Counters ──".to_string());
                let mut sorted: Vec<_> = counters.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (key, counter) in &sorted {
                    lines.push(format!("  {} = {}", key, counter.value()));
                }
            }
        }

        // Add gauges
        if let Ok(gauges) = self.gauges.read() {
            if !gauges.is_empty() {
                lines.push("\n── Gauges ──".to_string());
                let mut sorted: Vec<_> = gauges.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (key, gauge) in &sorted {
                    lines.push(format!("  {} = {}", key, gauge.value()));
                }
            }
        }

        lines.push("\n══════════════════════════════════════════════".to_string());
        lines.join("\n")
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        if let Ok(mut timers) = self.timers.write() {
            timers.clear();
        }
        if let Ok(mut counters) = self.counters.write() {
            counters.clear();
        }
        if let Ok(mut gauges) = self.gauges.write() {
            gauges.clear();
        }
    }

    /// Total number of active metric timers.
    pub fn timer_count(&self) -> usize {
        self.timers.read().map(|t| t.len()).unwrap_or(0)
    }

    /// Total number of counters.
    pub fn counter_count(&self) -> usize {
        self.counters.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Total number of gauges.
    pub fn gauge_count(&self) -> usize {
        self.gauges.read().map(|g| g.len()).unwrap_or(0)
    }

    /// All metric keys currently tracked.
    pub fn metric_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        if let Ok(timers) = self.timers.read() {
            keys.extend(timers.keys().cloned());
        }
        if let Ok(counters) = self.counters.read() {
            keys.extend(counters.keys().cloned());
        }
        if let Ok(gauges) = self.gauges.read() {
            keys.extend(gauges.keys().cloned());
        }
        keys.sort();
        keys.dedup();
        keys
    }

    /// Capture a serializable snapshot of all current metrics.
    ///
    /// Returns a [`PerformanceSnapshot`] containing timer stats, counter
    /// values, and gauge values. Timers are sorted alphabetically by key
    /// for deterministic output.
    pub fn snapshot(&self) -> PerformanceSnapshot {
        let timers = if let Ok(timers) = self.timers.read() {
            let mut entries: Vec<_> = timers
                .iter()
                .map(|(k, t)| (k.clone(), t.stats()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            entries
                .into_iter()
                .map(|(k, s)| crate::diagnostics::metrics::TimerSnapshot { key: k, stats: s })
                .collect()
        } else {
            Vec::new()
        };

        let counters = if let Ok(counters) = self.counters.read() {
            let mut entries: Vec<_> = counters
                .iter()
                .map(|(k, c)| (k.clone(), c.value()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            entries
                .into_iter()
                .map(|(k, v)| crate::diagnostics::metrics::CounterSnapshot { key: k, value: v })
                .collect()
        } else {
            Vec::new()
        };

        let gauges = if let Ok(gauges) = self.gauges.read() {
            let mut entries: Vec<_> = gauges
                .iter()
                .map(|(k, g)| (k.clone(), g.value()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            entries
                .into_iter()
                .map(|(k, v)| crate::diagnostics::metrics::GaugeSnapshot { key: k, value: v })
                .collect()
        } else {
            Vec::new()
        };

        PerformanceSnapshot {
            timers,
            counters,
            gauges,
        }
    }
}

impl MetricsAggregator for PerformanceMonitor {
    fn metrics(&self) -> ServiceMetrics {
        let snapshot = self.snapshot();

        let timers = snapshot
            .timers
            .into_iter()
            .map(|t| {
                let stats = t.stats;
                TimerMetric {
                    key: t.key,
                    count: stats.count,
                    window_count: stats.window_count,
                    min_ms: stats.min_ms,
                    max_ms: stats.max_ms,
                    avg_ms: stats.avg_ms,
                    p50_ms: stats.p50_ms,
                    p90_ms: stats.p90_ms,
                    p99_ms: stats.p99_ms,
                    sum_ms: stats.sum_ms,
                }
            })
            .collect();

        let counters = snapshot
            .counters
            .into_iter()
            .map(|c| CounterMetric {
                key: c.key,
                value: c.value,
            })
            .collect();

        let gauges = snapshot
            .gauges
            .into_iter()
            .map(|g| GaugeMetric {
                key: g.key,
                value: g.value,
            })
            .collect();

        ServiceMetrics {
            service: "performance_monitor".to_string(),
            timers,
            counters,
            gauges,
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_query() {
        let m = PerformanceMonitor::new();
        m.record("test.op", 42.0);
        let stats = m.stats("test.op");
        assert_eq!(stats.count, 1);
        assert!((stats.avg_ms - 42.0).abs() < 0.01);
    }

    #[test]
    fn test_subsystem_helpers() {
        let m = PerformanceMonitor::new();
        m.record_capture("ingest", 10.0);
        m.record_queue("enqueue", 5.0);
        m.record_processor("ocr", 200.0);
        m.record_storage("save", 15.0);
        m.record_indexer("index", 8.0);
        m.record_graph("add_node", 2.0);

        assert_eq!(m.capture_stats("ingest").count, 1);
        assert_eq!(m.queue_stats("enqueue").count, 1);
        assert_eq!(m.processor_stats("ocr").count, 1);
        assert_eq!(m.storage_stats("save").count, 1);
        assert_eq!(m.indexer_stats("index").count, 1);
        assert_eq!(m.graph_stats("add_node").count, 1);
    }

    #[test]
    fn test_counter_and_gauge() {
        let m = PerformanceMonitor::new();
        m.increment("test.count", 5);
        assert_eq!(m.counter("test.count"), 5);

        m.set_gauge("test.gauge", 10);
        assert_eq!(m.gauge("test.gauge"), 10);
        m.increment_gauge("test.gauge", 1);
        assert_eq!(m.gauge("test.gauge"), 11);
    }

    #[test]
    fn test_bulk_queries() {
        let m = PerformanceMonitor::new();
        m.record_capture("ingest", 1.0);
        m.record_capture("route", 2.0);
        m.record_queue("enqueue", 3.0);

        let capture_metrics = m.capture_metrics();
        assert_eq!(capture_metrics.len(), 2);

        let queue_metrics = m.queue_metrics();
        assert_eq!(queue_metrics.len(), 1);
    }

    #[test]
    fn test_report_format() {
        let m = PerformanceMonitor::new();
        m.record_capture("ingest", 42.0);
        let report = m.report();
        assert!(report.contains("Capture"));
        assert!(report.contains("ingest"));
        assert!(report.contains("local-only"));
    }

    #[test]
    fn test_reset() {
        let m = PerformanceMonitor::new();
        m.record("test", 1.0);
        assert!(m.timer_count() > 0);
        m.reset();
        assert_eq!(m.timer_count(), 0);
    }

    #[test]
    fn test_metric_keys() {
        let m = PerformanceMonitor::new();
        m.record("a", 1.0);
        m.increment("b", 1);
        let keys = m.metric_keys();
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
    }

    #[test]
    fn test_stats_by_prefix() {
        let m = PerformanceMonitor::new();
        m.record("capture.ingest", 1.0);
        m.record("capture.route", 2.0);
        m.record("queue.depth", 3.0);

        let capture = m.stats_by_prefix("capture");
        assert_eq!(capture.len(), 2);

        let queue = m.stats_by_prefix("queue");
        assert_eq!(queue.len(), 1);
    }
}
