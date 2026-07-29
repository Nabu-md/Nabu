use crate::jobs::cancellation::CancellationToken;
use crate::jobs::errors::JobResult;
use crate::jobs::job::Job;
use crate::jobs::workers::progress::ProgressReporter;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// The generic trait that all job executors must implement.
///
/// Executors are the bridge between the worker pool and the actual processing logic.
/// Each executor handles a specific job type (OCR, Whisper, embedding, etc.).
/// Executors know NOTHING about the queue, workers, or other infrastructure.
#[async_trait]
pub trait JobExecutor: Send + Sync {
    /// Execute a job and return the result.
    async fn execute(
        &self,
        job: &Job,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> JobResult<Job>;
}

/// Registry of job executors, indexed by processor name.
///
/// The registry is populated at startup by registering each processor.
/// Workers look up executors by job type to dispatch work.
pub struct ExecutorRegistry {
    executors: HashMap<String, Arc<dyn JobExecutor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register an executor for a specific processor name.
    pub fn register(&mut self, name: impl Into<String>, executor: Arc<dyn JobExecutor>) {
        self.executors.insert(name.into(), executor);
    }

    /// Get an executor by processor name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn JobExecutor>> {
        self.executors.get(name).cloned()
    }

    /// Check if an executor exists for the given name.
    pub fn has_executor(&self, name: &str) -> bool {
        self.executors.contains_key(name)
    }

    /// Number of registered executors.
    pub fn count(&self) -> usize {
        self.executors.len()
    }

    /// List all registered processor names.
    pub fn processor_names(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A no-op executor for testing.
pub struct NoopExecutor;

#[async_trait]
impl JobExecutor for NoopExecutor {
    async fn execute(
        &self,
        _job: &Job,
        _progress: ProgressReporter,
        _cancellation: CancellationToken,
    ) -> JobResult<Job> {
        Ok(Job::new(
            crate::jobs::job::JobType::Custom("noop".to_string()),
            serde_json::json!({}),
            "noop",
        ))
    }
}

/// A fallback executor for unregistered job types.
pub struct FallbackExecutor;

#[async_trait]
impl JobExecutor for FallbackExecutor {
    async fn execute(
        &self,
        job: &Job,
        _progress: ProgressReporter,
        _cancellation: CancellationToken,
    ) -> JobResult<Job> {
        Err(crate::jobs::errors::JobError::ExecutionFailed(format!(
            "No executor registered for processor: {}",
            job.processor_name
        )))
    }
}
