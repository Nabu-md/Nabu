//! Lifecycle integration tests for the `health_check` IPC endpoint and
//! `ServiceHealth` model.
//!
//! These tests verify that the reported health accurately reflects the
//! lifecycle state of registered services, with no stale or cached status.
//! Health information is collected directly from the LifecycleManager
//! (the single source of truth) and the ServiceRegistry.
//!
//! Test coverage:
//! - Healthy startup (health matches Running state)
//! - Lifecycle state reporting (health reflects each transition)
//! - Successful health queries (valid, populated ServiceHealth)
//! - Expected service counts (registered_services matches registry)
//! - Consistency between lifecycle manager and reported health
//! - Shutdown reporting (stopped_service_count increases)
//! - Serialization round-trip (ServiceHealth serde)

use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use nabu_core::capture::CaptureEngine;
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::jobs::{DurableJobQueue, ExecutorRegistry, WorkerPool};
use nabu_core::pipeline_migration::PipelineExecutor;
use nabu_core::processing::ProcessingPipeline;
use nabu_core::registry::context::{ApplicationContext, ApplicationContextBuilder};
use nabu_core::registry::health::{
    HealthStatus, LifecycleStageInfo, ServiceHealth,
};
use nabu_core::registry::lifecycle::{LifecycleStage};
use nabu_core::storage::StorageManager;
use nabu_core::plugin::capability::CapabilityRegistry;
use nabu_core::registry::ServiceRegistry;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Builds an `ApplicationContext` with all lifecycle-managed services
/// registered. The caller can then call `initialize()`, `start()`, and
/// `shutdown()` on the context to drive lifecycle transitions and verify
/// that `health_check()` accurately reflects each stage.
fn build_test_context(vault_path: std::path::PathBuf) -> ApplicationContext {
    let registry = ServiceRegistry::new();
    let capability_registry = {
        let mut cr = CapabilityRegistry::new();
        cr.register_builtin();
        cr
    };
    let ctx = ApplicationContextBuilder::new()
        .with_registry(Arc::new(std::sync::RwLock::new(registry)))
        .with_capability_registry(capability_registry)
        .build();

    // CaptureEngine
    ctx.register("capture_engine", Arc::new(CaptureEngine::new()));

    // ProcessingPipeline (required by validate_core_services)
    ctx.register("pipeline", Arc::new(ProcessingPipeline::new()));

    // PipelineExecutor
    let pipeline = Arc::new(ProcessingPipeline::new());
    ctx.register("pipeline_executor", Arc::new(PipelineExecutor::new(pipeline)));

    // StorageManager (requires a vault path)
    ctx.register("storage_manager", Arc::new(StorageManager::new(vault_path)));

    // VaultGraph (in-memory, no persistence)
    ctx.register("vault_graph", Arc::new(std::sync::RwLock::new(VaultGraph::new())));

    // Indexer (in-memory)
    ctx.register("indexer", Arc::new(StdMutex::new(Indexer::new())));

    // WorkerPool (requires a job queue)
    let queue_dir = std::env::temp_dir().join("nabu-health-check-queue");
    let _ = std::fs::create_dir_all(&queue_dir);
    let queue = Arc::new(DurableJobQueue::new(&queue_dir).unwrap());
    let executors = Arc::new(ExecutorRegistry::new());
    let pool = WorkerPool::new(2, queue, executors);
    ctx.register("worker_pool", Arc::new(pool));

    ctx
}

// ---------------------------------------------------------------------------
// 1. Healthy startup
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_after_healthy_startup() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    // Before initialization: Created stage
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
    assert!(!health.initialized);
    assert!(!health.running);
    assert!(!health.startup_success);
    assert_eq!(health.overall_status, HealthStatus::Healthy);

    // Initialize
    assert!(ctx.initialize().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Initialized);
    assert!(health.initialized);
    assert!(!health.running);
    assert!(health.startup_success);

    // Start
    assert!(ctx.start().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Running);
    assert!(health.initialized);
    assert!(health.running);
    assert!(health.startup_success);

    // After full startup, at least some services should be running
    assert!(health.running_service_count >= 4); // storage, pipeline_executor, capture_engine, indexer, vault_graph
    assert!(health.failed_service_count == 0);

    // Shutdown
    assert!(ctx.shutdown().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Shutdown);
    assert!(!health.running);
    assert!(health.stopped_service_count >= 4);
}

// ---------------------------------------------------------------------------
// 2. Lifecycle state reporting (health reflects each transition)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_reflects_lifecycle_transitions() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    // Created
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Created);
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
    assert_eq!(health.lifecycle_stage, ctx.lifecycle_stage().into());

    // Initialized
    assert!(ctx.initialize().is_ok());
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Initialized);
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Initialized);
    assert_eq!(health.lifecycle_stage, ctx.lifecycle_stage().into());

    // Running
    assert!(ctx.start().is_ok());
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Running);
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Running);
    assert_eq!(health.lifecycle_stage, ctx.lifecycle_stage().into());

    // Shutdown
    assert!(ctx.shutdown().is_ok());
    assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Shutdown);
    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Shutdown);
    assert_eq!(health.lifecycle_stage, ctx.lifecycle_stage().into());
}

// ---------------------------------------------------------------------------
// 3. Successful health queries (valid, populated ServiceHealth)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_returns_valid_report() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    // Even without initialization, health_check should return a valid report
    let health = ctx.health_check();
    assert_eq!(health.overall_status, HealthStatus::Healthy);
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
    assert!(!health.services.is_empty());

    // Verify all lifecycle-managed services are reported
    let service_names: Vec<&str> = health.services.iter().map(|s| s.name.as_str()).collect();
    assert!(service_names.contains(&"capture_engine"));
    assert!(service_names.contains(&"worker_pool"));
    assert!(service_names.contains(&"pipeline_executor"));
    assert!(service_names.contains(&"storage_manager"));
    assert!(service_names.contains(&"vault_graph"));
    assert!(service_names.contains(&"indexer"));
    assert!(service_names.contains(&"plugin_manager"));

    // Verify capability count is present (built-in capabilities registered)
    assert!(health.capability_count > 0);

    // Verify service_names contains event_bus (auto-registered by builder)
    assert!(health.service_names.contains(&"event_bus".to_string()));
}

// ---------------------------------------------------------------------------
// 4. Expected service counts
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_service_count_matches_registry() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    let health = ctx.health_check();
    let registry_count = ctx.service_count();

    assert_eq!(health.registered_services, registry_count);
    assert_eq!(health.registered_services, health.service_names.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn health_check_service_count_grows_with_registration() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    let initial_health = ctx.health_check();
    let initial_count = initial_health.registered_services;

    // Register an additional service
    ctx.register("extra_service", Arc::new(42i32));

    let health = ctx.health_check();
    assert_eq!(health.registered_services, initial_count + 1);
    assert!(health.service_names.contains(&"extra_service".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn health_check_reports_all_registered_service_names() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    let health = ctx.health_check();

    // The service_names should include all registered keys
    let registry_keys = {
        let reg = ctx.registry().read().expect("registry lock");
        reg.service_keys()
    };

    for key in &registry_keys {
        assert!(
            health.service_names.contains(key),
            "service_names missing registered key: {}",
            key
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Consistency between lifecycle manager and reported health
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_service_stages_match_actual() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    // Before any lifecycle transition, services are at Created
    let health = ctx.health_check();
    for entry in &health.services {
        assert_eq!(
            entry.stage, LifecycleStageInfo::Created,
            "service '{}' should be at Created, got {:?}",
            entry.name, entry.stage
        );
    }

    // After initialize(), lifecycle-managed services transition to Initialized
    assert!(ctx.initialize().is_ok());
    let health = ctx.health_check();
    for entry in &health.services {
        assert!(
            entry.stage == LifecycleStageInfo::Initialized || entry.stage == LifecycleStageInfo::Running,
            "service '{}' should be at least Initialized after initialize(), got {:?}",
            entry.name, entry.stage
        );
    }

    // After start(), services transition to Running
    assert!(ctx.start().is_ok());
    let health = ctx.health_check();
    for entry in &health.services {
        assert_eq!(
            entry.stage, LifecycleStageInfo::Running,
            "service '{}' should be at Running after start(), got {:?}",
            entry.name, entry.stage
        );
    }

    // After shutdown(), services transition to Shutdown
    assert!(ctx.shutdown().is_ok());
    let health = ctx.health_check();
    for entry in &health.services {
        assert_eq!(
            entry.stage, LifecycleStageInfo::Shutdown,
            "service '{}' should be at Shutdown after shutdown(), got {:?}",
            entry.name, entry.stage
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn health_check_running_service_count_matches() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    // Before start — no services running
    let health = ctx.health_check();
    assert_eq!(health.running_service_count, 0);

    // After initialize — services are Initialized (not Running)
    assert!(ctx.initialize().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.running_service_count, 0);

    // After start — services transition to Running
    assert!(ctx.start().is_ok());
    let health = ctx.health_check();
    // All lifecycle-managed services should be at Running
    assert!(health.running_service_count >= 6);

    // After shutdown — no services running, all stopped
    assert!(ctx.shutdown().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.running_service_count, 0);
    assert!(health.stopped_service_count >= 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn health_check_failed_service_count_is_zero_when_healthy() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    assert!(ctx.initialize().is_ok());
    assert!(ctx.start().is_ok());

    let health = ctx.health_check();
    assert_eq!(health.failed_service_count, 0);
    assert!(health.error.is_none());
    assert_eq!(health.overall_status, HealthStatus::Healthy);
}

// ---------------------------------------------------------------------------
// 6. Shutdown reporting
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_after_shutdown_reports_stopped() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    assert!(ctx.initialize().is_ok());
    assert!(ctx.start().is_ok());

    // Before shutdown — all services running
    let health = ctx.health_check();
    assert!(health.running_service_count >= 6);
    assert_eq!(health.stopped_service_count, 0);

    // After shutdown — all services stopped
    assert!(ctx.shutdown().is_ok());
    let health = ctx.health_check();
    assert_eq!(health.running_service_count, 0);
    assert!(health.stopped_service_count >= 6);
    assert!(!health.running);
    assert!(!health.initialized == false);
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Shutdown);
}

// ---------------------------------------------------------------------------
// 7. Serialization round-trip
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn service_health_serializes_for_ipc() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());
    assert!(ctx.initialize().is_ok());
    assert!(ctx.start().is_ok());

    let health = ctx.health_check();
    let json = serde_json::to_string(&health).unwrap();
    assert!(!json.is_empty());

    // Round-trip deserialization
    let deserialized: ServiceHealth = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.overall_status, health.overall_status);
    assert_eq!(deserialized.lifecycle_stage, health.lifecycle_stage);
    assert_eq!(deserialized.registered_services, health.registered_services);
    assert_eq!(deserialized.services.len(), health.services.len());
    assert_eq!(deserialized.running_service_count, health.running_service_count);
    assert_eq!(deserialized.capability_count, health.capability_count);
}

// ---------------------------------------------------------------------------
// 8. Minimal context (no lifecycle-managed services)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_with_minimal_context() {
    let ctx = ApplicationContext::builder().build();

    let health = ctx.health_check();
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
    assert!(!health.initialized);
    assert!(!health.running);
    assert_eq!(health.registered_services, 1); // event_bus auto-registered
    assert!(health.service_names.contains(&"event_bus".to_string()));
    // PluginManager is always constructed by the builder, so it appears
    // in the per-service lifecycle list even with a minimal context.
    assert_eq!(health.services.len(), 1);
    assert_eq!(health.services[0].name, "plugin_manager");
    assert_eq!(health.services[0].stage, LifecycleStageInfo::Created);
    // Health status should be healthy (no errors, no stopped services)
    assert_eq!(health.overall_status, HealthStatus::Healthy);
}

// ---------------------------------------------------------------------------
// 9. Error resilience (no panic on partial state)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_does_not_panic_on_empty_registry() {
    let registry = Arc::new(std::sync::RwLock::new(ServiceRegistry::new()));
    let ctx = ApplicationContext::builder()
        .with_registry(registry)
        .build();

    // Should not panic even with a fresh registry
    let health = ctx.health_check();
    assert!(health.error.is_none());
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
}

#[tokio::test(flavor = "multi_thread")]
async fn health_check_consistency_before_and_after_init() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());

    let before = ctx.health_check();
    assert!(ctx.initialize().is_ok());
    let after = ctx.health_check();

    // registered_services should not change
    assert_eq!(before.registered_services, after.registered_services);
    assert_eq!(before.service_names, after.service_names);

    // lifecycle_stage should change
    assert_ne!(before.lifecycle_stage, after.lifecycle_stage);
    assert_eq!(before.lifecycle_stage, LifecycleStageInfo::Created);
    assert_eq!(after.lifecycle_stage, LifecycleStageInfo::Initialized);

    // running_service_count should still be 0 (services are Initialized, not Running)
    assert_eq!(before.running_service_count, 0);
    assert_eq!(after.running_service_count, 0);
}

// ---------------------------------------------------------------------------
// 10. Per-service entry validation
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn health_check_service_entries_have_valid_data() {
    let dir = tempdir().unwrap();
    let ctx = build_test_context(dir.path().to_path_buf());
    assert!(ctx.initialize().is_ok());
    assert!(ctx.start().is_ok());

    let health = ctx.health_check();

    for entry in &health.services {
        // Every entry should have a non-empty name
        assert!(!entry.name.is_empty());
        // Every entry should be healthy when running
        assert!(entry.healthy, "service '{}' should be healthy when running", entry.name);
        assert_eq!(entry.stage, LifecycleStageInfo::Running);
    }
}

// ---------------------------------------------------------------------------
// 11. Serialization forward-compatibility
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn service_health_deserializes_with_missing_future_fields() {
    // Simulate a future version of the health schema that adds new fields.
    // All fields use #[serde(default)], so old clients should still parse
    // a JSON blob that contains unknown fields.
    let json_with_future = r#"{
        "overall_status": "healthy",
        "lifecycle_stage": "running",
        "initialized": true,
        "running": true,
        "startup_success": true,
        "registered_services": 7,
        "service_names": ["capture_engine", "worker_pool", "pipeline_executor", "storage_manager", "vault_graph", "plugin_manager", "event_bus"],
        "services": [
            {"name": "capture_engine", "stage": "running", "healthy": true},
            {"name": "plugin_manager", "stage": "running", "healthy": true}
        ],
        "running_service_count": 7,
        "stopped_service_count": 0,
        "failed_service_count": 0,
        "capability_count": 12,
        "error": null,
        "uptime_seconds": 3600,
        "version": "0.10.0",
        "memory_usage_mb": 128
    }"#;

    let health: ServiceHealth = serde_json::from_str(json_with_future).unwrap();
    assert_eq!(health.overall_status, HealthStatus::Healthy);
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Running);
    assert!(health.initialized);
    assert!(health.running);
    assert_eq!(health.registered_services, 7);
    assert_eq!(health.running_service_count, 7);
    assert_eq!(health.stopped_service_count, 0);
    assert_eq!(health.failed_service_count, 0);
    assert_eq!(health.capability_count, 12);
    assert!(health.error.is_none());
    assert_eq!(health.services.len(), 2);
    assert_eq!(health.services[0].name, "capture_engine");
    assert_eq!(health.services[1].name, "plugin_manager");
}

#[tokio::test(flavor = "multi_thread")]
async fn service_health_deserializes_minimal_json() {
    // An empty JSON object should deserialize to the Default ServiceHealth
    // because all fields use #[serde(default)].
    let json = r#"{}"#;
    let health: ServiceHealth = serde_json::from_str(json).unwrap();
    assert_eq!(health.overall_status, HealthStatus::default());
    assert_eq!(health.lifecycle_stage, LifecycleStageInfo::default());
    assert!(!health.initialized);
    assert!(!health.running);
    assert_eq!(health.registered_services, 0);
    assert!(health.services.is_empty());
    assert_eq!(health.running_service_count, 0);
    assert_eq!(health.stopped_service_count, 0);
    assert_eq!(health.failed_service_count, 0);
    assert_eq!(health.capability_count, 0);
    assert!(health.error.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn service_health_error_field_round_trips() {
    let health = ServiceHealth {
        overall_status: HealthStatus::Unhealthy,
        lifecycle_stage: LifecycleStageInfo::Created,
        initialized: false,
        running: false,
        startup_success: false,
        registered_services: 0,
        service_names: vec!["event_bus".to_string()],
        services: vec![],
        running_service_count: 0,
        stopped_service_count: 0,
        failed_service_count: 1,
        capability_count: 0,
        error: Some("Failed to initialize StorageManager: disk full".to_string()),
    };

    let json = serde_json::to_string(&health).unwrap();
    let deserialized: ServiceHealth = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.error, health.error);
    assert_eq!(deserialized.overall_status, HealthStatus::Unhealthy);
    assert_eq!(deserialized.failed_service_count, 1);
}
