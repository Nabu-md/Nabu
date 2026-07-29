use crate::jobs::cancellation::CancellationToken;
use crate::jobs::errors::{JobError, JobResult};
use crate::jobs::job::{Job, JobStatus};
use crate::jobs::queue::Queue;
use crate::jobs::workers::backpressure::BackpressureHandle;
use crate::jobs::workers::executor::ExecutorRegistry;
use crate::jobs::workers::progress::ProgressReporter;
use crate::jobs::workers::shutdown::ShutdownHandle;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

/// A generic async worker that executes jobs from the queue.
///
/// Workers are completely generic — they know nothing about OCR, AI, or indexing.
/// They simply pull jobs, find the right executor, and run them.
pub struct Worker {
    id: usize,
    queue: Arc<dyn Queue>,
    executors: Arc<ExecutorRegistry>,
    shutdown: ShutdownHandle,
    backpressure: BackpressureHandle,
}

impl Worker {
    pub fn new(
        id: usize,
        queue: Arc<dyn Queue>,
        executors: Arc<ExecutorRegistry>,
        shutdown: ShutdownHandle,
        backpressure: BackpressureHandle,
    ) -> Self {
        Self {
            id,
            queue,
            executors,
            shutdown,
            backpressure,
        }
    }

    /// Run the worker loop.
    ///
    /// The worker continuously:
    /// 1. Checks if shutdown was requested
    /// 2. Tries to dequeue a job
    /// 3. If a job is available, looks up the executor and runs it
    /// 4. Reports completion/failure back to the queue
    /// 5. Reports progress updates to the queue
    pub async fn run(self) {
        log::info!("Worker {} started", self.id);
        self.shutdown.register();

        loop {
            // Check for shutdown
            if self.shutdown.is_shutting_down() {
                log::info!("Worker {} shutting down", self.id);
                break;
            }

            // Try to dequeue a job
            let job = match self.queue.dequeue() {
                Ok(Some(job)) => job,
                Ok(None) => {
                    // No jobs available — wait before retrying
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => {
                    log::error!("Worker {} dequeue error: {}", self.id, e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            log::info!(
                "Worker {} picked up job {} ({})",
                self.id,
                job.id,
                job.processor_name
            );

            self.backpressure.job_started();

            // Find the executor for this job type
            let executor = match self.executors.get(&job.processor_name) {
                Some(executor) => executor,
                None => {
                    log::error!(
                        "Worker {}: no executor for '{}'",
                        self.id,
                        job.processor_name
                    );
                    let _ = self.queue.mark_failed(
                        &job.id.to_string(),
                        &format!("No executor for '{}'", job.processor_name),
                    );
                    self.backpressure.job_completed();
                    continue;
                }
            };

            // Create progress reporter that updates the queue
            let queue_ref = self.queue.clone();
            let job_id = job.id.to_string();
            let progress = ProgressReporter::new(move |value| {
                let _ = queue_ref.report_progress(&job_id, value, None);
            });

            // Run the executor
            let result = executor
                .execute(&job, progress.clone(), CancellationToken::new())
                .await;

            match result {
                Ok(_) => {
                    log::info!("Worker {} completed job {}", self.id, job.id);
                    let _ = self.queue.mark_completed(&job.id.to_string());
                }
                Err(e) => {
                    log::error!("Worker {} job {} failed: {}", self.id, job.id, e);
                    let _ = self.queue.mark_failed(
                        &job.id.to_string(),
                        &e.to_string(),
                    );
                }
            }

            progress.complete();
            self.backpressure.job_completed();
        }

        self.shutdown.unregister();
        log::info!("Worker {} stopped", self.id);
    }
}

/// Message sent from the queue to a worker.
#[derive(Debug)]
pub enum WorkerMessage {
    /// Execute this job
    Execute(Job),
    /// Shut down gracefully
    Shutdown,
}

/// A handle to send messages to a specific worker.
pub struct WorkerHandle {
    tx: mpsc::Sender<WorkerMessage>,
}

impl WorkerHandle {
    pub fn new(tx: mpsc::Sender<WorkerMessage>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, msg: WorkerMessage) -> Result<(), mpsc::error::SendError<WorkerMessage>> {
        self.tx.send(msg).await
    }
}
