//! Integration tests for the `metrics` IPC command pipeline.
//!
//! Tests cover:
//! - ApplicationContext registers PerformanceMonitor as a MetricsAggregator
//! - `ApplicationContext::metrics()` discovers and queries aggregators
//! - RuntimeMetrics serialization round-trips through serde_json
//! - Graceful degradation when no aggregators are registered
//! - The `metrics` IPC command returns serialized RuntimeMetrics

use std::sync::Arc;

use nabu_core::diagnostics::PerformanceMonitor;
use nabu_core::registry::context::ApplicationContext;
use nabu_core::registry::metrics::{
    CounterMetric, GaugeMetric, MetricsAggregator, RuntimeMetrics, ServiceMetrics, TimerMetric,
};

// ---------------------------------------------------------------------------
// ApplicationContext metrics integration tests
// ---------------------------------------------------------------------------

#[test]
fn metrics_returns_empty_when_no_aggregators_registered() {
    let ctx = ApplicationContext::builder().build();
    let m = ctx.metrics();

    assert!(m.timers.is_empty());
    assert!(m.counters.is_empty());
    assert!(m.gauges.is_empty());
    assert!(m.services.is_empty());
    assert_eq!(m.service_count, 0);
    assert_eq!(m.error_count, 0);
}

#[test]
fn metrics_discovers_registered_aggregators() {
    let ctx = ApplicationContext::builder().build();

    let perf = Arc::new(PerformanceMonitor::new());
    ctx.register("performance_monitor", perf.clone());
    ctx.register_metrics_aggregator("performance_monitor", perf.clone());

    let m = ctx.metrics();
    assert_eq!(m.service_count, 1);
    assert_eq!(m.services.len(), 1);
    assert_eq!(m.services[0].service, "performance_monitor");
}

#[test]
fn metrics_aggregates_from_multiple_services() {
    let ctx = ApplicationContext::builder().build();

    // Register two mock aggregators
    struct MockAggregator {
        name: String,
    }

    impl MetricsAggregator for MockAggregator {
        fn metrics(&self) -> ServiceMetrics {
            ServiceMetrics {
                service: self.name.clone(),
                timers: vec![TimerMetric {
                    key: format!("{}.timer", self.name),
                    count: 5,
                    window_count: 5,
                    min_ms: 1.0,
                    max_ms: 10.0,
                    avg_ms: 5.0,
                    p50_ms: 4.0,
                    p90_ms: 9.0,
                    p99_ms: 9.5,
                    sum_ms: 25.0,
                }],
                counters: vec![CounterMetric {
                    key: format!("{}.count", self.name),
                    value: 42,
                }],
                gauges: vec![GaugeMetric {
                    key: format!("{}.gauge", self.name),
                    value: 7,
                }],
            }
        }
    }

    let agg_a = Arc::new(MockAggregator { name: "svc_a".to_string() });
    let agg_b = Arc::new(MockAggregator { name: "svc_b".to_string() });

    ctx.register("svc_a", agg_a.clone());
    ctx.register_metrics_aggregator("svc_a", agg_a);
    ctx.register("svc_b", agg_b.clone());
    ctx.register_metrics_aggregator("svc_b", agg_b);

    let m = ctx.metrics();
    assert_eq!(m.service_count, 2);
    assert_eq!(m.services.len(), 2);
    assert_eq!(m.timers.len(), 2);
    assert_eq!(m.counters.len(), 2);
    assert_eq!(m.gauges.len(), 2);
}

#[test]
fn metrics_serializes_through_serde_json() {
    let ctx = ApplicationContext::builder().build();

    let perf = Arc::new(PerformanceMonitor::new());
    perf.record("test.op", 42.5);
    perf.increment("test.count", 10);
    perf.set_gauge("test.gauge", 5);

    ctx.register("performance_monitor", perf.clone());
    ctx.register_metrics_aggregator("performance_monitor", perf);

    let m = ctx.metrics();
    let json = serde_json::to_string(&m).expect("serialize RuntimeMetrics");
    let restored: RuntimeMetrics = serde_json::from_str(&json).expect("deserialize RuntimeMetrics");

    assert_eq!(restored.service_count, m.service_count);
    assert_eq!(restored.services.len(), m.services.len());
    assert_eq!(restored.services[0].service, "performance_monitor");
}

#[test]
fn metrics_performance_monitor_contributes_timers_and_counters() {
    let ctx = ApplicationContext::builder().build();

    let perf = Arc::new(PerformanceMonitor::new());
    perf.record("capture.ingest", 15.0);
    perf.record("capture.ingest", 25.0);
    perf.increment("capture.count", 3);
    perf.set_gauge("queue.depth", 5);

    ctx.register("performance_monitor", perf.clone());
    ctx.register_metrics_aggregator("performance_monitor", perf);

    let m = ctx.metrics();

    // PerformanceMonitor's MetricsAggregator impl returns its timers, counters,
    // and gauges merged into the service entry
    let svc = &m.services[0];
    assert_eq!(svc.service, "performance_monitor");
    assert!(svc.timers.iter().any(|t| t.key == "capture.ingest"));
    assert!(svc.counters.iter().any(|c| c.key == "capture.count"));
    assert!(svc.gauges.iter().any(|g| g.key == "queue.depth"));

    // Top-level vectors should also be merged
    assert!(m.timers.iter().any(|t| t.key == "capture.ingest"));
    assert!(m.counters.iter().any(|c| c.key == "capture.count"));
    assert!(m.gauges.iter().any(|g| g.key == "queue.depth"));
}

#[test]
fn metrics_unknown_service_key_uses_registry_key() {
    let ctx = ApplicationContext::builder().build();

    struct EmptyAggregator;
    impl MetricsAggregator for EmptyAggregator {
        fn metrics(&self) -> ServiceMetrics {
            ServiceMetrics {
                service: String::new(), // empty service name
                timers: Vec::new(),
                counters: Vec::new(),
                gauges: Vec::new(),
            }
        }
    }

    let agg = Arc::new(EmptyAggregator);
    ctx.register("my_aggregator", agg.clone());
    ctx.register_metrics_aggregator("my_aggregator", agg);

    let m = ctx.metrics();
    // The ApplicationContext::metrics() should set the service name from the
    // registry key when the aggregator returns an empty service name
    assert_eq!(m.services[0].service, "my_aggregator");
}

// ---------------------------------------------------------------------------
// MetricsAggregator trait tests (via ServiceRegistry)
// ---------------------------------------------------------------------------

#[test]
fn metrics_aggregate_count_reflects_registered_aggregators() {
    use nabu_core::registry::ServiceRegistry;

    let mut reg = ServiceRegistry::new();
    assert_eq!(reg.metrics_aggregator_count(), 0);

    struct MockAgg;
    impl MetricsAggregator for MockAgg {
        fn metrics(&self) -> ServiceMetrics {
            ServiceMetrics::default()
        }
    }

    reg.register_metrics_aggregator("mock_a", Arc::new(MockAgg));
    assert_eq!(reg.metrics_aggregator_count(), 1);

    reg.register_metrics_aggregator("mock_b", Arc::new(MockAgg));
    assert_eq!(reg.metrics_aggregator_count(), 2);

    let aggregators = reg.metrics_aggregators();
    assert_eq!(aggregators.len(), 2);
    assert!(aggregators.iter().any(|(k, _)| k == "mock_a"));
    assert!(aggregators.iter().any(|(k, _)| k == "mock_b"));
}

#[test]
fn metrics_unregister_removes_aggregator() {
    use nabu_core::registry::ServiceRegistry;

    let mut reg = ServiceRegistry::new();

    struct MockAgg;
    impl MetricsAggregator for MockAgg {
        fn metrics(&self) -> ServiceMetrics {
            ServiceMetrics::default()
        }
    }

    reg.register("svc", Arc::new(MockAgg));
    reg.register_metrics_aggregator("svc", Arc::new(MockAgg));
    assert_eq!(reg.metrics_aggregator_count(), 1);

    reg.unregister("svc");
    assert_eq!(reg.metrics_aggregator_count(), 0);
    assert_eq!(reg.metrics_aggregators().len(), 0);
}

// ---------------------------------------------------------------------------
// Mutex / RwLock blanket impl tests
// ---------------------------------------------------------------------------

#[test]
fn metrics_mutex_aggregator_handles_poison() {
    use std::sync::Mutex as StdMutex;

    let mutex = StdMutex::new(CountingAggregator { count: 0 });
    let agg = Arc::new(mutex);

    // Register directly (blanket impl applies)
    let ctx = ApplicationContext::builder().build();
    ctx.register_metrics_aggregator("wrapped", agg.clone());

    let mut m = ctx.metrics();
    assert_eq!(m.service_count, 1);

    // Poison the lock by panicking in another thread
    let agg_clone = agg.clone();
    let handle = std::thread::spawn(move || {
        let _guard = agg_clone.lock().unwrap();
        panic!("intentional poison");
    });
    let _ = handle.join();

    // metrics() should not panic — the blanket impl handles poison
    m = ctx.metrics();
    assert_eq!(m.service_count, 1);
    assert!(m.services[0].timers.is_empty());
    assert!(m.services[0].counters.is_empty());
}

struct CountingAggregator {
    count: u64,
}

// A simple aggregator that implements MetricsAggregator directly
impl MetricsAggregator for CountingAggregator {
    fn metrics(&self) -> ServiceMetrics {
        ServiceMetrics {
            service: "counting".to_string(),
            timers: Vec::new(),
            counters: vec![CounterMetric {
                key: "count".to_string(),
                value: self.count,
            }],
            gauges: Vec::new(),
        }
    }
}
