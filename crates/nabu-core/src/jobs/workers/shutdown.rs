use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

/// Coordinates graceful shutdown of the worker pool.
///
/// Shutdown proceeds in phases:
/// 1. **Signal**: All workers receive `Shutdown` message.
/// 2. **Drain**: Wait for active jobs to complete (up to `drain_timeout`).
/// 3. **Force**: If jobs remain after timeout, log warning and proceed.
/// 4. **Done**: All workers confirmed stopped.
#[derive(Debug, Clone)]
pub struct ShutdownCoordinator {
    /// Whether shutdown has been initiated.
    initiated: Arc<AtomicBool>,

    /// Whether shutdown has completed.
    completed: Arc<AtomicBool>,

    /// Number of active (in-progress) jobs.
    active_jobs: Arc<AtomicUsize>,

    /// Timeout for waiting for active jobs to complete (seconds).
    drain_timeout_secs: u64,

    /// Channel to notify completion of shutdown.
    notify: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

impl ShutdownCoordinator {
    /// Creates a new shutdown coordinator.
    pub fn new(drain_timeout_secs: u64) -> Self {
        ShutdownCoordinator {
            initiated: Arc::new(AtomicBool::new(false)),
            completed: Arc::new(AtomicBool::new(false)),
            active_jobs: Arc::new(AtomicUsize::new(0)),
            drain_timeout_secs,
            notify: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Returns the drain timeout.
    pub fn drain_timeout(&self) -> Duration {
        Duration::from_secs(self.drain_timeout_secs)
    }

    /// Returns `true` if shutdown has been initiated.
    pub fn is_shutdown_initiated(&self) -> bool {
        self.initiated.load(Ordering::Relaxed)
    }

    /// Returns `true` if shutdown has completed.
    pub fn is_shutdown_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    /// Returns the number of currently active jobs.
    pub fn active_jobs(&self) -> usize {
        self.active_jobs.load(Ordering::Relaxed)
    }

    /// Records that a job has started.
    pub fn job_started(&self) {
        self.active_jobs.fetch_add(1, Ordering::SeqCst);
    }

    /// Records that a job has finished.
    pub fn job_finished(&self) {
        self.active_jobs.fetch_sub(1, Ordering::SeqCst);
    }

    /// Returns `true` if there are no active jobs.
    pub fn is_drained(&self) -> bool {
        self.active_jobs.load(Ordering::Relaxed) == 0
    }

    /// Initiates shutdown. Returns after all workers have stopped or the timeout is reached.
    pub async fn initiate_shutdown(&self) {
        self.initiated.store(true, Ordering::Release);

        // Wait for active jobs to drain
        let timeout = self.drain_timeout();
        let deadline = tokio::time::Instant::now() + timeout;

        while tokio::time::Instant::now() < deadline {
            if self.is_drained() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !self.is_drained() {
            log::warn!(
                "Shutdown timeout: {} jobs still active after {}s, forcing shutdown",
                self.active_jobs(),
                self.drain_timeout_secs
            );
        }

        self.completed.store(true, Ordering::Release);

        // Notify any waiters
        let mut notify = self.notify.lock().await;
        if let Some(sender) = notify.take() {
            let _ = sender.send(());
        }
    }

    /// Sets up a notification channel for shutdown completion.
    pub async fn on_shutdown_complete(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let mut notify = self.notify.lock().await;
        *notify = Some(tx);
        rx
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new(30) // 30 second default drain timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_coordinator_initial_state() {
        let sc = ShutdownCoordinator::new(5);
        assert!(!sc.is_shutdown_initiated());
        assert!(!sc.is_shutdown_completed());
        assert_eq!(sc.active_jobs(), 0);
        assert!(sc.is_drained());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_job_tracking() {
        let sc = ShutdownCoordinator::new(5);
        sc.job_started();
        assert_eq!(sc.active_jobs(), 1);
        assert!(!sc.is_drained());

        sc.job_finished();
        assert_eq!(sc.active_jobs(), 0);
        assert!(sc.is_drained());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_drain() {
        let sc = ShutdownCoordinator::new(1);
        sc.job_started();

        // Start shutdown in background
        let sc_clone = sc.clone();
        let handle = tokio::spawn(async move {
            sc_clone.initiate_shutdown().await;
        });

        // Let the shutdown start and wait a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Complete the active job
        sc.job_finished();

        // Shutdown should complete now
        handle.await.unwrap();
        assert!(sc.is_shutdown_completed());
    }
}
