use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cooperative cancellation token for jobs.
/// Thread-safe, cloneable, and lightweight.
/// Does not force-cancel — the job must check `is_cancelled()` and stop.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Reset the cancellation token (for reuse).
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// A pair of (CancellationToken, CancellationGuard).
/// The CancellationToken is the handle for external cancellation requests.
/// The CancellationGuard auto-cancels when dropped if the token was cancelled.
pub struct CancellationScope {
    token: CancellationToken,
    guard: CancellationGuard,
}

impl CancellationScope {
    pub fn new() -> Self {
        let token = CancellationToken::new();
        let guard = CancellationGuard {
            token: token.clone(),
            was_cancelled: false,
        };
        Self { token, guard }
    }

    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    pub fn guard(&mut self) -> &mut CancellationGuard {
        &mut self.guard
    }
}

/// Guards cancellation scope — tracks whether cancellation occurred.
pub struct CancellationGuard {
    token: CancellationToken,
    was_cancelled: bool,
}

impl CancellationGuard {
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    pub fn mark_cancelled(&mut self) {
        self.was_cancelled = true;
    }
}

impl Default for CancellationScope {
    fn default() -> Self {
        Self::new()
    }
}
