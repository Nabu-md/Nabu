use std::collections::HashMap;
use std::sync::Arc;

use crate::jobs::cancellation::CancellationToken;
use crate::jobs::job::{Job, JobId, JobType};
use crate::jobs::worker_channel::WorkerHandle;

use super::errors::{WorkerError, WorkerResult};
use super::progress::ProgressReporter;

/// A context provided to the executor when running a job.
///
/// Contains everything the executor needs to process a job and report its
/// progress and results back to the worker pool.
#[derive(Clone)]
pub struct ExecuteContext {
    /// The job to be executed.
    pub job: Job,

    /// Cancellation token — the executor should check this periodically.
    pub cancellation: CancellationToken,

    /// A progress reporter for reporting job progress.
    pub progress: Arc<dyn ProgressReporter>,

    /// The worker handle for sending status updates back to the queue.
    pub worker_handle: Option<Arc<tokio::sync::Mutex<Option<WorkerHandle>>>>,
}

impl std::fmt::Debug for ExecuteContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteContext")
            .field("job", &self.job)
            .field("cancellation", &self.cancellation)
            .finish()
    }
}

impl ExecuteContext {
    /// Creates a new execute context.
    pub fn new(
        job: Job,
        cancellation: CancellationToken,
        progress: Arc<dyn ProgressReporter>,
    ) -> Self {
        ExecuteContext {
            job,
            cancellation,
            progress,
            worker_handle: None,
        }
    }

    /// Checks if the job has been cancelled. Returns `true` if cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Reports progress for the current job.
    pub fn report_progress(&self, progress: f64, message: String) {
        self.progress
            .report(self.job.id, progress, message);
    }
}

/// The result of executing a job.
#[derive(Debug, Clone)]
pub enum ExecuteResult {
    /// The job completed successfully.
    Completed,
    /// The job failed with an error message.
    Failed(String),
    /// The job was cancelled during execution.
    Cancelled,
}

/// The executor trait — implemented by specific job processors.
///
/// Executors are generic — they know nothing about OCR, AI, or indexing.
/// They simply receive a job context and produce a result.
pub trait JobExecutor: Send + Sync + std::fmt::Debug {
    /// Executes a job synchronously within the context provided.
    ///
    /// The executor should check `ctx.is_cancelled()` periodically and
    /// report progress via `ctx.report_progress()`.
    fn execute(&self, ctx: &ExecuteContext) -> ExecuteResult;
}

/// A registry mapping job types to their executors.
///
/// When a job is dispatched, the worker looks up the executor for the job's
/// type and delegates execution to it. This keeps workers generic and allows
/// new job types to be added simply by registering an executor.
#[derive(Debug)]
pub struct ExecutorRegistry {
    executors: HashMap<String, Box<dyn JobExecutor>>,
    fallback: Option<Box<dyn JobExecutor>>,
}

impl ExecutorRegistry {
    /// Creates a new empty executor registry.
    pub fn new() -> Self {
        ExecutorRegistry {
            executors: HashMap::new(),
            fallback: None,
        }
    }

    /// Registers an executor for a specific job type.
    pub fn register<E: JobExecutor + 'static>(&mut self, job_type: &str, executor: E) {
        self.executors
            .insert(job_type.to_string(), Box::new(executor));
    }

    /// Sets a fallback executor used when no specific executor is registered.
    pub fn set_fallback<E: JobExecutor + 'static>(&mut self, executor: E) {
        self.fallback = Some(Box::new(executor));
    }

    /// Gets the executor for a job type, falling back to the fallback if set.
    pub fn get(&self, job_type: &JobType) -> WorkerResult<&dyn JobExecutor> {
        self.executors
            .get(&job_type.0)
            .map(|e| e.as_ref())
            .or_else(|| self.fallback.as_ref().map(|e| e.as_ref()))
            .ok_or_else(|| WorkerError::NoExecutor(job_type.0.clone()))
    }

    /// Returns `true` if an executor is available for the given job type.
    pub fn has_executor(&self, job_type: &JobType) -> bool {
        self.executors.contains_key(&job_type.0) || self.fallback.is_some()
    }

    /// Returns the number of registered executors.
    pub fn count(&self) -> usize {
        self.executors.len()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple test executor that always completes successfully.
/// Used in testing and as a default stub.
#[derive(Debug)]
pub struct NoopExecutor;

impl JobExecutor for NoopExecutor {
    fn execute(&self, ctx: &ExecuteContext) -> ExecuteResult {
        // Report some progress to test the infrastructure
        ctx.report_progress(0.0, "starting".into());
        ctx.report_progress(0.5, "working".into());
        ctx.report_progress(1.0, "done".into());

        if ctx.is_cancelled() {
            return ExecuteResult::Cancelled;
        }

        ExecuteResult::Completed
    }
}

/// A test executor that always fails.
#[derive(Debug)]
pub struct FailExecutor {
    /// The error message to return.
    pub message: String,
}

impl FailExecutor {
    /// Creates a new fail executor.
    pub fn new(message: impl Into<String>) -> Self {
        FailExecutor {
            message: message.into(),
        }
    }
}

impl JobExecutor for FailExecutor {
    fn execute(&self, _ctx: &ExecuteContext) -> ExecuteResult {
        ExecuteResult::Failed(self.message.clone())
    }
}

/// A test executor that simulates a long-running job with cancellability.
#[derive(Debug)]
pub struct SlowExecutor {
    /// Number of steps to simulate.
    pub steps: u32,
    /// Delay between steps in milliseconds.
    pub delay_ms: u64,
}

impl SlowExecutor {
    /// Creates a new slow executor.
    pub fn new(steps: u32, delay_ms: u64) -> Self {
        SlowExecutor { steps, delay_ms }
    }
}

impl JobExecutor for SlowExecutor {
    fn execute(&self, ctx: &ExecuteContext) -> ExecuteResult {
        use std::thread;
        use std::time::Duration;

        for step in 0..self.steps {
            if ctx.is_cancelled() {
                return ExecuteResult::Cancelled;
            }

            let progress = (step as f64 + 1.0) / self.steps as f64;
            ctx.report_progress(progress, format!("step {}/{}", step + 1, self.steps));

            thread::sleep(Duration::from_millis(self.delay_ms));
        }

        ExecuteResult::Completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ExecutorRegistry::new();
        registry.register("test", NoopExecutor);

        let executor = registry.get(&JobType::new("test")).unwrap();
        assert!(executor.execute(&ExecuteContext::new(
            Job::new("test", crate::jobs::JobPayload::new()),
            CancellationToken::new(),
            Arc::new(super::super::progress::InMemoryProgressTracker::new()),
        )).is_success());
    }

    #[test]
    fn test_registry_missing_executor() {
        let registry = ExecutorRegistry::new();
        let result = registry.get(&JobType::new("unknown"));
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_fallback() {
        let mut registry = ExecutorRegistry::new();
        registry.set_fallback(NoopExecutor);

        let executor = registry.get(&JobType::new("anything")).unwrap();
        executor.execute(&ExecuteContext::new(
            Job::new("anything", crate::jobs::JobPayload::new()),
            CancellationToken::new(),
            Arc::new(super::super::progress::InMemoryProgressTracker::new()),
        ));
    }

    #[test]
    fn test_noop_executor() {
        let executor = NoopExecutor;
        let ctx = ExecuteContext::new(
            Job::new("test", crate::jobs::JobPayload::new()),
            CancellationToken::new(),
            Arc::new(super::super::progress::InMemoryProgressTracker::new()),
        );
        let result = executor.execute(&ctx);
        match result {
            ExecuteResult::Completed => {} // expected
            _ => panic!("expected Completed"),
        }
    }

    #[test]
    fn test_fail_executor() {
        let executor = FailExecutor::new("something broke");
        let ctx = ExecuteContext::new(
            Job::new("test", crate::jobs::JobPayload::new()),
            CancellationToken::new(),
            Arc::new(super::super::progress::InMemoryProgressTracker::new()),
        );
        let result = executor.execute(&ctx);
        match result {
            ExecuteResult::Failed(msg) => assert_eq!(msg, "something broke"),
            _ => panic!("expected Failed"),
        }
    }

    #[test]
    fn test_slow_executor_cancellation() {
        let executor = SlowExecutor::new(10, 50);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let ctx = ExecuteContext::new(
            Job::new("slow", crate::jobs::JobPayload::new()),
            cancel,
            Arc::new(super::super::progress::InMemoryProgressTracker::new()),
        );

        // Cancel after a short delay
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            cancel_clone.cancel();
        });

        let result = executor.execute(&ctx);
        match result {
            ExecuteResult::Cancelled => {} // expected
            other => panic!("expected Cancelled, got {:?}", other),
        }
    }
}

impl ExecuteResult {
    /// Returns `true` if the result is a success.
    pub fn is_success(&self) -> bool {
        matches!(self, ExecuteResult::Completed)
    }

    /// Returns `true` if the result is a failure.
    pub fn is_failure(&self) -> bool {
        matches!(self, ExecuteResult::Failed(_))
    }

    /// Returns `true` if the result is a cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, ExecuteResult::Cancelled)
    }

    /// Returns the error message if the result is a failure.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ExecuteResult::Failed(msg) => Some(msg.as_str()),
            _ => None,
        }
    }
}
