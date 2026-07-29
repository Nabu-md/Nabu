use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::cancellation::CancellationToken;
use super::errors::{JobError, JobResult};
use super::job::{Job, JobId, JobPayload, JobStatus};
use super::persistence::JobStore;
use super::priority::Priority;
use super::retry::RetryPolicy;
use super::scheduler::ScheduleSpec;
use super::worker_channel::WorkerChannel;

/// The core queue interface for job lifecycle management.
///
/// This trait defines the operations that any queue implementation must support.
/// It is storage-agnostic — implementors can use file-based persistence,
/// in-memory stores, or database-backed storage.
pub trait Queue: Send + Sync {
    /// Enqueues a new job for execution.
    ///
    /// Returns the assigned `JobId`.
    fn enqueue(&self, job: Job) -> impl std::future::Future<Output = JobResult<JobId>> + Send;

    /// Creates and enqueues a job with the given parameters.
    fn create_job<S: Into<String>>(
        &self,
        job_type: S,
        payload: JobPayload,
    ) -> impl std::future::Future<Output = JobResult<JobId>> + Send;

    /// Dequeues the next ready job for processing.
    ///
    /// Returns the highest-priority ready job, or `None` if no jobs are ready.
    fn dequeue(&self) -> impl std::future::Future<Output = JobResult<Option<Job>>> + Send;

    /// Peeks at the next ready job without removing it from the queue.
    fn peek(&self) -> impl std::future::Future<Output = JobResult<Option<Job>>> + Send;

    /// Cancels a queued or running job.
    fn cancel(&self, job_id: &JobId) -> impl std::future::Future<Output = JobResult<()>> + Send;

    /// Retries a failed job immediately (bypassing the retry schedule).
    fn retry(&self, job_id: &JobId) -> impl std::future::Future<Output = JobResult<()>> + Send;

    /// Reschedules a job to run at a future time.
    fn reschedule(
        &self,
        job_id: &JobId,
        schedule: ScheduleSpec,
    ) -> impl std::future::Future<Output = JobResult<()>> + Send;

    /// Removes a job from the queue permanently.
    fn remove(&self, job_id: &JobId) -> impl std::future::Future<Output = JobResult<()>> + Send;

    /// Returns the number of jobs currently in the queue (including all statuses).
    fn count(&self) -> impl std::future::Future<Output = usize> + Send;

    /// Returns the number of jobs in a specific status.
    fn count_by_status(
        &self,
        status: JobStatus,
    ) -> impl std::future::Future<Output = JobResult<usize>> + Send;

    /// Lists all jobs with a given status.
    fn list_by_status(
        &self,
        status: JobStatus,
    ) -> impl std::future::Future<Output = JobResult<Vec<Job>>> + Send;

    /// Loads a specific job by ID.
    fn get_job(&self, job_id: &JobId) -> impl std::future::Future<Output = JobResult<Job>> + Send;

    /// Shuts down the queue, persisting all state and preventing new enqueues.
    fn shutdown(&self) -> impl std::future::Future<Output = JobResult<()>> + Send;
}

/// The default durable job queue implementation.
///
/// This queue is:
/// - **Durable**: Jobs are persisted to disk under `.nabu/jobs/`
/// - **Reliable**: Survives crashes, power loss, and restarts
/// - **Prioritized**: Higher-priority jobs execute before lower-priority
/// - **Schedulable**: Jobs can be scheduled for future execution
/// - **Retryable**: Failed jobs can be retried with exponential backoff
/// - **Cancellable**: Jobs can be cancelled cooperatively
///
/// The queue is designed to be shared across threads via `Arc<DurableJobQueue>`.
#[derive(Clone)]
pub struct DurableJobQueue {
    /// The file-backed job store.
    store: Arc<JobStore>,

    /// The worker communication channel (wired in Prompt 36).
    worker_channel: Option<Arc<WorkerChannel>>,

    /// Global cancellation token for queue shutdown.
    shutdown_token: Arc<CancellationToken>,
}

impl DurableJobQueue {
    /// Creates a new durable job queue backed by the given store path.
    ///
    /// The path should be `.nabu/jobs/` relative to the vault root.
    /// This will create the directory structure and rebuild the in-memory index
    /// from any previously persisted jobs.
    pub async fn new<P: AsRef<std::path::Path>>(store_path: P) -> JobResult<Self> {
        let store = JobStore::new(store_path).await?;
        Ok(DurableJobQueue {
            store: Arc::new(store),
            worker_channel: None,
            shutdown_token: Arc::new(CancellationToken::new()),
        })
    }

    /// Attaches a worker channel for dispatching jobs to workers.
    ///
    /// This is called during initialization when the WorkerPool is created (Prompt 36).
    pub fn set_worker_channel(&mut self, channel: Arc<WorkerChannel>) {
        self.worker_channel = Some(channel);
    }

    /// Returns a reference to the underlying job store.
    pub fn store(&self) -> &Arc<JobStore> {
        &self.store
    }

    /// Returns the path where jobs are persisted.
    pub fn path(&self) -> &std::path::Path {
        self.store.path()
    }

    /// Returns `true` if the queue has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown_token.is_cancelled()
    }

    /// Recover jobs that were in `Running` state on the last shutdown/crash.
    ///
    /// This resets them to `Queued` so they can be retried after a crash.
    pub async fn recover_stuck_jobs(&self) -> JobResult<u32> {
        let stuck = self.store.list_by_status(JobStatus::Running).await?;
        let count = stuck.len() as u32;

        for mut job in stuck {
            job.status = JobStatus::Queued;
            job.last_error = Some("job was in Running state on startup; reset to Queued after crash recovery".into());
            self.store.update(&job).await?;
        }

        Ok(count)
    }
}

impl Queue for DurableJobQueue {
    async fn enqueue(&self, job: Job) -> JobResult<JobId> {
        if self.shutdown_token.is_cancelled() {
            return Err(JobError::QueueShuttingDown);
        }

        let id = job.id;
        self.store.store(&job).await?;

        // If worker channel is connected, try to dispatch immediately
        if let Some(ref channel) = self.worker_channel {
            if !channel.is_shutdown() {
                // Best-effort dispatch — the job is safely persisted regardless
                let _ = channel.dispatch(job).await;
            }
        }

        Ok(id)
    }

    async fn create_job<S: Into<String>>(&self, job_type: S, payload: JobPayload) -> JobResult<JobId> {
        let job = Job::new(job_type, payload);
        self.enqueue(job).await
    }

    async fn dequeue(&self) -> JobResult<Option<Job>> {
        let mut ready = self.store.list_ready().await?;

        if ready.is_empty() {
            return Ok(None);
        }

        // Take the highest-priority, oldest job
        let mut job = ready.remove(0);
        job.mark_running();
        self.store.update(&job).await?;

        Ok(Some(job))
    }

    async fn peek(&self) -> JobResult<Option<Job>> {
        let ready = self.store.list_ready().await?;
        Ok(ready.into_iter().next())
    }

    async fn cancel(&self, job_id: &JobId) -> JobResult<()> {
        let mut job = self.store.load(job_id).await?;

        match job.status {
            JobStatus::Queued | JobStatus::Scheduled => {
                job.mark_cancelled();
                self.store.update(&job).await?;

                // Notify worker if the job was dispatched
                if let Some(ref channel) = self.worker_channel {
                    let _ = channel.notify_cancelled(*job_id).await;
                }

                Ok(())
            }
            JobStatus::Running => {
                // Mark cancelled in the store; the worker will check its cancellation token
                job.mark_cancelled();
                self.store.update(&job).await?;

                if let Some(ref channel) = self.worker_channel {
                    let _ = channel.notify_cancelled(*job_id).await;
                }

                Ok(())
            }
            s if s.is_terminal() => {
                Err(JobError::InvalidState(
                    job_id.to_string(),
                    format!("{:?}", s),
                ))
            }
            _ => {
                job.mark_cancelled();
                self.store.update(&job).await?;
                Ok(())
            }
        }
    }

    async fn retry(&self, job_id: &JobId) -> JobResult<()> {
        let mut job = self.store.load(job_id).await?;

        match job.status {
            JobStatus::Failed | JobStatus::PermanentlyFailed => {
                // Reset for retry
                job.status = JobStatus::Queued;
                job.scheduled_at = Utc::now();
                job.started_at = None;
                job.finished_at = None;
                // Keep retry_count — we want to track total attempts
                self.store.update(&job).await?;
                Ok(())
            }
            s if s.is_terminal() => Err(JobError::InvalidState(
                job_id.to_string(),
                format!("{:?}", s),
            )),
            _ => Err(JobError::InvalidState(
                job_id.to_string(),
                format!("{:?}", job.status),
            )),
        }
    }

    async fn reschedule(&self, job_id: &JobId, schedule: ScheduleSpec) -> JobResult<()> {
        let mut job = self.store.load(job_id).await?;

        if job.status.is_terminal() {
            return Err(JobError::InvalidState(
                job_id.to_string(),
                format!("{:?}", job.status),
            ));
        }

        job.scheduled_at = schedule.resolve();

        if job.scheduled_at > Utc::now() {
            job.status = JobStatus::Scheduled;
        } else {
            job.status = JobStatus::Queued;
        }

        self.store.update(&job).await?;
        Ok(())
    }

    async fn remove(&self, job_id: &JobId) -> JobResult<()> {
        self.store.remove(job_id).await
    }

    async fn count(&self) -> usize {
        self.store.count().await
    }

    async fn count_by_status(&self, status: JobStatus) -> JobResult<usize> {
        let jobs = self.store.list_by_status(status).await?;
        Ok(jobs.len())
    }

    async fn list_by_status(&self, status: JobStatus) -> JobResult<Vec<Job>> {
        self.store.list_by_status(status).await
    }

    async fn get_job(&self, job_id: &JobId) -> JobResult<Job> {
        self.store.load(job_id).await
    }

    async fn shutdown(&self) -> JobResult<()> {
        self.shutdown_token.cancel();

        // Notify workers
        if let Some(ref channel) = self.worker_channel {
            channel.shutdown().await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_queue() -> (tempfile::TempDir, DurableJobQueue) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".nabu").join("jobs");
        let queue = DurableJobQueue::new(&path).await.unwrap();
        (dir, queue)
    }

    #[tokio::test]
    async fn test_enqueue_and_count() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        assert_eq!(queue.count().await, 1);

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.job_type.0, "test");
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_dequeue_returns_highest_priority() {
        let (_dir, queue) = setup_queue().await;

        let _low = queue
            .enqueue(Job::new("low", JobPayload::new()).with_priority(Priority::Low))
            .await
            .unwrap();
        let _high = queue
            .enqueue(Job::new("high", JobPayload::new()).with_priority(Priority::High))
            .await
            .unwrap();

        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.priority, Priority::High);
        assert_eq!(first.status, JobStatus::Running);

        let second = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(second.priority, Priority::Low);
    }

    #[tokio::test]
    async fn test_dequeue_empty_returns_none() {
        let (_dir, queue) = setup_queue().await;
        let result = queue.dequeue().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_dequeue_marks_as_running() {
        let (_dir, queue) = setup_queue().await;
        queue.create_job("test", JobPayload::new()).await.unwrap();

        let job = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Running);

        // Verify in store
        let stored = queue.get_job(&job.id).await.unwrap();
        assert_eq!(stored.status, JobStatus::Running);
        assert!(stored.started_at.is_some());
    }

    #[tokio::test]
    async fn test_peek_does_not_mark_running() {
        let (_dir, queue) = setup_queue().await;
        queue.create_job("test", JobPayload::new()).await.unwrap();

        let peeked = queue.peek().await.unwrap().unwrap();
        assert_eq!(peeked.status, JobStatus::Queued);

        // Job still queued in store
        let stored = queue.get_job(&peeked.id).await.unwrap();
        assert_eq!(stored.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_cancel_queued_job() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();

        queue.cancel(&id).await.unwrap();

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_running_job() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        queue.dequeue().await.unwrap(); // marks as running

        queue.cancel(&id).await.unwrap();

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_completed_job_fails() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        let mut job = queue.get_job(&id).await.unwrap();
        job.mark_completed();
        queue.store.update(&job).await.unwrap();

        let result = queue.cancel(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_failed_job() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        let mut job = queue.get_job(&id).await.unwrap();
        job.mark_failed("error".into());
        queue.store.update(&job).await.unwrap();

        queue.retry(&id).await.unwrap();

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_retry_queued_job_fails() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();

        let result = queue.retry(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reschedule_to_future() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();

        let future = Utc::now() + Duration::hours(2);
        queue
            .reschedule(&id, ScheduleSpec::At(future))
            .await
            .unwrap();

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Scheduled);
        assert!(job.scheduled_at > Utc::now());
    }

    #[tokio::test]
    async fn test_reschedule_to_past() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();

        let past = Utc::now() - Duration::minutes(5);
        queue
            .reschedule(&id, ScheduleSpec::At(past))
            .await
            .unwrap();

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_reschedule_terminal_fails() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        let mut job = queue.get_job(&id).await.unwrap();
        job.mark_completed();
        queue.store.update(&job).await.unwrap();

        let result = queue
            .reschedule(&id, ScheduleSpec::Immediate)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_job() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();
        assert_eq!(queue.count().await, 1);

        queue.remove(&id).await.unwrap();
        assert_eq!(queue.count().await, 0);
    }

    #[tokio::test]
    async fn test_count_by_status() {
        let (_dir, queue) = setup_queue().await;
        queue.create_job("a", JobPayload::new()).await.unwrap();
        queue.create_job("b", JobPayload::new()).await.unwrap();

        // Complete one
        let dequeued = queue.dequeue().await.unwrap().unwrap();
        let mut j = queue.get_job(&dequeued.id).await.unwrap();
        j.mark_completed();
        queue.store.update(&j).await.unwrap();

        let queued_count = queue.count_by_status(JobStatus::Queued).await.unwrap();
        let completed_count = queue.count_by_status(JobStatus::Completed).await.unwrap();
        assert_eq!(queued_count, 1);
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let (_dir, queue) = setup_queue().await;
        queue.create_job("a", JobPayload::new()).await.unwrap();
        queue.create_job("b", JobPayload::new()).await.unwrap();

        let queued = queue.list_by_status(JobStatus::Queued).await.unwrap();
        assert_eq!(queued.len(), 2);

        let completed = queue.list_by_status(JobStatus::Completed).await.unwrap();
        assert!(completed.is_empty());
    }

    #[tokio::test]
    async fn test_recover_stuck_jobs() {
        let (_dir, queue) = setup_queue().await;
        let id = queue.create_job("test", JobPayload::new()).await.unwrap();

        // Simulate a crash by manually setting to Running
        let mut job = queue.get_job(&id).await.unwrap();
        job.mark_running();
        queue.store.update(&job).await.unwrap();

        let recovered = queue.recover_stuck_jobs().await.unwrap();
        assert_eq!(recovered, 1);

        let job = queue.get_job(&id).await.unwrap();
        assert_eq!(job.status, JobStatus::Queued);
    }

    #[tokio::test]
    async fn test_shutdown_prevents_enqueue() {
        let (_dir, queue) = setup_queue().await;
        queue.shutdown().await.unwrap();

        let result = queue.create_job("test", JobPayload::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".nabu").join("jobs");

        // First session
        {
            let queue = DurableJobQueue::new(&path).await.unwrap();
            queue
                .enqueue(
                    Job::new("survivor", JobPayload::new())
                        .with_priority(Priority::High),
                )
                .await
                .unwrap();
            // Drop without explicit shutdown (simulating crash)
        }

        // Second session — queue recovers
        {
            let queue = DurableJobQueue::new(&path).await.unwrap();
            assert_eq!(queue.count().await, 1);

            let job = queue.dequeue().await.unwrap().unwrap();
            assert_eq!(job.job_type.0, "survivor");
            assert_eq!(job.priority, Priority::High);
        }
    }

    #[tokio::test]
    async fn test_priority_ordering_comprehensive() {
        let (_dir, queue) = setup_queue().await;

        // Enqueue in arbitrary order
        let _bg = queue
            .enqueue(Job::new("bg", JobPayload::new()).with_priority(Priority::Background))
            .await
            .unwrap();
        let _crit = queue
            .enqueue(Job::new("crit", JobPayload::new()).with_priority(Priority::Critical))
            .await
            .unwrap();
        let _norm = queue
            .enqueue(Job::new("norm", JobPayload::new()).with_priority(Priority::Normal))
            .await
            .unwrap();

        // Dequeue should respect priority
        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.job_type.0, "crit");

        let second = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(second.job_type.0, "norm");

        let third = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(third.job_type.0, "bg");
    }
}
