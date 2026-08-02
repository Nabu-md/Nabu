use crate::graph::integrity::{check_integrity, needs_rebuild, IntegrityReport, RebuildReason};
use crate::graph::persistence::GraphStore;
use crate::graph::serializer::GraphSnapshot;
use crate::graph::version::{check_compatibility, BuildSource, GraphVersion, VersionCompatibility};

/// Result of loading a graph.
#[derive(Debug, Clone)]
pub enum LoadResult {
    /// Graph loaded successfully and is ready to use
    Loaded(GraphSnapshot),

    /// No persisted graph found — first run
    NotFound,

    /// Graph is outdated but can be upgraded
    RequiresUpgrade {
        snapshot: GraphSnapshot,
        compatibility: VersionCompatibility,
    },

    /// Graph is corrupted — needs rebuild
    Corrupted {
        reason: String,
        report: IntegrityReport,
    },

    /// Graph is from a future version (incompatible)
    FutureVersion { version: GraphVersion },
}

/// Load a graph from the persistence store.
///
/// Performs the full startup loading sequence:
/// 1. Check if graph file exists
/// 2. Load and parse the graph data
/// 3. Quick structural validation
/// 4. Version compatibility check
/// 5. Full integrity verification
/// 6. Return appropriate LoadResult
pub fn load_graph(store: &GraphStore) -> LoadResult {
    // Step 1: Check if graph exists
    if !store.exists() {
        return LoadResult::NotFound;
    }

    // Step 2: Load the graph data
    let snapshot = match store.load() {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => return LoadResult::NotFound,
        Err(e) => {
            return LoadResult::Corrupted {
                reason: format!("Failed to load graph file: {}", e),
                report: IntegrityReport {
                    passed: false,
                    node_count: 0,
                    edge_count: 0,
                    orphan_edge_count: 0,
                    duplicate_node_count: 0,
                    self_ref_edge_count: 0,
                    checksum_valid: Some(false),
                    computed_checksum: String::new(),
                    stored_checksum: None,
                    errors: vec![e],
                    warnings: vec![],
                },
            };
        }
    };

    // Step 3: Quick structural validation
    if !crate::graph::integrity::quick_check(&snapshot) {
        return LoadResult::Corrupted {
            reason: "Graph failed quick integrity check".to_string(),
            report: check_integrity(&snapshot),
        };
    }

    // Step 4: Version compatibility check
    let compatibility = check_compatibility(&Some(snapshot.version.clone()));

    match &compatibility {
        VersionCompatibility::FutureVersion { .. } => {
            return LoadResult::FutureVersion {
                version: snapshot.version,
            };
        }
        VersionCompatibility::Outdated { .. } => {
            // Allow loading with upgrade flag — caller handles migration
            return LoadResult::RequiresUpgrade {
                snapshot,
                compatibility,
            };
        }
        VersionCompatibility::Missing => {
            // Fresh graph — proceed
        }
        VersionCompatibility::Compatible => {
            // Proceed to integrity check
        }
    }

    // Step 5: Full integrity verification
    let report = check_integrity(&snapshot);
    if !report.passed {
        return LoadResult::Corrupted {
            reason: "Graph integrity check failed".to_string(),
            report,
        };
    }

    LoadResult::Loaded(snapshot)
}

/// Check if the persisted graph should be rebuilt from canonical sources.
pub fn should_rebuild(load_result: &LoadResult) -> bool {
    match load_result {
        LoadResult::NotFound => true,
        LoadResult::Corrupted { .. } => true,
        LoadResult::FutureVersion { .. } => true,
        LoadResult::RequiresUpgrade { .. } => true,
        LoadResult::Loaded(snapshot) => {
            // Check if version drift requires rebuild
            let current = GraphVersion::new();
            matches!(
                needs_rebuild(&snapshot.version, &current),
                RebuildReason::None
            ) && snapshot.version.app_version == current.app_version
                && snapshot.version.schema_version == current.schema_version
        }
    }
}

/// Perform a version upgrade on a loaded snapshot.
///
/// This handles schema migrations between versions.
/// Currently, this is a no-op for v1.
pub fn upgrade_snapshot(snapshot: &mut GraphSnapshot, from_version: u32) -> Result<(), String> {
    match from_version {
        0..=0 => {
            // Upgrade from pre-v1 (initial format)
            // No migration needed for v1
            snapshot.version.schema_version = crate::graph::version::CURRENT_GRAPH_SCHEMA_VERSION;
            Ok(())
        }
        v if v == crate::graph::version::CURRENT_GRAPH_SCHEMA_VERSION => {
            // Already current — nothing to do
            Ok(())
        }
        v => Err(format!(
            "No upgrade path from schema v{} to v{}",
            v,
            crate::graph::version::CURRENT_GRAPH_SCHEMA_VERSION
        )),
    }
}

/// Create an empty initial graph snapshot for a fresh start.
pub fn create_initial_snapshot() -> GraphSnapshot {
    GraphSnapshot::new(GraphVersion::rebuilt(BuildSource::Initial))
}

/// Move a loaded snapshot's generation forwards to indicate it was used.
pub fn touch_generation(snapshot: &mut GraphSnapshot) {
    snapshot.version.increment_generation();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::serializer::{SerializedEdge, SerializedNode};
    use tempfile::tempdir;

    fn create_valid_snapshot() -> GraphSnapshot {
        let mut snapshot = GraphSnapshot::new(GraphVersion::new());
        let n1 = uuid::Uuid::new_v4();
        let n2 = uuid::Uuid::new_v4();
        snapshot.add_node(SerializedNode::new(n1, "note", Some("A".into()), "text"));
        snapshot.add_node(SerializedNode::new(n2, "note", Some("B".into()), "text"));
        snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));
        snapshot
    }

    #[test]
    fn test_load_not_found() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();

        let result = load_graph(&store);
        assert!(matches!(result, LoadResult::NotFound));
    }

    #[test]
    fn test_load_successful() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();

        store.save(&create_valid_snapshot()).unwrap();

        let result = load_graph(&store);
        match result {
            LoadResult::Loaded(snapshot) => {
                assert_eq!(snapshot.node_count(), 2);
            }
            other => panic!("Expected Loaded, got {:?}", other),
        }
    }

    #[test]
    fn test_upgrade_from_v0() {
        let mut snapshot = create_valid_snapshot();
        snapshot.version.schema_version = 0;

        upgrade_snapshot(&mut snapshot, 0).unwrap();
        assert_eq!(
            snapshot.version.schema_version,
            crate::graph::version::CURRENT_GRAPH_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_empty_graph_is_valid() {
        let snapshot = create_initial_snapshot();
        assert_eq!(snapshot.node_count(), 0);
        assert_eq!(snapshot.edge_count(), 0);
    }
}
