use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during job queue operations.
#[derive(Error, Debug)]
pub enum JobError {
    /// The job was not found in the queue.
    #[error("job not found: {0}")]
    NotFound(String),

    /// A job with the given ID already exists.
    #[error("job already exists: {0}")]
    AlreadyExists(String),

    /// The job is in a terminal state and cannot be modified.
    #[error("job {0} is in terminal state {1:?}")]
    InvalidState(String, String),

    /// The job has been cancelled.
    #[error("job cancelled: {0}")]
    Cancelled(String),

    /// The job has exhausted its retry budget.
    #[error("job {0} has exhausted retries (attempted {1}/{2})")]
    RetryExhausted(String, u32, u32),

    /// The queue is shutting down.
    #[error("queue is shutting down")]
    QueueShuttingDown,

    /// An I/O error occurred during persistence.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization error occurred.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An invalid job payload was provided.
    #[error("invalid payload: {0}")]
    InvalidPayload(String),

    /// The persistence layer failed to read the job storage directory.
    #[error("persistence error: cannot access {0}")]
    PersistenceError(PathBuf),

    /// A timeout occurred.
    #[error("operation timed out")]
    Timeout,

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type alias for job queue operations.
pub type JobResult<T> = Result<T, JobError>;
