pub mod commands;
pub mod export_engine;
pub mod models;
pub mod native;
pub mod native_messaging;
pub mod native_messaging_socket;
pub mod settings;
pub mod template_manager;
pub mod vault;

// ---------------------------------------------------------------------------
// Architectural note (Principle 6 — One Search Engine):
// The canonical Indexer lives in nabu_core and is the only Tantivy instance.
// No duplicate search engine exists — src-tauri/search.rs has been removed.
//
// Architectural note (Principle 7 — One Graph Engine):
// The canonical VaultGraph lives in nabu_core and is the only Petgraph instance.
// No duplicate graph engine exists — src-tauri/graph.rs has been removed.
// ---------------------------------------------------------------------------

pub use nabu_core::markdown::{Document, ParseError, parse};

use nabu_core::capture::{
    ArticleCaptureHandler, BrowserCaptureHandler, CaptureEngine, ClipboardHandler,
    GitHubRepositoryHandler, ScreenshotHandler, WatchFolderConfig, WatchFolderService,
    YouTubeCaptureHandler,
};
use nabu_core::event_bus::{
    EVENT_ITEM_PROCESSED, EVENT_ITEM_STORED, EventBus, ItemProcessed, ItemStored,
};
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::job_queue::{BackgroundJob, JobPriority, JobQueue, JobType, WorkerPool};
use nabu_core::processing::{
    AutoFiler, ContentClassifier, DuplicateDetector, MetadataEnricher, MetadataExtractor,
    OcrProcessor, PdfAnnotationProcessor, PdfMetadataProcessor, PdfTextProcessor,
    ProcessingPipeline, TimelineExtractor,
};
use nabu_core::processing::PROCESSING_HISTORY_KEY;
use nabu_core::registry::context::ApplicationContext;
use nabu_core::registry::{CATEGORY_CAPTURE_HANDLERS, CATEGORY_PROCESSORS};
use nabu_core::registry::ServiceRegistry;
use std::sync::{Arc, RwLock};
use tauri::Manager;

/// Builds the application context with all services registered.
///
/// This function centralizes all service construction and registration,
/// replacing the previous inline construction in the Tauri `setup` closure.
/// The returned [`ApplicationContext`] holds the [`ServiceRegistry`] with all
/// services, and the [`EventBus`] for publish/subscribe communication.
fn build_application_context() -> ApplicationContext {
    let event_bus = Arc::new(EventBus::new());
    let registry = Arc::new(RwLock::new(ServiceRegistry::new()));

    // Register the event bus itself (needed by many services)
    {
        let mut reg = registry.write().unwrap();
        reg.register("event_bus", event_bus.clone());
    }

    // ------------------------------------------------------------------
    // 1. Build and register the ProcessingPipeline
    // ------------------------------------------------------------------
    // Uses new_no_subscribe() because execution is managed by the background
    // JobQueue rather than running inline on the EventBus.
    let pipeline = ProcessingPipeline::new_no_subscribe(event_bus.clone());
    {
        let mut reg = registry.write().unwrap();
        reg.register("pipeline", pipeline.clone());
    }

    // Register all processors in order
    // Each processor is also registered in the "processors" category for
    // discovery by future tooling (e.g., diagnostics, admin UI).
    {
        let mut reg = registry.write().unwrap();

        let processors: Vec<(&str, Arc<dyn nabu_core::processing::Processor>)> = vec![
            ("content_classifier", Arc::new(ContentClassifier::new())),
            ("duplicate_detector", Arc::new(DuplicateDetector::without_storage())),
            ("timeline_extractor", Arc::new(TimelineExtractor::new())),
            ("metadata_extractor", Arc::new(MetadataExtractor::new())),
            ("metadata_enricher", Arc::new(MetadataEnricher::new())),
            ("auto_filer", Arc::new(AutoFiler::new())),
            ("pdf_text", Arc::new(PdfTextProcessor::new())),
            ("pdf_metadata", Arc::new(PdfMetadataProcessor::new())),
            ("pdf_annotations", Arc::new(PdfAnnotationProcessor::new())),
        ];

        for (key, processor) in &processors {
            reg.register(key, processor.clone());
            reg.register_in_category(CATEGORY_PROCESSORS, key);
        }

        // Register ocr_processor only on macOS/iOS
        #[cfg(all(target_os = "macos", target_os = "ios"))]
        {
            reg.register("ocr_processor", Arc::new(OcrProcessor::new()));
            reg.register_in_category(CATEGORY_PROCESSORS, "ocr_processor");
        }
    }

    // Register processors with the pipeline (preserving order from the
    // original setup to maintain identical behaviour).
    pipeline.register(Arc::new(ContentClassifier::new()));
    pipeline.register(Arc::new(DuplicateDetector::without_storage()));
    pipeline.register(Arc::new(TimelineExtractor::new()));
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    pipeline.register(Arc::new(OcrProcessor::new()));
    pipeline.register(Arc::new(MetadataExtractor::new()));
    pipeline.register(Arc::new(MetadataEnricher::new()));
    pipeline.register(Arc::new(AutoFiler::new()));
    pipeline.register(Arc::new(PdfTextProcessor::new()));
    pipeline.register(Arc::new(PdfMetadataProcessor::new()));
    pipeline.register(Arc::new(PdfAnnotationProcessor::new()));

    // ------------------------------------------------------------------
    // 2. Build and register the CaptureEngine
    // ------------------------------------------------------------------
    let engine = Arc::new(CaptureEngine::new(event_bus.clone()));
    {
        let mut reg = registry.write().unwrap();
        reg.register("capture_engine", engine.clone());
    }

    // Register capture handlers
    {
        let mut reg = registry.write().unwrap();

        let handlers: Vec<(&str, Arc<dyn nabu_core::capture::CaptureHandler>)> = vec![
            ("browser", Arc::new(BrowserCaptureHandler::new())),
            ("article", Arc::new(ArticleCaptureHandler::new())),
            ("youtube", Arc::new(YouTubeCaptureHandler::new())),
            ("github", Arc::new(GitHubRepositoryHandler::new())),
            ("clipboard", Arc::new(ClipboardHandler::default())),
            ("screenshot", Arc::new(ScreenshotHandler::default())),
        ];

        for (key, handler) in &handlers {
            reg.register(key, handler.clone());
            reg.register_in_category(CATEGORY_CAPTURE_HANDLERS, key);
        }
    }

    // Register handlers with the engine (preserving the exact same set)
    engine.register(Arc::new(BrowserCaptureHandler::new()));
    engine.register(Arc::new(ArticleCaptureHandler::new()));
    engine.register(Arc::new(YouTubeCaptureHandler::new()));
    engine.register(Arc::new(GitHubRepositoryHandler::new()));
    engine.register(Arc::new(ClipboardHandler::default()));
    engine.register(Arc::new(ScreenshotHandler::default()));

    // ------------------------------------------------------------------
    // 3. Build the JobQueue and WorkerPool
    // ------------------------------------------------------------------
    let job_queue = JobQueue::new(pipeline.clone(), event_bus.clone());
    let worker_pool = WorkerPool::new(4, job_queue.clone());
    {
        let mut reg = registry.write().unwrap();
        reg.register("job_queue", job_queue.clone());
        reg.register("worker_pool", worker_pool.clone());
    }

    // Subscribe to ItemProcessed — enqueue background jobs
    // instead of running the pipeline inline.
    let enqueue_jq = job_queue.clone();
    event_bus.subscribe(EVENT_ITEM_PROCESSED, move |event: &ItemProcessed| {
        if event
            .knowledge_object
            .metadata
            .custom
            .contains_key(PROCESSING_HISTORY_KEY)
        {
            return;
        }

        let job = BackgroundJob::new(
            nabu_core::job_queue::JobType::ProcessKnowledgeObject,
            JobPriority::Normal,
            event.knowledge_object.clone(),
        );
        enqueue_jq.enqueue(job);
    });

    // ------------------------------------------------------------------
    // 4. Wire the Indexer as EVENT_ITEM_STORED subscriber (Principle 6)
    // ------------------------------------------------------------------
    let indexer_path = std::path::PathBuf::from(".nabu/index");
    if let Ok(mut indexer) = Indexer::new(indexer_path) {
        let idx_bus = event_bus.clone();
        event_bus.subscribe(EVENT_ITEM_STORED, move |event: &ItemStored| {
            if let Err(e) = indexer.index_document(&event.knowledge_object) {
                tracing::error!(event.id = %event.id, error = %e, "Indexer failed to index document");
            }
        });
    } else {
        tracing::warn!("Could not initialize Indexer — search will be unavailable");
    }

    // ------------------------------------------------------------------
    // 5. Build and register the VaultGraph (Principle 7)
    // ------------------------------------------------------------------
    let graph_vault_path = std::path::PathBuf::from(".");
    let vault_graph = Arc::new(std::sync::RwLock::new(
        VaultGraph::with_storage(graph_vault_path),
    ));
    {
        let mut reg = registry.write().unwrap();
        reg.register("vault_graph", vault_graph.clone());
    }

    let vg = vault_graph.clone();
    event_bus.subscribe(EVENT_ITEM_STORED, move |event: &ItemStored| {
        let mut graph = vg.write().unwrap();
        graph.update_node(&event.knowledge_object);
    });

    // ------------------------------------------------------------------
    // 6. Build and return the ApplicationContext
    // ------------------------------------------------------------------
    let ctx = ApplicationContext::new(registry, event_bus);
    ctx.initialize();
    ctx.start();
    ctx
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings_path = std::env::var("NABU_SETTINGS_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| dirs_next().join("nabu").join("settings.json"));

    let settings_store = crate::settings::SettingsStore::load(&settings_path)
        .unwrap_or_else(|_| crate::settings::SettingsStore::new(settings_path));

    tauri::Builder::default()
        .manage(settings_store)
        .invoke_handler(tauri::generate_handler![
            crate::commands::check_vault_exists,
            crate::commands::get_current_vault,
            crate::commands::select_vault_dialog,
            crate::commands::create_vault_dialog,
            crate::commands::open_dictation_pill,
            crate::commands::close_dictation_pill,
            crate::commands::toggle_dictation_pill,
            crate::commands::start_dictation,
            crate::commands::stop_dictation,
            crate::commands::complete_setup,
            crate::commands::open_settings,
            crate::commands::note_create_file,
            crate::commands::note_daily,
            crate::commands::get_settings,
            crate::commands::settings_get,
            crate::commands::settings_set,
            crate::commands::settings_set_all,
            crate::commands::inbox_subscribe,
            crate::commands::inbox_get_queue,
            crate::commands::inbox_approve,
            crate::commands::inbox_reject,
            crate::commands::inbox_retry,
            crate::commands::inbox_delete,
            crate::commands::inbox_batch_approve,
            crate::commands::inbox_batch_reject,
            crate::commands::inbox_batch_delete,
            crate::commands::inbox_batch_retry,
            crate::commands::inbox_edit_metadata,
            crate::commands::inbox_move,
            crate::commands::queue_get_all,
            crate::commands::queue_set_status,
            crate::commands::queue_set_priority,
            crate::commands::queue_set_progress,
            crate::commands::queue_batch_set_status,
            crate::commands::queue_archive_completed,
        ])
        .setup(|app| {
            // Build the application context with all services
            let ctx = build_application_context();

            // Retrieve services for wiring into the Tauri managed state
            let engine: Arc<CaptureEngine> = ctx
                .resolve("capture_engine")
                .expect("CaptureEngine must be registered");
            let event_bus = ctx.event_bus().clone();

            // Start native messaging socket server
            let socket_state = Arc::new(crate::native_messaging_socket::SocketServerState {
                engine: engine.clone(),
            });
            match crate::native_messaging_socket::start_socket_server(socket_state) {
                Ok(_handle) => {
                    tracing::info!("Native messaging socket server started");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to start native messaging socket server");
                }
            }

            // Wire WatchFolderService (requires app.manage for Tauri state)
            let config = WatchFolderConfig::default();
            match WatchFolderService::new(config, engine.clone(), event_bus.clone()).start() {
                Ok(service) => {
                    app.manage(service);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Watch folders disabled");
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn dirs_next() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|h| std::path::PathBuf::from(h).join(".config"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}
