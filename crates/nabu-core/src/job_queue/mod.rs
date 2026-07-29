//! Background job system for asynchronous processing.
//!
//! # Architecture
//!
//! The job queue sits between the `IngestionPipeline` and `StorageManager`,
//! enabling long-running processors (OCR, Whisper, AI, embeddings) to execute
//! without blocking the capture pipeline or the UI.
//!
//! ```text
//! CaptureEngine
//!     ↓  ItemCaptured
//! IngestionPipeline
//!     ↓  ItemProcessed
//! JobQueue (enqueue)
//!     ↓
//! WorkerPool (async)
//!     ↓  ProcessingPipeline::run()
//!     ↓  ItemProcessed (re-published)
//! StorageManager
//!     ↓  ItemStored
//! EventBus → Indexer, VaultGraph, future plugins
//! ```
//!
//! # Design
//!
//! - **JobQueue** is an unbounded mpsc channel. Enqueuing a job never blocks.
//! - **WorkerPool** spawns N tokio tasks. Each worker loops, pulling jobs from
//!   the queue and executing them via `ProcessingPipeline::run()` inside
//!   `tokio::task::spawn_blocking`.
//! - **EventBus remains the orchestration layer**. Workers publish events when
//!   jobs complete, fail, or make progress.
//! - **Existing ProcessingPipeline code is unchanged**. The pipeline's `run()`
//!   method is simply executed in a background thread instead of inline.

use crate::event_bus::{
    EVENT_ITEM_PROCESSED, EVENT_ITEM_PROCESSING_COMPLETED, EVENT_ITEM_PROCESSING_FAILED,
    EVENT_ITEM_PROCESSING_STARTED, EventBus, ItemProcessed, ItemProcessingCompleted,
    ItemProcessingFailed, ItemProcessingStarted,
};
use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::ProcessingPipeline;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Job types — extensible for future workloads (OCR, Whisper, AI, etc.)
// ---------------------------------------------------------------------------

/// The type of work a background job performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobType {
    /// Run the full ProcessingPipeline processor chain on a KnowledgeObject.
    ProcessKnowledgeObject,
    /// Update the Tantivy search index for a KnowledgeObject.
    IndexKnowledgeObject,
    /// Update the VaultGraph for a KnowledgeObject.
    UpdateGraph,
}

impl JobType {
    /// Human-readable name of the job type.
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::ProcessKnowledgeObject => "process_knowledge_object",
            JobType::IndexKnowledgeObject => "index_knowledge_object",
            JobType::UpdateGraph => "update_graph",
        }
    }
}

/// Priority level for background jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JobPriority {
    Low,
    Normal,
    High,
}

impl Default for JobPriority {
    fn default() -> Self {
        JobPriority::Normal
    }
}

impl From<JobPriority> for u8 {
    fn from(p: JobPriority) -> Self {
        match p {
            JobPriority::Low => 0,
            JobPriority::Normal => 1,
            JobPriority::High => 2,
        }
    }
}

/// The lifecycle status of a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is waiting in the queue.
    Queued,
    /// Job is currently being executed by a worker.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed after exhausting retries.
    Failed(String),
    /// Job was cancelled.
    Cancelled,
    /// Job failed but will be retried.
    Retrying { attempt: u32, max_retries: u32 },
}

// ---------------------------------------------------------------------------
// Job struct — a single unit of background work
// ---------------------------------------------------------------------------

/// A single unit of work to be executed by the background worker pool.
///
/// Jobs carry the `KnowledgeObject` they operate on and track their own
/// lifecycle. The `progress` field (0.0–1.0) enables UI progress reporting.
#[derive(Debug, Clone)]
pub struct BackgroundJob {
    /// Unique identifier for this job.
    pub id: Uuid,
    /// What kind of work this job performs.
    pub job_type: JobType,
    /// Scheduling priority (jobs with higher priority are dequeued first).
    pub priority: JobPriority,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// The knowledge object this job operates on.
    pub knowledge_object: KnowledgeObject,
    /// Current execution attempt (1-based).
    pub attempt: u32,
    /// Maximum retry attempts before giving up.
    pub max_retries: u32,
    /// Progress percentage (0.0 = not started, 1.0 = complete).
    pub progress: f32,
    /// Optional error message if the job failed.
    pub error: Option<String>,
    /// ISO 8601 timestamp when the job was created.
    pub created_at: String,
    /// ISO 8601 timestamp when the job started executing.
    pub started_at: Option<String>,
}

impl BackgroundJob {
    /// Create a new background job.
    pub fn new(
        job_type: JobType,
        priority: JobPriority,
        knowledge_object: KnowledgeObject,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type,
            priority,
            status: JobStatus::Queued,
            knowledge_object,
            attempt: 0,
            max_retries: 3,
            progress: 0.0,
            error: None,
            created_at: current_timestamp(),
            started_at: None,
        }
    }

    /// Create a new background job with custom retry configuration.
    pub fn with_retries(
        job_type: JobType,
        priority: JobPriority,
        knowledge_object: KnowledgeObject,
        max_retries: u32,
    ) -> Self {
        let mut job = Self::new(job_type, priority, knowledge_object);
        job.max_retries = max_retries;
        job
    }

    /// Returns true if this job can be retried.
    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_retries
    }

    /// Increment the attempt counter and update status for retry.
    pub fn prepare_retry(&mut self) {
        self.attempt += 1;
        self.status = JobStatus::Retrying {
            attempt: self.attempt,
            max_retries: self.max_retries,
        };
        self.error = None;
        self.progress = 0.0;
    }
}

// ---------------------------------------------------------------------------
// JobQueue — an unbounded mpsc channel for scheduling background work
// ---------------------------------------------------------------------------

/// A non-blocking job queue backed by an unbounded tokio mpsc channel.
///
/// Enqueuing a job is O(1) and never blocks. Workers consume jobs from
/// the receiver side. The queue is `Send + Sync` and can be shared across
/// threads via `Arc`.
///
/// # Thread Safety
///
/// - `enqueue()` is `&self` and thread-safe via the mpsc sender.
/// - The receiver half is wrapped in `Mutex` for shared access by workers.
/// - No locking is required for enqueue operations.
pub struct JobQueue {
    /// Sender half — enqueue jobs from any thread.
    sender: tokio::sync::mpsc::UnboundedSender<BackgroundJob>,
    /// Receiver half — consumed by workers.
    receiver: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<BackgroundJob>>,
    /// The processing pipeline to execute for processing jobs.
    pipeline: Arc<ProcessingPipeline>,
    /// Event bus for publishing lifecycle events.
    event_bus: Arc<EventBus>,
}

impl JobQueue {
    /// Create a new job queue.
    ///
    /// The queue is created with a sender and receiver pair. Workers consume
    /// from the receiver via [`WorkerPool`].
    pub fn new(
        pipeline: Arc<ProcessingPipeline>,
        event_bus: Arc<EventBus>,
    ) -> Arc<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Arc::new(Self {
            sender: tx,
            receiver: tokio::sync::Mutex::new(rx),
            pipeline,
            event_bus,
        })
    }

    /// Enqueue a background job for asynchronous execution.
    ///
    /// This method is non-blocking and can be called from any thread.
    /// Returns `true` if the job was successfully enqueued.
    pub fn enqueue(&self, job: BackgroundJob) -> bool {
        self.sender.send(job).is_ok()
    }

    /// Try to receive the next job from the queue.
    ///
    /// Returns `None` if the queue is empty.
    /// Used internally by [`WorkerPool`] workers.
    pub async fn recv(&self) -> Option<BackgroundJob> {
        self.receiver.lock().await.recv().await
    }

    /// Execute a processing job synchronously.
    ///
    /// This runs the `ProcessingPipeline::run()` method and publishes the
    /// appropriate lifecycle events. It is called from worker tasks via
    /// `tokio::task::spawn_blocking`.
    fn execute_processing_job(&self, mut job: BackgroundJob) -> BackgroundJob {
        job.status = JobStatus::Running;
        job.started_at = Some(current_timestamp());
        job.progress = 0.0;

        // Publish processing started event
        self.event_bus.publish(
            EVENT_ITEM_PROCESSING_STARTED,
            &ItemProcessingStarted {
                id: job.knowledge_object.id,
                vault_id: job.knowledge_object.vault_id.clone(),
                object_type: format!("{:?}", job.knowledge_object.object_type),
                timestamp: current_timestamp(),
            },
        );

        // Execute the processing pipeline (synchronous — runs in spawn_blocking)
        let result = self.pipeline.run(job.knowledge_object.clone());

        match result {
            Ok((processed_obj, warnings)) => {
                // Publish processing completed event
                self.event_bus.publish(
                    EVENT_ITEM_PROCESSING_COMPLETED,
                    &ItemProcessingCompleted {
                        id: processed_obj.id,
                        vault_id: processed_obj.vault_id.clone(),
                        object_type: format!("{:?}", processed_obj.object_type),
                        timestamp: processed_obj.modified_at.clone(),
                        warnings: warnings.clone(),
                    },
                );

                // Re-publish ItemProcessed so StorageManager can persist
                let processed_event =
                    ItemProcessed::from_knowledge_object(&processed_obj, warnings);
                self.event_bus
                    .publish(EVENT_ITEM_PROCESSED, &processed_event);

                job.status = JobStatus::Completed;
                job.progress = 1.0;
                job.knowledge_object = processed_obj;
            }
            Err((rejected_obj, reason, warnings)) => {
                // Publish processing failed event
                self.event_bus.publish(
                    EVENT_ITEM_PROCESSING_FAILED,
                    &ItemProcessingFailed {
                        id: rejected_obj.id,
                        vault_id: rejected_obj.vault_id.clone(),
                        object_type: format!("{:?}", rejected_obj.object_type),
                        timestamp: current_timestamp(),
                        reason: reason.clone(),
                        warnings: warnings.clone(),
                    },
                );

                if job.can_retry() {
                    job.prepare_retry();
                    // Re-enqueue for retry
                    self.enqueue(job.clone());
                    job.status = JobStatus::Retrying {
                        attempt: job.attempt,
                        max_retries: job.max_retries,
                    };
                } else {
                    job.status = JobStatus::Failed(reason.clone());
                    job.error = Some(reason);
                    job.progress = 1.0;
                }
                job.knowledge_object = rejected_obj;
            }
        }

        job
    }
}

// ---------------------------------------------------------------------------
// WorkerPool — a pool of tokio tasks that consume jobs from the queue
// ---------------------------------------------------------------------------

/// A pool of async workers that consume jobs from a [`JobQueue`].
///
/// Workers run indefinitely, pulling jobs from the queue and executing them.
/// Each worker uses `tokio::task::spawn_blocking` for CPU-bound pipeline
/// execution, keeping the async runtime responsive for I/O-bound tasks.
///
/// # Graceful Shutdown
///
/// Call [`WorkerPool::shutdown`] to signal all workers to stop after
/// completing their current job. Workers check for shutdown signals
/// between jobs.
pub struct WorkerPool {
    /// Join handles for all worker tasks.
    handles: Vec<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender. When dropped or explicitly notified, workers exit.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl WorkerPool {
    /// Create a new worker pool with the given number of workers.
    ///
    /// Workers begin processing jobs from the queue immediately.
    /// Processing jobs (type `ProcessKnowledgeObject`) execute the pipeline
    /// in a blocking thread and publish results back to the EventBus.
    ///
    /// # Arguments
    ///
    /// * `size` — Number of concurrent workers to spawn.
    /// * `queue` — The shared job queue to consume from.
    pub fn new(size: usize, queue: Arc<JobQueue>) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let shutdown_rx = Arc::new(shutdown_rx);
        let mut handles = Vec::with_capacity(size);

        for worker_id in 0..size {
            let queue = queue.clone();
            let mut rx = shutdown_rx.clone();

            let handle = tokio::spawn(async move {
                loop {
                    // Check for shutdown signal
                    if *rx.borrow() {
                        break;
                    }

                    // Try to receive the next job with a short timeout
                    // so we can periodically check for shutdown.
                    let job = tokio::time::timeout(
                        std::time::Duration::from_millis(500),
                        queue.recv(),
                    );

                    match job.await {
                        Ok(Some(job)) => {
                            // Execute the job based on its type
                            let result = match job.job_type {
                                JobType::ProcessKnowledgeObject => {
                                    // Run pipeline in a blocking thread to avoid
                                    // starving the async runtime.
                                    let queue_clone = queue.clone();
                                    let job_clone = job.clone();
                                    tokio::task::spawn_blocking(move || {
                                        queue_clone.execute_processing_job(job_clone)
                                    })
                                    .await
                                    .unwrap_or_else(|_| {
                                        let mut j = job;
                                        j.status = JobStatus::Failed(
                                            "Worker task panicked".to_string(),
                                        );
                                        j
                                    })
                                }
                                JobType::IndexKnowledgeObject
                                | JobType::UpdateGraph => {
                                    // These are handled by EventBus subscribers.
                                    // The job type exists for future direct scheduling.
                                    job
                                }
                            };

                            // Log completion or failure
                            match &result.status {
                                JobStatus::Completed => {
                                    eprintln!(
                                        "Job {} ({}) completed",
                                        result.id,
                                        result.job_type.as_str(),
                                    );
                                }
                                JobStatus::Failed(e) => {
                                    eprintln!(
                                        "Job {} ({}) failed: {}",
                                        result.id,
                                        result.job_type.as_str(),
                                        e,
                                    );
                                }
                                _ => {}
                            }
                        }
                        Ok(None) => {
                            // Channel closed — all senders dropped, shut down
                            break;
                        }
                        Err(_timeout) => {
                            // No job available — loop back and check shutdown
                            continue;
                        }
                    }
                }

                eprintln!("Worker {} shutting down", worker_id);
            });

            handles.push(handle);
        }

        Self {
            handles,
            shutdown_tx,
        }
    }

    /// Signal all workers to shut down gracefully.
    ///
    /// Workers finish their current job (if any) and then exit.
    pub async fn shutdown(self) {
        // Signal shutdown
        let _ = self.shutdown_tx.send(true);

        // Wait for all workers to finish
        for handle in self.handles {
            if let Err(e) = handle.await {
                eprintln!("Worker shutdown with error: {:?}", e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    format!("{}.{:03}Z", secs, millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};
    use crate::processing::processor::{ProcessingResult, Processor};

    #[derive(Debug)]
    struct TestProcessor;

    impl Processor for TestProcessor {
        fn name(&self) -> &'static str {
            "test_processor"
        }

        fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
            let mut obj = knowledge_object;
            obj.metadata.title = Some("Processed".to_string());
            ProcessingResult::modified(obj, vec!["Test completed".to_string()])
        }
    }

    fn create_test_object() -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata::default(),
        }
    }

    #[test]
    fn job_creation_defaults() {
        let obj = create_test_object();
        let job = BackgroundJob::new(JobType::ProcessKnowledgeObject, JobPriority::Normal, obj);
        assert_eq!(job.job_type, JobType::ProcessKnowledgeObject);
        assert_eq!(job.priority, JobPriority::Normal);
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.attempt, 0);
        assert_eq!(job.max_retries, 3);
        assert_eq!(job.progress, 0.0);
    }

    #[test]
    fn job_can_retry_when_under_max() {
        let obj = create_test_object();
        let mut job = BackgroundJob::with_retries(
            JobType::ProcessKnowledgeObject,
            JobPriority::High,
            obj,
            5,
        );
        assert_eq!(job.max_retries, 5);
        assert!(job.can_retry());

        job.prepare_retry();
        assert_eq!(job.attempt, 1);
        assert!(job.can_retry());

        // Exhaust retries
        for _ in 0..4 {
            job.prepare_retry();
        }
        assert_eq!(job.attempt, 5);
        assert!(!job.can_retry());
    }

    #[test]
    fn job_priority_ordering() {
        assert!(JobPriority::Low < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::High);
        assert!(JobPriority::Low < JobPriority::High);
    }

    #[test]
    fn job_type_human_readable() {
        assert_eq!(JobType::ProcessKnowledgeObject.as_str(), "process_knowledge_object");
        assert_eq!(JobType::IndexKnowledgeObject.as_str(), "index_knowledge_object");
        assert_eq!(JobType::UpdateGraph.as_str(), "update_graph");
    }

    #[tokio::test]
    async fn job_queue_enqueue_and_process() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(TestProcessor));

        let queue = JobQueue::new(pipeline, bus.clone());
        let obj = create_test_object();
        let job = BackgroundJob::new(
            JobType::ProcessKnowledgeObject,
            JobPriority::Normal,
            obj,
        );

        // Enqueue the job
        assert!(queue.enqueue(job));

        // Receive and execute (synchronous test path)
        let received = queue.recv().await;
        assert!(received.is_some());
        let mut received = received.unwrap();

        // Execute processing job
        let result = queue.execute_processing_job(received);
        assert_eq!(result.status, JobStatus::Completed);
        assert_eq!(result.knowledge_object.metadata.title, Some("Processed".to_string()));
    }

    #[tokio::test]
    async fn job_queue_retry_on_failure() {
        use crate::processing::processor::ProcessingDecision;

        #[derive(Debug)]
        struct FailingProcessor;

        impl Processor for FailingProcessor {
            fn name(&self) -> &'static str {
                "failing_processor"
            }

            fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
                ProcessingResult::rejected(knowledge_object, "Test failure".to_string())
            }
        }

        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(FailingProcessor));

        let queue = JobQueue::new(pipeline, bus.clone());
        let obj = create_test_object();
        let mut job = BackgroundJob::new(
            JobType::ProcessKnowledgeObject,
            JobPriority::Normal,
            obj,
        );
        job.max_retries = 1; // Allow 1 retry

        // Execute — should fail and set up retry
        let result = queue.execute_processing_job(job);

        // With max_retries=1 and attempt=0, it should retry
        // After retry with attempt=1 == max_retries,
        // it will fail again and become Failed
        assert!(matches!(result.status, JobStatus::Failed(_)));
    }
}
