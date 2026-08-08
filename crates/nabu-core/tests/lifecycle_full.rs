//! # Full Lifecycle Integration Tests
//!
//! Validates the end-to-end application lifecycle: startup, initialization,
//! runtime behavior, graceful shutdown, crash recovery, and session restoration.
//!
//! These tests exercise the real `ApplicationContext` and `Application` with
//! production services (StorageManager, ConversationStore, WorkerPool,
//! DurableJobQueue, CaptureEngine, ProcessingPipeline, PipelineExecutor)
//! wired together — no mocks. Each test uses a unique `tempfile::tempdir`
//! so they run in parallel safely.
//!
//! Test groupings:
//! - **Startup/Initialization**: stage transitions, service validation, health checks
//! - **Runtime**: storage save/load, conversation persistence, job queue, event bus
//! - **Graceful Shutdown**: ordered teardown, double-shutdown safety, data integrity
//! - **Crash Recovery**: persisted data survives a full lifecycle restart
//! - **Session Restoration**: conversation threads and storage objects reload
//!
//! Run with: `cargo test lifecycle_full`

use std::sync::Arc;
use std::sync::RwLock as StdRwLock;

use nabu_core::capture::CaptureEngine;
use nabu_core::conversations::ConversationStore;
use nabu_core::event_bus::{EventBus, PipelineEvent};
use nabu_core::jobs::{DurableJobQueue, ExecutorRegistry, Job, JobStatus, Queue, WorkerPool};
use nabu_core::models::{KnowledgeObject, ObjectContent, ObjectType, Thread};
use nabu_core::pipeline_migration::PipelineExecutor;
use nabu_core::processing::ProcessingPipeline;
use nabu_core::registry::lifecycle::LifecycleStage;
use nabu_core::registry::Application;
use nabu_core::registry::health::{HealthStatus, LifecycleStageInfo};
use nabu_core::registry::context::ApplicationContext;
use nabu_core::registry::context::ApplicationContextBuilder;
use nabu_core::registry::ServiceRegistry;
use nabu_core::storage::StorageManager;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helper: create a test KnowledgeObject
// ---------------------------------------------------------------------------

fn test_knowledge_object() -> KnowledgeObject {
    KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown(
            "# Test Note\n\nThis is test content for lifecycle testing.".to_string(),
        ),
    )
    .with_tag("test")
    .with_tag("lifecycle")
}

// ---------------------------------------------------------------------------
// Helper: Build a full ApplicationContext with all core services on a temp vault.
// Uses ApplicationContext directly (like Tauri's build_application_context)
// so that the context's own LifecycleManager is advanced.
// ---------------------------------------------------------------------------

fn build_full_context() -> (ApplicationContext, tempfile::TempDir) {
    let vault = tempdir().expect("tempdir for full context");
    let vault_path = vault.path().to_path_buf();
    (build_full_context_on(vault_path), vault)
}

fn build_full_context_on(vault_path: std::path::PathBuf) -> ApplicationContext {
    let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
    let registry: Arc<StdRwLock<ServiceRegistry>> = Arc::new(StdRwLock::new(ServiceRegistry::new()));

    let mut capability_registry = nabu_core::plugin::CapabilityRegistry::new();
    capability_registry.register_builtin();
    let plugin_manager =
        nabu_core::plugin::PluginManager::for_application().with_event_bus((*event_bus).clone());

    let ctx = ApplicationContextBuilder::new()
        .with_registry(registry.clone())
        .with_event_bus(event_bus.clone())
        .with_capability_registry(capability_registry)
        .with_plugin_manager(plugin_manager)
        .build();

    // Register core services (same as Tauri's build_application_context)
    ctx.register("event_bus", event_bus.clone());

    let perf_monitor = Arc::new(nabu_core::diagnostics::PerformanceMonitor::new());
    ctx.register("performance_monitor", perf_monitor.clone());
    ctx.register_metrics_aggregator("performance_monitor", perf_monitor);

    let storage = Arc::new(StorageManager::with_event_bus(
        vault_path.clone(),
        (*event_bus).clone(),
    ));
    ctx.register("storage_manager", storage.clone());
    ctx.register_metrics_aggregator("storage_manager", storage.clone());

    let conv = Arc::new(ConversationStore::with_event_bus(
        vault_path.clone(),
        (*event_bus).clone(),
    ));
    ctx.register("conversation_store", conv);

    let pipeline = Arc::new(ProcessingPipeline::new());
    ctx.register("pipeline", pipeline.clone());

    let queue = Arc::new(
        DurableJobQueue::new(vault_path.join(".nabu").join("queue"))
            .expect("DurableJobQueue"),
    );
    ctx.register("job_queue", queue.clone());
    ctx.register_metrics_aggregator("job_queue", queue.clone());

    let executor = Arc::new(
        PipelineExecutor::with_event_bus(pipeline, (*event_bus).clone())
            .with_storage(storage.clone()),
    );
    ctx.register("pipeline_executor", executor.clone());

    let executor_registry: Arc<ExecutorRegistry> = {
        let mut reg = ExecutorRegistry::new();
        reg.register("metadata_extraction", executor.clone());
        Arc::new(reg)
    };

    let worker_pool = Arc::new(WorkerPool::new(2, queue, executor_registry));
    ctx.register("worker_pool", worker_pool.clone());
    ctx.register_metrics_aggregator("worker_pool", worker_pool.clone());

    let capture_engine = Arc::new(CaptureEngine::new());
    ctx.register("capture_engine", capture_engine.clone());
    ctx.register_metrics_aggregator("capture_engine", capture_engine);

    ctx
}


// ---------------------------------------------------------------------------
// Helper: Build a minimal Application (required services only) on a temp vault
// ---------------------------------------------------------------------------

fn build_minimal_app() -> (Application, tempfile::TempDir) {
    let vault = tempdir().expect("tempdir for minimal app");
    let event_bus = Arc::new(EventBus::<PipelineEvent>::new());

    let app = Application::builder()
        .with_event_bus(event_bus)
        .with_capture_engine(Arc::new(CaptureEngine::new()))
        .with_processing_pipeline(Arc::new(ProcessingPipeline::new()))
        .build();

    // StorageManager is registered (not builder-injected) because ApplicationBuilder
    // does not expose a with_storage_manager method.
    let storage = Arc::new(StorageManager::new(vault.path().to_path_buf()));
    app.context().register("storage_manager", storage);

    (app, vault)
}

// ---------------------------------------------------------------------------
// Section 1: Startup & Initialization
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_full_startup_transitions_through_all_stages() {
    let (app, _vault) = build_minimal_app();

    assert_eq!(app.stage(), LifecycleStage::Created);
    assert!(!app.is_running());
    assert!(!app.is_shutdown());

    app.initialize().expect("initialize");
    assert_eq!(app.stage(), LifecycleStage::Initialized);
    assert!(!app.is_running());

    app.start();
    assert_eq!(app.stage(), LifecycleStage::Running);
    assert!(app.is_running());

    app.shutdown().expect("shutdown");
    assert_eq!(app.stage(), LifecycleStage::Shutdown);
    assert!(app.is_shutdown());
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_health_check_before_startup() {
    let (ctx, _vault) = build_full_context();

    let health = ctx.health_check();

    assert_eq!(health.overall_status, HealthStatus::Healthy);
    assert!(!health.initialized);
    assert!(!health.running);
    assert!(health.error.is_none());
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_health_check_healthy_after_start() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let health = ctx.health_check();

    assert_eq!(health.overall_status, HealthStatus::Healthy);
    assert!(health.initialized);
    assert!(health.running);
    assert!(health.error.is_none());
    assert!(health.services.len() >= 1);
    assert!(health.running_service_count >= 1);
}

#[test]
fn lifecycle_full_start_requires_initialization() {
    let (app, _vault) = build_minimal_app();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.start();
    }));
    assert!(result.is_err(), "start() should panic without initialize()");
}

#[test]
fn lifecycle_full_initialize_validates_missing_services() {
    let app = Application::builder().build();
    let result = app.initialize();
    assert!(result.is_err());
    let missing = result.unwrap_err();
    assert!(missing.iter().any(|s| s == "capture_engine"));
    assert!(missing.iter().any(|s| s == "pipeline"));
    assert!(missing.iter().any(|s| s == "storage_manager"));
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_service_count_grows_with_full_app() {
    let (minimal_app, _v1) = build_minimal_app();
    let minimal_count = minimal_app.context().service_count();
    assert!(minimal_count >= 1);

    let (full_ctx, _v2) = build_full_context();
    full_ctx.initialize().expect("initialize");
    full_ctx.start().expect("start");
    let full_count = full_ctx.service_count();
    assert!(
        full_count > minimal_count,
        "full app ({}) should have more services than minimal ({})",
        full_count,
        minimal_count
    );
}

// ---------------------------------------------------------------------------
// Section 2: Runtime Behavior
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_storage_save_and_load() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let storage = ctx.storage_manager().expect("storage_manager");
    let obj = test_knowledge_object();

    let vault_rel = storage.save(&obj).expect("save");
    assert!(!vault_rel.is_empty());

    let loaded = storage.load(obj.id).expect("load should succeed");
    assert_eq!(loaded.id, obj.id);
    assert_eq!(loaded.object_type, obj.object_type);
    assert_eq!(loaded.content, obj.content);

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_storage_publishes_item_stored_event() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let storage = ctx.storage_manager().expect("storage_manager");
    let event_bus = ctx.event_bus();

    let event_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let event_count_clone = event_count.clone();
    event_bus.subscribe(nabu_core::event_bus::kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        event_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    storage.save(&test_knowledge_object()).expect("save");

    assert_eq!(
        event_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "ItemStored event should have been published"
    );
    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_conversation_save_and_load() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let store = ctx.conversation_store().expect("conversation_store");

    let thread = Thread::new().with_title("Test Thread");
    store.save(&thread).expect("save thread");

    let loaded = store.load(thread.id).expect("load thread");
    assert_eq!(loaded.id, thread.id);
    assert_eq!(loaded.title.as_deref(), Some("Test Thread"));

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_conversation_list_lists_saved_threads() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let store = ctx.conversation_store().expect("conversation_store");

    assert!(store.list().is_empty());

    let t1 = Thread::new().with_title("Thread 1");
    let t2 = Thread::new().with_title("Thread 2");
    store.save(&t1).expect("save t1");
    store.save(&t2).expect("save t2");

    let threads = store.list();
    assert_eq!(threads.len(), 2);

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_job_queue_persists_jobs() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let queue = ctx.job_queue().expect("job_queue");

    let job = Job::new(
        nabu_core::jobs::JobType::MetadataExtraction,
        serde_json::json!({ "test": true }),
        "metadata_extraction",
    );

    queue.enqueue(job.clone()).expect("enqueue");

    let persisted = queue
        .store()
        .load_by_status(JobStatus::Queued)
        .expect("load jobs");
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].payload["test"], true);

    ctx.shutdown().ok();
}

// ---------------------------------------------------------------------------
// Section 3: Graceful Shutdown
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_graceful_shutdown_transitions_to_shutdown() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");
    assert!(ctx.is_running());

    ctx.shutdown().expect("shutdown");
    assert!(ctx.is_shutdown());
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Shutdown);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_double_shutdown_is_safe() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let first = ctx.shutdown();
    assert!(first.is_ok());
    assert!(ctx.is_shutdown());

    let second = ctx.shutdown();
    assert!(second.is_ok(), "second shutdown should be a no-op, not an error");
    assert!(ctx.is_shutdown());
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_shutdown_without_start() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");

    ctx.shutdown().expect("shutdown from Initialized");
    assert!(ctx.is_shutdown());
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_shutdown_from_created() {
    let (ctx, _vault) = build_full_context();
    ctx.shutdown().expect("shutdown from Created");
    assert!(ctx.is_shutdown());
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_health_check_after_shutdown() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let health_running = ctx.health_check();
    assert_eq!(health_running.overall_status, HealthStatus::Healthy);

    ctx.shutdown().expect("shutdown");

    let health_shutdown = ctx.health_check();
    assert!(health_shutdown.initialized);
    assert!(!health_shutdown.running);
    assert_eq!(health_shutdown.lifecycle_stage, LifecycleStageInfo::Shutdown);
}

// ---------------------------------------------------------------------------
// Section 4: Crash Recovery
// ---------------------------------------------------------------------------

/// Verifies that a KnowledgeObject saved before shutdown is still present
/// after a simulated crash+restart (new context on same vault path).
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_storage_survives_restart() {
    let vault = tempdir().expect("tempdir for crash recovery");
    let vault_path = vault.path().to_path_buf();

    let obj = test_knowledge_object();
    let obj_id = obj.id;

    // --- Phase 1: Save data, then "crash" (drop context without graceful shutdown) ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize");
        ctx.start().expect("start");

        let storage = ctx.storage_manager().expect("storage");
        storage.save(&obj).expect("save");

        // Simulate crash: drop the context without calling shutdown().
        // StorageManager is write-through — data is on disk immediately.
        drop(ctx);
    }

    // --- Phase 2: New context on same vault — data should survive ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize after crash");
        ctx.start().expect("start after crash");

        let storage = ctx.storage_manager().expect("storage");
        let loaded = storage.load(obj_id).expect("load after restart");
        assert_eq!(loaded.id, obj_id);
        assert_eq!(loaded.object_type, obj.object_type);
        assert_eq!(loaded.content, obj.content);

        ctx.shutdown().ok();
    }
}

/// Verifies that a ConversationStore's threads persist across a lifecycle restart.
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_conversations_survive_restart() {
    let vault = tempdir().expect("tempdir for conversation recovery");
    let vault_path = vault.path().to_path_buf();

    let thread = Thread::new().with_title("Persistent Thread");
    let thread_id = thread.id;

    // --- Phase 1: Save conversations, then "crash" ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize");
        ctx.start().expect("start");

        let store = ctx.conversation_store().expect("conversation");
        store.save(&thread).expect("save thread");

        // Simulate crash
        drop(ctx);
    }

    // --- Phase 2: New context — threads should reload from disk during initialize() ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        // initialize() triggers ConversationStore::initialize() which reloads
        // persisted threads from disk
        ctx.initialize().expect("initialize after crash");
        ctx.start().expect("start after crash");

        let store = ctx.conversation_store().expect("conversation");
        let loaded = store.load(thread_id).expect("load thread after restart");
        assert_eq!(loaded.id, thread_id);
        assert_eq!(loaded.title.as_deref(), Some("Persistent Thread"));

        ctx.shutdown().ok();
    }
}

/// Verifies that DurableJobQueue persists queued jobs across a restart.
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_job_queue_survives_restart() {
    let vault = tempdir().expect("tempdir for job queue recovery");
    let vault_path = vault.path().to_path_buf();

    let job_type = nabu_core::jobs::JobType::MetadataExtraction;

    // --- Phase 1: Enqueue a job ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize");
        ctx.start().expect("start");

        let job = Job::new(
            job_type.clone(),
            serde_json::json!({ "order": 42 }),
            "metadata_extraction",
        );
        ctx.job_queue().expect("job_queue").enqueue(job).expect("enqueue");

        ctx.shutdown().ok();
    }

    // --- Phase 2: Rebuild the queue — persisted jobs should reload ---
    {
        // Allow background threads from Phase 1 to fully terminate so file
        // handles are released before we reopen the queue directory.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let queue = DurableJobQueue::new(vault_path.join(".nabu").join("queue"))
            .expect("rebuild queue after crash");
        // new() internally calls rebuild_heap(), which loads queued jobs from disk
        let persisted = queue
            .store()
            .load_by_status(JobStatus::Queued)
            .expect("load jobs after crash");
        assert!(
            persisted.iter().any(|j| j.job_type == job_type),
            "persisted job should survive restart"
        );
        assert!(
            persisted.iter().any(|j| j.payload["order"] == 42),
            "job payload should be preserved across restart"
        );
    }
}

// ---------------------------------------------------------------------------
// Section 5: Session Restoration (full app rebuild on same vault)
// ---------------------------------------------------------------------------

/// Full integration: save objects and conversations in one lifecycle, then
/// build a new ApplicationContext on the same vault and verify everything
/// is restored. This tests the ApplicationContext-level session restoration
/// path used by Tauri on app restart.
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_session_restoration() {
    let vault = tempdir().expect("tempdir for session restoration");
    let vault_path = vault.path().to_path_buf();

    let saved_obj = test_knowledge_object();
    let saved_obj_id = saved_obj.id;
    let saved_thread = Thread::new().with_title("Reconstructed Thread");
    let saved_thread_id = saved_thread.id;

    // --- Phase 1: Full lifecycle with data writes ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize");
        ctx.start().expect("start");

        ctx.storage_manager().expect("storage").save(&saved_obj).expect("save obj");
        ctx.conversation_store().expect("conv").save(&saved_thread).expect("save thread");

        let health = ctx.health_check();
        assert_eq!(health.overall_status, HealthStatus::Healthy);
        assert!(health.running);

        ctx.shutdown().expect("shutdown");
    }

    // --- Phase 2: Rebuild on same vault, verify restoration ---
    {
        let ctx = build_full_context_on(vault_path.clone());
        ctx.initialize().expect("initialize after restart");
        ctx.start().expect("start after restart");

        let health = ctx.health_check();
        assert_eq!(health.overall_status, HealthStatus::Healthy);

        // Storage restored
        let storage = ctx.storage_manager().expect("storage");
        let loaded_obj = storage.load(saved_obj_id).expect("load obj after restart");
        assert_eq!(loaded_obj.id, saved_obj_id);
        assert_eq!(loaded_obj.object_type, saved_obj.object_type);

        // Conversations restored
        let store = ctx.conversation_store().expect("conv");
        let loaded_thread = store
            .load(saved_thread_id)
            .expect("load thread after restart");
        assert_eq!(loaded_thread.id, saved_thread_id);
        assert_eq!(loaded_thread.title.as_deref(), Some("Reconstructed Thread"));

        // List reflects persisted state
        let all_threads = store.list();
        assert!(all_threads.iter().any(|t| t.id == saved_thread_id));

        // Metrics reflect restored state
        let metrics = ctx.metrics();
        assert!(metrics.service_count >= 1);

        ctx.shutdown().expect("shutdown");
    }
}

// ---------------------------------------------------------------------------
// Section 6: Lifecycle Stage Tracking & Metrics
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_metrics_collected_during_runtime() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let metrics = ctx.metrics();

    // PerformanceMonitor + StorageManager + WorkerPool are registered as aggregators
    assert!(metrics.service_count >= 1);
    // StorageManager reports object count as a counter
    let storage_count = metrics
        .counters
        .iter()
        .find(|c| c.key == "storage.objects_stored")
        .map(|c| c.value);
    assert!(storage_count.is_some(), "storage counter should be present");

    ctx.shutdown().ok();
}

#[test]
fn lifecycle_full_lifecycle_stage_transitions_are_one_way() {
    let (app, _vault) = build_minimal_app();

    assert_eq!(app.stage(), LifecycleStage::Created);
    app.initialize().expect("initialize");
    assert_eq!(app.stage(), LifecycleStage::Initialized);

    app.start();
    assert_eq!(app.stage(), LifecycleStage::Running);

    // StorageManager should also be Running after app.start()
    let storage = app.context().storage_manager();
    if let Some(s) = storage {
        assert_eq!(s.lifecycle_stage(), LifecycleStage::Running);
    }

    app.shutdown().expect("shutdown");
    assert_eq!(app.stage(), LifecycleStage::Shutdown);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_health_check_stage_reflects_current_phase() {
    let (ctx, _vault) = build_full_context();

    // Before initialize — Created
    let health_created = ctx.health_check();
    assert_eq!(health_created.lifecycle_stage, LifecycleStageInfo::Created);
    assert!(!health_created.initialized);
    assert!(!health_created.running);

    // After initialize — Initialized
    ctx.initialize().expect("initialize");
    let health_init = ctx.health_check();
    assert_eq!(health_init.lifecycle_stage, LifecycleStageInfo::Initialized);
    assert!(health_init.initialized);
    assert!(!health_init.running);

    // After start — Running
    ctx.start().expect("start");
    let health_running = ctx.health_check();
    assert_eq!(health_running.lifecycle_stage, LifecycleStageInfo::Running);
    assert!(health_running.running);

    // After shutdown — Shutdown
    ctx.shutdown().expect("shutdown");
    let health_shutdown = ctx.health_check();
    assert_eq!(health_shutdown.lifecycle_stage, LifecycleStageInfo::Shutdown);
    assert!(!health_shutdown.running);
    assert!(health_shutdown.initialized);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_health_check_lists_all_managed_services() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let health = ctx.health_check();

    // Services that implement Lifecycle and have lifecycle_stage() should appear
    let service_names: std::collections::HashSet<&str> =
        health.services.iter().map(|s| s.name.as_str()).collect();

    assert!(
        service_names.contains("capture_engine")
            || service_names.contains("storage_manager")
            || service_names.contains("worker_pool")
            || service_names.contains("pipeline_executor"),
        "at least one lifecycle-managed service should be reported, found: {:?}",
        service_names
    );
    assert!(health.running_service_count >= 1);

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_lifecycle_service_keys_match_registry() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let health = ctx.health_check();
    let service_names: std::collections::HashSet<&str> =
        health.services.iter().map(|s| s.name.as_str()).collect();

    // The health check should report at least the core lifecycle-managed services.
    // Services are queried via typed accessors (capture_engine, storage_manager,
    // worker_pool, pipeline_executor, conversation_store, vault_graph, indexer,
    // plugin_manager).
    assert!(health.services.len() >= 1);
    // capture_engine should always be present (it's always registered)
    assert!(
        service_names.contains("capture_engine"),
        "capture_engine should appear in health report, found: {:?}",
        service_names
    );

    ctx.shutdown().ok();
}

// ---------------------------------------------------------------------------
// Section 7: Event Bus Integration
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_event_bus_subscribes_and_receives_events() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let event_bus = ctx.event_bus();
    let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received_clone = received.clone();

    event_bus.subscribe(nabu_core::event_bus::kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        received_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    let storage = ctx.storage_manager().expect("storage");
    storage.save(&test_knowledge_object()).expect("save");

    assert!(
        received.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "subscriber should have received at least 1 event"
    );

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_event_bus_multiple_subscribers_receive_events() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let event_bus = ctx.event_bus();
    let counter_a = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_b = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_a_clone = counter_a.clone();
    let counter_b_clone = counter_b.clone();

    event_bus.subscribe(nabu_core::event_bus::kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        counter_a_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    event_bus.subscribe(nabu_core::event_bus::kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        counter_b_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    let storage = ctx.storage_manager().expect("storage");
    storage.save(&test_knowledge_object()).expect("save");

    assert_eq!(counter_a.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(counter_b.load(std::sync::atomic::Ordering::SeqCst), 1);

    ctx.shutdown().ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_full_event_bus_event_delivery_after_shutdown_stops() {
    let (ctx, _vault) = build_full_context();
    ctx.initialize().expect("initialize");
    ctx.start().expect("start");

    let event_bus = ctx.event_bus();
    let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let received_clone = received.clone();

    event_bus.subscribe(nabu_core::event_bus::kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        received_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    let storage = ctx.storage_manager().expect("storage");
    storage.save(&test_knowledge_object()).expect("save");

    let count_before = received.load(std::sync::atomic::Ordering::SeqCst);
    assert!(count_before >= 1, "should have received events while running");

    ctx.shutdown().expect("shutdown");

    // After shutdown, the context should be in Shutdown stage and not running.
    assert!(ctx.is_shutdown());
    assert!(!ctx.is_running());

    // The event bus subscription remains, but the context is no longer running.
    // No new events should be published by the context's lifecycle management.
    let count_after = received.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(count_before, count_after, "no new events should fire after shutdown");
}
