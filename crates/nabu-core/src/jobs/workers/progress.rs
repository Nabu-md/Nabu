use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::jobs::job::JobId;
use crate::jobs::worker_channel::WorkerHandle;

/// A snapshot of a job's progress at a point in time.
#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    /// The job this progress relates to.
    pub job_id: JobId,

    /// Progress value from 0.0 to 1.0.
    pub progress: f64,

    /// Human-readable description of the current progress.
    pub message: String,

    /// Unix timestamp when this snapshot was recorded.
    pub timestamp: f64,
}

/// The progress reporter interface.
///
/// Workers use this to report progress without knowing anything about
/// how progress is tracked or delivered.
pub trait ProgressReporter: Send + Sync {
    /// Reports progress for a job.
    fn report(&self, job_id: JobId, progress: f64, message: String);
}

/// A progress reporter that sends progress updates through the worker channel.
///
/// This is the primary implementation used in production — progress updates
/// flow back to the queue/application through the existing IPC channel.
#[derive(Debug, Clone)]
pub struct ChannelProgressReporter {
    /// Handle to the worker channel for sending progress updates.
    handle: Arc<tokio::sync::Mutex<Option<WorkerHandle>>>,

    /// How often (in milliseconds) to throttle progress updates.
    /// Default: 250ms (4 updates per second max).
    throttle_ms: u64,
}

impl ChannelProgressReporter {
    /// Creates a new channel progress reporter with the given throttle interval.
    pub fn new(throttle_ms: u64) -> Self {
        ChannelProgressReporter {
            handle: Arc::new(tokio::sync::Mutex::new(None)),
            throttle_ms,
        }
    }

    /// Sets the worker handle for sending updates.
    pub async fn set_handle(&self, handle: WorkerHandle) {
        let mut h = self.handle.lock().await;
        *h = Some(handle);
    }

    /// Returns the throttle interval in milliseconds.
    pub fn throttle_ms(&self) -> u64 {
        self.throttle_ms
    }
}

impl ProgressReporter for ChannelProgressReporter {
    fn report(&self, job_id: JobId, progress: f64, message: String) {
        let handle = self.handle.clone();
        let msg = message.clone();
        tokio::spawn(async move {
            let h = handle.lock().await;
            if let Some(ref h) = *h {
                let _ = h.report_progress(job_id, progress, msg).await;
            }
        });
    }
}

/// An in-memory progress tracker that records the latest progress for each job.
///
/// Useful for testing and diagnostics without a worker channel.
#[derive(Debug, Clone)]
pub struct InMemoryProgressTracker {
    inner: Arc<std::sync::Mutex<std::collections::HashMap<String, ProgressSnapshot>>>,
}

impl InMemoryProgressTracker {
    /// Creates a new in-memory progress tracker.
    pub fn new() -> Self {
        InMemoryProgressTracker {
            inner: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Returns the latest progress snapshot for a job, if any.
    pub fn get_progress(&self, job_id: &JobId) -> Option<ProgressSnapshot> {
        let map = self.inner.lock().unwrap();
        map.get(&job_id.to_string()).cloned()
    }

    /// Returns all recorded progress snapshots.
    pub fn all_progress(&self) -> Vec<ProgressSnapshot> {
        let map = self.inner.lock().unwrap();
        map.values().cloned().collect()
    }

    /// Clears all recorded progress.
    pub fn clear(&self) {
        let mut map = self.inner.lock().unwrap();
        map.clear();
    }
}

impl ProgressReporter for InMemoryProgressTracker {
    fn report(&self, job_id: JobId, progress: f64, message: String) {
        let snapshot = ProgressSnapshot {
            job_id,
            progress: progress.clamp(0.0, 1.0),
            message,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };
        let mut map = self.inner.lock().unwrap();
        map.insert(job_id.to_string(), snapshot);
    }
}

impl Default for InMemoryProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Config for progress reporting.
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// How often (in milliseconds) to throttle progress updates.
    /// 0 means no throttling (every update is sent).
    pub throttle_ms: u64,

    /// Whether to include timestamps in progress reports.
    pub include_timestamps: bool,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        ProgressConfig {
            throttle_ms: 250,
            include_timestamps: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobPayload;

    #[test]
    fn test_in_memory_tracker() {
        let tracker = InMemoryProgressTracker::new();
        let job_id = JobId::new();

        tracker.report(job_id, 0.5, "halfway".into());

        let snapshot = tracker.get_progress(&job_id).unwrap();
        assert!((snapshot.progress - 0.5).abs() < f64::EPSILON);
        assert_eq!(snapshot.message, "halfway");
    }

    #[test]
    fn test_progress_clamped() {
        let tracker = InMemoryProgressTracker::new();
        let job_id = JobId::new();

        tracker.report(job_id, 1.5, "over".into());
        let snapshot = tracker.get_progress(&job_id).unwrap();
        assert!((snapshot.progress - 1.0).abs() < f64::EPSILON);

        tracker.report(job_id, -0.5, "under".into());
        let snapshot = tracker.get_progress(&job_id).unwrap();
        assert!((snapshot.progress - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_progress() {
        let tracker = InMemoryProgressTracker::new();
        let id1 = JobId::new();
        let id2 = JobId::new();

        tracker.report(id1, 0.3, "first".into());
        tracker.report(id2, 0.7, "second".into());

        let all = tracker.all_progress();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_clear() {
        let tracker = InMemoryProgressTracker::new();
        tracker.report(JobId::new(), 1.0, "done".into());
        assert_eq!(tracker.all_progress().len(), 1);
        tracker.clear();
        assert_eq!(tracker.all_progress().len(), 0);
    }
}
