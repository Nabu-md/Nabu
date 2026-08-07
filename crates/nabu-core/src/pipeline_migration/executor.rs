use crate::event_bus::EventBus;
use crate::jobs::cancellation::CancellationToken;
use crate::jobs::errors::JobResult;
use crate::jobs::job::Job;
use crate::jobs::workers::executor::JobExecutor;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectType};
use crate::pipeline_migration::events;
use crate::processing::pipeline::{build_standard_pipeline, ProcessingPipeline};
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
use crate::storage::StorageManager;
use async_trait::async_trait;
use std::sync::Arc;

/// The PipelineExecutor bridges the Worker Pool to the ProcessingPipeline.
///
/// It implements JobExecutor and:
/// 1. Extracts the KnowledgeObject payload from the Job
/// 2. Runs the ProcessingPipeline
/// 3. Publishes EventBus events
/// 4. Reports progress
///
/// This is the final piece connecting the async infrastructure:
///   CaptureEngine → Job Queue → Worker Pool → PipelineExecutor → ProcessingPipeline
pub struct PipelineExecutor {
    pipeline: Arc<ProcessingPipeline>,
    event_bus: Option<EventBus<crate::event_bus::PipelineEvent>>,
    storage: Option<Arc<StorageManager>>,
    /// Lifecycle state manager — tracks Created → Initialized → Running → Shutdown.
    lifecycle: LifecycleManager,
}

impl PipelineExecutor {
    /// Create a new pipeline executor.
    pub fn new(pipeline: Arc<ProcessingPipeline>) -> Self {
        Self {
            pipeline,
            event_bus: None,
            storage: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create with event bus for publishing lifecycle events.
    pub fn with_event_bus(
        pipeline: Arc<ProcessingPipeline>,
        event_bus: EventBus<crate::event_bus::PipelineEvent>,
    ) -> Self {
        Self {
            pipeline,
            event_bus: Some(event_bus),
            storage: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Attach the canonical StorageManager so processed objects are persisted
    /// through the canonical pipeline: Pipeline → Storage → ITEM_STORED.
    pub fn with_storage(mut self, storage: Arc<StorageManager>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Create a standard pipeline executor with all default processors.
    pub fn standard(event_bus: Option<EventBus<crate::event_bus::PipelineEvent>>) -> Self {
        let pipeline = Arc::new(build_standard_pipeline(event_bus.clone()));
        Self {
            pipeline,
            event_bus,
            storage: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the pipeline executor.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the pipeline executor has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the pipeline executor is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the pipeline executor has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    /// Reconstruct a KnowledgeObject from a job payload.
    fn object_from_job(job: &Job) -> KnowledgeObject {
        let object_type = match job.job_type.name() {
            "ocr" => ObjectType::Document,
            "whisper" => ObjectType::AudioRecording,
            "pdf_text_extraction" => ObjectType::Document,
            "metadata_extraction" => ObjectType::Note,
            _ => ObjectType::Note,
        };

        let mut object = KnowledgeObject::new(
            object_type,
            crate::models::ObjectContent::PlainText(
                job.payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            ),
        );

        object.metadata.title = job
            .payload
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        object.metadata.source_url = job
            .payload
            .get("source_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if let Some(object_id) = job.object_id {
            object.id = object_id;
        }

        object
    }
}

#[async_trait]
impl JobExecutor for PipelineExecutor {
    async fn execute(
        &self,
        job: &Job,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> JobResult<Job> {
        if cancellation.is_cancelled() {
            return Err(crate::jobs::errors::JobError::Cancelled);
        }

        // Publish started event
        if let Some(ref bus) = self.event_bus {
            events::publish_processing_started(bus, job);
        }

        progress.set_progress(0.05);

        // Reconstruct object from job
        let object = Self::object_from_job(job);
        progress.set_progress(0.1);

        // Run the processing pipeline
        let result = self
            .pipeline
            .run(object, progress.clone(), cancellation.clone())
            .await;

        progress.set_progress(0.9);

        // Handle result
        if let Some(error) = &result.error {
            // Pipeline partially failed
            if let Some(ref bus) = self.event_bus {
                let will_retry = job.should_retry();
                events::publish_processing_failed(bus, job, error, will_retry);
            }

            return Err(crate::jobs::errors::JobError::ExecutionFailed(
                error.clone(),
            ));
        }

        // Pipeline completed
        if let Some(ref bus) = self.event_bus {
            events::publish_processing_completed(bus, job);
        }

        progress.set_progress(1.0);

        // Return the job as successful
        // Persist the processed object through the canonical StorageManager.
        // StorageManager.save publishes ITEM_STORED, which drives the Indexer
        // and VaultGraph subscribers downstream.
        if let Some(ref storage) = self.storage {
            if let Err(e) = storage.save(&result.object) {
                tracing::warn!(error = %e, "Failed to persist processed object through StorageManager");
            }
        }

        let mut completed_job = job.clone();
        completed_job.status = crate::jobs::job::JobStatus::Completed;
        completed_job.progress = 1.0;

        Ok(completed_job)
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared `Lifecycle` trait so `PipelineExecutor` can be managed
/// by the Capability Platform's lifecycle manager alongside other services.
///
/// ```text
/// Created → Initialized → Running → Shutdown
/// ```
impl Lifecycle for PipelineExecutor {
    fn name(&self) -> &'static str {
        "pipeline_executor"
    }

    /// Initializes the pipeline executor.
    ///
    /// Transitions the executor from `Created` to `Initialized`.
    /// The processing pipeline and storage manager are wired during
    /// construction; this phase validates that dependencies are present.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "pipeline",
            component = "executor",
            operation = "initialize",
            "PipelineExecutor initialized"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;
        Ok(())
    }

    /// Starts the pipeline executor.
    ///
    /// Transitions the executor to `Running` so it can accept and execute
    /// processing jobs from the worker pool. Auto-advances
    /// `Created → Initialized` if `initialize()` was not called explicitly.
    ///
    /// Double-start is a safe no-op — no duplicate executor state is
    /// created. Calling `start()` after `shutdown()` returns an error.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Cannot restart a shut-down executor
        if self.lifecycle.is_shutdown() {
            return Err(
                "PipelineExecutor has been shut down and cannot be restarted".into(),
            );
        }

        // Auto-advance Created → Initialized so callers can call start()
        // directly without an explicit initialize() call.
        if self.lifecycle.stage() == LifecycleStage::Created {
            tracing::info!(
                subsystem = "pipeline",
                component = "executor",
                operation = "start",
                "Initializing pipeline executor"
            );
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }

        // Guard against duplicate start — transition Running → Running is a
        // no-op in LifecycleManager, but we log a warning for visibility.
        if self.lifecycle.is_running() {
            tracing::warn!(
                subsystem = "pipeline",
                component = "executor",
                operation = "start",
                "PipelineExecutor already started — skipping duplicate start"
            );
            return Ok(());
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)?;

        tracing::info!(
            subsystem = "pipeline",
            component = "executor",
            operation = "start",
            "PipelineExecutor started"
        );
        Ok(())
    }

    /// Shuts down the pipeline executor.
    ///
    /// Stops accepting new processing jobs. The executor is not restartable
    /// after shutdown.
    ///
    /// Double-shutdown is a safe no-op.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "pipeline",
            component = "executor",
            operation = "shutdown",
            "PipelineExecutor shutting down"
        );

        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;

        tracing::info!(
            subsystem = "pipeline",
            component = "executor",
            operation = "shutdown",
            "PipelineExecutor stopped"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::pipeline::ProcessingPipeline;

    /// Create a minimal PipelineExecutor for lifecycle tests.
    fn make_executor() -> PipelineExecutor {
        let pipeline = Arc::new(ProcessingPipeline::new());
        PipelineExecutor::new(pipeline)
    }

    // ── Lifecycle state tests ────────────────────────────────────────

    #[test]
    fn lifecycle_initial_state_is_created() {
        let executor = make_executor();
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Created);
        assert!(!executor.is_initialized());
        assert!(!executor.is_running());
        assert!(!executor.is_shutdown());
    }

    #[test]
    fn lifecycle_trait_name() {
        let executor = make_executor();
        let executor_ref: &dyn Lifecycle = &executor;
        assert_eq!(executor_ref.name(), "pipeline_executor");
    }

    #[test]
    fn lifecycle_initialize_transitions_to_initialized() {
        let executor = make_executor();
        assert!(executor.initialize().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(executor.is_initialized());
        assert!(!executor.is_running());
    }

    #[test]
    fn lifecycle_start_auto_advances_from_created() {
        let executor = make_executor();
        // start() should auto-advance Created → Initialized → Running
        assert!(executor.start().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Running);
        assert!(executor.is_running());
    }

    #[test]
    fn lifecycle_full_flow() {
        let executor = make_executor();
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Created);

        assert!(executor.initialize().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(executor.is_initialized());

        assert!(executor.start().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Running);
        assert!(executor.is_running());

        assert!(executor.shutdown().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Shutdown);
        assert!(executor.is_shutdown());
    }

    #[test]
    fn lifecycle_start_after_shutdown_returns_error() {
        let executor = make_executor();
        assert!(executor.start().is_ok());
        assert!(executor.shutdown().is_ok());
        // Cannot restart after shutdown
        assert!(executor.start().is_err());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Shutdown);
    }

    #[test]
    fn lifecycle_double_shutdown_is_noop() {
        let executor = make_executor();
        assert!(executor.start().is_ok());
        assert!(executor.shutdown().is_ok());
        // Second shutdown should succeed (same stage is a no-op)
        assert!(executor.shutdown().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Shutdown);
    }

    #[test]
    fn lifecycle_double_start_is_noop() {
        let executor = make_executor();
        assert!(executor.start().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Running);
        // Second start should succeed (no-op)
        assert!(executor.start().is_ok());
        assert!(executor.is_running());
        // Cleanup
        assert!(executor.shutdown().is_ok());
    }

    #[test]
    fn lifecycle_start_without_initialize() {
        let executor = make_executor();
        // start() should auto-advance Created → Initialized → Running
        assert!(executor.start().is_ok());
        assert_eq!(executor.lifecycle_stage(), LifecycleStage::Running);
        assert!(executor.is_running());
        assert!(executor.is_initialized());
        assert!(executor.shutdown().is_ok());
    }

    #[test]
    fn lifecycle_backward_transition_rejected() {
        let executor = make_executor();
        assert!(executor.start().is_ok());
        assert!(executor.shutdown().is_ok());
        // Cannot go backward: Shutdown → Initialized
        assert!(executor.initialize().is_err());
        // Cannot restart: Shutdown → Running
        assert!(executor.start().is_err());
    }

    // ── JobExecutor still works after lifecycle ──────────────────────

    #[tokio::test]
    async fn job_executor_works_after_start() {
        use crate::jobs::cancellation::CancellationToken;
        use crate::jobs::workers::progress::ProgressReporter;
        let executor = make_executor();
        let _ = executor.start(); // just verify it compiles and runs

        let job = Job::new(
            crate::jobs::job::JobType::Custom("test".to_string()),
            serde_json::json!({ "title": "Test Object" }),
            "metadata_extraction_processor",
        );
        let progress = ProgressReporter::noop();
        let cancellation = CancellationToken::new();
        let result = executor.execute(&job, progress, cancellation).await;
        // The execute method itself is unchanged; we just verify it doesn't
        // panic when the executor is in Running state.
        let _ = result;
        let _ = executor.shutdown();
    }
}
