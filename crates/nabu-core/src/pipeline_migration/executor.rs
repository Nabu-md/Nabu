use std::sync::Arc;

use crate::event_bus::EventBus;
use crate::jobs::job::{Job, JobStatus};
use crate::jobs::persistence::JobStore;
use crate::jobs::workers::executor::{ExecuteContext, ExecuteResult, JobExecutor};
use crate::processing::pipeline::ProcessingPipeline;
use crate::processing::processor::{ProcessingContext, ProcessingResult};

use super::events::*;

/// An executor that runs the `ProcessingPipeline` for captured content jobs.
///
/// This is the bridge between the async worker pool and the synchronous
/// processing pipeline. It:
///
/// 1. Receives a job with captured content in the payload.
/// 2. Constructs a `ProcessingContext` from the job payload.
/// 3. Runs the `ProcessingPipeline`.
/// 4. Publishes lifecycle events on the EventBus.
/// 5. Persists the processed result via the store.
/// 6. Returns `ExecuteResult::Completed` or `ExecuteResult::Failed`.
///
/// ## Event Flow
///
/// ```text
/// PipelineExecutor.execute()
///     │
///     ├── publishes ItemProcessingStarted
///     ├── runs ProcessingPipeline
///     ├── on success:
///     │     ├── publications ItemProcessingCompleted
///     │     ├── publishes ItemProcessed { success: true }
///     │     ├── persists to StorageManager via ItemStored
///     │     └── returns Completed
///     └── on failure:
///           ├── publications ItemProcessingFailed
///           ├── publishes ItemProcessed { success: false }
///           └── returns Failed(error)
/// ```
#[derive(Debug)]
pub struct PipelineExecutor {
    /// The processing pipeline to run.
    pipeline: Arc<ProcessingPipeline>,

    /// The job store for persisting state changes.
    store: Arc<JobStore>,

    /// The event bus for publishing lifecycle events.
    event_bus: Option<EventBus<PipelineLifecycleEvent>>,
}

/// Lifecycle events published during pipeline execution.
#[derive(Debug, Clone)]
pub enum PipelineLifecycleEvent {
    ItemCaptured(ItemCaptured),
    ItemProcessingStarted(ItemProcessingStarted),
    ItemProcessingCompleted(ItemProcessingCompleted),
    ItemProcessingFailed(ItemProcessingFailed),
    ItemProcessed(ItemProcessed),
    ItemStored(ItemStored),
}

impl PipelineExecutor {
    /// Creates a new pipeline executor.
    pub fn new(
        pipeline: Arc<ProcessingPipeline>,
        store: Arc<JobStore>,
    ) -> Self {
        PipelineExecutor {
            pipeline,
            store,
            event_bus: None,
        }
    }

    /// Creates a new pipeline executor with an event bus.
    pub fn with_event_bus(
        pipeline: Arc<ProcessingPipeline>,
        store: Arc<JobStore>,
        event_bus: EventBus<PipelineLifecycleEvent>,
    ) -> Self {
        PipelineExecutor {
            pipeline,
            store,
            event_bus: Some(event_bus),
        }
    }

    /// Extracts a processing context from a job's payload.
    fn build_context(job: &Job) -> ProcessingContext {
        let content_type = job
            .payload
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let content = job
            .payload
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();

        let mut ctx = ProcessingContext::new(content_type, content);

        // Extract metadata from payload
        if let Some(metadata) = job.payload.get("metadata").and_then(|v| v.as_object()) {
            for (key, value) in metadata {
                if let Some(s) = value.as_str() {
                    ctx.add_metadata(key, s);
                }
            }
        }

        // Add source type
        if let Some(source) = job.payload.get("source_type").and_then(|v| v.as_str()) {
            ctx.add_metadata("source_type", source);
        }

        ctx
    }

    /// Publishes an event to the event bus if configured.
    fn publish_event(&self, event: PipelineLifecycleEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.publish(&event);
        }
    }
}

impl JobExecutor for PipelineExecutor {
    fn execute(&self, ctx: &ExecuteContext) -> ExecuteResult {
        let job = &ctx.job;
        let job_id = job.id.to_string();
        let content_type = job
            .payload
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Publish processing started
        self.publish_event(PipelineLifecycleEvent::ItemProcessingStarted(
            ItemProcessingStarted {
                job_id: job_id.clone(),
                content_type: content_type.clone(),
                processor_count: self.pipeline.processor_count() as u32,
            },
        ));

        // Build the processing context from the job payload
        let mut processing_ctx = Self::build_context(job);

        // Run the pipeline
        let pipeline_result = self.pipeline.run(&mut processing_ctx);

        match pipeline_result {
            ProcessingResult { success: true, context, .. } => {
                // Publish processing completed
                let processor_count = context.processor_results.len() as u32;
                self.publish_event(PipelineLifecycleEvent::ItemProcessingCompleted(
                    ItemProcessingCompleted {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        processor_results: processor_count,
                    },
                ));

                // Publish item processed (for StorageManager subscription)
                self.publish_event(PipelineLifecycleEvent::ItemProcessed(
                    ItemProcessed {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        success: true,
                        error: None,
                    },
                ));

                // Publish item stored (for Indexer and VaultGraph subscriptions)
                self.publish_event(PipelineLifecycleEvent::ItemStored(
                    ItemStored {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        storage_path: None,  // Set by StorageManager
                        object_id: None,     // Set by StorageManager
                    },
                ));

                ctx.report_progress(1.0, "processing complete".into());
                ExecuteResult::Completed
            }
            ProcessingResult { success: false, error: Some(err), .. } => {
                // Publish processing failed
                self.publish_event(PipelineLifecycleEvent::ItemProcessingFailed(
                    ItemProcessingFailed {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        error: err.clone(),
                        retry_count: job.retry_count,
                    },
                ));

                // Publish item processed (failure)
                self.publish_event(PipelineLifecycleEvent::ItemProcessed(
                    ItemProcessed {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        success: false,
                        error: Some(err.clone()),
                    },
                ));

                ExecuteResult::Failed(err)
            }
            ProcessingResult { success: false, error: None, .. } => {
                let err = "processing failed with no error message".to_string();
                self.publish_event(PipelineLifecycleEvent::ItemProcessingFailed(
                    ItemProcessingFailed {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        error: err.clone(),
                        retry_count: job.retry_count,
                    },
                ));

                self.publish_event(PipelineLifecycleEvent::ItemProcessed(
                    ItemProcessed {
                        job_id: job_id.clone(),
                        content_type: content_type.clone(),
                        success: false,
                        error: Some(err.clone()),
                    },
                ));

                ExecuteResult::Failed(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::job::{Job, JobPayload, JobType};
    use crate::jobs::cancellation::CancellationToken;
    use crate::jobs::workers::progress::InMemoryProgressTracker;

    #[test]
    fn test_pipeline_executor_completes() {
        // Create a simple pipeline with a counting processor
        let mut pipeline = ProcessingPipeline::new();
        pipeline.add_processor(crate::processing::processor::CountingProcessor::new("p1"));

        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join(".nabu").join("jobs");
        let store = std::sync::Arc::new(
            crate::jobs::persistence::JobStore::new(&store_path)
                .expect("store should be created")
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Store needs to be initialized in an async context
            let _ = store.count().await;
        });

        let executor = PipelineExecutor::new(
            Arc::new(pipeline),
            store,
        );

        let mut payload = JobPayload::new();
        payload.insert("content_type".into(), serde_json::Value::String("test".into()));
        payload.insert("content".into(), serde_json::Value::String("aGVsbG8=".into())); // base64 "hello"
        payload.insert("source_type".into(), serde_json::Value::String("test_source".into()));

        let job = Job::new("capture:test", payload);
        let exec_ctx = ExecuteContext::new(
            job,
            CancellationToken::new(),
            Arc::new(InMemoryProgressTracker::new()),
        );

        let result = executor.execute(&exec_ctx);
        match result {
            ExecuteResult::Completed => {} // expected
            other => panic!("expected Completed, got {:?}", other),
        }
    }

    #[test]
    fn test_build_context_from_payload() {
        let mut payload = JobPayload::new();
        payload.insert("content_type".into(), serde_json::Value::String("article".into()));
        payload.insert("content".into(), serde_json::Value::String("dGVzdA==".into())); // base64 "test"
        let mut metadata = serde_json::Map::new();
        metadata.insert("title".into(), serde_json::Value::String("My Article".into()));
        payload.insert("metadata".into(), serde_json::Value::Object(metadata));
        payload.insert("source_type".into(), serde_json::Value::String("browser".into()));

        let job = Job::new("capture:browser", payload);
        let ctx = PipelineExecutor::build_context(&job);

        assert_eq!(ctx.content_type, "article");
        assert_eq!(ctx.metadata.get("title"), Some(&"My Article".to_string()));
        assert_eq!(ctx.metadata.get("source_type"), Some(&"browser".to_string()));
    }
}
