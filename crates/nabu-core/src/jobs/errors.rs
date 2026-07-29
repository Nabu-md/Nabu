use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the job queue system.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum JobError {
    #[error("Job not found: {0}")]
    NotFound(String),

    #[error("Job already exists: {0}")]
    AlreadyExists(String),

    #[error("Job is in an invalid state for this operation: {0} -> {1}")]
    InvalidState(String, String),

    #[error("Queue is full")]
    QueueFull,

    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Cancellation requested")]
    Cancelled,

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    #[error("Shutdown in progress")]
    Shutdown,

    #[error("Backpressure limit reached")]
    BackpressureLimitReached,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for JobError {
    fn from(e: std::io::Error) -> Self {
        JobError::Persistence(e.to_string())
    }
}

impl From<serde_json::Error> for JobError {
    fn from(e: serde_json::Error) -> Self {
        JobError::Serialization(e.to_string())
    }
}

/// Result type for job queue operations.
pub type JobResult<T> = Result<T, JobError>;
