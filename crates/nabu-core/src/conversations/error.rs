use std::fmt;
use uuid::Uuid;

/// Structured errors for the conversation persistence layer.
///
/// All errors are non-panicking — they are returned as `Result::Err` values
/// so callers can decide how to handle them. The application should continue
/// operating whenever safe recovery is possible (e.g. skipping a corrupted
/// thread on startup).
#[derive(Debug, Clone)]
pub enum ConversationError {
    /// The persistence file or directory does not exist.
    NotFound {
        /// The thread ID that was not found.
        thread_id: Uuid,
    },
    /// The persistence file exists but contained invalid JSON.
    SerializationError {
        /// Human-readable serde_json error message.
        message: String,
    },
    /// The deserialization failed (missing fields, type mismatch, etc.).
    DeserializationError {
        /// The thread ID or file path that failed to deserialize.
        target: String,
        /// Human-readable serde error message.
        message: String,
    },
    /// The thread failed validation after deserialization (e.g. empty title,
    /// invalid message ordering).
    ValidationError {
        /// The thread ID that failed validation.
        thread_id: Uuid,
        /// Human-readable validation error.
        reason: String,
    },
    /// An I/O error occurred while reading or writing a persistence file.
    IoError {
        /// Human-readable I/O error message.
        message: String,
    },
    /// The persistence store has been shut down and cannot accept requests.
    Shutdown,
    /// The given thread ID conflicts with an existing thread (duplicate).
    DuplicateId {
        /// The conflicting thread ID.
        thread_id: Uuid,
    },
}

impl fmt::Display for ConversationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversationError::NotFound { thread_id } => {
                write!(f, "Thread not found: {}", thread_id)
            }
            ConversationError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            ConversationError::DeserializationError { target, message } => {
                write!(f, "Deserialization error for '{}': {}", target, message)
            }
            ConversationError::ValidationError { thread_id, reason } => {
                write!(
                    f,
                    "Validation failed for thread {}: {}",
                    thread_id, reason
                )
            }
            ConversationError::IoError { message } => {
                write!(f, "I/O error: {}", message)
            }
            ConversationError::Shutdown => {
                write!(f, "ConversationStore has been shut down")
            }
            ConversationError::DuplicateId { thread_id } => {
                write!(f, "Duplicate thread ID: {}", thread_id)
            }
        }
    }
}

impl std::error::Error for ConversationError {}

impl From<std::io::Error> for ConversationError {
    fn from(e: std::io::Error) -> Self {
        ConversationError::IoError {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for ConversationError {
    fn from(e: serde_json::Error) -> Self {
        ConversationError::SerializationError {
            message: e.to_string(),
        }
    }
}
