pub mod commands;
pub mod history;
pub mod native_messaging;
pub mod native_messaging_socket;
pub mod recovery;
pub mod settings;

// ---------------------------------------------------------------------------
// Architectural notes (R1 — Application Wiring & Canonical Runtime):
//
// 1. One runtime model: every service below is the canonical instance from
//    nabu-core. There is exactly one EventBus, one StorageManager, one
//    ProcessingPipeline, one DurableJobQueue, one WorkerPool, one CaptureEngine,
//    one Indexer and one VaultGraph.
//
// 2. One flow: all content flows Capture → Queue → Workers → Pipeline →
//    Storage → ITEM_STORED → Indexer + VaultGraph. There is no tokio::spawn
//    pipeline bypass and no manual processing path in this file.
//
// 3. Dependency injection: every service is constructed once here and resolved
//    through the ApplicationContext (registered in Tauri managed state). No
//    command or subsystem constructs its own EventBus / StorageManager / queue.
// ---------------------------------------------------------------------------

use nabu_core::capture::CaptureEngine;
use nabu_core::event_bus::kinds;
use nabu_core::event_bus::{EventBus, PipelineEvent};
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::jobs::{DurableJobQueue, ExecutorRegistry, WorkerPool};
use nabu_core::pipeline_migration::PipelineExecutor;
use nabu_core::processing::pipeline::build_standard_pipeline;
use nabu_core::registry::context::ApplicationContext;
use nabu_core::registry::{CATEGORY_CAPTURE_HANDLERS, ServiceRegistry};
use nabu_core::storage::StorageManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::Manager;

/// Builds the canonical application context with every runtime service wired.
///
/// Construction order mirrors the canonical pipeline:
///
/// EventBus
///   → StorageManager (publishes ITEM_STORED on save)
///   → ProcessingPipeline (standard 14-processor pipeline)
///   → DurableJobQueue (file-backed, survives restart)
///   → PipelineExecutor (Worker → Pipeline → Storage handoff)
///   → WorkerPool (pulls jobs, dispatches to the executor)
///   → CaptureEngine (routes captures, enqueues jobs)
///   → Indexer + VaultGraph (subscribe to ITEM_STORED)
///
/// Every service is registered exactly once in the ServiceRegistry and is
/// resolved by key through the [`ApplicationContext`].
fn build_application_context(vault_path: PathBuf) -> ApplicationContext {
    // ---- 1. One EventBus + registry + capabilities ----
    let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
    let registry = Arc::new(RwLock::new(ServiceRegistry::new()));

    let mut capability_registry = nabu_core::plugin::CapabilityRegistry::new();
    capability_registry.register_builtin();

    let ctx = ApplicationContext::new(registry.clone(), event_bus.clone(), capability_registry);

    // Register the event bus itself (ApplicationContext::new does not
    // auto-register it — only ApplicationContextBuilder::build does).
    {
        let mut reg = registry.write().expect("registry lock not poisoned");
        reg.register("event_bus", event_bus.clone());
    }

    // ---- 2. One StorageManager (canonical storage owner) ----
    let storage = Arc::new(StorageManager::with_event_bus(
        vault_path.clone(),
        (*event_bus).clone(),
    ));
    ctx.register("storage_manager", storage.clone());

    // ---- 3. One ProcessingPipeline (standard pipeline, 14 processors) ----
    let pipeline = Arc::new(build_standard_pipeline(Some((*event_bus).clone())));
    ctx.register("pipeline", pipeline.clone());

    // Register processors in the "processors" category for discovery.
    {
        let mut reg = registry.write().expect("registry lock not poisoned");
        for name in pipeline.processor_names() {
            reg.register_in_category(nabu_core::registry::CATEGORY_PROCESSORS, name);
        }
    }

    // ---- 4. One DurableJobQueue ----
    let queue_base = vault_path.join(".nabu").join("queue");
    let queue = Arc::new(DurableJobQueue::new(&queue_base).unwrap_or_else(|e| {
        panic!(
            "Failed to create job queue at {}: {}",
            queue_base.display(),
            e
        )
    }));
    ctx.register("job_queue", queue.clone());

    // ---- 5. One PipelineExecutor (Worker → Pipeline → Storage) ----
    // Registered under every processor name the CaptureEngine can enqueue.
    let executor: Arc<PipelineExecutor> = Arc::new(
        PipelineExecutor::with_event_bus(pipeline.clone(), (*event_bus).clone())
            .with_storage(storage.clone()),
    );
    let mut executors = ExecutorRegistry::new();
    for name in [
        "ocr_processor",
        "whisper_processor",
        "pdf_text_extraction_processor",
        "metadata_extraction_processor",
    ] {
        executors.register(name, executor.clone());
    }
    let executors = Arc::new(executors);

    // ---- 6. One WorkerPool ----
    let worker_pool = Arc::new(WorkerPool::new(4, queue.clone(), executors));
    ctx.register("worker_pool", worker_pool.clone());

    // ---- 7. One CaptureEngine (canonical handlers + queue) ----
    let capture_engine = Arc::new(nabu_core::capture::build_default_capture_engine(
        Some((*event_bus).clone()),
        Some(queue.clone()),
    ));
    ctx.register("capture_engine", capture_engine.clone());

    // Register capture handlers in the "capture_handlers" category.
    {
        let mut reg = registry.write().expect("registry lock not poisoned");
        for name in capture_engine.handler_names() {
            reg.register_in_category(CATEGORY_CAPTURE_HANDLERS, &name);
        }
    }

    // ---- 8. One Indexer (canonical search engine) ----
    let indexer = Arc::new(Mutex::new(Indexer::with_event_bus((*event_bus).clone())));
    ctx.register("indexer", indexer.clone());

    // ---- 9. One VaultGraph (canonical graph engine, persisted) ----
    let vault_graph = Arc::new(RwLock::new(
        VaultGraph::with_persistence(Some((*event_bus).clone()), vault_path)
            .unwrap_or_else(|e| panic!("Failed to initialize VaultGraph: {}", e)),
    ));
    ctx.register("vault_graph", vault_graph.clone());

    // ---- 10. One HistoryManager (universal undo/redo) ----
    let history_manager = Arc::new(RwLock::new(
        nabu_core::history::HistoryManager::new(),
    ));
    ctx.register("history_manager", history_manager.clone());

    // ---- 11. Canonical event flow: ITEM_STORED → Indexer + VaultGraph ----
    // StorageManager.save() publishes ITEM_STORED after persistence. These
    // subscribers are the ONLY consumers of that event: they index the stored
    // object and add it to the graph. No side paths, no skipped stages.
    let storage_for_events = storage.clone();
    let indexer_for_events = indexer.clone();
    let graph_for_events = vault_graph.clone();
    event_bus.subscribe(kinds::ITEM_STORED, move |event: &PipelineEvent| {
        if let PipelineEvent::ItemStored(stored) = event {
            if let Some(object) = storage_for_events.load(stored.object_id) {
                if let Ok(indexer) = indexer_for_events.lock() {
                    if let Err(e) = indexer.index_object(&object) {
                        tracing::error!(event.id = %stored.object_id, error = %e, "Indexer failed to index document");
                    }
                }
                if let Ok(graph) = graph_for_events.write() {
                    if let Err(e) = graph.add_node(&object) {
                        tracing::error!(event.id = %stored.object_id, error = %e, "VaultGraph failed to add node");
                    }
                }
            }
        }
    });

    ctx
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ------------------------------------------------------------------
    // Initialize the Nabu observability foundation.
    // ------------------------------------------------------------------
    // Uses NABU_LOG or RUST_LOG for filtering. Logs are written to
    // .nabu/logs/. Nothing is ever sent to external servers — zero telemetry.
    // ------------------------------------------------------------------
    nabu_core::diagnostics::init(None, "nabu");

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
            crate::history::history_status,
            crate::history::history_undo,
            crate::history::history_redo,
            crate::history::history_clear,
            crate::history::history_set_depth,
            crate::history::note_rename,
            crate::history::note_delete,
            crate::history::note_restore,
            crate::history::trash_list,
            crate::history::trash_delete,
            crate::history::trash_restore_many,
            crate::history::trash_purge_expired,
            crate::history::trash_empty,
            crate::history::folder_create,
            crate::history::folder_rename,
            crate::history::note_duplicate,
            crate::history::items_move,
            crate::commands::tree_list,
            crate::commands::reveal_in_file_manager,
            crate::commands::reveal_vault_in_file_manager,
            crate::commands::notes_index,
            crate::commands::notes_search,
            crate::commands::graph_data,
            crate::commands::note_links,
            crate::commands::link_mention,
            crate::commands::mention_ignore,
            crate::commands::mention_ignore_list,
            crate::commands::archive_note,
            crate::commands::archive_restore,
            crate::commands::archive_list,
            crate::commands::smart_folders_list,
            crate::commands::smart_folder_save,
            crate::commands::smart_folder_delete,
            crate::commands::smart_folder_evaluate,
            crate::commands::calendar_notes,
            crate::commands::daily_note_for,
            crate::commands::template_list,
            crate::commands::template_save,
            crate::commands::template_delete,
            crate::commands::template_duplicate,
            crate::commands::template_set_favourite,
            crate::commands::inbox_quick_capture,
            crate::commands::get_settings,
            crate::commands::settings_get,
            crate::commands::settings_set,
            crate::commands::settings_set_all,
            crate::commands::settings_export,
            crate::commands::settings_import,
            crate::commands::settings_reset,
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
            crate::commands::capture_file_drop,
            crate::commands::queue_get_all,
            crate::commands::queue_set_status,
            crate::commands::queue_set_priority,
            crate::commands::queue_set_progress,
            crate::commands::queue_batch_set_status,
            crate::commands::queue_archive_completed,
            crate::commands::canvas_list,
            crate::commands::canvas_get,
            crate::commands::canvas_save,
            crate::commands::canvas_delete,
            crate::commands::notes_diff,
            crate::commands::statistics_get,
            crate::recovery::note_save,
            crate::recovery::note_read,
            crate::recovery::versions_list,
            crate::recovery::versions_get,
            crate::recovery::versions_restore,
            crate::recovery::versions_duplicate,
            crate::recovery::versions_diff,
            crate::recovery::snapshot_create,
            crate::recovery::versions_all,
            crate::recovery::session_save,
            crate::recovery::session_load,
            crate::recovery::session_clear,
            crate::recovery::recovery_check,
            crate::recovery::recovery_discard,
            // Phase 15.1 — platform integrations.
            crate::commands::open_app_in_finder,
            crate::commands::show_macos_notification,
            crate::commands::pin_to_taskbar,
            crate::commands::open_in_explorer,
            crate::commands::open_in_file_manager,
            crate::commands::show_linux_notification,
            crate::commands::install_desktop_entry,
        ])
        .setup(|app| {
            // ------------------------------------------------------------------
            // Build the canonical application context from the current vault.
            // ------------------------------------------------------------------
            let vault_path = {
                let settings = app.state::<crate::settings::SettingsStore>().get();
                let path = settings.last_vault_path.trim().to_string();
                if path.is_empty() {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                } else {
                    PathBuf::from(path)
                }
            };

            // Crash recovery marker: a leftover `.running` file means the
            // previous run died unexpectedly. Write a `.recovery_pending`
            // marker so the frontend can offer to restore the last session.
            crate::recovery::mark_running(&vault_path);

            let ctx = build_application_context(vault_path);
            let engine: Arc<CaptureEngine> = ctx
                .resolve("capture_engine")
                .expect("CaptureEngine must be registered");
            let pool: Option<Arc<WorkerPool>> = ctx.worker_pool();

            // Initialize the context lifecycle (validates core services) BEFORE
            // moving ctx into Tauri managed state.
            if let Err(missing) = ctx.initialize() {
                tracing::warn!(missing = ?missing, "Application context initialization incomplete");
            }
            ctx.start();

            // Make the context available to commands via Tauri managed state.
            app.manage(ctx);

            // Start the canonical worker pool on the Tauri async runtime.
            if let Some(pool) = pool {
                tauri::async_runtime::spawn(async move {
                    pool.start().await;
                });
            }

            // Start native messaging socket server. The tokio listener requires a
            // running reactor, so the server is started inside the Tauri async
            // runtime rather than on the setup (main) thread.
            let socket_state = Arc::new(crate::native_messaging_socket::SocketServerState {
                engine: engine.clone(),
            });
            tauri::async_runtime::spawn(async move {
                match crate::native_messaging_socket::start_socket_server(socket_state) {
                    Ok(_handle) => {
                        tracing::info!("Native messaging socket server started");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to start native messaging socket server");
                    }
                }
            });

            // Safety net: the main window starts hidden (visible: false) and is
            // shown by on_page_load once the webview finishes painting. If the
            // page never finishes loading (e.g. dev-server hiccup or a wasm
            // fetch failure), force-show the window after a short delay so the
            // app is never left with no visible window at all.
            if let Some(main_window) = app.get_webview_window("main") {
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                    if !main_window.is_visible().unwrap_or(false) {
                        let _ = main_window.show();
                        let _ = main_window.set_focus();
                    }
                });
            }

            Ok(())
        })
        // The main window is created with `visible: false` (see tauri.conf.json)
        // so the webview is never shown mid-paint. Show it only once the page
        // has finished loading — otherwise macOS paints an opaque white
        // webview before any HTML renders, causing a white startup flash.
        .on_page_load(|window, payload| {
            if window.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                let _ = window.show();
                let _ = window.set_focus();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // On graceful exit, remove the `.running` marker so the next
            // launch knows the session ended cleanly (no crash recovery UI).
            if let tauri::RunEvent::Exit = event {
                let settings = app_handle.state::<crate::settings::SettingsStore>().get();
                let path = settings.last_vault_path.trim().to_string();
                if !path.is_empty() {
                    crate::recovery::mark_clean_exit(&std::path::PathBuf::from(path));
                }
            }
        });
}

fn dirs_next() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|h| std::path::PathBuf::from(h).join(".config"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}
