//! Integration tests for the Application composition root and dependency injection.
//!
//! These tests verify that the Application builder constructs services correctly,
//! manages lifecycle, and that all services are accessible through the context.
//! No global state, no singleton discovery — everything comes through the Application.

use std::sync::Arc;

use nabu_core::event_bus::{EventBus, PipelineEvent};
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};
use nabu_core::registry::Application;

// ---------------------------------------------------------------------------
// Application builder tests
// ---------------------------------------------------------------------------

#[test]
fn application_builder_creates_default_app() {
    let app = Application::builder().build();
    assert_eq!(app.stage(), LifecycleStage::Created);
    assert!(!app.is_running());
    assert!(!app.is_shutdown());
    app.context().event_bus();
}

#[test]
fn application_builder_with_custom_event_bus() {
    let bus = Arc::new(EventBus::<PipelineEvent>::new());
    let app = Application::builder().with_event_bus(bus.clone()).build();
    assert!(Arc::ptr_eq(app.context().event_bus(), &bus));
}

#[test]
fn application_builder_registers_event_bus() {
    let app = Application::builder().build();
    let resolved = app
        .context()
        .resolve::<EventBus<PipelineEvent>>("event_bus");
    assert!(resolved.is_some());
}

// ---------------------------------------------------------------------------
// Lifecycle tests
// ---------------------------------------------------------------------------

#[test]
fn initialize_validates_core_services() {
    let app = Application::builder().build();
    // capture_engine, pipeline, and storage_manager are missing
    let result = app.initialize();
    assert!(result.is_err());
    let missing = result.unwrap_err();
    assert!(missing.contains(&"capture_engine".to_string()));
    assert!(missing.contains(&"pipeline".to_string()));
    assert!(missing.contains(&"storage_manager".to_string()));
}

#[test]
fn initialize_when_all_services_present() {
    let app = Application::builder().build();

    // Register missing required services
    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    app.context().register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-app-init"),
        )),
    );

    let result = app.initialize();
    assert!(result.is_ok());
    assert_eq!(app.stage(), LifecycleStage::Initialized);
}

#[test]
fn start_requires_initialize() {
    let app = Application::builder().build();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.start();
    }));
    assert!(result.is_err());
}

#[test]
fn full_lifecycle_flow() {
    let app = Application::builder().build();

    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    app.context().register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-lifecycle"),
        )),
    );

    assert_eq!(app.stage(), LifecycleStage::Created);
    assert!(app.initialize().is_ok());
    assert_eq!(app.stage(), LifecycleStage::Initialized);

    app.start();
    assert!(app.is_running());

    assert!(app.shutdown().is_ok());
    assert!(app.is_shutdown());
}

// ---------------------------------------------------------------------------
// Shutdown tests
// ---------------------------------------------------------------------------

#[test]
fn shutdown_from_created() {
    let app = Application::builder().build();
    assert!(app.shutdown().is_ok());
    assert!(app.is_shutdown());
}

#[test]
fn shutdown_from_initialized() {
    let app = Application::builder().build();
    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    app.context().register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-shutdown"),
        )),
    );
    app.initialize().unwrap();
    assert!(app.shutdown().is_ok());
    assert!(app.is_shutdown());
}

#[test]
fn double_shutdown() {
    let app = Application::builder().build();
    assert!(app.shutdown().is_ok());
    // Second shutdown transitions from Shutdown → Shutdown, which is a no-op
    assert!(app.shutdown().is_ok());
}

// ---------------------------------------------------------------------------
// Context accessor tests
// ---------------------------------------------------------------------------

#[test]
fn context_accessors_return_expected_types() {
    let app = Application::builder().build();
    let ctx = app.context();

    // Without services registered, these should all be None
    assert!(ctx.capture_engine().is_none());
    assert!(ctx.processing_pipeline().is_none());
    assert!(ctx.job_queue().is_none());
    assert!(ctx.worker_pool().is_none());
    assert!(ctx.vault_graph().is_none());
    assert!(ctx.indexer().is_none());
    assert!(ctx.storage_manager().is_none());
    // Application::build() always constructs and registers a PerformanceMonitor
    assert!(ctx.performance_monitor().is_some());
}

#[test]
fn context_health_check() {
    let app = Application::builder().build();
    assert_eq!(
        app.context().check_health("event_bus"),
        nabu_core::registry::context::ServiceStatus::Healthy,
    );
    assert_eq!(
        app.context().check_health("nonexistent"),
        nabu_core::registry::context::ServiceStatus::NotFound,
    );
}

// ---------------------------------------------------------------------------
// Service count tests
// ---------------------------------------------------------------------------

#[test]
fn context_service_count_includes_event_bus() {
    let app = Application::builder().build();
    assert!(app.context().service_count() >= 1);
}

#[test]
fn context_service_count_increases_with_registration() {
    let app = Application::builder().build();
    let before = app.context().service_count();
    app.context().register("my_svc", Arc::new(42i32));
    let after = app.context().service_count();
    assert_eq!(after, before + 1);
}

// ---------------------------------------------------------------------------
// Validation report tests
// ---------------------------------------------------------------------------

#[test]
fn context_core_validation() {
    let app = Application::builder().build();
    let report = app.context().validate_core_services();
    assert!(!report.is_valid());
    // event_bus is auto-registered, so it should be in present
    assert!(report.present.contains(&"event_bus"));
    // capture_engine, pipeline, storage_manager are missing
    assert_eq!(report.required_services.len(), 4);
    assert_eq!(report.optional_services.len(), 5);
}

#[test]
fn validation_report_healthy_when_all_present() {
    let app = Application::builder().build();
    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    app.context().register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-validate"),
        )),
    );
    // Add optional services
    let queue = Arc::new(
        nabu_core::jobs::DurableJobQueue::new(std::env::temp_dir().join("nabu-test-jobqueue"))
            .unwrap(),
    );
    app.context().register("job_queue", queue.clone());
    app.context().register(
        "worker_pool",
        Arc::new(nabu_core::jobs::WorkerPool::new(
            1,
            queue.clone(),
            Arc::new(nabu_core::jobs::workers::ExecutorRegistry::new()),
        )),
    );

    let report = app.context().validate_core_services();
    assert_eq!(report.present.len(), 6); // event_bus + 3 required + 2 optional
}

// ---------------------------------------------------------------------------
// Direct ApplicationContext builder tests
// ---------------------------------------------------------------------------

#[test]
fn context_builder_creates_valid_context() {
    use nabu_core::registry::context::ApplicationContext;
    let ctx = ApplicationContext::builder().build();
    ctx.event_bus(); // event_bus always accessible
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Created);
    assert!(!ctx.is_initialized());
    assert!(!ctx.is_running());
    assert!(!ctx.is_shutdown());
}

// ---------------------------------------------------------------------------
// Category operations through Application context
// ---------------------------------------------------------------------------

#[test]
fn application_context_category_operations() {
    let app = Application::builder().build();
    struct TestProcessor;

    app.context().register("p1", Arc::new(TestProcessor));
    app.context().register("p2", Arc::new(TestProcessor));
    app.context()
        .register_in_category(nabu_core::registry::CATEGORY_PROCESSORS, "p1");
    app.context()
        .register_in_category(nabu_core::registry::CATEGORY_PROCESSORS, "p2");

    let procs: Vec<Arc<TestProcessor>> = app
        .context()
        .resolve_category(nabu_core::registry::CATEGORY_PROCESSORS);
    assert_eq!(procs.len(), 2);
}

// ---------------------------------------------------------------------------
// Registry category constants
// ---------------------------------------------------------------------------

#[test]
fn category_constants_are_defined() {
    assert_eq!(
        nabu_core::registry::CATEGORY_CAPTURE_HANDLERS,
        "capture_handlers"
    );
    assert_eq!(nabu_core::registry::CATEGORY_PROCESSORS, "processors");
    assert_eq!(nabu_core::registry::CATEGORY_AI_PROVIDERS, "ai_providers");
    assert_eq!(nabu_core::registry::CATEGORY_OCR_PROVIDERS, "ocr_providers");
    assert_eq!(
        nabu_core::registry::CATEGORY_EMBEDDING_PROVIDERS,
        "embedding_providers"
    );
    assert_eq!(nabu_core::registry::CATEGORY_EXPORTERS, "exporters");
    assert_eq!(
        nabu_core::registry::CATEGORY_STORAGE_PROVIDERS,
        "storage_providers"
    );
    assert_eq!(
        nabu_core::registry::CATEGORY_CONTENT_PROVIDERS,
        "content_providers"
    );
}

// ---------------------------------------------------------------------------
// PluginManager integration — PluginManager is always constructed during
// application startup and accessible through the ApplicationContext.
// ---------------------------------------------------------------------------

#[test]
fn application_context_has_plugin_manager() {
    let app = Application::builder().build();
    let pm = app.context().plugin_manager();
    assert_eq!(pm.name(), "plugin_manager");
}

#[test]
fn application_context_plugin_manager_is_singleton() {
    let app = Application::builder().build();
    // The ApplicationContext owns exactly one PluginManager instance.
    let pm1 = app.context().plugin_manager();
    let pm2 = app.context().plugin_manager();
    assert_eq!(pm1.plugin_count(), pm2.plugin_count());
    assert_eq!(
        pm1.capability_registry().capability_count(),
        pm2.capability_registry().capability_count()
    );
}

#[test]
fn application_context_plugin_manager_has_builtin_capabilities() {
    let app = Application::builder().build();
    let pm = app.context().plugin_manager();
    assert!(pm.capability_registry().has("nabu:event_bus"));
    assert!(pm.capability_registry().has("nabu:capture"));
    assert!(pm.capability_registry().has("nabu:plugin"));
}

#[test]
fn application_context_plugin_manager_has_nabu_version() {
    let app = Application::builder().build();
    let pm = app.context().plugin_manager();
    let expected = nabu_core::plugin::Version::parse(nabu_core::APPLICATION_VERSION)
        .unwrap_or(nabu_core::plugin::Version::new(0, 1, 0));
    assert_eq!(pm.nabu_version(), &expected);
}

// ---------------------------------------------------------------------------
// Lifecycle service registration tests
// ---------------------------------------------------------------------------

#[test]
fn context_lifecycle_service_keys_empty_by_default() {
    let app = Application::builder().build();
    // event_bus and performance_monitor are registered via register_lifecycle
    // in the builder, so they should appear in the lifecycle service list.
    let keys = app.context().lifecycle_service_keys();
    assert!(keys.contains(&"event_bus".to_string()) == false); // event_bus uses register, not register_lifecycle
    assert!(keys.contains(&"performance_monitor".to_string()));
}

#[test]
fn context_shutdown_calls_lifecycle_services() {
    use nabu_core::registry::lifecycle::Lifecycle;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TrackingService {
        name: &'static str,
        count: Arc<AtomicUsize>,
    }

    impl Lifecycle for TrackingService {
        fn name(&self) -> &'static str {
            self.name
        }
        fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let app = Application::builder().build();

    let svc = Arc::new(TrackingService {
        name: "tracking_svc",
        count: Arc::new(AtomicUsize::new(0)),
    });

    app.context()
        .register_lifecycle("tracking_svc", svc.clone());

    assert_eq!(app.context().lifecycle_service_count(), 2); // perf_monitor + tracking_svc

    assert!(app.shutdown().is_ok());
    assert!(app.is_shutdown());
    assert_eq!(svc.count.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// AgentManager and ProcessSupervisor lifecycle tests
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn builder_registers_agent_manager_and_supervisor() {
    use nabu_core::agent::AgentManager;
    use nabu_core::process_supervisor::ProcessSupervisor;

    let event_bus = Arc::new(EventBus::<PipelineEvent>::new());
    let supervisor = Arc::new(ProcessSupervisor::with_event_bus(event_bus.clone()));
    let manager = Arc::new(AgentManager::new(supervisor.clone(), event_bus));

    let app = Application::builder()
        .with_event_bus(event_bus)
        .with_process_supervisor(supervisor.clone())
        .with_agent_manager(manager.clone())
        .build();

    let keys = app.context().lifecycle_service_keys();
    assert!(keys.contains(&"process_supervisor".to_string()));
    assert!(keys.contains(&"agent_manager".to_string()));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn shutdown_shuts_down_agent_manager_and_supervisor() {
    use nabu_core::agent::AgentManager;
    use nabu_core::process_supervisor::ProcessSupervisor;

    let event_bus = Arc::new(EventBus::<PipelineEvent>::new());
    let supervisor = Arc::new(ProcessSupervisor::with_event_bus(event_bus.clone()));
    let manager = Arc::new(AgentManager::new(supervisor.clone(), event_bus));

    // Initialize and start both
    supervisor.initialize().unwrap();
    supervisor.start().unwrap();
    manager.initialize().unwrap();
    manager.start().unwrap();

    let app = Application::builder()
        .with_event_bus(Arc::new(EventBus::<PipelineEvent>::new()))
        .with_process_supervisor(supervisor.clone())
        .with_agent_manager(manager.clone())
        .build();

    // Before shutdown: both should be running
    assert!(supervisor.is_running());
    assert!(manager.is_running());

    assert!(app.shutdown().is_ok());
    assert!(app.is_shutdown());

    // After shutdown: both should be shut down (idempotent — AgentManager
    // calls supervisor.shutdown() internally, then the registry calls it
    // again as a no-op)
    assert!(supervisor.is_shutdown());
    assert!(manager.is_shutdown());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn shutdown_idempotent_for_process_supervisor() {
    use nabu_core::process_supervisor::ProcessSupervisor;

    let supervisor = Arc::new(ProcessSupervisor::new());
    supervisor.initialize().unwrap();
    supervisor.start().unwrap();
    supervisor.shutdown().unwrap();

    // Double shutdown should be a safe no-op
    assert!(supervisor.shutdown().is_ok());
    assert!(supervisor.is_shutdown());
}
