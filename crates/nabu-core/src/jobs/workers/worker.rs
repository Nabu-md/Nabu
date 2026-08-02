use crate::jobs::cancellation::CancellationToken;

use crate::jobs::job::Job;
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
        let span = tracing::info_span!(
            "nabu",
            subsystem = "worker",
            component = "pool",
            operation = "run",
            worker_id = self.id,
        );
        let _guard = span.enter();

        tracing::info!("Worker started");
        self.shutdown.register();

        loop {
            // Check for shutdown
            if self.shutdown.is_shutting_down() {
                tracing::info!("Worker shutting down");
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
                    tracing::error!(
                        subsystem = "worker",
                        component = "pool",
                        operation = "dequeue",
                        error = %e,
                        "Dequeue error"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            tracing::info!(
                subsystem = "worker",
                component = "pool",
                operation = "pickup",
                job_id = %job.id,
                job_type = %job.job_type.name(),
                processor = %job.processor_name,
                priority = %job.priority.name(),
                "Worker picked up job"
            );

            self.backpressure.job_started();

            // Find the executor for this job type
            let executor = match self.executors.get(&job.processor_name) {
                Some(executor) => executor,
                None => {
                    tracing::error!(
                        subsystem = "worker",
                        component = "pool",
                        operation = "execute",
                        executor = %job.processor_name,
                        "No executor found for job type"
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
                    tracing::info!(
                        subsystem = "worker",
                        component = "pool",
                        operation = "complete",
                        job_id = %job.id,
                        "Worker completed job"
                    );
                    let _ = self.queue.mark_completed(&job.id.to_string());
                }
                Err(e) => {
                    tracing::error!(
                        subsystem = "worker",
                        component = "pool",
                        operation = "fail",
                        job_id = %job.id,
                        error = %e,
                        "Worker job failed"
                    );
                    let _ = self.queue.mark_failed(&job.id.to_string(), &e.to_string());
                }
            }

            progress.complete();
            self.backpressure.job_completed();
        }

        self.shutdown.unregister();
        tracing::info!("Worker stopped");
    }
}

/// Message sent from the queue to a worker.
#[derive(Debug)]
pub enum WorkerMessage {
    /// Execute this job
    Execute(Box<Job>),
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

    pub async fn send(
        &self,
        msg: WorkerMessage,
    ) -> Result<(), mpsc::error::SendError<WorkerMessage>> {
        self.tx.send(msg).await
    }
}
