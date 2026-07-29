//! Integration tests for the ServiceRegistry and ApplicationContext.
//!
//! These tests verify the complete flow of service registration, resolution,
//! lifecycle management, health checking, and typed accessor methods.

use std::sync::Arc;

use nabu_core::registry::context::{
    ApplicationContext, ApplicationContextBuilder, ServiceHealth, ValidationReport,
};
use nabu_core::registry::lifecycle::{LifecycleError, LifecycleManager, LifecycleStage};
use nabu_core::registry::ServiceRegistry;

// ---------------------------------------------------------------------------
// ServiceRegistry core tests
// ---------------------------------------------------------------------------

#[test]
fn register_and_resolve_singleton() {
    let mut registry = ServiceRegistry::new();
    registry.register("my_service", Arc::new(42i32));
    let resolved: Option<Arc<i32>> = registry.resolve("my_service");
    assert_eq!(*resolved.unwrap(), 42);
}

#[test]
fn resolve_nonexistent_returns_none() {
    let registry = ServiceRegistry::new();
    let resolved: Option<Arc<i32>> = registry.resolve("nothing");
    assert!(resolved.is_none());
}

#[test]
fn singleton_is_reused() {
    let mut registry = ServiceRegistry::new();
    registry.register("svc", Arc::new("hello"));
    let a: Arc<&str> = registry.resolve("svc").unwrap();
    let b: Arc<&str> = registry.resolve("svc").unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn factory_produces_new_instances() {
    let mut registry = ServiceRegistry::new();
    registry.register_factory("counter", || Arc::new(0i32));
    let a: Arc<i32> = registry.resolve("counter").unwrap();
    let b: Arc<i32> = registry.resolve("counter").unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
}

#[test]
fn factory_replaces_singleton() {
    let mut registry = ServiceRegistry::new();
    registry.register("svc", Arc::new("original"));
    registry.register_factory("svc", || Arc::new("factory"));
    let resolved: Arc<&str> = registry.resolve("svc").unwrap();
    assert_eq!(*resolved, "factory");
}

#[test]
fn unregister_removes_service() {
    let mut registry = ServiceRegistry::new();
    registry.register("svc", Arc::new(true));
    assert!(registry.has("svc"));
    assert!(registry.unregister("svc"));
    assert!(!registry.has("svc"));
}

#[test]
fn category_operations() {
    let mut registry = ServiceRegistry::new();
    registry.register("p1", Arc::new(10i32));
    registry.register("p2", Arc::new(20i32));
    registry.register("p3", Arc::new(30i32));
    registry.register_in_category("processors", "p1");
    registry.register_in_category("processors", "p2");
    registry.register_in_category("processors", "p3");

    let procs: Vec<Arc<i32>> = registry.resolve_category("processors");
    assert_eq!(procs.len(), 3);

    // Unregister one, should be removed from category too
    registry.unregister("p2");
    let procs: Vec<Arc<i32>> = registry.resolve_category("processors");
    assert_eq!(procs.len(), 2);
}

#[test]
fn type_filtered_category_resolution() {
    let mut registry = ServiceRegistry::new();
    registry.register("int_svc", Arc::new(42i32));
    registry.register("str_svc", Arc::new("hello"));
    registry.register_in_category("mixed", "int_svc");
    registry.register_in_category("mixed", "str_svc");

    let ints: Vec<Arc<i32>> = registry.resolve_category("mixed");
    assert_eq!(ints.len(), 1);
    assert_eq!(*ints[0], 42);

    let strs: Vec<Arc<&str>> = registry.resolve_category("mixed");
    assert_eq!(strs.len(), 1);
    assert_eq!(*strs[0], "hello");
}

#[test]
fn batch_category_registration() {
    let mut registry = ServiceRegistry::new();
    registry.register("a", Arc::new(1i32));
    registry.register("b", Arc::new(2i32));
    registry.register_batch_in_category("batch", vec!["a".into(), "b".into()]);
    assert_eq!(registry.get_category("batch").len(), 2);
}

// ---------------------------------------------------------------------------
// ApplicationContext builder tests
// ---------------------------------------------------------------------------

#[test]
fn context_builder_event_bus_registered() {
    let ctx = ApplicationContext::builder().build();
    let bus: Option<Arc<nabu_core::event_bus::EventBus<nabu_core::event_bus::PipelineEvent>>> =
        ctx.resolve("event_bus");
    assert!(bus.is_some());
}

#[test]
fn context_builder_custom_registry() {
    let mut reg = ServiceRegistry::new();
    reg.register("custom", Arc::new("custom_svc"));
    let ctx = ApplicationContext::builder()
        .with_registry(Arc::new(std::sync::RwLock::new(reg)))
        .build();
    let svc: Option<Arc<&str>> = ctx.resolve("custom");
    assert!(svc.is_some());
    assert_eq!(*svc.unwrap(), "custom_svc");
}

#[test]
fn context_builder_custom_event_bus() {
    let bus = Arc::new(nabu_core::event_bus::EventBus::new());
    let ctx = ApplicationContext::builder()
        .with_event_bus(bus.clone())
        .build();
    assert!(Arc::ptr_eq(&ctx.event_bus, &bus));
}

// ---------------------------------------------------------------------------
// Lifecycle tests
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_initial_state() {
    let mgr = LifecycleManager::new();
    assert_eq!(mgr.stage(), LifecycleStage::Created);
}

#[test]
fn lifecycle_forward_transitions() {
    let mgr = LifecycleManager::new();
    assert!(mgr.transition_to(LifecycleStage::Initialized).is_ok());
    assert_eq!(mgr.stage(), LifecycleStage::Initialized);

    assert!(mgr.transition_to(LifecycleStage::Running).is_ok());
    assert_eq!(mgr.stage(), LifecycleStage::Running);

    assert!(mgr.transition_to(LifecycleStage::Shutdown).is_ok());
    assert_eq!(mgr.stage(), LifecycleStage::Shutdown);
}

#[test]
fn lifecycle_skip_transitions_allowed() {
    let mgr = LifecycleManager::new();
    assert!(mgr.transition_to(LifecycleStage::Shutdown).is_ok());
    assert_eq!(mgr.stage(), LifecycleStage::Shutdown);
}

#[test]
fn lifecycle_backward_transition_rejected() {
    let mgr = LifecycleManager::at(LifecycleStage::Running);
    assert!(mgr.transition_to(LifecycleStage::Initialized).is_err());
    assert!(mgr.transition_to(LifecycleStage::Created).is_err());
}

#[test]
fn lifecycle_shutdown_is_final() {
    let mgr = LifecycleManager::at(LifecycleStage::Shutdown);
    assert!(mgr.transition_to(LifecycleStage::Running).is_err());
    assert!(mgr.transition_to(LifecycleStage::Created).is_err());
}

// ---------------------------------------------------------------------------
// Context lifecycle integration tests
// ---------------------------------------------------------------------------

#[test]
fn context_initialize_validates_services() {
    let ctx = ApplicationContext::builder().build();
    // event_bus is auto-registered, but capture_engine, pipeline,
    // and storage_manager are missing
    let result = ctx.initialize();
    assert!(result.is_err());
    let missing = result.unwrap_err();
    assert!(missing.contains(&"capture_engine".to_string()));
    assert!(missing.contains(&"pipeline".to_string()));
    assert!(missing.contains(&"storage_manager".to_string()));
}

#[test]
fn context_validate_core_services() {
    let ctx = ApplicationContext::builder().build();
    let report = ctx.validate_core_services();
    assert!(!report.is_valid());
    assert_eq!(report.required_services.len(), 4);
    assert_eq!(report.optional_services.len(), 4);
}

#[test]
fn context_health_check() {
    let ctx = ApplicationContext::builder().build();
    assert_eq!(ctx.check_health("event_bus"), ServiceHealth::Healthy);
    assert_eq!(ctx.check_health("missing"), ServiceHealth::NotFound);
}

// ---------------------------------------------------------------------------
// Typed accessor tests
// ---------------------------------------------------------------------------

#[test]
fn typed_accessors_return_none_for_missing() {
    let ctx = ApplicationContext::builder().build();
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
fn register_and_access_through_context() {
    let ctx = ApplicationContext::builder().build();

    // Register a simple service through the context
    ctx.register("my_service", Arc::new(String::from("test_value")));
    let resolved: Option<Arc<String>> = ctx.resolve("my_service");
    assert_eq!(*resolved.unwrap(), "test_value");
}

// ---------------------------------------------------------------------------
// Service health and validation tests
// ---------------------------------------------------------------------------

#[test]
fn validation_report_counts() {
    let report = ValidationReport {
        required_services: vec!["a", "b", "c"],
        optional_services: vec!["d"],
        present: vec!["a", "d"],
        missing: vec!["b"],
        unhealthy: vec![],
    };
    assert!(!report.is_valid());
    assert_eq!(report.total_count(), 3);
    assert_eq!(report.missing_count(), 1);
    assert_eq!(report.present.len(), 2);
}

#[test]
fn validation_report_healthy() {
    let report = ValidationReport {
        required_services: vec!["a", "b"],
        optional_services: vec![],
        present: vec!["a", "b"],
        missing: vec![],
        unhealthy: vec![],
    };
    assert!(report.is_valid());
    assert!(report.summary().contains("2/2"));
}

#[test]
fn validation_report_with_unhealthy() {
    let report = ValidationReport {
        required_services: vec!["a"],
        optional_services: vec![],
        present: vec!["a"],
        missing: vec![],
        unhealthy: vec![("a".into(), "timeout".into())],
    };
    // is_valid checks both missing AND unhealthy
    assert!(!report.is_valid());
    assert!(report.summary().contains("timeout"));
}

// ---------------------------------------------------------------------------
// ServiceHealth enum tests
// ---------------------------------------------------------------------------

#[test]
fn service_health_variants() {
    assert_ne!(ServiceHealth::Healthy, ServiceHealth::NotFound);
    assert_ne!(ServiceHealth::Healthy, ServiceHealth::NotInitialized);
    assert_ne!(
        ServiceHealth::Unhealthy("err".into()),
        ServiceHealth::Healthy
    );
    match ServiceHealth::Unhealthy("reason".into()) {
        ServiceHealth::Unhealthy(r) => assert_eq!(r, "reason"),
        _ => panic!("Expected Unhealthy variant"),
    }
}

// ---------------------------------------------------------------------------
// LifecycleError tests
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_error_display() {
    let err = LifecycleError {
        current: LifecycleStage::Running,
        target: LifecycleStage::Initialized,
        message: "Cannot go backward".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Running"));
    assert!(display.contains("Initialized"));
    assert!(display.contains("Cannot go backward"));
}

#[test]
fn lifecycle_error_is_error() {
    let err = LifecycleError {
        current: LifecycleStage::Running,
        target: LifecycleStage::Initialized,
        message: "invalid transition".to_string(),
    };
    let err_ref: &dyn std::error::Error = &err;
    assert!(!err_ref.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// Context builder edge cases
// ---------------------------------------------------------------------------

#[test]
fn context_default_builder() {
    let builder = ApplicationContextBuilder::default();
    let ctx = builder.build();
    assert!(ctx.service_count() >= 1); // event_bus
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Created);
}

#[test]
fn context_capability_count() {
    use nabu_core::plugin::capability::CapabilityRegistry;
    let cr = CapabilityRegistry::new();
    let ctx = ApplicationContext::builder()
        .with_capability_registry(cr)
        .build();
    assert_eq!(ctx.capability_registry.capability_count(), 0);
}

// ---------------------------------------------------------------------------
// Registry edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_registry_counts() {
    let registry = ServiceRegistry::new();
    assert_eq!(registry.singleton_count(), 0);
    assert_eq!(registry.factory_count(), 0);
    assert_eq!(registry.category_count(), 0);
}

#[test]
fn unregister_nonexistent() {
    let mut registry = ServiceRegistry::new();
    assert!(!registry.unregister("nonexistent"));
}

#[test]
fn get_empty_category() {
    let registry = ServiceRegistry::new();
    assert!(registry.get_category("empty").is_empty());
}

#[test]
fn resolve_empty_category() {
    let registry = ServiceRegistry::new();
    let results: Vec<Arc<i32>> = registry.resolve_category("empty");
    assert!(results.is_empty());
}
