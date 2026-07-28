pub mod commands;
pub mod export_engine;
pub mod models;
pub mod native;
pub mod native_messaging;
pub mod native_messaging_socket;
pub mod settings;
pub mod template_manager;
pub mod vault;

pub use nabu_core::markdown::{Document, ParseError, parse};

use nabu_core::capture::{
    ArticleCaptureHandler, BrowserCaptureHandler, CaptureEngine, ClipboardHandler,
    GitHubRepositoryHandler, ScreenshotHandler, WatchFolderConfig, WatchFolderService,
    YouTubeCaptureHandler,
};
use nabu_core::event_bus::EventBus;
use nabu_core::processing::{
    DuplicateDetector, MetadataExtractor, OcrProcessor, PdfAnnotationProcessor, PdfMetadataProcessor, PdfTextProcessor, ProcessingPipeline, TimelineExtractor,
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
            crate::commands::settings_set_all
        ])
        .setup(|app| {
            let event_bus = Arc::new(EventBus::new());

            // Create processing pipeline and register processors in order
            let pipeline = Arc::new(ProcessingPipeline::new(event_bus.clone()));
            pipeline.register(Arc::new(DuplicateDetector::without_storage()));
            pipeline.register(Arc::new(TimelineExtractor::new()));
            #[cfg(all(target_os = "macos", target_os = "ios"))]
            pipeline.register(Arc::new(OcrProcessor::new()));
            pipeline.register(Arc::new(MetadataExtractor::new()));
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