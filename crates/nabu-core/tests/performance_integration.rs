//! Integration tests for Nabu's performance instrumentation system.
//!
//! Tests cover:
//! - PerformanceMonitor recording and querying
//! - Global monitor singleton
//! - Per-subsystem metric helpers
//! - Report generation
//! - Timer/Counter/Gauge/Histogram types
//! - Sliding window eviction
//! - Percentile computation

use nabu_core::diagnostics::metrics::*;
use nabu_core::diagnostics::performance::*;
use std::time::Duration;

// ---------------------------------------------------------------------------
// PerformanceMonitor integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_monitor_create_and_record() {
    let m = PerformanceMonitor::new();
    m.record("test.op", 42.5);
    let stats = m.stats("test.op");
    assert_eq!(stats.count, 1);
    assert!((stats.avg_ms - 42.5).abs() < 0.01);
}

#[test]
fn test_monitor_multiple_recordings() {
    let m = PerformanceMonitor::new();
    m.record("test.op", 10.0);
    m.record("test.op", 20.0);
    m.record("test.op", 30.0);

    let stats = m.stats("test.op");
    assert_eq!(stats.count, 3);
    assert_eq!(stats.window_count, 3);
    assert!((stats.min_ms - 10.0).abs() < 0.01);
    assert!((stats.max_ms - 30.0).abs() < 0.01);
    assert!((stats.avg_ms - 20.0).abs() < 0.01);
}

#[test]
fn test_capture_metrics() {
    let m = PerformanceMonitor::new();
    m.record_capture("ingest", 15.0);
    m.record_capture("route", 5.0);
    m.record_capture("enqueue", 3.0);

    assert_eq!(m.capture_stats("ingest").count, 1);
    assert_eq!(m.capture_stats("route").count, 1);
    assert_eq!(m.capture_stats("enqueue").count, 1);

    let all = m.capture_metrics();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_queue_metrics() {
    let m = PerformanceMonitor::new();
    m.record_queue("enqueue", 2.0);
    m.record_queue("dequeue", 1.0);
    m.record_queue("latency", 150.0);

    assert_eq!(m.queue_stats("enqueue").count, 1);
    assert_eq!(m.queue_stats("dequeue").count, 1);
    assert_eq!(m.queue_stats("latency").count, 1);

    let all = m.queue_metrics();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_worker_metrics() {
    let m = PerformanceMonitor::new();
    m.record_worker("execute", 500.0);
    m.record_worker("pickup", 0.5);

    assert_eq!(m.worker_stats("execute").count, 1);
    assert_eq!(m.worker_stats("pickup").count, 1);
}

#[test]
fn test_processor_metrics() {
    let m = PerformanceMonitor::new();
    m.record_processor("ocr", 1200.0);
    m.record_processor("whisper", 3000.0);
    m.record_processor("embedding", 800.0);

    assert_eq!(m.processor_stats("ocr").count, 1);
    assert_eq!(m.processor_stats("whisper").count, 1);
    assert_eq!(m.processor_stats("embedding").count, 1);

    let all = m.processing_metrics();
    assert!(all.len() >= 3);
}

#[test]
fn test_storage_metrics() {
    let m = PerformanceMonitor::new();
    m.record_storage("save", 20.0);
    m.record_storage("load", 5.0);

    assert_eq!(m.storage_stats("save").count, 1);
    assert_eq!(m.storage_stats("load").count, 1);
}

#[test]
fn test_indexer_metrics() {
    let m = PerformanceMonitor::new();
    m.record_indexer("index", 10.0);
    m.record_indexer("search", 2.0);

    assert_eq!(m.indexer_stats("index").count, 1);
    assert_eq!(m.indexer_stats("search").count, 1);
}

#[test]
fn test_graph_metrics() {
    let m = PerformanceMonitor::new();
    m.record_graph("add_node", 1.0);
    m.record_graph("add_edge", 0.5);
    m.record_graph("persist", 50.0);

    assert_eq!(m.graph_stats("add_node").count, 1);
    assert_eq!(m.graph_stats("add_edge").count, 1);
    assert_eq!(m.graph_stats("persist").count, 1);
}

#[test]
fn test_counters() {
    let m = PerformanceMonitor::new();
    m.increment("capture.count", 10);
    m.increment("error.count", 1);

    assert_eq!(m.counter("capture.count"), 10);
    assert_eq!(m.counter("error.count"), 1);
    assert_eq!(m.counter("nonexistent"), 0);
}

#[test]
fn test_gauges() {
    let m = PerformanceMonitor::new();
    m.set_gauge("worker.active", 4);
    m.set_gauge("queue.depth", 15);

    assert_eq!(m.gauge("worker.active"), 4);
    assert_eq!(m.gauge("queue.depth"), 15);

    m.increment_gauge("queue.depth", 1);
    assert_eq!(m.gauge("queue.depth"), 16);

    m.set_gauge("queue.depth", 0);
    assert_eq!(m.gauge("queue.depth"), 0);
}

#[test]
fn test_report_contains_all_subsystems() {
    let m = PerformanceMonitor::new();
    m.record_capture("ingest", 10.0);
    m.record_queue("enqueue", 5.0);
    m.record_processor("ocr", 200.0);
    m.record_storage("save", 15.0);
    m.record_indexer("index", 8.0);
    m.record_graph("add_node", 2.0);

    let report = m.report();
    assert!(report.contains("Capture"));
    assert!(report.contains("Queue"));
    assert!(report.contains("Processing"));
    assert!(report.contains("Storage"));
    assert!(report.contains("Indexer"));
    assert!(report.contains("Graph"));
    assert!(report.contains("local-only"));
}

#[test]
fn test_report_with_counters_and_gauges() {
    let m = PerformanceMonitor::new();
    m.record("test.op", 1.0);
    m.increment("test.count", 5);
    m.set_gauge("test.gauge", 10);

    let report = m.report();
    assert!(report.contains("counters"));
    assert!(report.contains("gauges"));
}

#[test]
fn test_empty_monitor_report() {
    let m = PerformanceMonitor::new();
    let report = m.report();
    assert!(report.contains("Nabu Performance Report"));
    assert!(report.contains("local-only"));
}

#[test]
fn test_stats_by_prefix_empty() {
    let m = PerformanceMonitor::new();
    let empty = m.stats_by_prefix("nonexistent");
    assert!(empty.is_empty());
}

// ---------------------------------------------------------------------------
// Metric type integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_timer_with_duration() {
    let t = Timer::new();
    t.record(Duration::from_millis(100));
    t.record(Duration::from_millis(200));
    let s = t.stats();
    assert_eq!(s.count, 2);
    assert!(s.min_ms >= 99.0 && s.min_ms <= 101.0);
    assert!(s.max_ms >= 199.0 && s.max_ms <= 201.0);
}

#[test]
fn test_timer_window_eviction_ring_buffer() {
    let t = Timer::with_capacity(3);
    t.record_ms(1.0);
    t.record_ms(2.0);
    t.record_ms(3.0);
    t.record_ms(4.0); // evicts 1.0
    t.record_ms(5.0); // evicts 2.0

    let s = t.stats();
    assert_eq!(s.count, 5);
    assert_eq!(s.window_count, 3);
    assert!((s.min_ms - 3.0).abs() < 0.01); // window has [3, 4, 5]
    assert!((s.max_ms - 5.0).abs() < 0.01);
}

#[test]
fn test_timer_reset_and_reuse() {
    let t = Timer::new();
    t.record_ms(10.0);
    assert_eq!(t.total_count(), 1);
    t.reset();
    assert_eq!(t.total_count(), 0);
    assert_eq!(t.window_count(), 0);

    t.record_ms(20.0);
    assert_eq!(t.total_count(), 1);
}

#[test]
fn test_counter_thread_safety() {
    let c = Counter::new();
    c.increment();
    c.add(10);
    assert_eq!(c.value(), 11);
}

#[test]
fn test_gauge_negative_values() {
    let g = Gauge::new();
    g.set(5);
    g.decrement();
    g.decrement();
    g.decrement();
    assert_eq!(g.value(), 2);
    g.add(-5);
    assert_eq!(g.value(), -3);
}

#[test]
fn test_histogram_basic() {
    let h = Histogram::default_latency();
    assert_eq!(h.total(), 0);

    h.record(5.0);
    h.record(50.0);
    h.record(500.0);

    assert_eq!(h.total(), 3);
    assert!((h.sum() - 555.0).abs() < 0.01);

    let buckets = h.buckets();
    assert!(!buckets.is_empty());
    assert_eq!(buckets.iter().map(|b| b.count).sum::<u64>(), 3);
}

#[test]
fn test_histogram_reset() {
    let h = Histogram::default_latency();
    h.record(100.0);
    assert_eq!(h.total(), 1);
    h.reset();
    assert_eq!(h.total(), 0);
}

#[test]
fn test_timing_scope_records_on_drop() {
    let timer = Timer::new();
    {
        let _scope = TimingScope::new(&timer);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(timer.stats().count >= 1);
}

#[test]
fn test_timing_scope_early_finish() {
    let timer = Timer::new();
    {
        let scope = TimingScope::new(&timer);
        std::thread::sleep(Duration::from_millis(1));
        scope.finish();
    } // No double-record
    assert_eq!(timer.stats().count, 1);
}

// ---------------------------------------------------------------------------
// Global monitor tests
// ---------------------------------------------------------------------------

#[test]
fn test_global_monitor_singleton() {
    let m1 = global_monitor();
    let m2 = global_monitor();

    m1.record("global.test", 42.0);
    assert_eq!(m2.stats("global.test").count, 1);

    // Same object
    assert!(Arc::ptr_eq(&m1, &m2));
}

#[test]
fn test_monitor_metric_keys_integration() {
    let m = PerformanceMonitor::new();
    m.record("a", 1.0);
    m.record("b", 2.0);
    m.increment("c", 1);
    m.set_gauge("d", 1);

    let keys = m.metric_keys();
    assert_eq!(keys.len(), 4);
    assert!(keys.contains(&"a".to_string()));
    assert!(keys.contains(&"b".to_string()));
    assert!(keys.contains(&"c".to_string()));
    assert!(keys.contains(&"d".to_string()));
}
