use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Backpressure controller for the worker pool.
/// Prevents unbounded memory growth and runaway scheduling.
///
/// Uses a simple semaphore-based approach:
/// - A soft limit triggers warnings/throttling
/// - A hard limit blocks new job submissions
pub struct BackpressureController {
    pending_jobs: Arc<AtomicUsize>,
    running_jobs: Arc<AtomicUsize>,
    soft_limit: usize,
    hard_limit: usize,
}

impl BackpressureController {
    /// Create a new backpressure controller with configurable limits.
    ///
    /// # Arguments
    /// * `soft_limit` - When pending exceeds this, start throttling (default: 1000)
    /// * `hard_limit` - When pending exceeds this, reject new jobs (default: 10000)
    pub fn new(soft_limit: usize, hard_limit: usize) -> Self {
        Self {
            pending_jobs: Arc::new(AtomicUsize::new(0)),
            running_jobs: Arc::new(AtomicUsize::new(0)),
            soft_limit,
            hard_limit,
        }
    }

    /// Create a backpressure controller with default limits.
    pub fn default_limits() -> Self {
        Self::new(1000, 10000)
    }

    /// Try to accept a new job. Returns false if the hard limit is reached.
    pub fn try_accept(&self) -> bool {
        let pending = self.pending_jobs.load(Ordering::Acquire);
        if pending >= self.hard_limit {
            return false;
        }
        self.pending_jobs.fetch_add(1, Ordering::Release);
        true
    }

    /// Mark a job as started (moves from pending to running).
    pub fn job_started(&self) {
        self.pending_jobs.fetch_sub(1, Ordering::Release);
        self.running_jobs.fetch_add(1, Ordering::Release);
    }

    /// Mark a job as completed (removes from running).
    pub fn job_completed(&self) {
        self.running_jobs.fetch_sub(1, Ordering::Release);
    }

    /// Number of pending (queued but not started) jobs.
    pub fn pending_count(&self) -> usize {
        self.pending_jobs.load(Ordering::Acquire)
    }

    /// Number of currently running jobs.
    pub fn running_count(&self) -> usize {
        self.running_jobs.load(Ordering::Acquire)
    }

    /// Total active jobs (pending + running).
    pub fn active_count(&self) -> usize {
        self.pending_count() + self.running_count()
    }

    /// Whether the soft limit has been reached (throttle recommended).
    pub fn is_throttled(&self) -> bool {
        self.pending_jobs.load(Ordering::Acquire) >= self.soft_limit
    }

    /// Whether the hard limit has been reached (reject new jobs).
    pub fn is_full(&self) -> bool {
        self.pending_jobs.load(Ordering::Acquire) >= self.hard_limit
    }

    /// Reset all counters (for testing / recovery).
    pub fn reset(&self) {
        self.pending_jobs.store(0, Ordering::Release);
        self.running_jobs.store(0, Ordering::Release);
    }

    /// Configure limits at runtime.
    pub fn set_limits(&mut self, soft_limit: usize, hard_limit: usize) {
        self.soft_limit = soft_limit;
        self.hard_limit = hard_limit;
    }

    /// Get a cloneable handle for worker use.
    pub fn handle(&self) -> BackpressureHandle {
        BackpressureHandle {
            pending_jobs: self.pending_jobs.clone(),
            running_jobs: self.running_jobs.clone(),
        }
    }
}

/// A lightweight, cloneable handle for workers to report backpressure state.
#[derive(Clone)]
pub struct BackpressureHandle {
    pending_jobs: Arc<AtomicUsize>,
    running_jobs: Arc<AtomicUsize>,
}

impl BackpressureHandle {
    pub fn job_started(&self) {
        self.pending_jobs.fetch_sub(1, Ordering::Release);
        self.running_jobs.fetch_add(1, Ordering::Release);
    }

    pub fn job_completed(&self) {
        self.running_jobs.fetch_sub(1, Ordering::Release);
    }
}
