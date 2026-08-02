use nabu_core::graph::integrity::{check_integrity, compute_graph_checksum, quick_check};
use nabu_core::graph::persistence::GraphStore;
use nabu_core::graph::recovery::{GraphRecovery, RecoveryResult, build_graph_from_objects};
use nabu_core::graph::serializer::{EdgeDirection, GraphSnapshot, SerializedEdge, SerializedNode};
use nabu_core::graph::version::{BuildSource, GraphVersion, VersionCompatibility, check_compatibility, CURRENT_GRAPH_SCHEMA_VERSION};
use nabu_core::graph::VaultGraph;
use nabu_core::models::{KnowledgeObject, ObjectContent, ObjectType};
use tempfile::tempdir;
use uuid::Uuid;

// ─── Test 1: Full save/load roundtrip ──────────────────────────────────

#[test]
fn test_full_save_load_roundtrip() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();

    let mut snapshot = GraphSnapshot::new(GraphVersion::new());
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    let n3 = Uuid::new_v4();

    snapshot.add_node(SerializedNode::new(n1, "note", Some("Note A".into()), "text/markdown"));
    snapshot.add_node(SerializedNode::new(n2, "article", Some("Article B".into()), "text/html"));
    snapshot.add_node(SerializedNode::new(n3, "bookmark", Some("Bookmark C".into()), "text/uri-list"));

    snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));
    snapshot.add_edge(SerializedEdge::new(n2, n3, "related").with_direction(EdgeDirection::Undirected));

    store.save(&snapshot).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.node_count(), 3);
    assert_eq!(loaded.edge_count(), 2);
    assert_eq!(loaded.nodes[0].object_type, "note");
}

// ─── Test 2: Graph survives restart ────────────────────────────────────

#[test]
fn test_graph_survives_restart() {
    let dir = tempdir().unwrap();

    // Save in one session
    let n1;
    {
        let store = GraphStore::new(dir.path()).unwrap();
        let mut snapshot = GraphSnapshot::new(GraphVersion::new());
        n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        snapshot.add_node(SerializedNode::new(n1, "note", Some("Persistent".into()), "text"));
        snapshot.add_node(SerializedNode::new(n2, "note", Some("Survivor".into()), "text"));
        snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));
        store.save(&snapshot).unwrap();
    }

    // Load in new session
    {
        let store = GraphStore::new(dir.path()).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.node_count(), 2);
        assert!(loaded.nodes.iter().any(|n| n.id == n1));
    }
}

// ─── Test 3: Checksum verification ─────────────────────────────────────

#[test]
fn test_checksum_verification() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();

    let snapshot = create_test_snapshot();
    store.save(&snapshot).unwrap();

    // Load and verify
    let loaded = store.load().unwrap().unwrap();
    let report = check_integrity(&loaded);

    assert!(report.passed);
    assert!(report.checksum_valid == Some(true));
}

// ─── Test 4: Deterministic checksums ───────────────────────────────────

#[test]
fn test_deterministic_checksum() {
    let snapshot1 = create_test_snapshot();
    let snapshot2 = create_test_snapshot();

    let hash1 = compute_graph_checksum(&snapshot1);
    let hash2 = compute_graph_checksum(&snapshot2);

    assert_eq!(hash1, hash2, "Same data should produce same checksum");
}

// ─── Test 5: Corruption detection ──────────────────────────────────────

#[test]
fn test_corruption_detection() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();

    // Write invalid JSON
    std::fs::write(store.graph_dir().join("graph.json"), b"not valid json").unwrap();

    let result = store.load();
    assert!(result.is_err());
}

// ─── Test 6: Recovery from valid store ─────────────────────────────────

#[test]
fn test_recovery_from_valid_store() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();
    store.save(&create_test_snapshot()).unwrap();

    let recovery = GraphRecovery::new(store);
    let result = recovery.recover();

    match result {
        RecoveryResult::Recovered(s) => assert_eq!(s.node_count(), 3),
        other => panic!("Expected Recovered, got {:?}", other),
    }
}

// ─── Test 7: Recovery from empty store ─────────────────────────────────

#[test]
fn test_recovery_from_empty_store() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();

    let recovery = GraphRecovery::new(store);
    let result = recovery.recover();

    assert!(matches!(result, RecoveryResult::NeedsRebuild(_)));
}

// ─── Test 8: Version compatibility ─────────────────────────────────────

#[test]
fn test_version_compatibility() {
    // Current version should be compatible
    assert_eq!(
        check_compatibility(&Some(GraphVersion::new())),
        VersionCompatibility::Compatible
    );

    // Future version should be detected
    let mut future = GraphVersion::new();
    future.schema_version = CURRENT_GRAPH_SCHEMA_VERSION + 1;
    assert!(matches!(
        check_compatibility(&Some(future)),
        VersionCompatibility::FutureVersion { .. }
    ));

    // Missing version
    assert_eq!(check_compatibility(&None), VersionCompatibility::Missing);
}

// ─── Test 9: Full graph rebuild ────────────────────────────────────────

#[test]
fn test_full_graph_rebuild() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();
    let recovery = GraphRecovery::new(store);

    let result = recovery
        .rebuild(
            || {
                let mut nodes = Vec::new();
                let mut edges = Vec::new();
                let n1 = Uuid::new_v4();
                let n2 = Uuid::new_v4();
                nodes.push(SerializedNode::new(n1, "note", Some("Node 1".into()), "text"));
                nodes.push(SerializedNode::new(n2, "note", Some("Node 2".into()), "text"));
                edges.push(SerializedEdge::new(n1, n2, "references"));
                Ok((nodes, edges))
            },
            BuildSource::Manual,
        )
        .unwrap();

    assert_eq!(result.node_count(), 2);
    assert_eq!(result.edge_count(), 1);
    assert_eq!(result.version.build_source, BuildSource::Manual);
}

// ─── Test 10: Rebuild from KnowledgeObjects ────────────────────────────

#[test]
fn test_rebuild_from_knowledge_objects() {
    let object = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Rebuild test".to_string()));
    let (nodes, edges) = build_graph_from_objects(&[object]);
    assert_eq!(nodes.len(), 1);
    assert_eq!(edges.len(), 0);
}

// ─── Test 11: VaultGraph with persistence ──────────────────────────────

#[test]
fn test_vaultgraph_with_persistence() {
    let dir = tempdir().unwrap();
    let graph = VaultGraph::with_persistence(None, dir.path().to_path_buf()).unwrap();

    // Should be a fresh graph
    assert_eq!(graph.node_count(), 0);
    assert!(!graph.loaded_from_disk());

    // Add data and persist
    let obj1 = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Persisted node".to_string()));
    graph.add_node(&obj1).unwrap();
    graph.persist().unwrap();
}

// ─── Test 12: Serialization format determinism ─────────────────────────

#[test]
fn test_serialization_determinism() {
    let snapshot = create_test_snapshot();
    let json1 = snapshot.to_json_string().unwrap();

    // Re-serialize (should be identical)
    let json2 = snapshot.to_json_string().unwrap();
    assert_eq!(json1, json2);

    // Round-trip
    let deserialized = GraphSnapshot::from_json_string(&json1).unwrap();
    let json3 = deserialized.to_json_string().unwrap();
    assert_eq!(json1, json3);
}

// ─── Test 13: Multiple edge directions ─────────────────────────────────

#[test]
fn test_edge_directions() {
    let mut snapshot = create_test_snapshot();

    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();

    snapshot.add_node(SerializedNode::new(n1, "note", None, "text"));
    snapshot.add_node(SerializedNode::new(n2, "note", None, "text"));

    snapshot.add_edge(SerializedEdge::new(n1, n2, "directed").with_direction(EdgeDirection::Directed));
    snapshot.add_edge(
        SerializedEdge::new(n1, n2, "undirected").with_direction(EdgeDirection::Undirected),
    );
    snapshot.add_edge(
        SerializedEdge::new(n1, n2, "bidirectional").with_direction(EdgeDirection::Bidirectional),
    );

    let json = snapshot.to_json_string().unwrap();
    let loaded = GraphSnapshot::from_json_string(&json).unwrap();

    assert_eq!(loaded.edges.len(), snapshot.edges.len());
}

// ─── Test 14: Orphan detection ─────────────────────────────────────────

#[test]
fn test_orphan_detection_integrity() {
    let version = GraphVersion::new();
    let mut snapshot = GraphSnapshot::new(version);

    let n1 = Uuid::new_v4();
    snapshot.add_node(SerializedNode::new(n1, "note", None, "text"));
    snapshot.add_edge(SerializedEdge::new(n1, Uuid::new_v4(), "orphan"));

    let report = check_integrity(&snapshot);
    assert!(!report.passed);
    assert!(report.orphan_edge_count > 0);
}

// ─── Test 15: Quick check validation ───────────────────────────────────

#[test]
fn test_quick_check_validation() {
    let valid = create_test_snapshot();
    assert!(quick_check(&valid));

    let empty = GraphSnapshot::new(GraphVersion::new());
    assert!(quick_check(&empty));

    // Corrupt version
    let mut corrupt = create_test_snapshot();
    corrupt.version.schema_version = 0;
    assert!(!quick_check(&corrupt));
}

// ─── Test 16: Graph metadata persistence ───────────────────────────────

#[test]
fn test_graph_metadata_persistence() {
    let dir = tempdir().unwrap();
    let store = GraphStore::new(dir.path()).unwrap();

    let mut snapshot = create_test_snapshot();
    snapshot.set_metadata("generated_by", "test_suite");
    snapshot.set_metadata("vault", "test_vault");

    store.save(&snapshot).unwrap();

    let loaded = store.load().unwrap().unwrap();
    assert_eq!(
        loaded.metadata.get("generated_by").map(|s| s.as_str()),
        Some("test_suite")
    );
    assert_eq!(
        loaded.metadata.get("vault").map(|s| s.as_str()),
        Some("test_vault")
    );
}

// ─── Helper ────────────────────────────────────────────────────────────

fn create_test_snapshot() -> GraphSnapshot {
    let version = GraphVersion::new();
    let mut snapshot = GraphSnapshot::new(version);

    // Fixed UUIDs: two calls must yield byte-identical graphs so that
    // checksum/determinism tests are meaningful.
    let n1 = Uuid::from_u128(1);
    let n2 = Uuid::from_u128(2);
    let n3 = Uuid::from_u128(3);

    snapshot.add_node(SerializedNode::new(n1, "note", Some("Alpha".into()), "text/markdown"));
    snapshot.add_node(SerializedNode::new(n2, "article", Some("Beta".into()), "text/html"));
    snapshot.add_node(SerializedNode::new(n3, "bookmark", Some("Gamma".into()), "text/uri-list"));

    snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));
    snapshot.add_edge(SerializedEdge::new(n2, n3, "related"));

    snapshot
}
