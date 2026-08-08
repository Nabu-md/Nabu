//! # Agent Manager Errors
//!
//! Structured error types for the [`AgentManager`](super::AgentManager).
//!
//! All errors are returned as [`AgentManagerError`] — no panics. Every
//! variant maps to a specific failure category with context for logging
//! and IPC responses.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::process_supervisor::{ProcessState, ProcessSupervisorError};use crate::registry::lifecycle::LifecycleStage;

/// Errors that can occur during agent process management.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentManagerError {
    /// The agent with the given name was not found in the registry.
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    /// An agent with the given name is already registered.
    #[error("agent already registered: {0}")]
    AlreadyRegistered(String),

    /// The underlying process supervisor returned an error.
    #[error("process supervisor error: {0}")]
    Supervisor(#[from] ProcessSupervisorError),

    /// The agent manager is not in a state that allows this operation.
    #[error("agent manager is not in the correct lifecycle stage (current: {current:?}, required: {required:?})")]
    NotReady {
        /// The current lifecycle stage.
        current: LifecycleStage,
        /// The minimum stage required for this operation.
        required: LifecycleStage,
    },

    /// The executable for the agent was not found or is not executable.
    #[error("executable not found or not executable: {0}")]
    ExecutableNotFound(String),

    /// The agent's working directory does not exist.
    #[error("working directory does not exist: {0}")]
    WorkingDirectoryNotFound(String),

    /// The agent is in a process state that does not allow this operation.
    #[error("agent '{name}' is not in a valid state (current: {state:?})")]
    InvalidProcessState {
        /// The agent name.
        name: String,
        /// The current process state.
    #[allow(private_interfaces)]
    state: ProcessState,
    },

    /// A restart was requested but the agent is already running or in a
    /// state where restart is not applicable.
    #[error("restart not applicable for agent '{name}' (state: {state:?})")]
    RestartNotApplicable {
        /// The agent name.
        name: String,
        /// The current process state.
    #[allow(private_interfaces)]
    state: ProcessState,
    },

    /// The agent manager is shutting down and cannot accept new operations.
    #[error("agent manager is shutting down")]
    ShuttingDown,

    /// No tokio runtime is available.
    #[error("no tokio runtime available — call from within a tokio runtime context")]
    NoRuntime,

    /// An unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// The agent configuration is invalid.
    #[error("invalid agent configuration: {0}")]
    InvalidConfig(String),
}

impl AgentManagerError {
    /// Returns `true` if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::AgentNotFound(_)
            | Self::AlreadyRegistered(_)
            | Self::NotReady { .. }
            | Self::InvalidProcessState { .. }
            | Self::RestartNotApplicable { .. }
            | Self::ShuttingDown
            | Self::InvalidConfig(_)
            | Self::Internal(_) => false,
            Self::Supervisor(e) => e.is_retryable(),
            Self::ExecutableNotFound(_) => true,
            Self::WorkingDirectoryNotFound(_) => true,
            Self::NoRuntime => true,
        }
    }

    /// Returns `true` if this error indicates the agent was not found.
    pub fn is_agent_not_found(&self) -> bool {
        matches!(self, Self::AgentNotFound(_))
    }
}

/// Result type for agent manager operations.
pub type AgentResult<T> = Result<T, AgentManagerError>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_supervisor::ProcessId;

    #[test]
    fn error_display_formats() {
        let err = AgentManagerError::AgentNotFound("test".to_string());
        assert!(format!("{}", err).contains("agent not found"));

        let err = AgentManagerError::AlreadyRegistered("test".to_string());
        assert!(format!("{}", err).contains("already registered"));

        let err = AgentManagerError::ShuttingDown;
        assert!(format!("{}", err).contains("shutting down"));

        let err = AgentManagerError::NoRuntime;
        assert!(format!("{}", err).contains("no tokio runtime"));

        let err = AgentManagerError::ExecutableNotFound("/bin/foo".to_string());
        assert!(format!("{}", err).contains("executable not found"));
    }

    #[test]
    fn error_implements_std_error() {
        let err = AgentManagerError::AgentNotFound("test".to_string());
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn not_ready_is_not_retryable() {
        let err = AgentManagerError::NotReady {
            current: LifecycleStage::Created,
            required: LifecycleStage::Running,
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn already_registered_is_not_retryable() {
        let err = AgentManagerError::AlreadyRegistered("test".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn executable_not_found_is_retryable() {
        let err = AgentManagerError::ExecutableNotFound("/bin/foo".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn no_runtime_is_retryable() {
        let err = AgentManagerError::NoRuntime;
        assert!(err.is_retryable());
    }

    #[test]
    fn supervisor_error_delegates_retryability() {
        let err = AgentManagerError::Supervisor(ProcessSupervisorError::NoRuntime);
        assert!(err.is_retryable());

        let err = AgentManagerError::Supervisor(ProcessSupervisorError::NotFound(ProcessId::nil()));
        assert!(!err.is_retryable());
    }

    #[test]
    fn agent_not_found_check() {
        let err = AgentManagerError::AgentNotFound("test".to_string());
        assert!(err.is_agent_not_found());

        let err = AgentManagerError::ShuttingDown;
        assert!(!err.is_agent_not_found());
    }

    #[test]
    fn error_serializes() {
        let err = AgentManagerError::AgentNotFound("test".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let back: AgentManagerError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn fmt_debug() {
        let err = AgentManagerError::AgentNotFound("test".to_string());
        let s = format!("{:?}", err);
        assert!(s.contains("AgentNotFound"));
        assert!(s.contains("test"));
    }
}
