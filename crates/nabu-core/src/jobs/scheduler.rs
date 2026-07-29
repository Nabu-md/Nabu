use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Specifies when a job should be scheduled for execution.
///
/// This is the user-facing scheduling API used when enqueueing jobs.
/// The scheduler internally converts this to an absolute `DateTime<Utc>`
/// stored in the job's `scheduled_at` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScheduleSpec {
    /// Schedule at an absolute time.
    At(DateTime<Utc>),

    /// Schedule after a duration from now.
    After(Duration),

    /// Schedule immediately (or as soon as possible).
    Immediate,
}

impl ScheduleSpec {
    /// Resolves this spec to an absolute `DateTime<Utc>`.
    pub fn resolve(&self) -> DateTime<Utc> {
        match self {
            ScheduleSpec::At(dt) => *dt,
            ScheduleSpec::After(dur) => Utc::now() + *dur,
            ScheduleSpec::Immediate => Utc::now(),
        }
    }
}

impl From<DateTime<Utc>> for ScheduleSpec {
    fn from(dt: DateTime<Utc>) -> Self {
        ScheduleSpec::At(dt)
    }
}

impl From<Duration> for ScheduleSpec {
    fn from(dur: Duration) -> Self {
        ScheduleSpec::After(dur)
    }
}

/// A simple scheduler for determining when jobs are due for execution.
///
/// This is a lightweight utility that checks whether a job's `scheduled_at`
/// timestamp has passed. It does **not** manage a separate timer/clock thread —
/// that responsibility belongs to the `WorkerPool` in Prompt 36.
#[derive(Debug, Clone)]
pub struct Scheduler {
    /// The current time source (can be overridden for testing).
    now: fn() -> DateTime<Utc>,
}

impl Scheduler {
    /// Creates a new scheduler using the real system clock.
    pub fn new() -> Self {
        Scheduler { now: Utc::now }
    }

    /// Creates a scheduler with a custom time source (for testing).
    pub fn with_clock(clock: fn() -> DateTime<Utc>) -> Self {
        Scheduler { now: clock }
    }

    /// Returns `true` if the job is due for execution based on its scheduled time.
    pub fn is_due(&self, scheduled_at: &DateTime<Utc>) -> bool {
        *scheduled_at <= (self.now)()
    }

    /// Calculates how long until the job becomes due.
    /// Returns `Duration::zero()` if the job is already due or past due.
    pub fn time_until_due(&self, scheduled_at: &DateTime<Utc>) -> Duration {
        let now = (self.now)();
        if *scheduled_at <= now {
            Duration::zero()
        } else {
            *scheduled_at - now
        }
    }

    /// Returns all ready jobs from a list (those whose scheduled time has passed).
    pub fn filter_ready<'a>(&self, jobs: &'a [crate::jobs::Job]) -> Vec<&'a crate::jobs::Job> {
        jobs.iter().filter(|j| self.is_due(&j.scheduled_at)).collect()
    }

    /// Partitions jobs into those ready now and those scheduled for the future.
    pub fn partition<'a>(
        &self,
        jobs: &'a [crate::jobs::Job],
    ) -> (Vec<&'a crate::jobs::Job>, Vec<&'a crate::jobs::Job>) {
        jobs.iter().partition(|j| self.is_due(&j.scheduled_at))
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::{Job, JobPayload};
    use crate::jobs::priority::Priority;

    #[test]
    fn test_schedule_spec_at() {
        let future = Utc::now() + Duration::hours(1);
        let spec = ScheduleSpec::At(future);
        assert_eq!(spec.resolve(), future);
    }

    #[test]
    fn test_schedule_spec_after() {
        let dur = Duration::seconds(30);
        let spec = ScheduleSpec::After(dur);
        let resolved = spec.resolve();
        let diff = resolved - Utc::now();
        assert!(diff.num_seconds() >= 28 && diff.num_seconds() <= 32);
    }

    #[test]
    fn test_schedule_spec_immediate() {
        let spec = ScheduleSpec::Immediate;
        let resolved = spec.resolve();
        let diff = (resolved - Utc::now()).num_seconds().abs();
        assert!(diff <= 1);
    }

    #[test]
    fn test_is_due() {
        let scheduler = Scheduler::new();
        let past = Utc::now() - Duration::minutes(5);
        let future = Utc::now() + Duration::hours(1);

        assert!(scheduler.is_due(&past));
        assert!(!scheduler.is_due(&future));
    }

    #[test]
    fn test_time_until_due() {
        let scheduler = Scheduler::new();

        let past = Utc::now() - Duration::minutes(5);
        assert_eq!(scheduler.time_until_due(&past), Duration::zero());

        let future = Utc::now() + Duration::seconds(30);
        let remaining = scheduler.time_until_due(&future);
        assert!(remaining.num_seconds() >= 28 && remaining.num_seconds() <= 32);
    }

    #[test]
    fn test_filter_ready() {
        let scheduler = Scheduler::new();

        let jobs = vec![
            Job::scheduled("past", JobPayload::new(), Utc::now() - Duration::minutes(5)),
            Job::scheduled("future", JobPayload::new(), Utc::now() + Duration::hours(1)),
            Job::new("immediate", JobPayload::new()),
        ];

        let ready = scheduler.filter_ready(&jobs);
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].job_type.0, "past");
        assert_eq!(ready[1].job_type.0, "immediate");
    }

    #[test]
    fn test_partition() {
        let scheduler = Scheduler::new();

        let jobs = vec![
            Job::scheduled("past", JobPayload::new(), Utc::now() - Duration::minutes(5)),
            Job::scheduled("future", JobPayload::new(), Utc::now() + Duration::hours(1)),
        ];

        let (ready, later) = scheduler.partition(&jobs);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].job_type.0, "past");
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].job_type.0, "future");
    }
}
