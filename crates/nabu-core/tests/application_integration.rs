//! Integration tests for the Application composition root and dependency injection.
//!
//! These tests verify that the Application builder constructs services correctly,
//! manages lifecycle, and that all services are accessible through the context.
//! No global state, no singleton discovery — everything comes through the Application.

use std::sync::Arc;

use nabu_core::event_bus::EventBus;
use nabu_core::registry::context::ValidationReport;
use nabu_core::registry::lifecycle::LifecycleStage;
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
    assert!(app.context().event_bus().is_some());
}

#[test]
fn application_builder_with_custom_event_bus() {
    let bus = Arc::new(EventBus::new());
    let app = Application::builder()
        .with_event_bus(bus.clone())
        .build();
    assert!(Arc::ptr_eq(&app.context().event_bus(), &bus));
}

#[test]
fn application_builder_registers_event_bus() {
    let app = Application::builder().build();
    let resolved = app.context().resolve::<EventBus>("event_bus");
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
    let bus = Arc::new(EventBus::new());

    // Register missing required services
    app.context().register("capture_engine", Arc::new(nabu_core::capture::CaptureEngine::new(bus.clone())));
    app.context().register(
        "pipeline",
        nabu_core::processing::ProcessingPipeline::new_no_subscribe(bus.clone()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-app-init"),
            bus,
        )),
    );

    let result = app.initialize();
    assert!(result.is_ok());
    assert_eq!(app.stage(), LifecycleStage::Initialized);
}

#[test]
fn start_requires_initialize() {
    let app = Application::builder().build();
    let result = std::panic::catch_unwind(|| {
        app.start();
    });
    assert!(result.is_err());
}

#[test]
fn full_lifecycle_flow() {
    let app = Application::builder().build();
    let bus = Arc::new(EventBus::new());

    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new(bus.clone())),
    );
    app.context().register(
        "pipeline",
        nabu_core::processing::ProcessingPipeline::new_no_subscribe(bus.clone()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(
            std::env::temp_dir().join("nabu-test-lifecycle"),
            bus,
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
    let bus = Arc::new(EventBus::new());
    app.context().register("capture_engine", Arc::new(nabu_core::capture::CaptureEngine::new(bus.clone())));
    app.context().register("pipeline", nabu_core::processing::ProcessingPipeline::new_no_subscribe(bus.clone()));
    app.context().register("storage_manager", Arc::new(nabu_core::storage::StorageManager::new(
        std::env::temp_dir().join("nabu-test-shutdown"), bus,
    )));
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
    assert!(ctx.performance_monitor().is_none());
}

#[test]
fn context_health_check() {
    let app = Application::builder().build();
    assert_eq!(
        app.context().check_health("event_bus"),
        nabu_core::registry::context::ServiceHealth::Healthy,
    );
    assert_eq!(
        app.context().check_health("nonexistent"),
        nabu_core::registry::context::ServiceHealth::NotFound,
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
    assert_eq!(report.optional_services.len(), 4);
}

#[test]
fn validation_report_healthy_when_all_present() {
    let app = Application::builder().build();
    let bus = Arc::new(EventBus::new());
    app.context().register("capture_engine", Arc::new(nabu_core::capture::CaptureEngine::new(bus.clone())));
    app.context().register("pipeline", nabu_core::processing::ProcessingPipeline::new_no_subscribe(bus.clone()));
    app.context().register("storage_manager", Arc::new(nabu_core::storage::StorageManager::new(
        std::env::temp_dir().join("nabu-test-validate"), bus,
    )));
    // Add optional services
    app.context().register("job_queue", Arc::new(nabu_core::jobs::DurableJobQueue::new(
        std::env::temp_dir().join("nabu-test-jobqueue"),
    ).unwrap()));
    app.context().register("worker_pool", Arc::new(nabu_core::jobs::WorkerPool::new(
        1,
        Arc::new(nabu_core::jobs::DurableJobQueue::new(
            std::env::temp_dir().join("nabu-test-workerpool"),
        ).unwrap()),
        Arc::new(nabu_core::jobs::workers::executor::ExecutorRegistry::new()),
    )));

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
    assert!(ctx.event_bus().is_some());
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
    app.context().register_in_category(
        nabu_core::registry::CATEGORY_PROCESSORS,
        "p1",
    );
    app.context().register_in_category(
        nabu_core::registry::CATEGORY_PROCESSORS,
        "p2",
    );

    let procs: Vec<Arc<TestProcessor>> =
        app.context().resolve_category(nabu_core::registry::CATEGORY_PROCESSORS);
    assert_eq!(procs.len(), 2);
}

// ---------------------------------------------------------------------------
// Registry category constants
// ---------------------------------------------------------------------------

#[test]
fn category_constants_are_defined() {
    assert_eq!(nabu_core::registry::CATEGORY_CAPTURE_HANDLERS, "capture_handlers");
    assert_eq!(nabu_core::registry::CATEGORY_PROCESSORS, "processors");
    assert_eq!(nabu_core::registry::CATEGORY_AI_PROVIDERS, "ai_providers");
    assert_eq!(nabu_core::registry::CATEGORY_OCR_PROVIDERS, "ocr_providers");
    assert_eq!(nabu_core::registry::CATEGORY_EMBEDDING_PROVIDERS, "embedding_providers");
    assert_eq!(nabu_core::registry::CATEGORY_EXPORTERS, "exporters");
    assert_eq!(nabu_core::registry::CATEGORY_STORAGE_PROVIDERS, "storage_providers");
    assert_eq!(nabu_core::registry::CATEGORY_CONTENT_PROVIDERS, "content_providers");
}
