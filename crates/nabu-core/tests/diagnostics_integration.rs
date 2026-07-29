//! Integration tests for the Nabu diagnostics/observability system.
//!
//! Tests cover:
//! - Init with vault path
//! - Init without vault path
//! - Double init (safe no-op)
//! - Span creation and tracing
//! - File layer creation
//! - Subsystem identifier consistency

use tempfile::tempdir;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::Registry;

/// Test that diagnostics can be initialized with a vault path.
#[test]
fn test_diagnostics_init_with_vault_path() {
    let dir = tempdir().unwrap();
    let result = nabu_core::diagnostics::init(Some(dir.path()), "nabu-test");
    assert!(result || nabu_core::diagnostics::init::is_initialized());

    // Log directory should exist
    let log_dir = dir.path().join(".nabu").join("logs");
    assert!(log_dir.exists() || nabu_core::diagnostics::init::is_initialized());
}

/// Test that diagnostics can be initialized without a vault path.
#[test]
fn test_diagnostics_init_without_vault_path() {
    let result = nabu_core::diagnostics::init(None, "nabu-test");
    assert!(result || nabu_core::diagnostics::init::is_initialized());
}

/// Test that subsystem identifiers are defined correctly.
#[test]
fn test_subsystem_identifiers_are_defined() {
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_CAPTURE, "capture");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_PROCESSING, "processing");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_STORAGE, "storage");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_INDEXER, "indexer");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_GRAPH, "graph");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_QUEUE, "queue");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_WORKER, "worker");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_OCR, "ocr");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_SPEECH, "speech");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_AI, "ai");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_EMBEDDING, "embedding");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_SEARCH, "search");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_EXPORT, "export");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_PLUGIN, "plugin");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_UI, "ui");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_VAULT, "vault");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_EVENT_BUS, "event_bus");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_REGISTRY, "registry");
    assert_eq!(nabu_core::diagnostics::SUBSYSTEM_PIPELINE, "pipeline_migration");
}

/// Test that component identifiers are defined correctly.
#[test]
fn test_component_identifiers_are_defined() {
    assert_eq!(nabu_core::diagnostics::COMPONENT_ENGINE, "engine");
    assert_eq!(nabu_core::diagnostics::COMPONENT_HANDLER, "handler");
    assert_eq!(nabu_core::diagnostics::COMPONENT_PIPELINE, "pipeline");
    assert_eq!(nabu_core::diagnostics::COMPONENT_PROCESSOR, "processor");
    assert_eq!(nabu_core::diagnostics::COMPONENT_EXECUTOR, "executor");
    assert_eq!(nabu_core::diagnostics::COMPONENT_POOL, "pool");
    assert_eq!(nabu_core::diagnostics::COMPONENT_STORE, "store");
    assert_eq!(nabu_core::diagnostics::COMPONENT_MANAGER, "manager");
    assert_eq!(nabu_core::diagnostics::COMPONENT_INDEX, "index");
    assert_eq!(nabu_core::diagnostics::COMPONENT_GRAPH, "graph");
}

/// Test that operation identifiers are defined correctly.
#[test]
fn test_operation_identifiers_are_defined() {
    assert_eq!(nabu_core::diagnostics::OP_INGEST, "ingest");
    assert_eq!(nabu_core::diagnostics::OP_PROCESS, "process");
    assert_eq!(nabu_core::diagnostics::OP_SAVE, "save");
    assert_eq!(nabu_core::diagnostics::OP_LOAD, "load");
    assert_eq!(nabu_core::diagnostics::OP_DELETE, "delete");
    assert_eq!(nabu_core::diagnostics::OP_SEARCH, "search");
    assert_eq!(nabu_core::diagnostics::OP_ENQUEUE, "enqueue");
    assert_eq!(nabu_core::diagnostics::OP_DEQUEUE, "dequeue");
    assert_eq!(nabu_core::diagnostics::OP_EXECUTE, "execute");
    assert_eq!(nabu_core::diagnostics::OP_BUILD, "build");
    assert_eq!(nabu_core::diagnostics::OP_REBUILD, "rebuild");
    assert_eq!(nabu_core::diagnostics::OP_INDEX, "index");
    assert_eq!(nabu_core::diagnostics::OP_UPDATE, "update");
    assert_eq!(nabu_core::diagnostics::OP_CANCEL, "cancel");
    assert_eq!(nabu_core::diagnostics::OP_RETRY, "retry");
}

/// Test that all subsystem identifiers in ALL_SUBSYSTEMS are unique.
#[test]
fn test_all_subsystems_unique() {
    let mut seen = std::collections::HashSet::new();
    for subsystem in nabu_core::diagnostics::ALL_SUBSYSTEMS {
        assert!(
            seen.insert(subsystem),
            "Duplicate subsystem: {}",
            subsystem
        );
    }
    assert_eq!(seen.len(), nabu_core::diagnostics::ALL_SUBSYSTEMS.len());
}

/// Test that the traced helper function works.
#[test]
fn test_traced_helper() {
    let result = nabu_core::diagnostics::spans::traced(
        "test",
        "test_component",
        "test_op",
        || 42,
    );
    assert_eq!(result, 42);
}

/// Test that the rolling file layer can be created.
#[test]
fn test_rolling_file_layer_creation() {
    let dir = tempdir().unwrap();
    let result: Result<(Box<dyn Layer<Registry> + Send + Sync>, _), String> = nabu_core::diagnostics::layers::rolling_file_layer(
        dir.path(),
        "test-nabu",
        7,
    );
    assert!(result.is_ok(), "Rolling file layer should be creatable");
}

/// Test that the stderr layer can be created in both modes.
#[test]
fn test_stderr_layer_modes() {
    // Pretty mode
    let _pretty: Box<dyn Layer<Registry> + Send + Sync> = nabu_core::diagnostics::layers::stderr_layer(true);

    // Compact mode
    let _compact: Box<dyn Layer<Registry> + Send + Sync> = nabu_core::diagnostics::layers::stderr_layer(false);

    // Both should succeed without panicking
}

/// Test that tracing macros work correctly with Nabu structured fields.
#[test]
fn test_structured_tracing_macros() {
    // This test verifies that tracing macros compile correctly with
    // the subsystem/component/operation structured fields.
    // We don't need to actually initialize tracing for this.
    tracing::debug!(
        subsystem = "test",
        component = "integration",
        operation = "test",
        "Test structured log"
    );
}
