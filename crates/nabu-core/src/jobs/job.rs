use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::priority::Priority;
use super::retry::RetryPolicy;

/// A unique identifier for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Creates a new random job ID.
    pub fn new() -> Self {
        JobId(Uuid::new_v4())
    }

    /// Creates a job ID from a UUID string.
    pub fn from_string(s: &str) -> Option<Self> {
        Uuid::parse_str(s).ok().map(JobId)
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The type of work a job represents.
///
/// This is a string-based type that can be extended by the rest of the platform
/// without modifying the job queue core. Each JobType maps to a specific processor
/// or handler registered elsewhere in the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobType(pub String);

impl JobType {
    /// Creates a new job type.
    pub fn new<S: Into<String>>(ty: S) -> Self {
        JobType(ty.into())
    }
}

impl<S: Into<String>> From<S> for JobType {
    fn from(s: S) -> Self {
        JobType::new(s)
    }
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The lifecycle status of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// The job is queued and waiting to be picked up.
    Queued,
    /// The job is currently being executed.
    Running,
    /// The job completed successfully.
    Completed,
    /// The job failed and may be retried.
    Failed,
    /// The job was cancelled.
    Cancelled,
    /// The job is scheduled for future execution.
    Scheduled,
    /// The job has permanently failed after exhausting retries.
    PermanentlyFailed,
}

impl JobStatus {
    /// Returns `true` if the job is in a terminal state (will never run again).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Cancelled | JobStatus::PermanentlyFailed
        )
    }

    /// Returns `true` if the job is eligible for execution.
    pub fn is_executable(&self) -> bool {
        matches!(self, JobStatus::Queued | JobStatus::Scheduled)
    }
}

/// The serialisable payload of a job.
///
/// This is a flexible key-value map that can carry any data required by
/// a specific job type. The keys and expected values are defined by each
/// processor that consumes a given `JobType`.
pub type JobPayload = HashMap<String, serde_json::Value>;

/// The canonical job model.
///
/// Every job in the system is represented by this struct. It carries all
/// metadata required for scheduling, execution, retry, and cancellation.
///
/// Jobs are serialisable and can be persisted to disk for durability across
/// restarts and crashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique identifier for this job.
    pub id: JobId,

    /// The type of work this job represents (e.g., "ocr", "whisper", "embedding").
    pub job_type: JobType,

    /// The job's data payload.
    pub payload: JobPayload,

    /// Execution priority. Higher-priority jobs execute first.
    pub priority: Priority,

    /// Current lifecycle status.
    pub status: JobStatus,

    /// When the job was created.
    pub created_at: DateTime<Utc>,

    /// When the job is scheduled to run (may be in the future).
    pub scheduled_at: DateTime<Utc>,

    /// When execution started.
    pub started_at: Option<DateTime<Utc>>,

    /// When execution completed (success or permanent failure).
    pub finished_at: Option<DateTime<Utc>>,

    /// How many times this job has been retried.
    pub retry_count: u32,

    /// Maximum number of retries before permanent failure.
    pub max_retries: u32,

    /// The retry policy governing backoff and delay.
    pub retry_policy: RetryPolicy,

    /// Human-readable description of the last error (if any).
    pub last_error: Option<String>,

    /// Arbitrary metadata for job routing and tracking.
    pub metadata: HashMap<String, String>,

    /// The version of the job model (for forward compatibility).
    pub version: u32,
}

impl Job {
    /// Creates a new job with the given type and payload.
    ///
    /// The job is created in `Queued` status with `Normal` priority, no retries,
    /// and immediate scheduling.
    pub fn new<S: Into<String>>(job_type: S, payload: JobPayload) -> Self {
        let now = Utc::now();
        Job {
            id: JobId::new(),
            job_type: JobType::new(job_type),
            payload,
            priority: Priority::Normal,
            status: JobStatus::Queued,
            created_at: now,
            scheduled_at: now,
            started_at: None,
            finished_at: None,
            retry_count: 0,
            max_retries: 0,
            retry_policy: RetryPolicy::default(),
            last_error: None,
            metadata: HashMap::new(),
            version: 1,
        }
    }

    /// Creates a new job scheduled for future execution.
    pub fn scheduled<S: Into<String>>(
        job_type: S,
        payload: JobPayload,
        scheduled_at: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Job {
            id: JobId::new(),
            job_type: JobType::new(job_type),
            payload,
            priority: Priority::Normal,
            status: JobStatus::Scheduled,
            created_at: now,
            scheduled_at,
            started_at: None,
            finished_at: None,
            retry_count: 0,
            max_retries: 0,
            retry_policy: RetryPolicy::default(),
            last_error: None,
            metadata: HashMap::new(),
            version: 1,
        }
    }

    /// Sets the priority of this job.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the retry policy for this job.
    pub fn with_retries(mut self, max_retries: u32, policy: RetryPolicy) -> Self {
        self.max_retries = max_retries;
        self.retry_policy = policy;
        self
    }

    /// Adds a metadata key-value pair.
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns `true` if this job is ready to execute now (status is Queued or Scheduled and time has passed).
    pub fn is_ready(&self) -> bool {
        match self.status {
            JobStatus::Queued => true,
            JobStatus::Scheduled => self.scheduled_at <= Utc::now(),
            _ => false,
        }
    }

    /// Returns `true` if this job can be retried (retry count < max retries).
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Calculates the delay before the next retry using the retry policy's backoff.
    pub fn next_retry_delay(&self) -> chrono::Duration {
        self.retry_policy.backoff_delay(self.retry_count)
    }

    /// Marks the job as running.
    pub fn mark_running(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Marks the job as completed successfully.
    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(Utc::now());
    }

    /// Marks the job as failed. If retries remain, it transitions back to Queued.
    /// Returns `true` if the job will be retried.
    pub fn mark_failed(&mut self, error: String) -> bool {
        self.retry_count += 1;
        self.last_error = Some(error);

        if self.can_retry() {
            self.status = JobStatus::Queued;
            self.scheduled_at = Utc::now() + self.next_retry_delay();
            self.started_at = None;
            true
        } else {
            self.status = JobStatus::PermanentlyFailed;
            self.finished_at = Some(Utc::now());
            false
        }
    }

    /// Marks the job as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = JobStatus::Cancelled;
        self.finished_at = Some(Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_job_creation() {
        let payload = JobPayload::new();
        let job = Job::new("test_job", payload);
        assert_eq!(job.job_type.0, "test_job");
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.priority, Priority::Normal);
        assert_eq!(job.retry_count, 0);
        assert_eq!(job.version, 1);
        assert!(job.is_ready());
    }

    #[test]
    fn test_job_scheduled() {
        let future = Utc::now() + Duration::hours(1);
        let job = Job::scheduled("delayed_job", JobPayload::new(), future);
        assert_eq!(job.status, JobStatus::Scheduled);
        assert!(!job.is_ready());
    }

    #[test]
    fn test_job_ready_when_scheduled_time_passed() {
        let past = Utc::now() - Duration::minutes(5);
        let job = Job::scheduled("past_job", JobPayload::new(), past);
        assert!(job.is_ready());
    }

    #[test]
    fn test_job_lifecycle_completed() {
        let mut job = Job::new("test", JobPayload::new());
        job.mark_running();
        assert_eq!(job.status, JobStatus::Running);
        assert!(job.started_at.is_some());

        job.mark_completed();
        assert_eq!(job.status, JobStatus::Completed);
        assert!(job.finished_at.is_some());
        assert!(job.status.is_terminal());
    }

    #[test]
    fn test_job_lifecycle_failed_with_retries() {
        let mut job = Job::new("test", JobPayload::new())
            .with_retries(3, RetryPolicy::default());

        let will_retry = job.mark_failed("something went wrong".into());
        assert!(will_retry);
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.retry_count, 1);
        assert_eq!(job.last_error, Some("something went wrong".into()));
    }

    #[test]
    fn test_job_lifecycle_failed_exhausted() {
        let mut job = Job::new("test", JobPayload::new())
            .with_retries(1, RetryPolicy::default());

        job.mark_failed("attempt 1".into()); // retry_count = 1, can_retry = false (max_retries=1, retry_count >= max_retries)
        // After first failure, retry_count becomes 1 (since max_retries = 1)
        // can_retry checks retry_count < max_retries, and since they're equal, this returns false.

        // Simulating the real flow: after the first failure, we don't retry
        let will_retry = job.mark_failed("attempt 2".into());
        assert!(!will_retry);
        assert_eq!(job.status, JobStatus::PermanentlyFailed);
        assert_eq!(job.retry_count, 2);
        assert!(job.status.is_terminal());
    }

    #[test]
    fn test_job_cancellation() {
        let mut job = Job::new("test", JobPayload::new());
        job.mark_cancelled();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.finished_at.is_some());
        assert!(job.status.is_terminal());
    }

    #[test]
    fn test_job_serialization_roundtrip() {
        let job = Job::new("test", JobPayload::new())
            .with_priority(Priority::High)
            .with_retries(5, RetryPolicy::exponential(Duration::seconds(10), 2.0, Duration::hours(1)))
            .with_metadata("source", "cli");

        let json = serde_json::to_string_pretty(&job).unwrap();
        let deserialized: Job = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, job.id);
        assert_eq!(deserialized.job_type, job.job_type);
        assert_eq!(deserialized.priority, job.priority);
        assert_eq!(deserialized.max_retries, job.max_retries);
        assert_eq!(
            deserialized.metadata.get("source"),
            Some(&"cli".to_string())
        );
    }

    #[test]
    fn test_terminal_statuses() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(JobStatus::PermanentlyFailed.is_terminal());
        assert!(!JobStatus::Queued.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(!JobStatus::Failed.is_terminal());
        assert!(!JobStatus::Scheduled.is_terminal());
    }

    #[test]
    fn test_executable_statuses() {
        assert!(JobStatus::Queued.is_executable());
        assert!(JobStatus::Scheduled.is_executable());
        assert!(!JobStatus::Running.is_executable());
        assert!(!JobStatus::Completed.is_executable());
        assert!(!JobStatus::Failed.is_executable());
        assert!(!JobStatus::Cancelled.is_executable());
        assert!(!JobStatus::PermanentlyFailed.is_executable());
    }
}
