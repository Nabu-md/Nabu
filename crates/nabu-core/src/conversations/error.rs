use uuid::Uuid;

/// Structured errors for the conversation persistence layer.
///
/// All errors are non-panicking — they are returned as `Result::Err` values
/// so callers can decide how to handle them. The application should continue
/// operating whenever safe recovery is possible (e.g. skipping a corrupted
/// thread on startup).
#[derive(Debug, Clone)]
pub enum PersistenceError {
    /// The thread was not found in the cache or on disk.
    ThreadNotFound {
        thread_id: Uuid,
    },
    /// The persistence file exists but contained invalid JSON.
    SerializationError {
        message: String,
    },
    /// The deserialization failed (missing fields, type mismatch, etc.).
    DeserializationError {
        target: String,
        message: String,
    },
    /// The thread failed model-level validation after deserialization.
    ValidationError {
        thread_id: Uuid,
        reason: String,
    },
    /// An I/O error occurred while reading or writing a persistence file.
    IoError {
        message: String,
    },
    /// The store has been shut down and cannot accept requests.
    Shutdown,
    /// The given thread ID conflicts with an existing thread (duplicate).
    DuplicateId {
        thread_id: Uuid,
    },
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistenceError::ThreadNotFound { thread_id } => {
                write!(f, "Thread not found: {}", thread_id)
            }
            PersistenceError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            PersistenceError::DeserializationError { target, message } => {
                write!(f, "Deserialization error for '{}': {}", target, message)
            }
            PersistenceError::ValidationError { thread_id, reason } => {
                write!(
                    f,
                    "Validation failed for thread {}: {}",
                    thread_id, reason
                )
            }
            PersistenceError::IoError { message } => {
                write!(f, "I/O error: {}", message)
            }
            PersistenceError::Shutdown => {
                write!(f, "ConversationStore has been shut down")
            }
            PersistenceError::DuplicateId { thread_id } => {
                write!(f, "Duplicate thread ID: {}", thread_id)
            }
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<std::io::Error> for PersistenceError {
    fn from(e: std::io::Error) -> Self {
        PersistenceError::IoError {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for PersistenceError {
    fn from(e: serde_json::Error) -> Self {
        PersistenceError::SerializationError {
            message: e.to_string(),
        }
    }
}

/// Result type alias for persistence operations.
pub type PersistenceResult<T> = Result<T, PersistenceError>;
