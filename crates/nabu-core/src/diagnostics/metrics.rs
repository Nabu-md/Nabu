//! # Metrics — Lightweight Local Performance Types
//!
//! Reusable metric types for Nabu's local performance instrumentation.
//!
//! ## Types
//!
//! - `Timer` — Duration measurement (min, max, avg, p50, p90, p99, count)
//! - `Counter` — Monotonically increasing count
//! - `Gauge` — Point-in-time value
//! - `Histogram` — Value distribution bucketing
//! - `TimingScope` — RAII stopwatch that records into a Timer
//!
//! All metrics are thread-safe. All metrics are local-only.

use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Default maximum samples in a sliding window.
pub const DEFAULT_WINDOW_SIZE: usize = 1000;

/// Default histogram bucket boundaries in milliseconds.
pub const DEFAULT_HISTOGRAM_BUCKETS_MS: &[f64] = &[
    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

// ---------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------

/// A sliding-window duration timer.
///
/// Records execution durations and computes statistics on demand.
pub struct Timer {
    inner: RwLock<TimerInner>,
}

struct TimerInner {
    samples: Vec<f64>,
    capacity: usize,
    cursor: usize,
    total_count: u64,
    running_sum: f64,
}

impl Timer {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_WINDOW_SIZE)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(TimerInner {
                samples: Vec::with_capacity(capacity),
                capacity,
                cursor: 0,
                total_count: 0,
                running_sum: 0.0,
            }),
        }
    }

    /// Record a duration in milliseconds.
    pub fn record_ms(&self, ms: f64) {
        if let Ok(mut inner) = self.inner.write() {
            inner.total_count += 1;
            inner.running_sum += ms;
            if inner.samples.len() < inner.capacity {
                inner.samples.push(ms);
            } else {
                let cursor = inner.cursor;
                let evicted = inner.samples[cursor];
                inner.running_sum -= evicted;
                inner.samples[cursor] = ms;
                inner.cursor = (inner.cursor + 1) % inner.capacity;
            }
        }
    }

    /// Record a `Duration`.
    pub fn record(&self, duration: Duration) {
        self.record_ms(duration.as_secs_f64() * 1000.0);
    }

    /// Compute statistics from the current window.
    pub fn stats(&self) -> TimerStats {
        if let Ok(inner) = self.inner.read() {
            if inner.samples.is_empty() {
                return TimerStats::default();
            }
            let mut sorted = inner.samples.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = sorted.len();
            let sum: f64 = sorted.iter().sum();
            TimerStats {
                count: inner.total_count,
                window_count: len as u64,
                min_ms: sorted[0],
                max_ms: sorted[len - 1],
                avg_ms: sum / len as f64,
                p50_ms: percentile(&sorted, 50.0),
                p90_ms: percentile(&sorted, 90.0),
                p99_ms: percentile(&sorted, 99.0),
                sum_ms: sum,
            }
        } else {
            TimerStats::default()
        }
    }

    pub fn reset(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.samples.clear();
            inner.cursor = 0;
            inner.total_count = 0;
            inner.running_sum = 0.0;
        }
    }

    pub fn window_count(&self) -> u64 {
        self.inner
            .read()
            .map(|i| i.samples.len() as u64)
            .unwrap_or(0)
    }

    pub fn total_count(&self) -> u64 {
        self.inner.read().map(|i| i.total_count).unwrap_or(0)
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics from a Timer's sliding window.
#[derive(Debug, Clone, Copy)]
pub struct TimerStats {
    pub count: u64,
    pub window_count: u64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p99_ms: f64,
    pub sum_ms: f64,
}

impl Default for TimerStats {
    fn default() -> Self {
        Self {
            count: 0,
            window_count: 0,
            min_ms: 0.0,
            max_ms: 0.0,
            avg_ms: 0.0,
            p50_ms: 0.0,
            p90_ms: 0.0,
            p99_ms: 0.0,
            sum_ms: 0.0,
        }
    }
}

impl std::fmt::Display for TimerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "count={} window={} min={:.1}ms max={:.1}ms avg={:.1}ms p50={:.1}ms p90={:.1}ms p99={:.1}ms",
               self.count, self.window_count, self.min_ms, self.max_ms, self.avg_ms,
               self.p50_ms, self.p90_ms, self.p99_ms)
    }
}

// ---------------------------------------------------------------------------
// Counter
// ---------------------------------------------------------------------------

/// A monotonically increasing counter (atomic).
pub struct Counter {
    value: std::sync::atomic::AtomicU64,
}

impl Counter {
    pub fn new() -> Self {
        Self {
            value: std::sync::atomic::AtomicU64::new(0),
        }
    }
    pub fn increment(&self) -> u64 {
        self.add(1)
    }
    pub fn add(&self, delta: u64) -> u64 {
        self.value
            .fetch_add(delta, std::sync::atomic::Ordering::Relaxed)
            + delta
    }
    pub fn value(&self) -> u64 {
        self.value.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn reset(&self) {
        self.value.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Gauge
// ---------------------------------------------------------------------------

/// A point-in-time value (atomic, signed).
pub struct Gauge {
    value: std::sync::atomic::AtomicI64,
}

impl Gauge {
    pub fn new() -> Self {
        Self {
            value: std::sync::atomic::AtomicI64::new(0),
        }
    }
    pub fn set(&self, val: i64) {
        self.value.store(val, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn increment(&self) -> i64 {
        self.add(1)
    }
    pub fn decrement(&self) -> i64 {
        self.add(-1)
    }
    pub fn add(&self, delta: i64) -> i64 {
        self.value
            .fetch_add(delta, std::sync::atomic::Ordering::Relaxed)
            + delta
    }
    pub fn value(&self) -> i64 {
        self.value.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn reset(&self) {
        self.value.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------

/// A value distribution histogram with configurable bucket boundaries.
pub struct Histogram {
    inner: RwLock<HistogramInner>,
}

struct HistogramInner {
    buckets: Vec<f64>,
    counts: Vec<u64>,
    total: u64,
    sum: f64,
}

impl Histogram {
    pub fn new(buckets: &[f64]) -> Self {
        let mut sorted = buckets.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let bucket_count = sorted.len() + 1;
        Self {
            inner: RwLock::new(HistogramInner {
                buckets: sorted,
                counts: vec![0u64; bucket_count],
                total: 0,
                sum: 0.0,
            }),
        }
    }

    pub fn default_latency() -> Self {
        Self::new(DEFAULT_HISTOGRAM_BUCKETS_MS)
    }

    pub fn record(&self, value: f64) {
        if let Ok(mut inner) = self.inner.write() {
            inner.total += 1;
            inner.sum += value;
            let idx = inner.buckets.partition_point(|&b| b < value);
            inner.counts[idx] += 1;
        }
    }

    pub fn buckets(&self) -> Vec<HistogramBucket> {
        if let Ok(inner) = self.inner.read() {
            inner
                .buckets
                .iter()
                .enumerate()
                .map(|(i, &boundary)| HistogramBucket {
                    le: boundary,
                    gt: if i == 0 { 0.0 } else { inner.buckets[i - 1] },
                    count: inner.counts[i],
                })
                .chain(std::iter::once(HistogramBucket {
                    le: f64::MAX,
                    gt: inner.buckets.last().copied().unwrap_or(f64::MAX),
                    count: inner.counts[inner.buckets.len()],
                }))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn total(&self) -> u64 {
        self.inner.read().map(|i| i.total).unwrap_or(0)
    }
    pub fn sum(&self) -> f64 {
        self.inner.read().map(|i| i.sum).unwrap_or(0.0)
    }
    pub fn reset(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.counts.fill(0);
            inner.total = 0;
            inner.sum = 0.0;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HistogramBucket {
    pub le: f64,
    pub gt: f64,
    pub count: u64,
}

// ---------------------------------------------------------------------------
// TimingScope — RAII stopwatch
// ---------------------------------------------------------------------------

/// Records elapsed time into a Timer when dropped.
pub struct TimingScope<'a> {
    timer: Option<&'a Timer>,
    start: Instant,
}

impl<'a> TimingScope<'a> {
    pub fn new(timer: &'a Timer) -> Self {
        Self {
            timer: Some(timer),
            start: Instant::now(),
        }
    }
    /// Finish early and record the duration.
    pub fn finish(mut self) {
        let elapsed = self.start.elapsed();
        if let Some(timer) = self.timer.take() {
            timer.record(elapsed);
        }
    }
}

impl<'a> Drop for TimingScope<'a> {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.record(self.start.elapsed());
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() <= 1 {
        return sorted.first().copied().unwrap_or(0.0);
    }
    let k = (p / 100.0) * (sorted.len() - 1) as f64;
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        sorted[f]
    } else {
        sorted[f] * (c as f64 - k) + sorted[c] * (k - f as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_record_and_stats() {
        let t = Timer::new();
        t.record_ms(10.0);
        t.record_ms(20.0);
        t.record_ms(30.0);
        let s = t.stats();
        assert_eq!(s.count, 3);
        assert_eq!(s.window_count, 3);
        assert!((s.min_ms - 10.0).abs() < 0.01);
        assert!((s.max_ms - 30.0).abs() < 0.01);
        assert!((s.avg_ms - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_timer_window_eviction() {
        let t = Timer::with_capacity(2);
        t.record_ms(1.0);
        t.record_ms(2.0);
        t.record_ms(3.0);
        let s = t.stats();
        assert_eq!(s.count, 3);
        assert_eq!(s.window_count, 2);
    }

    #[test]
    fn test_counter() {
        let c = Counter::new();
        c.increment();
        c.increment();
        c.add(3);
        assert_eq!(c.value(), 5);
        c.reset();
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn test_gauge() {
        let g = Gauge::new();
        g.set(10);
        assert_eq!(g.value(), 10);
        g.increment();
        assert_eq!(g.value(), 11);
        g.decrement();
        assert_eq!(g.value(), 10);
    }

    #[test]
    fn test_histogram() {
        let h = Histogram::default_latency();
        h.record(5.0);
        h.record(50.0);
        h.record(500.0);
        assert_eq!(h.total(), 3);
    }

    #[test]
    fn test_timing_scope() {
        let timer = Timer::new();
        {
            let _s = TimingScope::new(&timer);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(timer.stats().count, 1);
    }

    #[test]
    fn test_empty_timer_stats() {
        let s = Timer::new().stats();
        assert_eq!(s.count, 0);
    }

    #[test]
    fn test_percentile_edge_cases() {
        assert!((percentile(&[], 50.0) - 0.0).abs() < 0.01);
        assert!((percentile(&[5.0], 50.0) - 5.0).abs() < 0.01);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&data, 50.0) - 3.0).abs() < 0.01);
    }
}
