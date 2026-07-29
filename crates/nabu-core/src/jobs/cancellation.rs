use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cooperative cancellation token shared between the queue and a running job.
///
/// When a job is cancelled, this token is flagged. The job should periodically
/// check `is_cancelled()` during execution and clean up if true.
///
/// This enables safe, cooperative cancellation without forcefully aborting
/// threads or corrupting state.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new, un-cancelled cancellation token.
    pub fn new() -> Self {
        CancellationToken {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Creates a new, pre-cancelled cancellation token.
    /// Useful for testing cancellation paths.
    pub fn cancelled() -> Self {
        CancellationToken {
            cancelled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Returns `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Requests cancellation. Once called, `is_cancelled()` returns `true`.
    /// This is safe to call from any thread.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Resets the token to an un-cancelled state.
    /// Used internally when a cancelled job is retried.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_starts_uncancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_token_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_token_reset() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_pre_cancelled() {
        let token = CancellationToken::cancelled();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_clone_shares_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn test_thread_safety() {
        let token = Arc::new(CancellationToken::new());
        let token2 = token.clone();

        let handle = std::thread::spawn(move || {
            assert!(!token2.is_cancelled());
            token2.cancel();
            assert!(token2.is_cancelled());
        });

        assert!(!token.is_cancelled());
        handle.join().unwrap();
        assert!(token.is_cancelled());
    }
}
