use crate::jobs::queue::Queue;
use crate::jobs::workers::backpressure::BackpressureController;
use crate::jobs::workers::executor::ExecutorRegistry;
use crate::jobs::workers::shutdown::ShutdownCoordinator;
use crate::jobs::workers::worker::Worker;
use crate::registry::lifecycle::{
    Lifecycle, LifecycleManager, LifecycleStage,
};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

/// A configurable worker pool that manages the lifecycle of workers.
///
/// The pool is the primary entry point for executing jobs.
/// It manages worker creation, health monitoring, and graceful shutdown.
///
/// `WorkerPool` implements the shared [`Lifecycle`] trait, making it a
/// first-class managed service:
///
/// ```text
/// Created → Initialized → Running → Shutdown
/// ```
///
/// # Lifecycle
///
/// - **Created** — Pool allocated, no workers spawned.
/// - **Initialized** — Resources validated, ready to start.
/// - **Running** — Worker tasks spawned and actively pulling from the queue.
/// - **Shutdown** — All workers stopped, queues drained, resources released.
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
    handles: StdMutex<Vec<JoinHandle<()>>>,
    /// Lifecycle state manager — tracks Created → Initialized → Running → Shutdown
    lifecycle: LifecycleManager,
    /// Maximum duration to wait for workers to drain during shutdown
    drain_timeout: Duration,
}

impl WorkerPool {
    /// Create a new worker pool.
    ///
    /// The pool starts in the `Created` lifecycle stage. Call
    /// [`WorkerPool::start`] (or [`Lifecycle::start`]) to spawn workers.
    ///
    /// # Arguments
    /// * `worker_count` - Number of worker tasks to spawn
    /// * `queue` - The job queue workers will pull from
    /// * `executors` - Registry of job executors
    pub fn new(
        worker_count: usize,
        queue: Arc<dyn Queue>,
        executors: Arc<ExecutorRegistry>,
    ) -> Self {
        Self {
            worker_count,
            queue,
            executors,
            shutdown: Arc::new(ShutdownCoordinator::new(Duration::from_secs(30))),
            backpressure: Arc::new(BackpressureController::default_limits()),
            handles: StdMutex::new(Vec::new()),
            lifecycle: LifecycleManager::new(),
            drain_timeout: Duration::from_secs(30),
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the worker pool.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the worker pool has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the worker pool is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the worker pool has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    // -----------------------------------------------------------------------
    // Lifecycle operations
    // -----------------------------------------------------------------------

    /// Start the worker pool, spawning all workers.
    ///
    /// # Lifecycle transition
    ///
    /// `Created → Initialized → Running` (or `Initialized → Running`
    /// if [`WorkerPool::initialize`] was called first).
    ///
    /// Double-start is a safe no-op — no duplicate workers are spawned.
    ///
    /// # Runtime requirement
    ///
    /// Must be called within a tokio runtime context. Uses
    /// `Handle::try_current()` to obtain the runtime for spawning worker
    /// tasks.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pool has already been shut down (cannot restart)
    /// - No tokio runtime is available on the current thread
    /// - A lifecycle transition is invalid
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let span = tracing::info_span!(
            "nabu",
            subsystem = "worker",
            component = "pool",
            operation = "start",
            worker_count = self.worker_count,
        );
        let _guard = span.enter();

        // Cannot restart a shut-down pool
        if self.lifecycle.is_shutdown() {
            return Err(
                "Worker pool has been shut down and cannot be restarted".into(),
            );
        }

        // Verify runtime context is available before mutating state.
        // Worker spawning requires a tokio runtime; we cannot proceed without one.
        let runtime = Handle::try_current().map_err(|e| {
            format!(
                "WorkerPool::start() requires a tokio runtime context: {}",
                e
            )
        })?;

        // --- Lifecycle state transitions ---
        // Auto-advance Created → Initialized so callers can call start()
        // directly without an explicit initialize() call.
        if self.lifecycle.stage() == LifecycleStage::Created {
            tracing::info!(
                subsystem = "worker",
                component = "pool",
                operation = "start",
                "Initializing worker pool"
            );
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }
        self.lifecycle.transition_to(LifecycleStage::Running)?;

        // Guard against duplicate worker spawn on repeated start()
        let mut handles = self.handles.lock().expect("handles mutex not poisoned");
        if !handles.is_empty() {
            tracing::warn!(
                subsystem = "worker",
                component = "pool",
                operation = "start",
                "Worker pool already started — skipping duplicate start"
            );
            return Ok(());
        }

        tracing::info!("Starting worker pool");

        // --- Worker spawning (preserves existing scheduling model) ---
        // Each worker is spawned as an independent tokio task. The worker
        // loop, queue access, executor dispatch, and backpressure reporting
        // are unchanged from the original implementation.
        for i in 0..self.worker_count {
            let worker = Worker::new(
                i,
                self.queue.clone(),
                self.executors.clone(),
                self.shutdown.handle(),
                self.backpressure.handle(),
            );

            let handle = runtime.spawn(async move {
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

        Ok(())
    }

    /// Initiate graceful shutdown of the worker pool.
    ///
    /// This will:
    /// 1. Signal all workers to stop accepting new work
    /// 2. Wait for active workers to finish their current job
    /// 3. Abort any workers that don't finish within the drain timeout
    ///
    /// # Lifecycle transition
    ///
    /// `Running → Shutdown` (or `Initialized → Shutdown` if start() was
    /// never called but initialize() was).
    ///
    /// Double-shutdown is a safe no-op.
    ///
    /// # Thread safety
    ///
    /// This method is synchronous. When the pool was started on a
    /// multi-threaded tokio runtime, worker tasks continue to run on
    /// the runtime's worker threads during the drain loop, ensuring
    /// active jobs complete. The drain loop polls on the calling thread
    /// with 50 ms intervals until all workers unregister or the timeout
    /// expires.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle transition is invalid.
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let span = tracing::info_span!(
            "nabu",
            subsystem = "worker",
            component = "pool",
            operation = "shutdown",
        );
        let _guard = span.enter();

        tracing::info!("Shutting down worker pool");

        // 1. Signal all workers to stop accepting new work
        self.shutdown.initiate();

        // 2. Wait for active workers to finish their current job
        //
        // Workers are tokio tasks running on the runtime's worker threads.
        // They periodically check the shutdown flag in their loop and
        // unregister when they exit. We poll synchronously on the calling
        // thread — the runtime's worker threads continue to drive the
        // worker tasks during this wait.
        let start = std::time::Instant::now();
        let mut drained = true;
        while !self.shutdown.all_workers_finished() {
            if start.elapsed() >= self.drain_timeout {
                drained = false;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if !drained {
            tracing::warn!(
                subsystem = "worker",
                component = "pool",
                operation = "shutdown",
                "Worker pool drain timeout — force killing remaining workers"
            );
        }

        // 3. Abort any remaining (timed-out) task handles
        let mut handles = self.handles.lock().expect("handles mutex not poisoned");
        for handle in handles.drain(..) {
            handle.abort();
        }

        // --- Lifecycle state transition ---
        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;

        tracing::info!(
            subsystem = "worker",
            component = "pool",
            operation = "shutdown",
            "Worker pool shutdown complete"
        );

        Ok(())
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
            lifecycle_stage: self.lifecycle.stage(),
        }
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared `Lifecycle` trait so `WorkerPool` can be managed
/// by the Capability Platform's lifecycle manager alongside other services.
///
/// The trait methods delegate to the inherent `start()` / `shutdown()` methods
/// defined above. Both the inherent methods and the trait methods are
/// available — inherent methods take priority for direct calls.
impl Lifecycle for WorkerPool {
    fn name(&self) -> &'static str {
        "worker_pool"
    }

    /// Initializes the worker pool.
    ///
    /// Transitions the pool from `Created` to `Initialized`.
    /// No resource allocation or worker spawning occurs here —
    /// [`Lifecycle::start`] performs the actual worker spawning.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "worker",
            component = "pool",
            operation = "initialize",
            "Initializing worker pool"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;
        Ok(())
    }

    /// Starts the worker pool by spawning all workers.
    ///
    /// Delegates to [`WorkerPool::start`].
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        WorkerPool::start(self)
    }

    /// Shuts down the worker pool gracefully.
    ///
    /// Delegates to [`WorkerPool::shutdown`].
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        WorkerPool::shutdown(self)
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
    /// Current lifecycle stage of the pool.
    pub lifecycle_stage: LifecycleStage,
}

impl PoolHealth {
    pub fn all_workers_busy(&self) -> bool {
        self.running_jobs >= self.worker_count
    }
}

/// Convenience function to create and start a fully configured worker pool.
///
/// The caller must be within a tokio runtime context (for worker spawning).
/// Returns an error if the pool fails to start.
pub fn create_worker_pool(
    queue: Arc<dyn Queue>,
    executors: ExecutorRegistry,
    worker_count: usize,
) -> Result<WorkerPool, Box<dyn std::error::Error>> {
    let pool = WorkerPool::new(worker_count, queue, Arc::new(executors));
    Lifecycle::start(&pool)?;
    Ok(pool)
}
