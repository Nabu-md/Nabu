use crate::jobs::cancellation::CancellationToken;
use crate::jobs::errors::{JobError, JobResult};
use crate::jobs::job::{Job, JobStatus, JobType};
use crate::jobs::persistence::JobStore;
use crate::jobs::priority::PriorityItem;
use crate::jobs::retry::RetryPolicy;
use crate::jobs::scheduler::{ScheduleSpec, Scheduler};
use crate::jobs::worker_channel::{QueueHandle, WorkerChannel};
use chrono::Utc;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};


/// The Queue trait defines the interface for all queue implementations.
/// The queue is storage-agnostic — any backend can implement this trait.
pub trait Queue: Send + Sync {
    /// Enqueue a new job for execution.
    fn enqueue(&self, job: Job) -> JobResult<()>;

    /// Dequeue the highest-priority ready job.
    fn dequeue(&self) -> JobResult<Option<Job>>;

    /// Peek at the highest-priority job without removing it.
    fn peek(&self) -> JobResult<Option<Job>>;

    /// Cancel a queued or running job.
    fn cancel(&self, job_id: &str) -> JobResult<Job>;

    /// Retry a failed job.
    fn retry(&self, job_id: &str) -> JobResult<Job>;

    /// Reschedule a job to execute later.
    fn reschedule(&self, job_id: &str, spec: ScheduleSpec) -> JobResult<Job>;

    /// Remove a job entirely from the queue.
    fn remove(&self, job_id: &str) -> JobResult<()>;

    /// Count of all jobs in the queue.
    fn count(&self) -> JobResult<usize>;

    /// Count jobs with a specific status.
    fn count_by_status(&self, status: JobStatus) -> JobResult<usize>;

    /// Load a specific job by ID.
    fn load_job(&self, job_id: &str) -> JobResult<Option<Job>>;

    /// List all jobs with a given status.
    fn list_jobs(&self, status: JobStatus) -> JobResult<Vec<Job>>;

    /// Mark a job as running.
    fn mark_running(&self, job_id: &str) -> JobResult<Job>;

    /// Mark a job as completed.
    fn mark_completed(&self, job_id: &str) -> JobResult<Job>;

    /// Mark a job as failed.
    fn mark_failed(&self, job_id: &str, error: &str) -> JobResult<Job>;

    /// Report progress for a running job.
    fn report_progress(&self, job_id: &str, progress: f64, message: Option<&str>) -> JobResult<()>;
}

/// A durable, persistent, priority-ordered job queue.
///
/// This is the SINGLE job queue for the entire Nabu platform.
/// All async work flows through this queue.
/// No duplicate queue systems exist.
pub struct DurableJobQueue {
    store: Arc<JobStore>,
    scheduler: Arc<Scheduler>,
    channel: WorkerChannel,
    running: Arc<AtomicBool>,
    retry_policies: Mutex<HashMap<JobType, RetryPolicy>>,
    /// In-memory priority heap for fast dequeue
    heap: Mutex<BinaryHeap<Reverse<PriorityItem<String>>>>,
}

impl DurableJobQueue {
    /// Create a new durable job queue with file-backed persistence.
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> JobResult<Self> {
        let store = Arc::new(JobStore::new(base_path)?);
        let scheduler = Arc::new(Scheduler::new(store.clone()));

        let queue = Self {
            store,
            scheduler,
            channel: WorkerChannel::new(),
            running: Arc::new(AtomicBool::new(true)),
            retry_policies: Mutex::new(HashMap::new()),
            heap: Mutex::new(BinaryHeap::new()),
        };

        // Load existing queued jobs into the heap on startup
        queue.rebuild_heap()?;

        Ok(queue)
    }

    /// Register a retry policy for a specific job type.
    pub fn set_retry_policy(&self, job_type: JobType, policy: RetryPolicy) {
        let mut policies = self.retry_policies.lock().unwrap();
        policies.insert(job_type, policy);
    }

    /// Get the retry policy for a job type, falling back to standard.
    pub fn get_retry_policy(&self, job_type: &JobType) -> RetryPolicy {
        let policies = self.retry_policies.lock().unwrap();
        policies.get(job_type).cloned().unwrap_or(RetryPolicy::standard())
    }

    /// Get a handle for the queue side (enqueue results).
    pub fn handle(&self) -> QueueHandle {
        QueueHandle::new(self.channel.clone())
    }

    /// Get a reference to the scheduler.
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Get a reference to the job store.
    pub fn store(&self) -> &JobStore {
        &self.store
    }

    /// Rebuild the in-memory heap from persistent storage.
    fn rebuild_heap(&self) -> JobResult<()> {
        let mut heap = self.heap.lock().unwrap();
        heap.clear();

        let queued = self.store.load_by_status(JobStatus::Queued)?;
        for job in queued {
            if job.is_ready() {
                heap.push(Reverse(PriorityItem::new(
                    job.priority,
                    job.created_at,
                    job.id.to_string(),
                )));
            }
        }

        Ok(())
    }

    /// Get the worker channel (for workers to connect).
    pub fn worker_channel(&self) -> &WorkerChannel {
        &self.channel
    }

    /// Create a cancellation token for a job.
    pub fn cancellation_token(&self, _job_id: &str) -> CancellationToken {
        CancellationToken::new()
    }

    /// Process due scheduled jobs (called periodically).
    pub fn process_scheduled(&self) -> JobResult<Vec<Job>> {
        let due = self.scheduler.process_due_jobs()?;
        let mut heap = self.heap.lock().unwrap();
        for job in &due {
            heap.push(Reverse(PriorityItem::new(
                job.priority,
                job.created_at,
                job.id.to_string(),
            )));
        }
        Ok(due)
    }
}

impl Queue for DurableJobQueue {
    fn enqueue(&self, job: Job) -> JobResult<()> {
        if !self.running.load(Ordering::Acquire) {
            return Err(JobError::Shutdown);
        }

        let mut job = job;

        // Apply default retry policy based on job type
        let max_retries = self.get_retry_policy(&job.job_type).max_retries;
        job.maximum_retries = max_retries;

        job.status = JobStatus::Queued;
        job.created_at = Utc::now();

        self.store.store(&job)?;

        // Add to in-memory heap
        if job.is_ready() {
            let mut heap = self.heap.lock().unwrap();
            heap.push(Reverse(PriorityItem::new(
                job.priority,
                job.created_at,
                job.id.to_string(),
            )));
        }

        tracing::debug!(
            subsystem = "queue",
            component = "engine",
            operation = "enqueue",
            job_id = %job.id,
            job_type = %job.job_type.name(),
            priority = %job.priority.name(),
            processor = %job.processor_name,
            "Job enqueued"
        );

        Ok(())
    }

    fn dequeue(&self) -> JobResult<Option<Job>> {
        if !self.running.load(Ordering::Acquire) {
            return Err(JobError::Shutdown);
        }

        let mut heap = self.heap.lock().unwrap();

        // Peek at the highest-priority job
        loop {
            let next_id = match heap.peek() {
                Some(Reverse(item)) => item.item.clone(),
                None => return Ok(None),
            };

            // Load the job to check if it's still valid
            let job = match self.store.load(&next_id)? {
                Some(job) if job.status == JobStatus::Queued && job.is_ready() => {
                    heap.pop();
                    job
                }
                Some(_) => {
                    // Job is no longer queued — remove from heap
                    heap.pop();
                    continue;
                }
                None => {
                    // Job disappeared — remove from heap
                    heap.pop();
                    continue;
                }
            };

            // Mark as running
            let running = self.store.move_job(
                &job.id.to_string(),
                JobStatus::Queued,
                JobStatus::Running,
            )?;

            tracing::debug!(
                subsystem = "queue",
                component = "engine",
                operation = "dequeue",
                job_id = %running.id,
                job_type = %running.job_type.name(),
                priority = %running.priority.name(),
                "Job dequeued and marked running"
            );

            return Ok(Some(running));
        }
    }

    fn peek(&self) -> JobResult<Option<Job>> {
        let heap = self.heap.lock().unwrap();
        match heap.peek() {
            Some(Reverse(item)) => {
                let job = self.store.load(&item.item)?;
                Ok(job.filter(|j| j.status == JobStatus::Queued && j.is_ready()))
            }
            None => Ok(None),
        }
    }

    fn cancel(&self, job_id: &str) -> JobResult<Job> {
        let job = self
            .store
            .load(job_id)?
            .ok_or_else(|| JobError::NotFound(job_id.to_string()))?;

        match job.status {
            JobStatus::Queued | JobStatus::Scheduled => {
                let cancelled = self
                    .store
                    .move_job(job_id, job.status.clone(), JobStatus::Cancelled)?;
                Ok(cancelled)
            }
            JobStatus::Running => {
                // For running jobs, we mark as cancelled.
                // The worker is responsible for cooperative cancellation.
                let mut cancelled = job.clone();
                cancelled.status = JobStatus::Cancelled;
                cancelled.finished_at = Some(Utc::now());
                self.store.store(&cancelled)?;
                Ok(cancelled)
            }
            _ => Err(JobError::InvalidState(
                job_id.to_string(),
                format!("cannot cancel {:?} job", job.status),
            )),
        }
    }

    fn retry(&self, job_id: &str) -> JobResult<Job> {
        let job = self
            .store
            .load(job_id)?
            .ok_or_else(|| JobError::NotFound(job_id.to_string()))?;

        if !matches!(job.status, JobStatus::Failed) {
            return Err(JobError::InvalidState(
                job_id.to_string(),
                format!("cannot retry {:?} job", job.status),
            ));
        }

        let mut job = job;
        job.retry_count += 1;
        job.status = JobStatus::Queued;
        job.last_error = None;
        job.started_at = None;
        job.finished_at = None;

        self.store.store(&job)?;

        let mut heap = self.heap.lock().unwrap();
        heap.push(Reverse(PriorityItem::new(
            job.priority,
            job.created_at,
            job.id.to_string(),
        )));

        Ok(job)
    }

    fn reschedule(&self, job_id: &str, spec: ScheduleSpec) -> JobResult<Job> {
        let job = self.store.load(job_id)?.ok_or_else(|| {
            JobError::NotFound(job_id.to_string())
        })?;

        match job.status {
            JobStatus::Queued | JobStatus::Scheduled => {
                self.scheduler.reschedule(job_id, spec)
            }
            _ => Err(JobError::InvalidState(
                job_id.to_string(),
                format!("cannot reschedule {:?} job", job.status),
            )),
        }
    }

    fn remove(&self, job_id: &str) -> JobResult<()> {
        self.store.remove(job_id)
    }

    fn count(&self) -> JobResult<usize> {
        self.store.active_count()
    }

    fn count_by_status(&self, status: JobStatus) -> JobResult<usize> {
        self.store.count(status)
    }

    fn load_job(&self, job_id: &str) -> JobResult<Option<Job>> {
        self.store.load(job_id)
    }

    fn list_jobs(&self, status: JobStatus) -> JobResult<Vec<Job>> {
        self.store.load_by_status(status)
    }

    fn mark_running(&self, job_id: &str) -> JobResult<Job> {
        let job = self.store.load(job_id)?.ok_or_else(|| {
            JobError::NotFound(job_id.to_string())
        })?;

        if job.status != JobStatus::Queued {
            return Err(JobError::InvalidState(
                job_id.to_string(),
                format!("cannot start {:?} job", job.status),
            ));
        }

        self.store.move_job(job_id, JobStatus::Queued, JobStatus::Running)
    }

    fn mark_completed(&self, job_id: &str) -> JobResult<Job> {
        let mut job = self.store.move_job(job_id, JobStatus::Running, JobStatus::Completed)?;
        job.finished_at = Some(Utc::now());
        self.store.store(&job)?;
        Ok(job)
    }

    fn mark_failed(&self, job_id: &str, error: &str) -> JobResult<Job> {
        let mut job = self.store.load(job_id)?.ok_or_else(|| {
            JobError::NotFound(job_id.to_string())
        })?;

        let _policy = self.get_retry_policy(&job.job_type);
        job.retry_count += 1;
        job.last_error = Some(error.to_string());

        if job.should_retry() {
            job.status = JobStatus::Queued;
            self.store.store(&job)?;

            let mut heap = self.heap.lock().unwrap();
            heap.push(Reverse(PriorityItem::new(
                job.priority,
                job.created_at,
                job.id.to_string(),
            )));

            Ok(job)
        } else {
            job.status = JobStatus::Failed;
            job.finished_at = Some(Utc::now());
            self.store.store(&job)?;
            Ok(job)
        }
    }

    fn report_progress(&self, job_id: &str, progress: f64, message: Option<&str>) -> JobResult<()> {
        let mut job = self.store.load(job_id)?.ok_or_else(|| {
            JobError::NotFound(job_id.to_string())
        })?;

        job.progress = progress.clamp(0.0, 1.0);
        job.progress_message = message.map(|s| s.to_string());
        self.store.store(&job)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::priority::Priority;
    use crate::jobs::retry::policies;
    use tempfile::tempdir;

    fn test_job(job_type: JobType, priority: Priority) -> Job {
        Job::new(
            job_type,
            serde_json::json!({"test": true}),
            "test_processor",
        )
        .with_priority(priority)
    }

    #[test]
    fn test_enqueue_dequeue() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();

        let job = test_job(JobType::Ocr, Priority::High);
        queue.enqueue(job).unwrap();

        let dequeued = queue.dequeue().unwrap().unwrap();
        assert_eq!(dequeued.priority, Priority::High);
        assert_eq!(dequeued.status, JobStatus::Running);
    }

    #[test]
    fn test_priority_ordering() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();

        queue.enqueue(test_job(JobType::Whisper, Priority::Low)).unwrap();
        queue.enqueue(test_job(JobType::Ocr, Priority::Critical)).unwrap();
        queue.enqueue(test_job(JobType::Export, Priority::Background)).unwrap();
        queue.enqueue(test_job(JobType::Ocr, Priority::High)).unwrap();

        // Should dequeue Critical first, then High, then Low, then Background
        let first = queue.dequeue().unwrap().unwrap();
        assert_eq!(first.priority, Priority::Critical);

        let second = queue.dequeue().unwrap().unwrap();
        assert_eq!(second.priority, Priority::High);

        let third = queue.dequeue().unwrap().unwrap();
        assert_eq!(third.priority, Priority::Low);

        let fourth = queue.dequeue().unwrap().unwrap();
        assert_eq!(fourth.priority, Priority::Background);
    }

    #[test]
    fn test_survives_restart() {
        let dir = tempdir().unwrap();

        let job_id;
        {
            let queue = DurableJobQueue::new(dir.path()).unwrap();
            let job = test_job(JobType::Ocr, Priority::Normal);
            job_id = job.id;
            queue.enqueue(job).unwrap();
        }

        {
            let queue = DurableJobQueue::new(dir.path()).unwrap();
            // Queue should have persisted the job
            let dequeued = queue.dequeue().unwrap().unwrap();
            assert_eq!(dequeued.id, job_id);
        }
    }

    #[test]
    fn test_cancel_queued_job() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();

        let job = test_job(JobType::Ocr, Priority::Normal);
        let job_id = job.id.to_string();
        queue.enqueue(job).unwrap();

        let cancelled = queue.cancel(&job_id).unwrap();
        assert_eq!(cancelled.status, JobStatus::Cancelled);

        // Should not dequeue cancelled job
        let dequeued = queue.dequeue().unwrap();
        assert!(dequeued.is_none());
    }

    #[test]
    fn test_retry_logic() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();
        queue.set_retry_policy(JobType::Ocr, RetryPolicy { max_retries: 2, ..Default::default() });

        let job = test_job(JobType::Ocr, Priority::Normal);
        let job_id = job.id.to_string();
        queue.enqueue(job).unwrap();

        // Mark failed — should be auto-retried
        let failed = queue.mark_failed(&job_id, "test error").unwrap();
        assert_eq!(failed.status, JobStatus::Queued); // retried
        assert_eq!(failed.retry_count, 1);
    }

    #[test]
    fn test_retry_max_retries() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();
        queue.set_retry_policy(JobType::Ocr, RetryPolicy { max_retries: 1, ..Default::default() });

        let job = test_job(JobType::Ocr, Priority::Normal);
        let job_id = job.id.to_string();
        queue.enqueue(job).unwrap();

        // First failure: retry (retry_count becomes 1)
        let failed1 = queue.mark_failed(&job_id, "error 1").unwrap();
        assert_eq!(failed1.status, JobStatus::Queued);

        // Second failure: permanent (retry_count becomes 2, but max_retries is 1)
        // Wait, let's check: retry_count goes from 1 to 2 after mark_failed
        // should_retry checks retry_count < max_retries, so 2 < 1 is false
        let failed2 = queue.mark_failed(&job_id, "error 2").unwrap();
        assert_eq!(failed2.status, JobStatus::Failed);
    }

    #[test]
    fn test_peek() {
        let dir = tempdir().unwrap();
        let queue = DurableJobQueue::new(dir.path()).unwrap();

        let job = test_job(JobType::Ocr, Priority::Critical);
        queue.enqueue(job).unwrap();

        let peeked = queue.peek().unwrap().unwrap();
        assert_eq!(peeked.priority, Priority::Critical);

        // Peek shouldn't dequeue
        let dequeued = queue.dequeue().unwrap().unwrap();
        assert_eq!(dequeued.id, peeked.id);
    }
}
