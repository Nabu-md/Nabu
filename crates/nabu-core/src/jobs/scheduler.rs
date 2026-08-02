use crate::jobs::errors::{JobError, JobResult};
use crate::jobs::job::{Job, JobStatus};
use crate::jobs::persistence::JobStore;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A specification for scheduled/delayed job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleSpec {
    /// Execute at a specific time
    At(DateTime<Utc>),
    /// Execute after a delay from now
    After(Duration),
    /// Execute immediately
    Immediate,
}

impl ScheduleSpec {
    /// Calculate the scheduled time from this spec.
    pub fn scheduled_time(&self) -> DateTime<Utc> {
        match self {
            ScheduleSpec::At(time) => *time,
            ScheduleSpec::After(duration) => Utc::now() + *duration,
            ScheduleSpec::Immediate => Utc::now(),
        }
    }
}

impl From<DateTime<Utc>> for ScheduleSpec {
    fn from(time: DateTime<Utc>) -> Self {
        ScheduleSpec::At(time)
    }
}

impl From<Duration> for ScheduleSpec {
    fn from(duration: Duration) -> Self {
        ScheduleSpec::After(duration)
    }
}

/// The Scheduler manages delayed job execution.
/// It tracks scheduled jobs and makes them available when their time arrives.
pub struct Scheduler {
    store: Arc<JobStore>,
    running: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new(store: Arc<JobStore>) -> Self {
        Self {
            store,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Schedule a job for future execution.
    pub fn schedule(&self, mut job: Job, spec: ScheduleSpec) -> JobResult<Job> {
        let scheduled_at = spec.scheduled_time();
        job.scheduled_at = Some(scheduled_at);
        job.status = JobStatus::Scheduled;
        self.store.store(&job)?;
        Ok(job)
    }

    /// Find all scheduled jobs that are due for execution.
    /// Moves them from Scheduled → Queued.
    pub fn process_due_jobs(&self) -> JobResult<Vec<Job>> {
        if !self.running.load(Ordering::Acquire) {
            return Err(JobError::Shutdown);
        }

        let scheduled = self.store.load_by_status(JobStatus::Scheduled)?;
        let mut due = Vec::new();
        let now = Utc::now();

        for job in scheduled {
            if let Some(scheduled_at) = job.scheduled_at {
                if now >= scheduled_at && job.is_ready() {
                    let moved = self.store.move_job(
                        &job.id.to_string(),
                        JobStatus::Scheduled,
                        JobStatus::Queued,
                    )?;
                    due.push(moved);
                }
            }
        }

        Ok(due)
    }

    /// Cancel a scheduled job.
    pub fn cancel_scheduled(&self, job_id: &str) -> JobResult<Job> {
        let job = self
            .store
            .move_job(job_id, JobStatus::Scheduled, JobStatus::Cancelled)?;
        Ok(job)
    }

    /// Reschedule a job to a new time.
    pub fn reschedule(&self, job_id: &str, new_spec: ScheduleSpec) -> JobResult<Job> {
        let mut job = self
            .store
            .load(job_id)?
            .ok_or_else(|| JobError::NotFound(job_id.to_string()))?;

        let new_time = new_spec.scheduled_time();
        job.scheduled_at = Some(new_time);
        job.status = JobStatus::Scheduled;
        self.store.store(&job)?;
        Ok(job)
    }

    /// Count of scheduled jobs.
    pub fn scheduled_count(&self) -> JobResult<usize> {
        self.store.count(JobStatus::Scheduled)
    }

    /// Stop the scheduler.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Whether the scheduler is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

/// Common delay constants for scheduling.
pub mod delays {
    use chrono::Duration;

    pub const THIRTY_SECONDS: Duration = Duration::seconds(30);
    pub const FIVE_MINUTES: Duration = Duration::minutes(5);
    pub const FIFTEEN_MINUTES: Duration = Duration::minutes(15);
    pub const ONE_HOUR: Duration = Duration::hours(1);
    pub const TWO_HOURS: Duration = Duration::hours(2);
    pub const ONE_DAY: Duration = Duration::days(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::JobType;
    use tempfile::tempdir;

    #[test]
    fn test_schedule_immediate() {
        let dir = tempdir().unwrap();
        let store = Arc::new(JobStore::new(dir.path()).unwrap());
        let scheduler = Scheduler::new(store);

        let job = Job::new(JobType::Ocr, serde_json::json!({}), "ocr");
        let scheduled = scheduler
            .schedule(job.clone(), ScheduleSpec::Immediate)
            .unwrap();
        assert_eq!(scheduled.status, JobStatus::Scheduled);

        let due = scheduler.process_due_jobs().unwrap();
        assert!(!due.is_empty(), "Immediate jobs should be due immediately");
    }

    #[test]
    fn test_schedule_delayed_not_due() {
        let dir = tempdir().unwrap();
        let store = Arc::new(JobStore::new(dir.path()).unwrap());
        let scheduler = Scheduler::new(store);

        let job = Job::new(JobType::Whisper, serde_json::json!({}), "whisper");
        let future = Utc::now() + Duration::hours(1);
        scheduler.schedule(job, ScheduleSpec::At(future)).unwrap();

        let due = scheduler.process_due_jobs().unwrap();
        assert!(due.is_empty(), "Future jobs should not be due");
    }

    #[test]
    fn test_reschedule() {
        let dir = tempdir().unwrap();
        let store = Arc::new(JobStore::new(dir.path()).unwrap());
        let scheduler = Scheduler::new(store);

        let job = Job::new(JobType::Ocr, serde_json::json!({}), "ocr");
        let scheduled = scheduler.schedule(job, ScheduleSpec::Immediate).unwrap();
        let id = scheduled.id.to_string();

        let future = Utc::now() + Duration::hours(2);
        scheduler.reschedule(&id, ScheduleSpec::At(future)).unwrap();

        let due = scheduler.process_due_jobs().unwrap();
        assert!(due.is_empty(), "Rescheduled future job should not be due");
    }
}
