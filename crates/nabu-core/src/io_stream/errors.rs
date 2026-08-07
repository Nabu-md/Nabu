//! Structured errors for stdio transport operations.
//!
//! Every error variant corresponds to a specific failure mode in the stdio
//! transport layer, enabling callers to distinguish between malformed input,
//! deserialization failures, write errors, and shutdown states.
//!
//! These errors complement the JSON-RPC core errors ([`JsonRpcError`]) which
//! handle protocol-level failures. Transport errors handle I/O-level failures
//! that occur before or after a message reaches the router.

use std::io;

/// Errors that can occur during stdio transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// A low-level I/O error occurred (read from stdin, write to stdout, flush).
    #[error("IO error: {source}")]
    Io {
        #[source]
        source: io::Error,
    },

    /// The incoming data could not be deserialized as a JSON-RPC message.
    ///
    /// This wraps serde_json deserialization failures so callers can produce
    /// a [`JsonRpcError::from(ErrorCode::ParseError)`] response if needed.
    #[error("Failed to deserialize message: {source}")]
    Deserialize {
        #[source]
        source: serde_json::Error,
    },

    /// The message was valid JSON but did not conform to the expected
    /// JSON-RPC request structure (e.g. missing required fields).
    #[error("Invalid JSON-RPC message: {message}")]
    InvalidMessage { message: String },

    /// A write or flush operation failed on stdout.
    #[error("Failed to write response: {source}")]
    WriteFailed {
        #[source]
        source: io::Error,
    },

    /// The transport was shut down while an operation was in progress.
    #[error("Transport is shutting down or has been shut down")]
    Shutdown,

    /// The reader encountered EOF on stdin before any complete message was read.
    ///
    /// This is the normal termination signal for a stdio transport — the
    /// upstream process closed stdin, indicating no more requests will arrive.
    #[error("End of input (stdin closed)")]
    Eof,

    /// The reader encountered a line that exceeded the maximum message size.
    #[error("Message exceeds maximum size of {max_bytes} bytes (got {actual_bytes})")]
    MessageTooLarge {
        max_bytes: usize,
        actual_bytes: usize,
    },

    /// The router returned `None` for a response, indicating the request
    /// was a notification (no response expected) or the router declined to
    /// produce a response.
    #[error("Router produced no response for request")]
    NoResponse,

    /// The tokio runtime was not available on the current thread — required
    /// for spawning background tasks.
    #[error("No tokio runtime available")]
    NoRuntime,

    /// A lifecycle transition was attempted that is not valid.
    #[error("Invalid lifecycle transition: {message}")]
    Lifecycle { message: String },
}

impl TransportError {
    /// Wraps a `std::io::Error` as a [`TransportError::Io`].
    pub fn io(source: io::Error) -> Self {
        TransportError::Io { source }
    }

    /// Creates a [`TransportError::InvalidMessage`] with a descriptive message.
    pub fn invalid(message: impl Into<String>) -> Self {
        TransportError::InvalidMessage {
            message: message.into(),
        }
    }

    /// Creates a [`TransportError::MessageTooLarge`] with the size details.
    pub fn message_too_large(max_bytes: usize, actual_bytes: usize) -> Self {
        TransportError::MessageTooLarge {
            max_bytes,
            actual_bytes,
        }
    }

    /// Creates a [`TransportError::Lifecycle`] with a descriptive message.
    pub fn lifecycle(message: impl Into<String>) -> Self {
        TransportError::Lifecycle {
            message: message.into(),
        }
    }

    /// Returns `true` if this error is an EOF condition.
    pub fn is_eof(&self) -> bool {
        matches!(self, TransportError::Eof)
    }

    /// Returns `true` if this error is due to the transport being shut down.
    pub fn is_shutdown(&self) -> bool {
        matches!(self, TransportError::Shutdown)
    }
}

impl From<io::Error> for TransportError {
    fn from(source: io::Error) -> Self {
        TransportError::Io { source }
    }
}

impl From<serde_json::Error> for TransportError {
    fn from(source: serde_json::Error) -> Self {
        TransportError::Deserialize { source }
    }
}

/// Convenience type alias for results in the stdio transport module.
pub type TransportResult<T> = Result<T, TransportError>;
