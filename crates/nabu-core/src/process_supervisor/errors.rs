//! Structured errors for the process supervision layer.
//!
//! [`ProcessSupervisorError`] covers all failure categories called out by
//! the platform specification:
//!
//! - Spawn failures (invalid command, missing executable)
//! - State transition errors (invalid transition)
//! - Process lookup errors (not found)
//! - Shutdown errors (already shutting down)
//!
//! All methods return `Result<_, ProcessSupervisorError>` rather than
//! panicking.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::state::ProcessState;
use super::ProcessId;

/// Errors that can occur during process supervision.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessSupervisorError {
    /// The specified process ID was not found in the supervisor's registry.
    #[error("process not found: {0}")]
    NotFound(ProcessId),

    /// The process could not be spawned (invalid command, missing executable,
    /// insufficient permissions, etc.).
    #[error("spawn failed: {0}")]
    SpawnFailed(String),

    /// The process is not in a state that allows this operation
    /// (e.g. trying to stop a process that is `Stopped`).
    #[error("process is not running (state: {state:?})")]
    NotRunning {
        /// The current state of the process.
        state: ProcessState,
    },

    /// An invalid state transition was attempted.
    #[error("invalid state transition: {from:?} → {to:?}")]
    InvalidStateTransition {
        /// The current state.
        from: ProcessState,
        /// The attempted target state.
        to: ProcessState,
    },

    /// The supervisor is shutting down and cannot accept new operations.
    #[error("supervisor is shutting down")]
    ShuttingDown,

    /// The process is already being managed (duplicate ID).
    #[error("process with ID {0} is already managed")]
    AlreadyManaged(ProcessId),

    /// No tokio runtime is available to spawn monitoring tasks.
    ///
    /// The supervisor requires a tokio runtime context for spawning and
    /// monitoring subprocesses. Ensure `spawn()` is called from within a
    /// tokio runtime.
    #[error("no tokio runtime available — call from within a tokio runtime context")]
    NoRuntime,

    /// The platform does not support subprocess spawning.
    #[error("subprocess management is not supported on this platform")]
    UnsupportedPlatform,

    /// An unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for process supervisor operations.
pub type ProcessResult<T> = Result<T, ProcessSupervisorError>;

impl ProcessSupervisorError {
    /// Returns `true` if this error is retryable (i.e. the operation
    /// can be retried without user intervention).
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::SpawnFailed(_) | Self::NoRuntime => true,
            Self::NotFound(_)
            | Self::NotRunning { .. }
            | Self::InvalidStateTransition { .. }
            | Self::ShuttingDown
            | Self::AlreadyManaged(_)
            | Self::UnsupportedPlatform
            | Self::Internal(_) => false,
        }
    }

    /// Returns `true` if this error indicates the process was not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_supervisor::state::ProcessState;

    #[test]
    fn error_display_formats() {
        let id = ProcessId::nil();
        assert!(format!("{}", ProcessSupervisorError::NotFound(id)).contains("not found"));
        assert!(format!("{}", ProcessSupervisorError::SpawnFailed("bad cmd".into()))
            .contains("spawn failed"));
        assert!(format!(
            "{}",
            ProcessSupervisorError::NotRunning {
                state: ProcessState::Stopped
            }
        )
        .contains("not running"));
        assert!(format!(
            "{}",
            ProcessSupervisorError::ShuttingDown
        )
        .contains("shutting down"));
        assert!(format!(
            "{}",
            ProcessSupervisorError::NoRuntime
        )
        .contains("no tokio runtime"));
        assert!(format!(
            "{}",
            ProcessSupervisorError::UnsupportedPlatform
        )
        .contains("not supported"));
    }

    #[test]
    fn error_implements_error_trait() {
        let err = ProcessSupervisorError::NotFound(ProcessId::nil());
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn spawn_failed_is_retryable() {
        assert!(ProcessSupervisorError::SpawnFailed("test".into()).is_retryable());
    }

    #[test]
    fn not_found_not_retryable() {
        assert!(!ProcessSupervisorError::NotFound(ProcessId::nil()).is_retryable());
    }

    #[test]
    fn shutting_down_not_retryable() {
        assert!(!ProcessSupervisorError::ShuttingDown.is_retryable());
    }

    #[test]
    fn no_runtime_is_retryable() {
        assert!(ProcessSupervisorError::NoRuntime.is_retryable());
    }

    #[test]
    fn unsupported_platform_not_retryable() {
        assert!(!ProcessSupervisorError::UnsupportedPlatform.is_retryable());
    }

    #[test]
    fn error_serializes() {
        let err = ProcessSupervisorError::NotFound(ProcessId::nil());
        let json = serde_json::to_string(&err).unwrap();
        let back: ProcessSupervisorError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }
}
