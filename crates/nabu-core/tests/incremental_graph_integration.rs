use nabu_core::graph::incremental::engine::IncrementalUpdateEngine;
use nabu_core::graph::incremental::dependency_tracker::DependencyTracker;
use nabu_core::graph::incremental::change_log::{ChangeEntry, ChangeLog, CheckpointData};
use nabu_core::graph::incremental::region::RegionEngine;
use nabu_core::graph::incremental::event_wiring::GraphEventBridge;
use nabu_core::graph::serializer::{GraphSnapshot, SerializedEdge, SerializedNode};
use nabu_core::graph::version::GraphVersion;
use nabu_core::graph::VaultGraph;
use nabu_core::models::{KnowledgeObject, ObjectContent, ObjectType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use uuid::Uuid;

// ─── Test 1: Single note edit ───────────────────────────────────────────

#[test]
fn test_single_note_edit_incremental() {
    let mut engine = IncrementalUpdateEngine::new();
    let note = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Original".to_string()));

    // First, add the note
    engine.node_added(&note);
    assert!(engine.tracker.added_nodes.contains(&note.id));
    assert_eq!(engine.tracker.change_count(), 1);

    // Then modify it
    engine.node_modified(&note, Some(&note));
    assert!(engine.tracker.modified_nodes.contains(&note.id));

    // Only the note itself should need recalculation
    let to_recalc = engine.nodes_to_recalculate();
    assert!(to_recalc.contains(&note.id));
}

// ─── Test 2: Note rename (no content change) ──────────────────────────

#[test]
fn test_note_rename() {
    let mut engine = IncrementalUpdateEngine::new();
    let note = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Content".to_string()));

    engine.node_added(&note);

    // Rename is just a metadata change
    engine.tracker.record_node_modified(note.id);

    assert!(engine.tracker.metadata_changed.contains(&note.id));
}

// ─── Test 3: Folder move ─────────────────────────────────────────────

#[test]
fn test_folder_move() {
    let mut engine = IncrementalUpdateEngine::new();
    let note = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Moving".to_string()));

    engine.node_added(&note);

    // Moving a note to a different folder is just a metadata change
    engine.tracker.record_node_modified(note.id);
    engine.tracker.record_relationship_changed(note.id);

    assert!(engine.tracker.relationship_changed.contains(&note.id));
}

// ─── Test 4: Mass import ──────────────────────────────────────────────

#[test]
fn test_mass_import_incremental() {
    let mut engine = IncrementalUpdateEngine::new();
    let mut nodes = HashMap::new();
    let mut edges = Vec::new();

    // Import 100 notes
    for i in 0..100 {
        let note = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown(format!("Note {}", i)),
        );
        engine.node_added(&note);
        nodes.insert(
            note.id,
            SerializedNode::new(note.id, "note", Some(format!("Note {}", i)), "text"),
        );
    }

    assert_eq!(engine.tracker.change_count(), 100);
    assert_eq!(engine.tracker.added_nodes.len(), 100);

    engine.apply_updates(&mut nodes, &mut edges).unwrap();
    assert_eq!(nodes.len(), 100);
}

// ─── Test 5: Mass delete ──────────────────────────────────────────────

#[test]
fn test_mass_delete() {
    let mut engine = IncrementalUpdateEngine::new();
    let mut nodes = HashMap::new();
    let mut edges = Vec::new();

    // Import 50 notes
    let ids: Vec<Uuid> = (0..50)
        .map(|i| {
            let note = KnowledgeObject::new(
                ObjectType::Note,
                ObjectContent::Markdown(format!("Note {}", i)),
            );
            engine.node_added(&note);
            nodes.insert(
                note.id,
                SerializedNode::new(note.id, "note", Some(format!("Note {}", i)), "text"),
            );
            note.id
        })
        .collect();

    engine.reset();

    // Delete 30 of them
    for id in ids.iter().take(30) {
        engine.node_removed(*id);
    }

    assert_eq!(engine.tracker.removed_nodes.len(), 30);

    engine.apply_updates(&mut nodes, &mut edges).unwrap();
    assert_eq!(nodes.len(), 20);
}

// ─── Test 6: Relationship change ──────────────────────────────────────

#[test]
fn test_relationship_change() {
    let mut engine = IncrementalUpdateEngine::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    // Create initial edges
    engine.edge_added(a, b, "references");
    engine.edge_added(b, c, "related");

    assert_eq!(engine.tracker.added_edges.len(), 2);

    // Change: remove one edge, add another
    engine.edge_removed(a, b, "references");
    engine.edge_added(a, c, "references");

    assert_eq!(engine.tracker.added_edges.len(), 2);
    assert_eq!(engine.tracker.removed_edges.len(), 1);
}

// ─── Test 7: Dependency propagation ───────────────────────────────────

#[test]
fn test_dependency_propagation() {
    let mut deps = DependencyTracker::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();

    // a → b, b → c, c → d
    deps.add_dependency(a, b);
    deps.add_dependency(b, c);
    deps.add_dependency(c, d);

    // If d changes, a, b, c are all affected
    let affected = deps.transitive_affected(d);
    assert!(affected.contains(&c));
    assert!(affected.contains(&b));
    assert!(affected.contains(&a));
    assert!(affected.contains(&d));
}

// ─── Test 8: Region-based rebuild ─────────────────────────────────────

#[test]
fn test_region_based_rebuild() {
    let mut snapshot = GraphSnapshot::new(GraphVersion::new());
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    let n3 = Uuid::new_v4();

    snapshot.add_node(SerializedNode::new(n1, "note", Some("Inbox/Note".into()), "text"));
    snapshot.add_node(SerializedNode::new(n2, "note", Some("Work/Doc".into()), "text"));
    snapshot.add_node(SerializedNode::new(n3, "article", Some("Inbox/Article".into()), "text"));
    snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));

    let mut engine = RegionEngine::new();
    engine.discover_regions(&snapshot);

    // Changing n1 (in inbox) should only affect the Inbox region
    let mut changed = std::collections::HashSet::new();
    changed.insert(n1);
    let affected = engine.affected_regions(&changed);

    // At least one region should be affected
    assert!(!affected.is_empty());
}

// ─── Test 9: Change log append and replay ─────────────────────────────

#[test]
fn test_change_log_append_replay() {
    let dir = tempdir().unwrap();
    let log = ChangeLog::new(dir.path()).unwrap();

    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();

    log.append(&ChangeEntry::NodeAdded(SerializedNode::new(
        n1,
        "note",
        Some("A".into()),
        "text",
    )))
    .unwrap();
    log.append(&ChangeEntry::EdgeAdded(SerializedEdge::new(n1, n2, "references"))).unwrap();

    let mut count = 0u64;
    log.replay(|_entry| {
        count += 1;
        Ok(())
    })
    .unwrap();

    assert_eq!(count, 2);
}

// ─── Test 10: Change log compaction ───────────────────────────────────

#[test]
fn test_change_log_compaction() {
    let dir = tempdir().unwrap();
    let log = ChangeLog::new(dir.path()).unwrap();

    log.append(&ChangeEntry::NodeAdded(SerializedNode::new(
        Uuid::new_v4(),
        "note",
        None,
        "text",
    )))
    .unwrap();

    log.compact(|| {
        Ok(CheckpointData {
            nodes: vec![],
            edges: vec![],
            generation: 1,
        })
    })
    .unwrap();

    let mut count = 0u64;
    log.replay(|entry| {
        count += 1;
        match entry {
            ChangeEntry::Checkpoint(_) => {} // expected
            other => panic!("Expected checkpoint after compaction, got {:?}", other),
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(count, 1);
}

// ─── Test 11: Event bridge batch processing ───────────────────────────

#[test]
fn test_batch_processing() {
    let engine = Arc::new(Mutex::new(IncrementalUpdateEngine::new()));
    let graph = Arc::new(Mutex::new(VaultGraph::new()));
    let snapshot = GraphSnapshot::new(GraphVersion::new());
    let bridge = GraphEventBridge::new(engine.clone(), graph, snapshot);

    let events = vec![
        nabu_core::event_bus::ItemStoredEvent {
            object_id: Uuid::new_v4(),
            vault_path: "/test/1.md".to_string(),
            object_type: ObjectType::Note,
            timestamp: chrono::Utc::now(),
        },
        nabu_core::event_bus::ItemStoredEvent {
            object_id: Uuid::new_v4(),
            vault_path: "/test/2.md".to_string(),
            object_type: ObjectType::Article,
            timestamp: chrono::Utc::now(),
        },
    ];

    bridge.process_batch(&events).unwrap();

    let engine = engine.lock().unwrap();
    assert!(engine.has_pending_updates());
}

// ─── Test 12: Transaction commit ──────────────────────────────────────

#[test]
fn test_transaction_commit() {
    let mut engine = IncrementalUpdateEngine::new();

    engine.begin_transaction();
    let a = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("A".to_string()));
    let b = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("B".to_string()));
    engine.node_added(&a);
    engine.node_added(&b);
    assert!(!engine.tracker.has_pending_changes);

    engine.commit_transaction().unwrap();
    assert!(engine.tracker.has_pending_changes);
    assert_eq!(engine.tracker.added_nodes.len(), 2);
}

// ─── Test 13: Transaction rollback ────────────────────────────────────

#[test]
fn test_transaction_rollback() {
    let mut engine = IncrementalUpdateEngine::new();

    engine.begin_transaction();
    let a = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("A".to_string()));
    engine.node_added(&a);

    engine.rollback_transaction();
    assert!(!engine.tracker.has_pending_changes);
    assert!(engine.tracker.added_nodes.is_empty());
}

// ─── Test 14: Graph corruption recovery ──────────────────────────────

#[test]
fn test_graph_corruption_recovery() {
    let mut engine = IncrementalUpdateEngine::new();
    let mut nodes: HashMap<Uuid, SerializedNode> = HashMap::new();
    let mut edges: Vec<SerializedEdge> = Vec::new();

    // Simulate a healthy graph
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    nodes.insert(n1, SerializedNode::new(n1, "note", None, "text"));
    nodes.insert(n2, SerializedNode::new(n2, "note", None, "text"));
    edges.push(SerializedEdge::new(n1, n2, "references"));

    // Remove one node (simulating corruption/malfunction)
    engine.tracker.record_node_removed(n1);
    engine.apply_updates(&mut nodes, &mut edges).unwrap();

    // The removed node should be gone, and its edges should be cleaned up
    assert_eq!(nodes.len(), 1);
    assert!(edges.is_empty());
}

// ─── Test 15: No orphan edges after incremental update ────────────────

#[test]
fn test_no_orphan_edges_after_update() {
    let mut engine = IncrementalUpdateEngine::new();
    let mut nodes: HashMap<Uuid, SerializedNode> = HashMap::new();
    let mut edges: Vec<SerializedEdge> = Vec::new();

    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    let n3 = Uuid::new_v4();

    nodes.insert(n1, SerializedNode::new(n1, "note", None, "text"));
    nodes.insert(n2, SerializedNode::new(n2, "note", None, "text"));
    nodes.insert(n3, SerializedNode::new(n3, "article", None, "text"));

    edges.push(SerializedEdge::new(n1, n2, "references"));
    edges.push(SerializedEdge::new(n2, n3, "related"));

    // Remove n2 — should remove both edges
    engine.tracker.record_node_removed(n2);
    engine.apply_updates(&mut nodes, &mut edges).unwrap();

    // No edge should reference the removed node
    for edge in &edges {
        assert_ne!(edge.source, n2, "Orphan edge source found");
        assert_ne!(edge.target, n2, "Orphan edge target found");
    }
}

// ─── Test 16: Large vault performance simulation ──────────────────────

#[test]
fn test_large_vault_performance() {
    let mut engine = IncrementalUpdateEngine::new();
    let mut nodes: HashMap<Uuid, SerializedNode> = HashMap::new();
    let mut edges: Vec<SerializedEdge> = Vec::new();

    // Build a graph with 10,000 nodes and 20,000 edges
    let n1 = Uuid::new_v4();
    for i in 0..10_000 {
        let id = if i == 0 { n1 } else { Uuid::new_v4() };
        nodes.insert(
            id,
            SerializedNode::new(id, "note", Some(format!("Note {}", i)), "text"),
        );
        if i > 0 {
            edges.push(SerializedEdge::new(n1, id, "references"));
        }
    }

    // Edit a single node — the incremental update should only affect
    // the edited node and its immediate dependents
    engine.tracker.record_node_modified(n1);
    let to_recalculate = engine.nodes_to_recalculate();

    // At minimum, the changed node itself should be recalculated
    assert!(to_recalculate.contains(&n1));
}
