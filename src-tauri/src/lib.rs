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
    EVENT_ITEM_STORED, EventBus, ItemStored,
};
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::processing::{
    AutoFiler, ContentClassifier, DuplicateDetector, MetadataEnricher, MetadataExtractor, OcrProcessor, PdfAnnotationProcessor, PdfMetadataProcessor, PdfTextProcessor, ProcessingPipeline, TimelineExtractor,
};
use std::sync::Arc;
use tauri::Manager;

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
            let event_bus = Arc::new(EventBus::new());

            // Create processing pipeline and register processors in order
            let pipeline = Arc::new(ProcessingPipeline::new(event_bus.clone()));
            // 1. ContentClassifier - classify documents before other processing
            pipeline.register(Arc::new(ContentClassifier::new()));
            // 2. DuplicateDetector - detect duplicates early
            pipeline.register(Arc::new(DuplicateDetector::without_storage()));
            // 3. TimelineExtractor - extract dates from content
            pipeline.register(Arc::new(TimelineExtractor::new()));
            // 4. OCR - extract text from images/scans
            #[cfg(all(target_os = "macos", target_os = "ios"))]
            pipeline.register(Arc::new(OcrProcessor::new()));
            // 5. MetadataExtractor - extract HTML metadata
            pipeline.register(Arc::new(MetadataExtractor::new()));
            // 6. MetadataEnricher - fill in missing metadata
            pipeline.register(Arc::new(MetadataEnricher::new()));
            // 7. AutoFiler - suggest organisation
            pipeline.register(Arc::new(AutoFiler::new()));
            // 8. PDF processors
            pipeline.register(Arc::new(PdfTextProcessor::new()));
            pipeline.register(Arc::new(PdfMetadataProcessor::new()));
            pipeline.register(Arc::new(PdfAnnotationProcessor::new()));
            let engine = Arc::new(CaptureEngine::new(event_bus.clone()));

            // Register browser capture handlers
            engine.register(Arc::new(BrowserCaptureHandler::new()));
            engine.register(Arc::new(ArticleCaptureHandler::new()));
            engine.register(Arc::new(YouTubeCaptureHandler::new()));
            engine.register(Arc::new(GitHubRepositoryHandler::new()));
            engine.register(Arc::new(ClipboardHandler::default()));
            engine.register(Arc::new(ScreenshotHandler::default()));

            let config = WatchFolderConfig::default();
            match WatchFolderService::new(config, engine.clone(), event_bus.clone()).start() {
                Ok(service) => {
                    app.manage(service);
                }
                Err(e) => {
                    eprintln!("Watch folders disabled: {}", e);
                }
            }

            // Wire canonical Indexer as EVENT_ITEM_STORED subscriber
            // (Principle 6 — One Search Engine)
            let indexer_path = std::path::PathBuf::from(".nabu/index");
            if let Ok(mut indexer) = Indexer::new(indexer_path) {
                let idx_bus = event_bus.clone();
                event_bus.subscribe(EVENT_ITEM_STORED, move |event: &ItemStored| {
                    if let Err(e) = indexer.index_document(&event.knowledge_object) {
                        eprintln!("Indexer failed to index {}: {}", event.id, e);
                    }
                });
            } else {
                eprintln!("Warning: Could not initialize Indexer — search will be unavailable");
            }

            // Wire canonical VaultGraph as EVENT_ITEM_STORED subscriber
            // (Principle 7 — One Graph Engine)
            let vault_graph = Arc::new(std::sync::RwLock::new(VaultGraph::new()));
            let vg = vault_graph.clone();
            event_bus.subscribe(EVENT_ITEM_STORED, move |event: &ItemStored| {
                let mut graph = vg.write().unwrap();
                graph.update_node(&event.knowledge_object);
            });

            // Start native messaging socket server
            let socket_state = Arc::new(crate::native_messaging_socket::SocketServerState {
                engine: engine.clone(),
            });
            match crate::native_messaging_socket::start_socket_server(socket_state) {
                Ok(_handle) => {
                    println!("Native messaging socket server started");
                }
                Err(e) => {
                    eprintln!("Failed to start native messaging socket server: {}", e);
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