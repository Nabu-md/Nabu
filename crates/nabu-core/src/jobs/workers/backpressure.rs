use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Backpressure configuration and tracking for the worker pool.
///
/// The backpressure system prevents unbounded memory growth by:
/// 1. **Bounded channel capacity**: The worker channel has a fixed capacity.
///    When it's full, dispatch blocks, naturally slowing the enqueue path.
/// 2. **Configurable max pending jobs**: A high-water mark for jobs dispatched
///    to workers but not yet completed.
/// 3. **Soft/hard limits**: Backpressure warnings at the soft limit, rejections
///    at the hard limit.
///
/// The queue itself provides the primary backpressure mechanism — when the
/// worker channel is full, `dispatch()` returns `TrySendError::Full`, and
/// the job remains safely persisted in the DurableJobQueue storage. The
/// backpressure system here provides observability and fine-grained control.
#[derive(Debug, Clone)]
pub struct Backpressure {
    /// Soft limit — at this level, warnings are emitted.
    soft_limit: usize,

    /// Hard limit — at this level, new dispatches are rejected.
    hard_limit: usize,

    /// Current number of jobs dispatched to workers but not yet completed.
    pending_jobs: Arc<AtomicUsize>,

    /// Whether the system is currently in a backpressure state.
    pressured: Arc<AtomicUsize>, // 0 = normal, 1 = above soft, 2 = above hard
}

impl Backpressure {
    /// Creates a new backpressure controller.
    ///
    /// - `soft_limit`: Level at which warnings are triggered (e.g., 75% of capacity).
    /// - `hard_limit`: Level at which new dispatches are rejected (e.g., 100% of capacity).
    pub fn new(soft_limit: usize, hard_limit: usize) -> Self {
        Backpressure {
            soft_limit,
            hard_limit,
            pending_jobs: Arc::new(AtomicUsize::new(0)),
            pressured: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Creates a backpressure controller with sensible defaults based on channel capacity.
    ///
    /// Soft limit = 75% of capacity, hard limit = 100% of capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let soft = (capacity as f64 * 0.75).ceil() as usize;
        Backpressure {
            soft_limit: soft.max(1),
            hard_limit: capacity,
            pending_jobs: Arc::new(AtomicUsize::new(0)),
            pressured: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Records that a job has been dispatched to a worker.
    /// Returns `Ok(())` if within limits, or `Err` if the hard limit is reached.
    pub fn record_dispatch(&self) -> Result<(), usize> {
        let current = self.pending_jobs.fetch_add(1, Ordering::SeqCst) + 1;
        self.update_state(current);
        if current > self.hard_limit {
            Err(current)
        } else {
            Ok(())
        }
    }

    /// Records that a job has completed (or failed or been cancelled).
    pub fn record_completion(&self) {
        let current = self.pending_jobs.fetch_sub(1, Ordering::SeqCst).saturating_sub(1);
        self.update_state(current);
    }

    /// Returns the current number of pending (dispatched but not completed) jobs.
    pub fn pending_count(&self) -> usize {
        self.pending_jobs.load(Ordering::Relaxed)
    }

    /// Returns `true` if the hard limit has been reached.
    pub fn is_overloaded(&self) -> bool {
        self.pending_jobs.load(Ordering::Relaxed) > self.hard_limit
    }

    /// Returns `true` if we're above the soft limit but below the hard limit.
    pub fn is_warning(&self) -> bool {
        let current = self.pending_jobs.load(Ordering::Relaxed);
        current > self.soft_limit && current <= self.hard_limit
    }

    /// Returns `true` if we're within normal operating limits.
    pub fn is_normal(&self) -> bool {
        self.pending_jobs.load(Ordering::Relaxed) <= self.soft_limit
    }

    /// Returns a human-readable description of the current backpressure state.
    pub fn status(&self) -> BackpressureStatus {
        let current = self.pending_jobs.load(Ordering::Relaxed);
        if current > self.hard_limit {
            BackpressureStatus::Overloaded {
                pending: current,
                hard_limit: self.hard_limit,
            }
        } else if current > self.soft_limit {
            BackpressureStatus::Warning {
                pending: current,
                soft_limit: self.soft_limit,
                hard_limit: self.hard_limit,
            }
        } else {
            BackpressureStatus::Normal {
                pending: current,
                soft_limit: self.soft_limit,
            }
        }
    }

    /// Returns the soft limit.
    pub fn soft_limit(&self) -> usize {
        self.soft_limit
    }

    /// Returns the hard limit.
    pub fn hard_limit(&self) -> usize {
        self.hard_limit
    }

    fn update_state(&self, current: usize) {
        let state = if current > self.hard_limit {
            2
        } else if current > self.soft_limit {
            1
        } else {
            0
        };
        self.pressured.store(state, Ordering::Relaxed);
    }
}

/// The current state of backpressure.
#[derive(Debug, Clone, PartialEq)]
pub enum BackpressureStatus {
    /// Operating normally within the soft limit.
    Normal {
        /// Current number of pending jobs.
        pending: usize,
        /// The soft limit.
        soft_limit: usize,
    },
    /// Above the soft limit — warning state.
    Warning {
        /// Current number of pending jobs.
        pending: usize,
        /// The soft limit.
        soft_limit: usize,
        /// The hard limit.
        hard_limit: usize,
    },
    /// Above the hard limit — overloaded state.
    Overloaded {
        /// Current number of pending jobs.
        pending: usize,
        /// The hard limit.
        hard_limit: usize,
    },
}

impl std::fmt::Display for BackpressureStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackpressureStatus::Normal { pending, soft_limit } => {
                write!(f, "normal ({} pending, limit {})", pending, soft_limit)
            }
            BackpressureStatus::Warning { pending, soft_limit, hard_limit } => {
                write!(f, "warning ({} pending, soft {}, hard {})", pending, soft_limit, hard_limit)
            }
            BackpressureStatus::Overloaded { pending, hard_limit } => {
                write!(f, "OVERLOADED ({} pending, hard limit {})", pending, hard_limit)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_normal() {
        let bp = Backpressure::with_capacity(10);
        assert!(bp.is_normal());
        assert!(!bp.is_warning());
        assert!(!bp.is_overloaded());
    }

    #[test]
    fn test_backpressure_warning() {
        let bp = Backpressure::new(3, 10);
        for _ in 0..5 {
            bp.record_dispatch().unwrap();
        }
        assert!(!bp.is_normal());
        assert!(bp.is_warning());
        assert!(!bp.is_overloaded());
    }

    #[test]
    fn test_backpressure_overloaded() {
        let bp = Backpressure::new(3, 10);
        for _ in 0..12 {
            let result = bp.record_dispatch();
            if 12 > 10 {
                assert!(result.is_err());
            }
        }
        assert!(bp.is_overloaded());
    }

    #[test]
    fn test_backpressure_completion_reduces_count() {
        let bp = Backpressure::with_capacity(10);
        bp.record_dispatch().unwrap();
        bp.record_dispatch().unwrap();
        assert_eq!(bp.pending_count(), 2);

        bp.record_completion();
        assert_eq!(bp.pending_count(), 1);

        bp.record_completion();
        assert_eq!(bp.pending_count(), 0);
    }

    #[test]
    fn test_backpressure_status_display() {
        let bp = Backpressure::with_capacity(10);
        let status = bp.status();
        match status {
            BackpressureStatus::Normal { .. } => {} // expected
            _ => panic!("expected normal status"),
        }
    }
}
