use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

const PROGRESS_DENOM: u32 = 1000;

/// A progress reporter that can be cloned and shared across threads.
/// Jobs report progress as a value between 0 and 1000 (permille).
#[derive(Clone)]
pub struct ProgressReporter {
    permille: Arc<AtomicU32>,
    on_update: Arc<dyn Fn(f64) + Send + Sync>,
}

impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressReporter")
            .field("permille", &self.permille)
            .field("progress", &self.progress())
            .finish()
    }
}

impl ProgressReporter {
    /// Create a new progress reporter with a callback for updates.
    pub fn new(on_update: impl Fn(f64) + Send + Sync + 'static) -> Self {
        Self {
            permille: Arc::new(AtomicU32::new(0)),
            on_update: Arc::new(on_update),
        }
    }

    /// Create a no-op progress reporter (for jobs that don't report progress).
    pub fn noop() -> Self {
        Self {
            permille: Arc::new(AtomicU32::new(0)),
            on_update: Arc::new(|_| {}),
        }
    }

    /// Get the current progress as a float (0.0–1.0).
    pub fn progress(&self) -> f64 {
        self.permille.load(Ordering::Acquire) as f64 / PROGRESS_DENOM as f64
    }

    /// Set progress from a float (0.0–1.0).
    pub fn set_progress(&self, value: f64) {
        let clamped = value.clamp(0.0, 1.0);
        let permille = (clamped * PROGRESS_DENOM as f64) as u32;
        self.permille
            .store(permille.min(PROGRESS_DENOM), Ordering::Release);
        (self.on_update)(clamped);
    }

    /// Set progress as a percentage (0–100).
    pub fn set_percent(&self, percent: f64) {
        self.set_progress(percent / 100.0);
    }

    /// Increment progress by a delta (0.0–1.0).
    pub fn increment(&self, delta: f64) {
        let current = self.progress();
        self.set_progress(current + delta);
    }

    /// Mark progress as complete (1.0).
    pub fn complete(&self) {
        self.set_progress(1.0);
    }

    /// Reset progress to 0.
    pub fn reset(&self) {
        self.set_progress(0.0);
    }
}

/// A progress tracker that stores progress history.
pub struct InMemoryProgressTracker {
    updates: Vec<ProgressUpdate>,
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub progress: f64,
    pub message: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for InMemoryProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryProgressTracker {
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
        }
    }

    pub fn record(&mut self, progress: f64, message: Option<String>) {
        self.updates.push(ProgressUpdate {
            progress,
            message,
            timestamp: chrono::Utc::now(),
        });
    }

    pub fn updates(&self) -> &[ProgressUpdate] {
        &self.updates
    }

    pub fn last_progress(&self) -> Option<f64> {
        self.updates.last().map(|u| u.progress)
    }

    pub fn clear(&mut self) {
        self.updates.clear();
    }
}

/// Progress threshold for throttling updates.
/// Ensures we don't emit too many progress events.
pub struct ProgressThrottle {
    last_reported: Arc<AtomicU32>,
    threshold_permille: u32,
}

impl ProgressThrottle {
    pub fn new(threshold_percent: f64) -> Self {
        Self {
            last_reported: Arc::new(AtomicU32::new(0)),
            threshold_permille: (threshold_percent / 100.0 * PROGRESS_DENOM as f64) as u32,
        }
    }

    /// Check if the current progress should be reported based on the threshold.
    pub fn should_report(&self, progress: f64) -> bool {
        let current_permille = (progress * PROGRESS_DENOM as f64) as u32;
        let last = self.last_reported.load(Ordering::Acquire);
        current_permille >= last + self.threshold_permille
    }

    /// Mark progress as reported.
    pub fn mark_reported(&self, progress: f64) {
        let permille = (progress * PROGRESS_DENOM as f64) as u32;
        self.last_reported.store(permille, Ordering::Release);
    }
}

impl Default for ProgressThrottle {
    fn default() -> Self {
        Self::new(5.0) // 5% threshold by default
    }
}
