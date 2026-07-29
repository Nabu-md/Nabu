use thiserror::Error;

/// Errors that can occur during worker pool operations.
#[derive(Error, Debug)]
pub enum WorkerError {
    /// The worker pool has not been started yet.
    #[error("worker pool not started")]
    NotStarted,

    /// The worker pool is already running.
    #[error("worker pool already running")]
    AlreadyRunning,

    /// The worker pool has already been shut down.
    #[error("worker pool is shut down")]
    ShutDown,

    /// The worker received a cancellation signal for an unknown job.
    #[error("received cancellation for unknown job: {0}")]
    UnknownCancellation(String),

    /// The executor failed to process a job.
    #[error("executor error: {0}")]
    Executor(String),

    /// A timeout occurred during shutdown.
    #[error("shutdown timeout after {0} seconds")]
    ShutdownTimeout(u64),

    /// All workers are busy and the backpressure limit has been reached.
    #[error("backpressure limit reached: {0} jobs queued for workers")]
    BackpressureLimit(usize),

    /// No executor registered for this job type.
    #[error("no executor registered for job type: {0}")]
    NoExecutor(String),

    /// An internal error occurred.
    #[error("worker internal error: {0}")]
    Internal(String),
}

/// Result type alias for worker operations.
pub type WorkerResult<T> = Result<T, WorkerError>;
