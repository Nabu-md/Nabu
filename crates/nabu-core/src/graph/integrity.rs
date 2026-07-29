use crate::graph::serializer::{GraphSnapshot, SerializedNode};
use crate::graph::version::GraphVersion;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

/// Result of a full integrity check.
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    /// Whether the graph passed all checks
    pub passed: bool,

    /// Total nodes in graph
    pub node_count: usize,

    /// Total edges in graph
    pub edge_count: usize,

    /// Number of orphan edges (edges referencing non-existent nodes)
    pub orphan_edge_count: usize,

    /// Number of duplicate node IDs found
    pub duplicate_node_count: usize,

    /// Number of self-referential edges
    pub self_ref_edge_count: usize,

    /// Whether the checksum matches (if available)
    pub checksum_valid: Option<bool>,

    /// Computed SHA-256 checksum of the serialized graph
    pub computed_checksum: String,

    /// Stored checksum from version metadata (if any)
    pub stored_checksum: Option<String>,

    /// Detailed error messages
    pub errors: Vec<String>,

    /// Warnings (non-critical issues)
    pub warnings: Vec<String>,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.passed && self.orphan_edge_count == 0 && self.duplicate_node_count == 0
    }
}

/// Run a full integrity check on a graph snapshot.
pub fn check_integrity(snapshot: &GraphSnapshot) -> IntegrityReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Build set of all valid node IDs
    let node_ids: HashSet<Uuid> = snapshot.nodes.iter().map(|n| n.id).collect();

    // Check for duplicate node IDs
    let mut seen_ids = HashSet::new();
    let mut duplicate_count = 0;
    for node in &snapshot.nodes {
        if !seen_ids.insert(node.id) {
            duplicate_count += 1;
            errors.push(format!("Duplicate node ID: {}", node.id));
        }
    }

    // Check edges for orphan and self-referential
    let mut orphan_count = 0;
    let mut self_ref_count = 0;
    for edge in &snapshot.edges {
        let source_exists = node_ids.contains(&edge.source);
        let target_exists = node_ids.contains(&edge.target);

        if !source_exists {
            orphan_count += 1;
            errors.push(format!("Orphan edge source: {} (node doesn't exist)", edge.source));
        }
        if !target_exists {
            orphan_count += 1;
            errors.push(format!("Orphan edge target: {} (node doesn't exist)", edge.target));
        }
        if edge.source == edge.target {
            self_ref_count += 1;
            warnings.push(format!("Self-referential edge: {}", edge.source));
        }
    }

    // Compute checksum
    let computed_checksum = compute_graph_checksum(snapshot);
    let stored_checksum = snapshot.version.checksum.clone();
    let checksum_valid = stored_checksum.as_ref().map(|stored| *stored == computed_checksum);

    if let Some(false) = checksum_valid {
        errors.push("Checksum mismatch — graph may be corrupted".to_string());
    }

    // Version compatibility warnings
    if snapshot.version.is_outdated() {
        warnings.push(format!(
            "Graph was created with schema v{}, current is v{}",
            snapshot.version.schema_version,
            crate::graph::version::CURRENT_GRAPH_SCHEMA_VERSION
        ));
    }

    let passed = errors.is_empty();

    IntegrityReport {
        passed,
        node_count: snapshot.nodes.len(),
        edge_count: snapshot.edges.len(),
        orphan_edge_count: orphan_count,
        duplicate_node_count: duplicate_count,
        self_ref_edge_count: self_ref_count,
        checksum_valid,
        computed_checksum,
        stored_checksum,
        errors,
        warnings,
    }
}

/// Compute a deterministic SHA-256 checksum of the graph snapshot.
pub fn compute_graph_checksum(snapshot: &GraphSnapshot) -> String {
    let mut hasher = Sha256::new();

    // Hash nodes in sorted order for determinism
    let mut sorted_nodes: Vec<&SerializedNode> = snapshot.nodes.iter().collect();
    sorted_nodes.sort_by(|a, b| a.id.cmp(&b.id));

    for node in &sorted_nodes {
        hasher.update(node.id.to_string().as_bytes());
        hasher.update(node.object_type.as_bytes());
        hasher.update(node.title.as_deref().unwrap_or("").as_bytes());
        for tag in &node.tags {
            hasher.update(tag.as_bytes());
        }
    }

    // Hash edges in sorted order
    let mut sorted_edges = snapshot.edges.clone();
    sorted_edges.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.relationship.cmp(&b.relationship))
    });

    for edge in &sorted_edges {
        hasher.update(edge.source.to_string().as_bytes());
        hasher.update(edge.target.to_string().as_bytes());
        hasher.update(edge.relationship.as_bytes());
        hasher.update(edge.weight.to_le_bytes());
    }

    // Hash version metadata
    hasher.update(snapshot.version.schema_version.to_string().as_bytes());
    hasher.update(snapshot.version.generation.to_string().as_bytes());

    hex::encode(hasher.finalize())
}

/// Quick integrity check — just checks if the graph file is structurally valid.
/// This is a lightweight check suitable for startup validation.
pub fn quick_check(snapshot: &GraphSnapshot) -> bool {
    if snapshot.nodes.is_empty() && snapshot.edges.is_empty() {
        return true; // Empty graph is valid
    }

    // Node count sanity (no more nodes than there are atoms in the universe)
    if snapshot.nodes.len() > 10_000_000 {
        return false;
    }

    // Edge count sanity (every node has approximately reasonable edge count)
    if snapshot.edges.len() > snapshot.nodes.len() * 1000 {
        return false;
    }

    // Version must be present
    if snapshot.version.schema_version == 0 {
        return false;
    }

    true
}

/// Compare two graph versions to determine if a rebuild is needed.
pub fn needs_rebuild(
    persisted_version: &GraphVersion,
    current_version: &GraphVersion,
) -> RebuildReason {
    if persisted_version.schema_version != current_version.schema_version {
        return RebuildReason::SchemaUpgrade {
            from: persisted_version.schema_version,
            to: current_version.schema_version,
        };
    }

    if persisted_version.app_version != current_version.app_version {
        return RebuildReason::AppUpgrade {
            from: persisted_version.app_version.clone(),
            to: current_version.app_version.clone(),
        };
    }

    RebuildReason::None
}

/// Reason for a graph rebuild.
#[derive(Debug, Clone, PartialEq)]
pub enum RebuildReason {
    None,
    SchemaUpgrade { from: u32, to: u32 },
    AppUpgrade { from: String, to: String },
    CorruptionDetected(String),
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::serializer::{GraphSnapshot, SerializedEdge, SerializedNode};
    use crate::graph::version::GraphVersion;

    #[test]
    fn test_integrity_healthy_graph() {
        let snapshot = create_healthy_snapshot();
        let report = check_integrity(&snapshot);
        assert!(report.passed);
        assert!(report.is_healthy());
        assert_eq!(report.node_count, 3);
        assert_eq!(report.edge_count, 2);
    }

    #[test]
    fn test_integrity_or_phan_edges() {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        let node = SerializedNode::new(Uuid::new_v4(), "note", None, "text");
        snapshot.add_node(node.clone());

        snapshot.add_edge(SerializedEdge::new(
            node.id,
            Uuid::new_v4(), // orphan
            "references",
        ));

        let report = check_integrity(&snapshot);
        assert!(!report.passed);
        assert!(report.orphan_edge_count > 0);
    }

    #[test]
    fn test_deterministic_checksum() {
        let snapshot1 = create_healthy_snapshot();
        let snapshot2 = create_healthy_snapshot();

        let hash1 = compute_graph_checksum(&snapshot1);
        let hash2 = compute_graph_checksum(&snapshot2);

        assert_eq!(hash1, hash2, "Checksums should be deterministic");
    }

    #[test]
    fn test_quick_check_valid() {
        let snapshot = create_healthy_snapshot();
        assert!(quick_check(&snapshot));
    }

    #[test]
    fn test_quick_check_empty() {
        let snapshot = GraphSnapshot::new(GraphVersion::new());
        assert!(quick_check(&snapshot));
    }

    #[test]
    fn test_needs_rebuild_schema_upgrade() {
        let persisted = GraphVersion::new();
        let mut current = GraphVersion::new();
        current.schema_version = 2;

        let reason = needs_rebuild(&persisted, &current);
        assert!(matches!(reason, RebuildReason::SchemaUpgrade { .. }));
    }

    fn create_healthy_snapshot() -> GraphSnapshot {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let node3 = Uuid::new_v4();

        snapshot.add_node(SerializedNode::new(node1, "note", Some("A".into()), "text"));
        snapshot.add_node(SerializedNode::new(node2, "note", Some("B".into()), "text"));
        snapshot.add_node(SerializedNode::new(node3, "article", Some("C".into()), "text"));

        snapshot.add_edge(SerializedEdge::new(node1, node2, "references"));
        snapshot.add_edge(SerializedEdge::new(node2, node3, "related"));

        snapshot
    }
}
