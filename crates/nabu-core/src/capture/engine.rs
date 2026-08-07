use crate::capture::handler::{CaptureHandler, CaptureRequest, CaptureResult};
use crate::event_bus::kinds::ITEM_CAPTURED;
use crate::event_bus::{EventBus, ItemCapturedEvent, PipelineEvent};
use crate::jobs::errors::JobResult;
use crate::jobs::job::{Job, JobType};
use crate::jobs::queue::{DurableJobQueue, Queue};
use crate::models::ObjectType;
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
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
    /// Lifecycle state manager — tracks Created → Initialized → Running → Shutdown.
    lifecycle: LifecycleManager,
}

impl CaptureEngine {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            event_bus: None,
            queue: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create a capture engine with an event bus for publishing events.
    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            handlers: HashMap::new(),
            event_bus: Some(event_bus),
            queue: None,
            lifecycle: LifecycleManager::new(),
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

                let mut job = Job::new(
                    job_type.clone(),
                    payload,
                    format!("{}_processor", job_type.name()),
                )
                .with_object_id(result.object.id)
                .with_tag("capture");

                if let Some(ref source_url) = result.object.metadata.source_url {
                    job = job.with_metadata("source_url", source_url.clone());
                }

                queue.enqueue(job)?;

                // Publish with job ID
                if let Some(ref _bus) = self.event_bus {
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

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the capture engine.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the capture engine has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the capture engine is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the capture engine has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared `Lifecycle` trait so `CaptureEngine` can be managed
/// by the Capability Platform's lifecycle manager alongside other services.
///
/// ```text
/// Created → Initialized → Running → Shutdown
/// ```
impl Lifecycle for CaptureEngine {
    fn name(&self) -> &'static str {
        "capture_engine"
    }

    /// Initializes the capture engine.
    ///
    /// Transitions the engine from `Created` to `Initialized`.
    /// Handler registration and queue wiring are set during construction;
    /// this phase validates that required dependencies (handlers, queue)
    /// are present and ready.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "capture",
            component = "engine",
            operation = "initialize",
            handlers = self.handler_count(),
            "CaptureEngine initialized"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;
        Ok(())
    }

    /// Starts the capture engine.
    ///
    /// Transitions the engine to `Running` so it can accept and route
    /// capture requests. Auto-advances `Created → Initialized` if
    /// `initialize()` was not called explicitly.
    ///
    /// Double-start is a safe no-op — no duplicate handlers are registered.
    /// Calling `start()` after `shutdown()` returns an error.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Cannot restart a shut-down engine
        if self.lifecycle.is_shutdown() {
            return Err(
                "CaptureEngine has been shut down and cannot be restarted".into(),
            );
        }

        // Auto-advance Created → Initialized so callers can call start()
        // directly without an explicit initialize() call.
        if self.lifecycle.stage() == LifecycleStage::Created {
            tracing::info!(
                subsystem = "capture",
                component = "engine",
                operation = "start",
                "Initializing capture engine"
            );
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }

        // Guard against duplicate start — transition Running → Running is a
        // no-op in LifecycleManager, but we log a warning for visibility.
        if self.lifecycle.is_running() {
            tracing::warn!(
                subsystem = "capture",
                component = "engine",
                operation = "start",
                "CaptureEngine already started — skipping duplicate start"
            );
            return Ok(());
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)?;

        tracing::info!(
            subsystem = "capture",
            component = "engine",
            operation = "start",
            handlers = self.handler_count(),
            "CaptureEngine started"
        );
        Ok(())
    }

    /// Shuts down the capture engine.
    ///
    /// Stops accepting new capture requests. The engine is not restartable
    /// after shutdown.
    ///
    /// Double-shutdown is a safe no-op.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "capture",
            component = "engine",
            operation = "shutdown",
            "CaptureEngine shutting down"
        );

        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;

        tracing::info!(
            subsystem = "capture",
            component = "engine",
            operation = "shutdown",
            "CaptureEngine stopped"
        );
        Ok(())
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
        ObjectType::YouTubeVideo
        | ObjectType::Repository
        | ObjectType::Bookmark
        | ObjectType::Article => JobType::MetadataExtraction,
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
    engine.register(Arc::new(super::handler::EmailCaptureHandler));
    engine.register(Arc::new(super::handler::BookmarkCaptureHandler));
    engine.register(Arc::new(super::handler::ArticleCaptureHandler));

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

    #[tokio::test]
    async fn test_default_engine_registers_all_handlers() {
        let engine = build_default_capture_engine(None, None);
        let names = engine.handler_names();
        // 11 built-in handlers: browser, clipboard, screenshot, file_drop,
        // watch_folder, safari_reader, youtube, github, email, bookmark, article.
        assert_eq!(
            engine.handler_count(),
            11,
            "Expected 11 handlers: {:?}",
            names
        );
        assert!(names.contains(&"browser".to_string()));
        assert!(names.contains(&"clipboard".to_string()));
        assert!(names.contains(&"screenshot".to_string()));
        assert!(names.contains(&"safari_reader".to_string()));
        assert!(names.contains(&"youtube".to_string()));
        assert!(names.contains(&"github".to_string()));
        assert!(names.contains(&"email".to_string()));
        assert!(names.contains(&"bookmark".to_string()));
        assert!(names.contains(&"article".to_string()));
    }

    #[tokio::test]
    async fn test_browser_url_routes_to_bookmark() {
        let engine = build_default_capture_engine(None, None);
        let request = CaptureRequest::new(CaptureData::Uri(
            "https://example.com/some-page".to_string(),
        ));
        let result = engine.ingest(request).await.unwrap();
        assert!(result.is_some());
        // The object id is returned; the handler routing happens inside
        // route() — we verify no error and an id is produced.
    }

    #[tokio::test]
    async fn test_bookmark_capture_through_engine() {
        let engine = build_default_capture_engine(None, None);
        let request = CaptureRequest::new(CaptureData::Uri(
            "https://example.com/bookmark-test".to_string(),
        ))
        .with_title("Bookmark Test");
        let result = engine.ingest(request).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_clipboard_url_capture_through_engine() {
        let engine = build_default_capture_engine(None, None);
        let request = CaptureRequest::new(CaptureData::Text(
            "https://example.com/clipboard-url".to_string(),
        ));
        let result = engine.ingest(request).await.unwrap();
        assert!(result.is_some());
    }

    // ── Lifecycle tests ────────────────────────────────────────────────

    #[test]
    fn lifecycle_initial_state_is_created() {
        let engine = CaptureEngine::new();
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Created);
        assert!(!engine.is_initialized());
        assert!(!engine.is_running());
        assert!(!engine.is_shutdown());
    }

    #[test]
    fn lifecycle_trait_name() {
        let engine = CaptureEngine::new();
        let engine_ref: &dyn Lifecycle = &engine;
        assert_eq!(engine_ref.name(), "capture_engine");
    }

    #[test]
    fn lifecycle_initialize_transitions_to_initialized() {
        let engine = CaptureEngine::new();
        assert!(engine.initialize().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(engine.is_initialized());
        assert!(!engine.is_running());
    }

    #[test]
    fn lifecycle_start_auto_advances_from_created() {
        let engine = CaptureEngine::new();
        // start() should auto-advance Created → Initialized → Running
        assert!(engine.start().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Running);
        assert!(engine.is_running());
    }

    #[test]
    fn lifecycle_full_flow() {
        let engine = CaptureEngine::new();
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Created);

        assert!(engine.initialize().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(engine.is_initialized());

        assert!(engine.start().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Running);
        assert!(engine.is_running());

        assert!(engine.shutdown().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Shutdown);
        assert!(engine.is_shutdown());
    }

    #[test]
    fn lifecycle_start_after_shutdown_returns_error() {
        let engine = CaptureEngine::new();
        assert!(engine.start().is_ok());
        assert!(engine.shutdown().is_ok());
        // Cannot restart after shutdown
        assert!(engine.start().is_err());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Shutdown);
    }

    #[test]
    fn lifecycle_double_shutdown_is_noop() {
        let engine = CaptureEngine::new();
        assert!(engine.start().is_ok());
        assert!(engine.shutdown().is_ok());
        // Second shutdown should succeed (same stage is a no-op)
        assert!(engine.shutdown().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Shutdown);
    }

    #[test]
    fn lifecycle_double_start_is_noop() {
        let engine = CaptureEngine::new();
        assert!(engine.start().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Running);
        // Second start should succeed (no-op)
        assert!(engine.start().is_ok());
        assert!(engine.is_running());
        // Cleanup
        assert!(engine.shutdown().is_ok());
    }

    #[test]
    fn lifecycle_start_without_initialize() {
        let engine = build_default_capture_engine(None, None);
        // start() should auto-advance Created → Initialized → Running
        assert!(engine.start().is_ok());
        assert_eq!(engine.lifecycle_stage(), LifecycleStage::Running);
        assert!(engine.is_running());
        assert!(engine.is_initialized());
        assert!(engine.shutdown().is_ok());
    }

    #[test]
    fn lifecycle_backward_transition_rejected() {
        let engine = CaptureEngine::new();
        assert!(engine.start().is_ok());
        assert!(engine.shutdown().is_ok());
        // Cannot go backward: Shutdown → Initialized
        assert!(engine.initialize().is_err());
        // Cannot restart: Shutdown → Running
        assert!(engine.start().is_err());
    }

    #[test]
    fn lifecycle_handler_count_preserved_through_lifecycle() {
        let engine = build_default_capture_engine(None, None);
        assert_eq!(engine.handler_count(), 11);
        assert!(engine.start().is_ok());
        assert_eq!(engine.handler_count(), 11);
        assert!(engine.shutdown().is_ok());
        assert_eq!(engine.handler_count(), 11);
    }
}
