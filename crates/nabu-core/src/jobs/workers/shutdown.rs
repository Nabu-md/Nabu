use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Coordinates graceful shutdown of the worker pool.
///
/// Shutdown sequence:
/// 1. Signal shutdown (no new jobs accepted)
/// 2. Wait for active workers to finish their current job
/// 3. Force-kill remaining workers after drain timeout
pub struct ShutdownCoordinator {
    /// Whether shutdown has been initiated
    shutting_down: Arc<AtomicBool>,
    /// Number of active workers still running
    active_workers: Arc<AtomicUsize>,
    /// Maximum time to wait for workers to drain
    drain_timeout: Duration,
}

impl ShutdownCoordinator {
    pub fn new(drain_timeout: Duration) -> Self {
        Self {
            shutting_down: Arc::new(AtomicBool::new(false)),
            active_workers: Arc::new(AtomicUsize::new(0)),
            drain_timeout,
        }
    }

    /// Default drain timeout of 30 seconds.
    pub fn default_timeout() -> Self {
        Self::new(Duration::from_secs(30))
    }

    /// Check if shutdown has been requested.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// Initiate graceful shutdown.
    pub fn initiate(&self) {
        self.shutting_down.store(true, Ordering::Release);
    }

    /// Register an active worker (called when a worker starts).
    pub fn register_worker(&self) {
        self.active_workers.fetch_add(1, Ordering::Release);
    }

    /// Unregister a worker (called when a worker stops).
    pub fn unregister_worker(&self) {
        self.active_workers.fetch_sub(1, Ordering::Release);
    }

    /// Number of workers still active.
    pub fn active_worker_count(&self) -> usize {
        self.active_workers.load(Ordering::Acquire)
    }

    /// Whether all workers have finished.
    pub fn all_workers_finished(&self) -> bool {
        self.active_workers.load(Ordering::Acquire) == 0
    }

    /// Wait for all workers to finish, up to the drain timeout.
    /// Returns true if all workers finished, false if timeout.
    pub async fn drain(&self) -> bool {
        let start = std::time::Instant::now();
        while !self.all_workers_finished() {
            if start.elapsed() >= self.drain_timeout {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        true
    }

    /// Get a cloneable handle for workers.
    pub fn handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            shutting_down: self.shutting_down.clone(),
            active_workers: self.active_workers.clone(),
        }
    }

    /// Reset shutdown state (for testing/recovery).
    pub fn reset(&self) {
        self.shutting_down.store(false, Ordering::Release);
        self.active_workers.store(0, Ordering::Release);
    }
}

/// A lightweight, cloneable handle for workers to check shutdown state.
#[derive(Clone)]
pub struct ShutdownHandle {
    shutting_down: Arc<AtomicBool>,
    active_workers: Arc<AtomicUsize>,
}

impl ShutdownHandle {
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    pub fn register(&self) {
        self.active_workers.fetch_add(1, Ordering::Release);
    }

    pub fn unregister(&self) {
        self.active_workers.fetch_sub(1, Ordering::Release);
    }
}
