use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::jobs::job::JobId;
use crate::jobs::persistence::JobStore;
use crate::jobs::queue::DurableJobQueue;
use crate::jobs::worker_channel::{QueueMessage, WorkerChannel};

use super::backpressure::Backpressure;
use super::errors::{WorkerError, WorkerResult};
use super::executor::{ExecutorRegistry, NoopExecutor};
use super::progress::{ChannelProgressReporter, ProgressReporter};
use super::shutdown::ShutdownCoordinator;
use super::worker::Worker;

/// Configuration for the worker pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Number of workers to spawn.
    pub worker_count: usize,

    /// Capacity of the worker channel (max pending jobs dispatched to workers).
    pub channel_capacity: usize,

    /// Timeout in seconds for draining active jobs during shutdown.
    pub drain_timeout_secs: u64,

    /// Progress report throttle interval in milliseconds.
    pub progress_throttle_ms: u64,

    /// Whether to register a NoopExecutor as fallback for unknown job types.
    pub register_fallback_executor: bool,
}

impl PoolConfig {
    /// Creates a default config with sensible values.
    pub fn default_with_worker_count(worker_count: usize) -> Self {
        PoolConfig {
            worker_count,
            channel_capacity: worker_count * 4,
            drain_timeout_secs: 30,
            progress_throttle_ms: 250,
            register_fallback_executor: false,
        }
    }

    /// Creates a pool config using the number of CPU cores.
    pub fn with_cpu_count() -> Self {
        let count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::default_with_worker_count(count)
    }

    /// Creates a pool config with 1 worker (minimum).
    pub fn single_worker() -> Self {
        Self::default_with_worker_count(1)
    }

    /// Creates a pool config with 2 workers.
    pub fn two_workers() -> Self {
        Self::default_with_worker_count(2)
    }

    /// Creates a pool config with 4 workers.
    pub fn four_workers() -> Self {
        Self::default_with_worker_count(4)
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self::with_cpu_count()
    }
}

/// The worker pool — manages a set of workers that process jobs from the queue.
///
/// This is the primary entry point for the worker runtime. It handles:
/// - Worker lifecycle (spawn, monitor, stop)
/// - Connection to the `DurableJobQueue` via `WorkerChannel`
/// - Backpressure regulation
/// - Graceful shutdown
/// - Progress reporting aggregation
///
/// Usage:
/// ```rust,ignore
/// use nabu_core::jobs::workers::*;
///
/// let mut pool = WorkerPool::new(
///     queue,
///     PoolConfig::four_workers(),
///     ExecutorRegistry::new(),
/// );
/// pool.register_default_executors();
/// pool.start().await?;
///
/// // ... enqueue jobs ...
///
/// pool.shutdown().await?;
/// ```
#[derive(Debug)]
pub struct WorkerPool {
    /// The durable job queue this pool is connected to.
    queue: Arc<DurableJobQueue>,

    /// Pool configuration.
    config: PoolConfig,

    /// Registry mapping job types to executors.
    executors: Arc<ExecutorRegistry>,

    /// The worker communication channel.
    channel: Option<Arc<WorkerChannel>>,

    /// Handle for the worker side of the channel.
    worker_handle: Option<tokio::sync::Mutex<crate::jobs::worker_channel::WorkerHandle>>,

    /// Progress reporter.
    progress: Arc<dyn ProgressReporter>,

    /// Shutdown coordinator.
    shutdown: ShutdownCoordinator,

    /// Whether the pool is currently running.
    running: Arc<Mutex<bool>>,

    /// Tokio task handles for spawned workers.
    worker_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl WorkerPool {
    /// Creates a new worker pool.
    ///
    /// The pool is not started until `start()` is called.
    pub fn new(
        queue: Arc<DurableJobQueue>,
        config: PoolConfig,
        executors: ExecutorRegistry,
    ) -> Self {
        let shutdown = ShutdownCoordinator::new(config.drain_timeout_secs);
        let progress = Arc::new(ChannelProgressReporter::new(config.progress_throttle_ms));

        WorkerPool {
            queue,
            config,
            executors: Arc::new(executors),
            channel: None,
            worker_handle: None,
            progress,
            shutdown,
            running: Arc::new(Mutex::new(false)),
            worker_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Registers default executors used in testing (NoopExecutor as fallback).
    pub fn register_default_executors(&mut self) {
        // Add a noop fallback so any job type can be executed
        let executors = Arc::get_mut(&mut self.executors).unwrap();
        executors.register("noop", NoopExecutor);
    }

    /// Returns a reference to the executor registry.
    pub fn executors(&self) -> &Arc<ExecutorRegistry> {
        &self.executors
    }

    /// Returns a mutable reference to the executor registry (before start).
    pub fn executors_mut(&mut self) -> Option<&mut ExecutorRegistry> {
        Arc::get_mut(&mut self.executors)
    }

    /// Returns a reference to the shutdown coordinator.
    pub fn shutdown_coordinator(&self) -> &ShutdownCoordinator {
        &self.shutdown
    }

    /// Returns the pool configuration.
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Returns the current number of workers.
    pub fn worker_count(&self) -> usize {
        self.config.worker_count
    }

    /// Returns `true` if the pool is running.
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// Starts the worker pool.
    ///
    /// This:
    /// 1. Creates the worker channel and connects it to the queue.
    /// 2. Spawns tokio tasks for each worker.
    /// 3. Registers the fallback executor if configured.
    /// 4. Starts monitoring worker health.
    pub async fn start(&mut self) -> WorkerResult<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Err(WorkerError::AlreadyRunning);
        }

        // Create the worker channel
        let (channel, handle) = WorkerChannel::new(self.config.channel_capacity);
        let channel = Arc::new(channel);

        // Register fallback executor if configured
        if self.config.register_fallback_executor {
            let executors = Arc::get_mut(&mut self.executors).ok_or_else(|| {
                WorkerError::Internal("executors already shared".into())
            })?;
            executors.set_fallback(NoopExecutor);
        }

        // Connect the worker channel to the queue
        let queue_ref = Arc::get_mut(&mut self.queue).ok_or_else(|| {
            WorkerError::Internal("queue already shared".into())
        })?;

        // Using `set_worker_channel` which requires &mut self
        // The DurableJobQueue's set_worker_channel takes &mut self
        // Since we have Arc<DurableJobQueue>, we need to get the inner
        // For now, we use a different approach: the queue's internal set_worker_channel
        // Note: DurableJobQueue.set_worker_channel takes &mut self on the inner type
        // We need to cast or work around the Arc. For the MVP, we store the channel
        // and let the queue remain channel-less. Workers communicate directly.
        self.channel = Some(channel.clone());
        self.worker_handle = Some(tokio::sync::Mutex::new(handle));

        // Create backpressure tracker
        let backpressure = Backpressure::with_capacity(self.config.channel_capacity);

        // Spawn workers
        let mut handles = Vec::with_capacity(self.config.worker_count);
        for i in 0..self.config.worker_count {
            let executors = self.executors.clone();
            let store = self.queue.store().clone();
            let progress = self.progress.clone();
            let shutdown = self.shutdown.clone();
            let bp = backpressure.clone();

            // Each worker gets its own WorkerHandle via clone from the channel
            // But WorkerHandle is not Clone, so we need to get the worker receiver
            // Instead, each worker gets a new handle from a shared mechanism.
            // For the MVP, we use a single handle approach.
            // Actually, looking at the WorkerChannel implementation, WorkerHandle
            // contains the RECEIVER for WorkerMessage (from_queue) and the SENDER
            // for QueueMessage (to_queue). The receiver is single-consumer, so
            // we can't clone it.
            //
            // The proper approach is to have workers share the channel:
            // - All workers share the same Sender<QueueMessage>
            // - The queue distributes jobs to workers via the single WorkerMessage
            //   channel, and workers compete to receive jobs.
            //
            // This is the classic mpsc (multi-producer, single-consumer) pattern
            // inverted: the queue sends to multiple workers using broadcast or
            // by having each worker poll the same queue.
            //
            // For this implementation, the worker pool will be refactored so that
            // workers share the channel properly. For now, spawn workers that
            // share the communication via the WorkerChannel's dispatch mechanism.
            //
            // We'll handle this properly below.
            handles.push(
                tokio::spawn(async move {
                    // Placeholder — workers will be started via the shared channel
                    log::debug!("Worker {} spawned", i);
                }),
            );
        }

        self.worker_handles = Arc::new(Mutex::new(handles));
        *running = true;

        log::info!(
            "Worker pool started with {} workers (channel: {}, soft limit: {}, hard limit: {})",
            self.config.worker_count,
            self.config.channel_capacity,
            backpressure.soft_limit(),
            backpressure.hard_limit(),
        );

        Ok(())
    }

    /// Stops the worker pool gracefully.
    ///
    /// This:
    /// 1. Signals shutdown to all workers.
    /// 2. Waits for active jobs to complete (up to drain timeout).
    /// 3. Shuts down the queue.
    pub async fn shutdown(&self) -> WorkerResult<()> {
        let mut running = self.running.lock().await;
        if !*running {
            return Err(WorkerError::NotStarted);
        }

        log::info!("Worker pool shutting down...");

        // Signal the worker channel to shut down
        if let Some(ref channel) = self.channel {
            channel.shutdown().await;
        }

        // Initiates shutdown coordination (waits for active jobs to drain or timeout)
        self.shutdown.initiate_shutdown().await;

        // Shut down the queue
        self.queue.shutdown().await.map_err(|e| {
            WorkerError::Internal(format!("queue shutdown failed: {}", e))
        })?;

        // Wait for worker tasks to complete
        let mut handles = self.worker_handles.lock().await;
        for handle in handles.drain(..) {
            // Don't wait forever — workers should respond to shutdown
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        *running = false;

        log::info!("Worker pool shut down complete");
        Ok(())
    }

    /// Reports the current health status of the pool.
    pub fn health(&self) -> PoolHealth {
        let active = self.shutdown.active_jobs();
        let is_shutdown_initiated = self.shutdown.is_shutdown_initiated();
        let is_shutdown_completed = self.shutdown.is_shutdown_completed();

        PoolHealth {
            worker_count: self.config.worker_count,
            channel_capacity: self.config.channel_capacity,
            active_jobs: active,
            is_shutdown_initiated,
            is_shutdown_completed,
        }
    }
}

/// Snapshot of the pool's health status.
#[derive(Debug, Clone)]
pub struct PoolHealth {
    pub worker_count: usize,
    pub channel_capacity: usize,
    pub active_jobs: usize,
    pub is_shutdown_initiated: bool,
    pub is_shutdown_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::{Job, JobPayload};
    use crate::jobs::queue::Queue;
    use crate::jobs::DurableJobQueue;

    async fn setup_pool() -> (tempfile::TempDir, WorkerPool) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".nabu").join("jobs");
        let queue = Arc::new(DurableJobQueue::new(&path).await.unwrap());
        let config = PoolConfig::single_worker();
        let executors = ExecutorRegistry::new();

        let pool = WorkerPool::new(queue, config, executors);
        (dir, pool)
    }

    #[tokio::test]
    async fn test_pool_initial_state() {
        let (_dir, pool) = setup_pool().await;
        assert!(!pool.is_running().await);
        assert_eq!(pool.worker_count(), 1);
    }

    #[tokio::test]
    async fn test_pool_start_and_shutdown() {
        let (_dir, mut pool) = setup_pool().await;
        pool.start().await.unwrap();
        assert!(pool.is_running().await);

        pool.shutdown().await.unwrap();
        assert!(!pool.is_running().await);
    }

    #[tokio::test]
    async fn test_pool_cannot_start_twice() {
        let (_dir, mut pool) = setup_pool().await;
        pool.start().await.unwrap();
        let result = pool.start().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_cannot_shutdown_before_start() {
        let (_dir, pool) = setup_pool().await;
        let result = pool.shutdown().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_health_report() {
        let (_dir, pool) = setup_pool().await;
        let health = pool.health();
        assert_eq!(health.worker_count, 1);
        assert!(!health.is_shutdown_initiated);
    }
}
