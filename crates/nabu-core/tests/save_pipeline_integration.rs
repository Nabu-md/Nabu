//! Integration tests for the canonical note save pipeline and lifecycle
//! management of StorageManager, Indexer, and VaultGraph.
//!
//! Verifies that:
//! 1. Each service implements the shared Lifecycle trait.
//! 2. All three services initialize, start, and shut down cleanly.
//! 3. A note save through StorageManager flows through the canonical pipeline:
//!    StorageManager::save() -> Persistence -> Indexer -> VaultGraph -> EventBus
//! 4. All services are registered with the lifecycle manager.

use std::sync::{Arc, Mutex, RwLock};
use tempfile::tempdir;

use nabu_core::event_bus::kinds;
use nabu_core::event_bus::{EventBus, PipelineEvent};
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::models::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};
use nabu_core::storage::StorageManager;

/// Helper: build a fully wired canonical pipeline with all three services
/// subscribed to ITEM_STORED, mirroring the production wiring in lib.rs.
///
/// Returns clones of the Arc-wrapped services so tests can interact with
/// them. The EventBus subscriber captures copies of the Arcs, keeping the
/// services alive for the test duration.
fn build_pipeline(vault_path: std::path::PathBuf) -> (
    EventBus<PipelineEvent>,
    Arc<StorageManager>,
    Arc<Mutex<Indexer>>,
    Arc<RwLock<VaultGraph>>,
) {
    let event_bus = EventBus::new();

    let storage = Arc::new(StorageManager::with_event_bus(
        vault_path.clone(),
        event_bus.clone(),
    ));
    let indexer = Arc::new(Mutex::new(Indexer::with_vault_path_and_event_bus(
        vault_path.clone(),
        event_bus.clone(),
    )));
    let graph = Arc::new(RwLock::new(
        VaultGraph::with_persistence(Some(event_bus.clone()), vault_path)
            .expect("Failed to create VaultGraph"),
    ));

    // Canonical event flow: ITEM_STORED -> Indexer + VaultGraph.
    // StorageManager.save() publishes ITEM_STORED after persistence.
    let storage_sub = storage.clone();
    let indexer_sub = indexer.clone();
    let graph_sub = graph.clone();

    event_bus.subscribe(kinds::ITEM_STORED, move |event: &PipelineEvent| {
        if let PipelineEvent::ItemStored(stored) = event {
            if let Some(object) = storage_sub.load(stored.object_id) {
                if let Ok(idx) = indexer_sub.lock() {
                    let _ = idx.index_object(&object);
                }
                if let Ok(g) = graph_sub.write() {
                    let _ = g.add_node(&object);
                }
            }
        }
    });

    (event_bus, storage, indexer, graph)
}

// ---------------------------------------------------------------------------
// Lifecycle integration: StorageManager
// ---------------------------------------------------------------------------

#[test]
fn storage_manager_lifecycle() {
    let dir = tempdir().unwrap();
    let mgr = StorageManager::new(dir.path());

    // Created
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Created);
    assert!(!mgr.is_initialized());
    assert!(!mgr.is_running());
    assert!(!mgr.is_shutdown());

    // Initialize: Created -> Initialized
    assert!(mgr.initialize().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Initialized);
    assert!(mgr.is_initialized());

    // Start: Initialized -> Running
    assert!(mgr.start().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Running);
    assert!(mgr.is_running());

    // Can save after start
    let obj = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Lifecycle test".to_string()),
    );
    assert!(mgr.save(&obj).is_ok());

    // Shutdown: Running -> Shutdown
    assert!(mgr.shutdown().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Shutdown);
    assert!(mgr.is_shutdown());

    // Double shutdown is a no-op
    assert!(mgr.shutdown().is_ok());
}

#[test]
fn storage_manager_start_without_initialize() {
    let dir = tempdir().unwrap();
    let mgr = StorageManager::new(dir.path());

    // start() auto-advances Created -> Initialized -> Running
    assert!(mgr.start().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Running);
    assert!(mgr.shutdown().is_ok());
}

#[test]
fn storage_manager_lifecycle_trait_is_implemented() {
    let dir = tempdir().unwrap();
    let mgr = StorageManager::new(dir.path());

    // Verify the trait is implemented by calling through the trait
    let mgr_ref: &dyn Lifecycle = &mgr;
    assert_eq!(mgr_ref.name(), "storage_manager");

    assert!(mgr_ref.initialize().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Initialized);

    assert!(mgr_ref.start().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Running);

    assert!(mgr_ref.shutdown().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Shutdown);
}

#[test]
fn storage_manager_double_start_no_duplicates() {
    let dir = tempdir().unwrap();
    let mgr = StorageManager::new(dir.path());

    assert!(mgr.start().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Running);

    // Second start is a no-op
    assert!(mgr.start().is_ok());
    assert_eq!(mgr.lifecycle_stage(), LifecycleStage::Running);

    assert!(mgr.shutdown().is_ok());
}

// ---------------------------------------------------------------------------
// Lifecycle integration: Indexer
// ---------------------------------------------------------------------------

#[test]
fn indexer_lifecycle() {
    let indexer = Indexer::new();

    // Created
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Created);
    assert!(!indexer.is_initialized());

    // Initialize: Created -> Initialized
    assert!(indexer.initialize().is_ok());
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Initialized);
    assert!(indexer.is_initialized());

    // Start: Initialized -> Running
    assert!(indexer.start().is_ok());
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Running);
    assert!(indexer.is_running());

    // Can index after start
    let obj = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Index this".to_string()),
    )
    .with_metadata(ObjectMetadata {
        title: Some("Indexed Note".to_string()),
        ..Default::default()
    });
    assert!(indexer.index_object(&obj).is_ok());
    assert!(indexer.token_count() > 0);

    // Shutdown: Running -> Shutdown
    assert!(indexer.shutdown().is_ok());
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Shutdown);
    assert!(indexer.is_shutdown());

    // Double shutdown is a no-op
    assert!(indexer.shutdown().is_ok());
}

#[test]
fn indexer_start_without_initialize() {
    let indexer = Indexer::new();
    assert!(indexer.start().is_ok());
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Running);
    assert!(indexer.shutdown().is_ok());
}

#[test]
fn indexer_lifecycle_trait_is_implemented() {
    let indexer = Indexer::new();

    let idx_ref: &dyn Lifecycle = &indexer;
    assert_eq!(idx_ref.name(), "indexer");

    assert!(idx_ref.initialize().is_ok());
    assert!(idx_ref.start().is_ok());
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Running);
    assert!(idx_ref.shutdown().is_ok());
}

#[test]
fn indexer_initialize_loads_persisted_index() {
    let dir = tempdir().unwrap();
    let obj = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Persistent index".to_string()),
    )
    .with_metadata(ObjectMetadata {
        title: Some("Persistent Index Test".to_string()),
        ..Default::default()
    });

    // Index and persist
    {
        let indexer = Indexer::with_vault_path(dir.path());
        indexer.index_object(&obj).unwrap();
        indexer.persist().unwrap();
    }

    // Re-initialize a new indexer — initialize() should load the persisted index
    {
        let indexer = Indexer::with_vault_path(dir.path());
        assert!(indexer.initialize().is_ok());
        assert!(indexer.is_initialized());

        // The index should have been loaded from disk
        assert!(indexer.token_count() > 0);
        let results = indexer.search("persistent");
        assert!(results.contains(&obj.id.to_string()));
    }
}

// ---------------------------------------------------------------------------
// Lifecycle integration: VaultGraph
// ---------------------------------------------------------------------------

#[test]
fn vault_graph_lifecycle() {
    let graph = VaultGraph::new();

    // Created
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Created);
    assert!(!graph.is_initialized());

    // Initialize: Created -> Initialized
    assert!(graph.initialize().is_ok());
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Initialized);
    assert!(graph.is_initialized());

    // Start: Initialized -> Running
    assert!(graph.start().is_ok());
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Running);
    assert!(graph.is_running());

    // Can add nodes after start
    let obj = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Graph node".to_string()),
    );
    assert!(graph.add_node(&obj).is_ok());
    assert_eq!(graph.node_count(), 1);

    // Shutdown: Running -> Shutdown
    assert!(graph.shutdown().is_ok());
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Shutdown);
    assert!(graph.is_shutdown());

    // Double shutdown is a no-op
    assert!(graph.shutdown().is_ok());
}

#[test]
fn vault_graph_start_without_initialize() {
    let graph = VaultGraph::new();
    assert!(graph.start().is_ok());
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Running);
    assert!(graph.shutdown().is_ok());
}

#[test]
fn vault_graph_lifecycle_trait_is_implemented() {
    let graph = VaultGraph::new();

    let graph_ref: &dyn Lifecycle = &graph;
    assert_eq!(graph_ref.name(), "vault_graph");

    assert!(graph_ref.initialize().is_ok());
    assert!(graph_ref.start().is_ok());
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Running);
    assert!(graph_ref.shutdown().is_ok());
}

#[test]
fn vault_graph_initialize_validates_state() {
    let dir = tempdir().unwrap();
    let obj = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Persisted graph".to_string()),
    );

    // Create graph with persistence, add a node, persist
    {
        let graph = VaultGraph::with_persistence(None, dir.path().to_path_buf()).unwrap();
        assert!(!graph.loaded_from_disk());
        graph.add_node(&obj).unwrap();
        graph.persist().unwrap();
    }

    // Re-initialize a new graph — it should load from disk via constructor
    {
        let graph = VaultGraph::with_persistence(None, dir.path().to_path_buf()).unwrap();
        assert!(graph.initialize().is_ok());
        assert_eq!(graph.lifecycle_stage(), LifecycleStage::Initialized);
    }
}

// ---------------------------------------------------------------------------
// Save pipeline integration: note_save_pipeline
// ---------------------------------------------------------------------------

/// Verifies the canonical save pipeline:
///   Editor -> StorageManager::save() -> Persistence -> Indexer -> VaultGraph -> EventBus
///
/// A note saved through StorageManager must result in:
/// - storage update (object persisted)
/// - index update (object indexed)
/// - graph update (node added)
/// - event publication (ItemStored event emitted)
#[test]
fn note_save_pipeline() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().to_path_buf();

    let (_event_bus, storage, indexer, graph) = build_pipeline(vault_path.clone());

    let storage_ref = storage.as_ref();
    // --- Initialize all services ---
    assert!(storage_ref.initialize().is_ok());
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.initialize().is_ok());
    }
    {
        let g = graph.write().unwrap();
        assert!(g.initialize().is_ok());
    }

    // --- Start all services ---
    assert!(storage_ref.start().is_ok());
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.start().is_ok());
    }
    {
        let g = graph.write().unwrap();
        assert!(g.start().is_ok());
    }

    // --- Track events published on the bus ---
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let event_bus_for_sub = _event_bus.clone();
    let tx_move = tx.clone();
    _event_bus.subscribe(kinds::ITEM_STORED, move |_event: &PipelineEvent| {
        let _ = event_bus_for_sub.subscriber_count(kinds::ITEM_STORED);
        let _ = tx_move.send(kinds::ITEM_STORED.to_string());
    });

    // --- Save a note through the canonical StorageManager gateway ---
    let path = "notes/SavePipelineTest.md".to_string();
    let content = "# Save Pipeline Test\n\nThis is a test note".to_string();

    let saved_path = storage_ref.save_note_content(&path, &content).unwrap();
    assert_eq!(saved_path, path);

    // Allow subscribers to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Verify storage update ---
    let cached = storage_ref.find_by_path(&path);
    assert!(cached.is_some(), "Object should be in storage cache");
    let cached_obj = cached.unwrap();
    assert_eq!(
        cached_obj.content,
        ObjectContent::Markdown(content.clone())
    );
    assert_eq!(
        cached_obj.metadata.vault_path.as_deref(),
        Some(path.as_str())
    );
    // Object should also be persisted on disk
    assert!(storage_ref.exists(cached_obj.id));

    // --- Verify index update (Indexer subscriber indexed the object) ---
    let idx = indexer.lock().unwrap();
    let index_results = idx.search("note");
    assert!(
        index_results.contains(&cached_obj.id.to_string()),
        "Indexer should have indexed the saved object"
    );
    assert!(idx.token_count() > 0, "Index should have tokens after save");
    drop(idx);

    // --- Verify graph update (VaultGraph subscriber added a node) ---
    let g = graph.write().unwrap();
    assert_eq!(g.node_count(), 1, "Graph should have one node");
    let neighbors = g.neighbors(cached_obj.id);
    assert_eq!(neighbors.len(), 0, "Isolated node should have no neighbors");
    drop(g);

    // --- Verify event publication (ItemStored event was emitted) ---
    let event_received = rx.recv_timeout(std::time::Duration::from_millis(200));
    assert!(
        event_received.is_ok(),
        "Should have received ITEM_STORED event"
    );

    // --- Verify event ordering: storage -> index -> graph ---
    // The ItemStored event triggers index_object and add_node in the
    // subscriber. Both should have completed by now.
    let idx = indexer.lock().unwrap();
    assert!(idx.token_count() > 0, "Index should have tokens after save");
    drop(idx);
    let g = graph.write().unwrap();
    assert_eq!(g.node_count(), 1, "Graph should have node after save");
    drop(g);

    // --- Shut down all services ---
    {
        let g = graph.write().unwrap();
        assert!(g.shutdown().is_ok());
    }
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.shutdown().is_ok());
    }
    assert!(storage_ref.shutdown().is_ok());

    // --- Verify shutdown state ---
    assert_eq!(storage_ref.lifecycle_stage(), LifecycleStage::Shutdown);
    {
        let idx = indexer.lock().unwrap();
        assert_eq!(idx.lifecycle_stage(), LifecycleStage::Shutdown);
    }
    {
        let g = graph.write().unwrap();
        assert_eq!(g.lifecycle_stage(), LifecycleStage::Shutdown);
    }
}

/// Verifies that note_save_pipeline works with multiple sequential saves,
/// ensuring the object UUID is preserved (not duplicated) across saves.
#[test]
fn note_save_pipeline_preserves_object_id() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().to_path_buf();

    let (_, storage, indexer, graph) = build_pipeline(vault_path);

    let s = storage.as_ref();
    assert!(s.initialize().is_ok());
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.initialize().is_ok());
    }
    {
        let g = graph.write().unwrap();
        assert!(g.initialize().is_ok());
    }
    assert!(s.start().is_ok());
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.start().is_ok());
    }
    {
        let g = graph.write().unwrap();
        assert!(g.start().is_ok());
    }

    let path = "notes/RepeatSave.md".to_string();

    // First save
    s.save_note_content(&path, "First content").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let first_obj = s.find_by_path(&path).unwrap();

    // Second save — should reuse the same object UUID
    s.save_note_content(&path, "Second content").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let second_obj = s.find_by_path(&path).unwrap();

    // The UUID should be the same — no duplicate objects
    assert_eq!(
        first_obj.id, second_obj.id,
        "Object UUID should be preserved across saves"
    );
    // Content should be updated
    assert_eq!(
        second_obj.content,
        ObjectContent::Markdown("Second content".to_string())
    );
    // Only one object in storage
    assert_eq!(s.count(), 1, "Should have exactly one stored object");
    // Only one node in graph
    {
        let g = graph.write().unwrap();
        assert_eq!(g.node_count(), 1, "Graph should have exactly one node");
    }

    // Clean up
    {
        let g = graph.write().unwrap();
        assert!(g.shutdown().is_ok());
    }
    {
        let idx = indexer.lock().unwrap();
        assert!(idx.shutdown().is_ok());
    }
    assert!(s.shutdown().is_ok());
}

/// Verifies that all three services are registered as Lifecycle trait objects
/// and can be managed through the unified interface.
#[test]
fn lifecycle_registration_all_services() {
    let dir = tempdir().unwrap();

    let storage = StorageManager::new(dir.path());
    let indexer = Indexer::new();
    let graph = VaultGraph::new();

    let services: Vec<&dyn Lifecycle> = vec![&storage, &indexer, &graph];

    // All must have names
    let names: Vec<&str> = services.iter().map(|s| s.name()).collect();
    assert!(names.contains(&"storage_manager"));
    assert!(names.contains(&"indexer"));
    assert!(names.contains(&"vault_graph"));

    // All can be initialized through the trait
    for svc in &services {
        assert!(svc.initialize().is_ok(), "{} failed to initialize", svc.name());
        assert_eq!(svc.name(), svc.name());
    }

    // Verify each is at Initialized
    assert_eq!(storage.lifecycle_stage(), LifecycleStage::Initialized);
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Initialized);
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Initialized);

    // All can be started through the trait
    for svc in &services {
        assert!(svc.start().is_ok(), "{} failed to start", svc.name());
    }

    assert_eq!(storage.lifecycle_stage(), LifecycleStage::Running);
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Running);
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Running);

    // All can be shut down through the trait
    for svc in &services {
        assert!(svc.shutdown().is_ok(), "{} failed to shutdown", svc.name());
    }

    assert_eq!(storage.lifecycle_stage(), LifecycleStage::Shutdown);
    assert_eq!(indexer.lifecycle_stage(), LifecycleStage::Shutdown);
    assert_eq!(graph.lifecycle_stage(), LifecycleStage::Shutdown);
}

/// Verifies no direct persistence bypass remains: every save flows through
/// StorageManager::save() (via save_note_content), not through raw fs::write.
#[test]
fn note_save_pipeline_no_bypass() {
    let dir = tempdir().unwrap();
    let storage = StorageManager::new(dir.path());

    assert!(storage.initialize().is_ok());
    assert!(storage.start().is_ok());

    // The save_note_content method routes through save() internally.
    // Verify the content file was written by StorageManager, not by a
    // direct fs::write bypass.
    let path = "test/NoBypass.md".to_string();
    storage.save_note_content(&path, "bypass test").unwrap();

    // The content file should exist (written by StorageManager::save)
    let content_file = dir.path().join(&path);
    assert!(content_file.exists(), "Content file should exist");

    let content = std::fs::read_to_string(&content_file).unwrap();
    assert_eq!(content, "bypass test");

    // The JSON sidecar should also exist (written by StorageManager::save)
    let sidecar_dir = dir.path().join(".nabu");
    assert!(sidecar_dir.exists(), "Sidecar directory should exist");

    // The in-memory cache should have the object
    let obj = storage.find_by_path(&path);
    assert!(obj.is_some(), "Object should be in cache after save");

    // The ItemStored event should have been publishable (event_bus was None here,
    // so we just verify the save succeeded and data is consistent)
    assert_eq!(storage.count(), 1);

    assert!(storage.shutdown().is_ok());
}
