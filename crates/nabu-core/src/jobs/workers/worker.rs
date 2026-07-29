use std::sync::Arc;
use std::time::Duration;

use crate::jobs::cancellation::CancellationToken;
use crate::jobs::job::{Job, JobId, JobStatus};
use crate::jobs::persistence::JobStore;
use crate::jobs::worker_channel::{QueueMessage, WorkerHandle, WorkerMessage};

use super::backpressure::Backpressure;
use super::errors::WorkerResult;
use super::executor::{ExecuteContext, ExecuteResult, ExecutorRegistry};
use super::progress::{ChannelProgressReporter, ProgressReporter};
use super::shutdown::ShutdownCoordinator;

/// A single asynchronous worker.
///
/// Each worker is a long-running tokio task that:
/// 1. Waits for jobs from the queue via the `WorkerHandle`.
/// 2. Looks up the executor for the job's type.
/// 3. Executes the job, reporting progress and results.
/// 4. Handles cancellation and shutdown signals.
///
/// Workers are generic — they know nothing about OCR, AI, or indexing.
#[derive(Debug)]
pub struct Worker {
    /// Unique identifier for this worker.
    pub id: usize,

    /// Handle for receiving messages from the queue.
    handle: WorkerHandle,

    /// Registry of job type → executor mappings.
    executors: Arc<ExecutorRegistry>,

    /// The job store for persisting state changes.
    store: Arc<JobStore>,

    /// Progress reporter for job progress.
    progress: Arc<dyn ProgressReporter>,

    /// Coordinator for shutdown.
    shutdown: ShutdownCoordinator,

    /// Backpressure tracker.
    backpressure: Backpressure,
}

impl Worker {
    /// Creates a new worker.
    pub fn new(
        id: usize,
        handle: WorkerHandle,
        executors: Arc<ExecutorRegistry>,
        store: Arc<JobStore>,
        progress: Arc<dyn ProgressReporter>,
        shutdown: ShutdownCoordinator,
        backpressure: Backpressure,
    ) -> Self {
        Worker {
            id,
            handle,
            executors,
            store,
            progress,
            shutdown,
            backpressure,
        }
    }

    /// Runs the worker loop. This blocks until a shutdown signal is received.
    ///
    /// The loop:
    /// 1. Receives a message from the queue.
    /// 2. On `NewJob(job)`: executes it, reports result, handles retry.
    /// 3. On `JobCancelled(id)`: acknowledges cancellation.
    /// 4. On `Shutdown`: exits the loop.
    pub async fn run(&mut self) {
        log::info!("Worker {} started", self.id);

        loop {
            let msg = self.handle.recv().await;

            match msg {
                Some(WorkerMessage::NewJob(job)) => {
                    log::debug!("Worker {} received job {}", self.id, job.id);
                    self.execute_job(job).await;
                }
                Some(WorkerMessage::JobCancelled(job_id)) => {
                    log::debug!("Worker {} received cancellation for {}", self.id, job_id);
                    self.handle_cancellation(job_id).await;
                }
                Some(WorkerMessage::Shutdown) => {
                    log::info!("Worker {} received shutdown signal", self.id);
                    break;
                }
                None => {
                    // Channel closed — queue must have shut down
                    log::info!("Worker {} channel closed, exiting", self.id);
                    break;
                }
            }
        }

        log::info!("Worker {} stopped", self.id);
    }

    /// Executes a single job.
    async fn execute_job(&self, job: Job) {
        self.shutdown.job_started();
        self.backpressure.record_dispatch().ok();

        // Report started
        let _ = self.handle.report_started(job.id).await;

        // Update job status in store
        let mut job = job;
        job.mark_running();

        let cancellation = CancellationToken::new();

        if let Err(e) = self.store.update(&job).await {
            log::error!("Worker {}: failed to persist job start: {}", self.id, e);
            self.shutdown.job_finished();
            self.backpressure.record_completion();
            return;
        }

        // Look up the executor
        let executor = match self.executors.get(&job.job_type) {
            Ok(e) => e,
            Err(e) => {
                log::error!(
                    "Worker {}: no executor for job type '{}': {}",
                    self.id,
                    job.job_type,
                    e
                );
                let _ = self.handle.report_failed(job.id, e.to_string()).await;
                self.handle_failure(job, e.to_string()).await;
                self.shutdown.job_finished();
                self.backpressure.record_completion();
                return;
            }
        };

        // Create execution context
        let ctx = ExecuteContext::new(job.clone(), cancellation, self.progress.clone());

        // Execute
        let result = executor.execute(&ctx);

        match result {
            ExecuteResult::Completed => {
                log::debug!("Worker {}: job {} completed", self.id, job.id);
                let mut job = job;
                job.mark_completed();
                let _ = self.handle.report_completed(job.id).await;

                if let Err(e) = self.store.update(&job).await {
                    log::error!("Worker {}: failed to persist job completion: {}", self.id, e);
                }

                self.shutdown.job_finished();
                self.backpressure.record_completion();
            }
            ExecuteResult::Failed(error) => {
                log::warn!("Worker {}: job {} failed: {}", self.id, job.id, error);
                let _ = self.handle.report_failed(job.id, error.clone()).await;
                self.handle_failure(job, error).await;
                self.shutdown.job_finished();
                self.backpressure.record_completion();
            }
            ExecuteResult::Cancelled => {
                log::info!("Worker {}: job {} was cancelled", self.id, job.id);
                let mut job = job;
                job.mark_cancelled();

                let _ = self.handle.report_completed(job.id).await; // Signal completion

                if let Err(e) = self.store.update(&job).await {
                    log::error!("Worker {}: failed to persist job cancellation: {}", self.id, e);
                }

                self.shutdown.job_finished();
                self.backpressure.record_completion();
            }
        }
    }

    /// Handles a job failure, potentially scheduling a retry.
    async fn handle_failure(&self, mut job: Job, error: String) {
        let will_retry = job.mark_failed(error);

        if let Err(e) = self.store.update(&job).await {
            log::error!(
                "Worker {}: failed to persist job failure state: {}",
                self.id,
                e
            );
        }

        if will_retry {
            log::info!(
                "Worker {}: job {} will be retried (attempt {}/{})",
                self.id,
                job.id,
                job.retry_count,
                job.max_retries
            );
        } else {
            log::warn!(
                "Worker {}: job {} permanently failed after {} attempts",
                self.id,
                job.id,
                job.retry_count
            );
        }
    }

    /// Handles a cancellation notification for a job.
    async fn handle_cancellation(&self, job_id: JobId) {
        // Load the job and mark it as cancelled in the store
        if let Ok(mut job) = self.store.load(&job_id).await {
            if job.status == JobStatus::Running || job.status == JobStatus::Queued {
                job.mark_cancelled();
                if let Err(e) = self.store.update(&job).await {
                    log::error!(
                        "Worker {}: failed to persist cancellation for {}: {}",
                        self.id,
                        job_id,
                        e
                    );
                }
                self.backpressure.record_completion();
            }
        }
    }
}
