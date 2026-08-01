use crate::event_bus::EventBus;
use crate::jobs::cancellation::CancellationToken;
use crate::jobs::errors::JobResult;
use crate::jobs::job::Job;
use crate::jobs::workers::executor::JobExecutor;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectType};
use crate::pipeline_migration::events;
use crate::processing::pipeline::{build_standard_pipeline, ProcessingPipeline};
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
}

impl PipelineExecutor {
    /// Create a new pipeline executor.
    pub fn new(pipeline: Arc<ProcessingPipeline>) -> Self {
        Self {
            pipeline,
            event_bus: None,
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
        }
    }

    /// Create a standard pipeline executor with all default processors.
    pub fn standard(event_bus: Option<EventBus<crate::event_bus::PipelineEvent>>) -> Self {
        let pipeline = Arc::new(build_standard_pipeline(event_bus.clone()));
        Self {
            pipeline,
            event_bus,
        }
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
        let mut completed_job = job.clone();
        completed_job.status = crate::jobs::job::JobStatus::Completed;
        completed_job.progress = 1.0;

        Ok(completed_job)
    }
}
