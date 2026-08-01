
use crate::jobs::queue::Queue;
use crate::jobs::workers::backpressure::BackpressureController;
use crate::jobs::workers::executor::ExecutorRegistry;
use crate::jobs::workers::shutdown::ShutdownCoordinator;
use crate::jobs::workers::worker::Worker;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// A configurable worker pool that manages the lifecycle of workers.
///
/// The pool is the primary entry point for executing jobs.
/// It manages worker creation, health monitoring, and graceful shutdown.
pub struct WorkerPool {
    /// Number of workers in the pool
    worker_count: usize,
    /// The queue workers pull jobs from
    queue: Arc<dyn Queue>,
    /// Registered executors
    executors: Arc<ExecutorRegistry>,
    /// Shutdown coordinator
    shutdown: Arc<ShutdownCoordinator>,
    /// Backpressure controller
    backpressure: Arc<BackpressureController>,
    /// Join handles for running worker tasks
    handles: tokio::sync::Mutex<Vec<JoinHandle<()>>>,
}

impl WorkerPool {
    /// Create a new worker pool.
    ///
    /// # Arguments
    /// * `worker_count` - Number of worker tasks to spawn
    /// * `queue` - The job queue workers will pull from
    /// * `executors` - Registry of job executors
    pub fn new(worker_count: usize, queue: Arc<dyn Queue>, executors: Arc<ExecutorRegistry>) -> Self {
        Self {
            worker_count,
            queue,
            executors,
            shutdown: Arc::new(ShutdownCoordinator::new(Duration::from_secs(30))),
            backpressure: Arc::new(BackpressureController::default_limits()),
            handles: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Start the worker pool, spawning all workers.
    pub async fn start(&self) {
        let span = tracing::info_span!(
            "nabu",
            subsystem = "worker",
            component = "pool",
            operation = "start",
            worker_count = self.worker_count,
        );
        let _guard = span.enter();

        tracing::info!("Starting worker pool");

        let mut handles = self.handles.lock().await;

        for i in 0..self.worker_count {
            let worker = Worker::new(
                i,
                self.queue.clone(),
                self.executors.clone(),
                self.shutdown.handle(),
                self.backpressure.handle(),
            );

            let handle = tokio::spawn(async move {
                worker.run().await;
            });

            handles.push(handle);
        }

        tracing::info!(
            subsystem = "worker",
            component = "pool",
            operation = "start",
            workers_spawned = handles.len(),
            "Worker pool started"
        );
    }

    /// Initiate graceful shutdown of the worker pool.
    ///
    /// This will:
    /// 1. Signal all workers to stop
    /// 2. Wait for active workers to finish their current job
    /// 3. Force-kill remaining workers after drain timeout
    pub async fn shutdown(&self) {
        tracing::info!(
            subsystem = "worker",
            component = "pool",
            operation = "shutdown",
            "Shutting down worker pool"
        );
        self.shutdown.initiate();

        // Wait for workers to drain
        let drained = self.shutdown.drain().await;
        if !drained {
            tracing::warn!(
                subsystem = "worker",
                component = "pool",
                operation = "shutdown",
                "Worker pool drain timeout — force killing remaining workers"
            );
        }

        let mut handles = self.handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }

        tracing::info!(
            subsystem = "worker",
            component = "pool",
            operation = "shutdown",
            "Worker pool shutdown complete"
        );
    }

    /// Number of workers in the pool.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Whether the pool is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.is_shutting_down()
    }

    /// Backpressure controller reference.
    pub fn backpressure(&self) -> &BackpressureController {
        &self.backpressure
    }

    /// Shutdown coordinator reference.
    pub fn shutdown_coordinator(&self) -> &ShutdownCoordinator {
        &self.shutdown
    }

    /// Get a snapshot of pool health.
    pub fn health(&self) -> PoolHealth {
        PoolHealth {
            worker_count: self.worker_count,
            shutting_down: self.shutdown.is_shutting_down(),
            pending_jobs: self.backpressure.pending_count(),
            running_jobs: self.backpressure.running_count(),
            active_workers: self.shutdown.active_worker_count(),
            is_throttled: self.backpressure.is_throttled(),
            is_full: self.backpressure.is_full(),
        }
    }
}

/// Snapshot of pool health metrics.
#[derive(Debug, Clone)]
pub struct PoolHealth {
    pub worker_count: usize,
    pub shutting_down: bool,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub active_workers: usize,
    pub is_throttled: bool,
    pub is_full: bool,
}

impl PoolHealth {
    pub fn all_workers_busy(&self) -> bool {
        self.running_jobs >= self.worker_count
    }
}

/// Convenience function to create and start a fully configured worker pool.
pub async fn create_worker_pool(
    queue: Arc<dyn Queue>,
    executors: ExecutorRegistry,
    worker_count: usize,
) -> WorkerPool {
    let pool = WorkerPool::new(worker_count, queue, Arc::new(executors));
    pool.start().await;
    pool
}
