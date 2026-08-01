use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the worker pool system.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum WorkerError {
    #[error("Worker panicked: {0}")]
    WorkerPanic(String),

    #[error("Worker timed out")]
    WorkerTimeout,

    #[error("No executor registered for job type: {0}")]
    NoExecutor(String),

    #[error("Executor execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Job cancelled during execution")]
    JobCancelled,

    #[error("Pool is full")]
    PoolFull,

    #[error("Pool is shutting down")]
    PoolShuttingDown,

    #[error("Worker channel closed")]
    ChannelClosed,

    #[error("Backpressure: too many pending jobs")]
    BackpressureLimitReached,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<tokio::sync::oneshot::error::RecvError> for WorkerError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        WorkerError::Internal("oneshot channel closed".to_string())
    }
}

/// Result type for worker operations.
pub type WorkerResult<T> = Result<T, WorkerError>;
