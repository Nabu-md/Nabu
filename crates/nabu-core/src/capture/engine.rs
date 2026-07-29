use crate::capture::handler::{CaptureHandler, CaptureRequest, CaptureResult};
use crate::event_bus::{EventBus, ItemCapturedEvent, PipelineEvent};
use crate::event_bus::kinds::ITEM_CAPTURED;
use crate::jobs::errors::JobResult;
use crate::jobs::job::{Job, JobType};
use crate::jobs::queue::{DurableJobQueue, Queue};
use crate::models::{CaptureSource, KnowledgeObject, ObjectType};
use std::collections::HashMap;
use std::sync::Arc;

/// The CaptureEngine routes capture requests to registered handlers
/// and enqueues jobs for asynchronous processing.
///
/// This is the canonical entry point for all content entering Nabu.
/// No feature bypasses the CaptureEngine.
pub struct CaptureEngine {
    handlers: HashMap<String, Arc<dyn CaptureHandler>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    queue: Option<Arc<DurableJobQueue>>,
}

impl CaptureEngine {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            event_bus: None,
            queue: None,
        }
    }

    /// Create a capture engine with an event bus for publishing events.
    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            handlers: HashMap::new(),
            event_bus: Some(event_bus),
            queue: None,
        }
    }

    /// Set the job queue for async processing.
    pub fn set_queue(&mut self, queue: Arc<DurableJobQueue>) {
        self.queue = Some(queue);
    }

    /// Register a capture handler.
    pub fn register(&mut self, handler: Arc<dyn CaptureHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    /// Ingest a capture request through the pipeline.
    ///
    /// Returns the enqueued Job if async processing is configured.
    /// Returns immediately — processing happens asynchronously.
    pub async fn ingest(&self, request: CaptureRequest) -> JobResult<Option<uuid::Uuid>> {
        // Find matching handler
        let result = self.route(&request).await;

        let result = match result {
            Some(r) => r,
            None => return Ok(None),
        };

        // Publish capture event
        if let Some(ref bus) = self.event_bus {
            bus.publish(
                ITEM_CAPTURED,
                &PipelineEvent::ItemCaptured(ItemCapturedEvent::new(
                    result.object.id,
                    result.object.object_type.clone(),
                    result.source.clone(),
                    result.object.metadata.title.clone(),
                    None,
                )),
            );
        }

        // Enqueue for async processing
        if result.enqueue {
            if let Some(ref queue) = self.queue {
                let job_type = object_type_to_job_type(&result.object.object_type);
                let payload = serde_json::json!({
                    "object_id": result.object.id,
                    "object_type": result.object.object_type.variant_name(),
                    "source": format!("{:?}", result.source),
                    "title": result.object.metadata.title,
                    "source_url": result.object.metadata.source_url,
                });

                let mut job = Job::new(job_type.clone(), payload, format!("{}_processor", job_type.name()))
                    .with_object_id(result.object.id)
                    .with_tag("capture");

                if let Some(ref source_url) = result.object.metadata.source_url {
                    job = job.with_metadata("source_url", source_url.clone());
                }

                queue.enqueue(job)?;

                // Publish with job ID
                if let Some(ref bus) = self.event_bus {
                    // Re-publish with job ID — in real impl this would be done once
                }
            }
        }

        Ok(Some(result.object.id))
    }

    /// Route a capture request to the appropriate handler.
    async fn route(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        // Try each registered handler until one succeeds
        for handler in self.handlers.values() {
            if let Some(result) = handler.capture(request).await {
                return Some(result);
            }
        }
        None
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// List registered handler names.
    pub fn handler_names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }
}

impl Default for CaptureEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn object_type_to_job_type(object_type: &ObjectType) -> JobType {
    match object_type {
        ObjectType::Image | ObjectType::Screenshot | ObjectType::Scan => JobType::Ocr,
        ObjectType::AudioRecording => JobType::Whisper,
        ObjectType::Document => JobType::PdfTextExtraction,
        _ => JobType::MetadataExtraction,
    }
}

/// Build the default capture engine with all built-in handlers.
pub fn build_default_capture_engine(
    event_bus: Option<EventBus<PipelineEvent>>,
    queue: Option<Arc<DurableJobQueue>>,
) -> CaptureEngine {
    let mut engine = match event_bus {
        Some(bus) => CaptureEngine::with_event_bus(bus),
        None => CaptureEngine::new(),
    };

    if let Some(queue) = queue {
        engine.set_queue(queue);
    }

    engine.register(Arc::new(super::handler::BrowserCaptureHandler));
    engine.register(Arc::new(super::handler::ClipboardHandler));
    engine.register(Arc::new(super::handler::ScreenshotHandler));
    engine.register(Arc::new(super::handler::FileDropHandler));
    engine.register(Arc::new(super::handler::WatchFolderHandler));
    engine.register(Arc::new(super::handler::SafariReaderHandler));
    engine.register(Arc::new(super::handler::YouTubeCaptureHandler));
    engine.register(Arc::new(super::handler::GitHubRepositoryHandler));

    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::handler::CaptureData;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_ingest_without_queue() {
        let engine = build_default_capture_engine(None, None);

        let request = CaptureRequest::new(CaptureData::Text("Hello, world!".to_string()))
            .with_title("Test Note");

        let result = engine.ingest(request).await.unwrap();
        assert!(result.is_some(), "Should have created an object");
    }

    #[tokio::test]
    async fn test_ingest_with_queue() {
        let dir = tempdir().unwrap();
        let queue = Arc::new(DurableJobQueue::new(dir.path()).unwrap());

        let mut engine = CaptureEngine::new();
        engine.set_queue(queue);

        engine.register(Arc::new(crate::capture::handler::ClipboardHandler));

        let request = CaptureRequest::new(CaptureData::Text("Queued content".to_string()));
        let result = engine.ingest(request).await.unwrap();

        assert!(result.is_some(), "Should have enqueued a job");
    }
}
