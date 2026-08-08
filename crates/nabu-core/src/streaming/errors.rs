//! Structured errors for the streaming pipeline.
//!
//! All errors are returned as [`StreamManagerError`] — no panics.
//! Every variant maps to a specific failure category with context for
//! logging, IPC responses, and structured error handling.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::event_bus::StreamId;

/// Errors that can occur during streaming operations.
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamManagerError {
    /// The stream with the given ID was not found.
    #[error("stream not found: {0}")]
    StreamNotFound(StreamId),

    /// The stream is already in a terminal state (Completed, Cancelled, or
    /// Failed) and cannot accept further operations.
    #[error("stream '{stream_id}' is in terminal state ({state:?})")]
    StreamAlreadyTerminal {
        /// The stream ID that had the terminal state.
        stream_id: StreamId,
        /// The state the stream was in.
        state: crate::event_bus::StreamState,
    },

    /// The EventBus is not available (was not provided during construction).
    #[error("EventBus is not available")]
    NoEventBus,

    /// An EventBus publish operation failed (e.g. no subscribers).
    ///
    /// This is currently non-fatal — the stream continues but the subscriber
    /// did not receive the event. The error is surfaced so callers can decide
    /// whether to fail the stream or log a warning.
    #[error("EventBus publish failed: {message}")]
    EventBusPublish {
        /// Human-readable description of the failure.
        message: String,
    },

    /// The stream was cancelled and this operation is not permitted on a
    /// cancelled stream.
    #[error("stream '{stream_id}' was cancelled and cannot accept new tokens")]
    Cancelled {
        /// The stream ID that was cancelled.
        stream_id: StreamId,
    },

    /// The stream failed and this operation is not permitted.
    #[error("stream '{stream_id}' failed and cannot accept new tokens")]
    Failed {
        /// The stream ID that failed.
        stream_id: StreamId,
    },

    /// The streaming manager has been shut down.
    #[error("streaming manager is shut down")]
    ShuttingDown,

    /// No tokio runtime is available.
    #[error("no tokio runtime available")]
    NoRuntime,

    /// The stream state transition was invalid.
    #[error("invalid stream state transition: {from:?} -> {to:?}")]
    InvalidStateTransition {
        /// The current state.
        from: crate::event_bus::StreamState,
        /// The attempted target state.
        to: crate::event_bus::StreamState,
    },

    /// An unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl StreamManagerError {
    /// Returns `true` if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::StreamNotFound(_)
            | Self::StreamAlreadyTerminal { .. }
            | Self::Cancelled { .. }
            | Self::Failed { .. }
            | Self::ShuttingDown
            | Self::InvalidStateTransition { .. }
            | Self::NoEventBus
            | Self::Internal(_) => false,
            Self::EventBusPublish { .. } | Self::NoRuntime => true,
        }
    }

    /// Returns `true` if this error indicates the stream was not found.
    pub fn is_stream_not_found(&self) -> bool {
        matches!(self, Self::StreamNotFound(_))
    }

    /// Returns `true` if this error indicates the stream is in a terminal state.
    pub fn is_stream_terminal(&self) -> bool {
        matches!(
            self,
            Self::StreamAlreadyTerminal { .. } | Self::Cancelled { .. } | Self::Failed { .. }
        )
    }
}

/// Result type for streaming operations.
pub type StreamResult<T> = Result<T, StreamManagerError>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::StreamState;
    use uuid::Uuid;

    fn test_stream_id() -> StreamId {
        Uuid::nil()
    }

    #[test]
    fn stream_not_found_display() {
        let err = StreamManagerError::StreamNotFound(test_stream_id());
        assert!(format!("{}", err).contains("stream not found"));
    }

    #[test]
    fn stream_already_terminal_display() {
        let err = StreamManagerError::StreamAlreadyTerminal {
            stream_id: test_stream_id(),
            state: StreamState::Completed,
        };
        assert!(format!("{}", err).contains("terminal state"));
    }

    #[test]
    fn cancelled_display() {
        let err = StreamManagerError::Cancelled {
            stream_id: test_stream_id(),
        };
        assert!(format!("{}", err).contains("cancelled"));
    }

    #[test]
    fn failed_display() {
        let err = StreamManagerError::Failed {
            stream_id: test_stream_id(),
        };
        assert!(format!("{}", err).contains("failed"));
    }

    #[test]
    fn shutting_down_display() {
        let err = StreamManagerError::ShuttingDown;
        assert!(format!("{}", err).contains("shut down"));
    }

    #[test]
    fn invalid_state_transition_display() {
        let err = StreamManagerError::InvalidStateTransition {
            from: StreamState::Active,
            to: StreamState::Active,
        };
        assert!(format!("{}", err).contains("invalid stream state transition"));
    }

    #[test]
    fn internal_display() {
        let err = StreamManagerError::Internal("something broke".to_string());
        assert!(format!("{}", err).contains("something broke"));
    }

    #[test]
    fn stream_not_found_is_not_retryable() {
        let err = StreamManagerError::StreamNotFound(test_stream_id());
        assert!(!err.is_retryable());
    }

    #[test]
    fn stream_already_terminal_is_not_retryable() {
        let err = StreamManagerError::StreamAlreadyTerminal {
            stream_id: test_stream_id(),
            state: StreamState::Completed,
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn cancelled_is_not_retryable() {
        let err = StreamManagerError::Cancelled {
            stream_id: test_stream_id(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn failed_is_not_retryable() {
        let err = StreamManagerError::Failed {
            stream_id: test_stream_id(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn shutting_down_is_not_retryable() {
        let err = StreamManagerError::ShuttingDown;
        assert!(!err.is_retryable());
    }

    #[test]
    fn no_event_bus_is_not_retryable() {
        let err = StreamManagerError::NoEventBus;
        assert!(!err.is_retryable());
    }

    #[test]
    fn event_bus_publish_is_retryable() {
        let err = StreamManagerError::EventBusPublish {
            message: "no subscribers".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn no_runtime_is_retryable() {
        let err = StreamManagerError::NoRuntime;
        assert!(err.is_retryable());
    }

    #[test]
    fn invalid_state_transition_is_not_retryable() {
        let err = StreamManagerError::InvalidStateTransition {
            from: StreamState::Active,
            to: StreamState::Active,
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn internal_is_not_retryable() {
        let err = StreamManagerError::Internal("test".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn stream_not_found_check() {
        let err = StreamManagerError::StreamNotFound(test_stream_id());
        assert!(err.is_stream_not_found());

        let err = StreamManagerError::ShuttingDown;
        assert!(!err.is_stream_not_found());
    }

    #[test]
    fn stream_terminal_check() {
        let err = StreamManagerError::StreamAlreadyTerminal {
            stream_id: test_stream_id(),
            state: StreamState::Completed,
        };
        assert!(err.is_stream_terminal());

        let err = StreamManagerError::Cancelled {
            stream_id: test_stream_id(),
        };
        assert!(err.is_stream_terminal());

        let err = StreamManagerError::Failed {
            stream_id: test_stream_id(),
        };
        assert!(err.is_stream_terminal());

        let err = StreamManagerError::StreamNotFound(test_stream_id());
        assert!(!err.is_stream_terminal());
    }

    #[test]
    fn error_serializes() {
        let err = StreamManagerError::StreamNotFound(test_stream_id());
        let json = serde_json::to_string(&err).unwrap();
        let back: StreamManagerError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn error_implements_std_error() {
        let err = StreamManagerError::StreamNotFound(test_stream_id());
        let _: &dyn std::error::Error = &err;
    }
}
