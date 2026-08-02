use crate::graph::version::GraphVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The complete serialized graph snapshot.
///
/// This is the persisted format stored under `.nabu/graph/`.
/// It contains only derived graph state — never canonical data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    /// Version metadata
    pub version: GraphVersion,

    /// All graph nodes (minimal metadata — no KnowledgeObject content)
    pub nodes: Vec<SerializedNode>,

    /// All graph edges
    pub edges: Vec<SerializedEdge>,

    /// Graph-level metadata
    pub metadata: HashMap<String, String>,
}

/// A serialized graph node — minimal representation.
/// Does NOT include KnowledgeObject content or binary data.
/// Only the information needed for graph traversal and display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    /// Unique node identifier (matches KnowledgeObject.id)
    pub id: Uuid,

    /// Object type discriminant
    pub object_type: String,

    /// Human-readable title
    pub title: Option<String>,

    /// Content type hint (for icon selection in UI)
    pub content_hint: String,

    /// Node-level tags
    pub tags: Vec<String>,

    /// Additional node properties (non-content metadata)
    pub properties: HashMap<String, String>,
}

/// A serialized graph edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEdge {
    /// Source node ID
    pub source: Uuid,

    /// Target node ID
    pub target: Uuid,

    /// Relationship type label
    pub relationship: String,

    /// Edge weight (for traversal algorithms)
    pub weight: f64,

    /// Directionality of the relationship
    pub direction: EdgeDirection,
}

/// Direction of a graph edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeDirection {
    /// Directed edge: source → target
    Directed,
    /// Undirected edge: source ↔ target
    Undirected,
    /// Bidirectional edge: source → target and target → source (separate semantics)
    Bidirectional,
}

impl GraphSnapshot {
    /// Create a new empty graph snapshot.
    pub fn new(version: GraphVersion) -> Self {
        Self {
            version,
            nodes: Vec::new(),
            edges: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a node to the snapshot.
    pub fn add_node(&mut self, node: SerializedNode) {
        self.nodes.push(node);
    }

    /// Add an edge to the snapshot.
    pub fn add_edge(&mut self, edge: SerializedEdge) {
        self.edges.push(edge);
    }

    /// Set graph-level metadata.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Number of nodes in the snapshot.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges in the snapshot.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Serialize to JSON bytes.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize from JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Deserialization error: {}", e))
    }

    /// Serialize to JSON string.
    pub fn to_json_string(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize from JSON string.
    pub fn from_json_string(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| format!("Deserialization error: {}", e))
    }

    /// Validate basic structural integrity.
    pub fn validate_structure(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Collect all node IDs for reference checking
        let node_ids: std::collections::HashSet<Uuid> = self.nodes.iter().map(|n| n.id).collect();

        // Check for duplicate node IDs
        let mut seen_ids = std::collections::HashSet::new();
        for node in &self.nodes {
            if !seen_ids.insert(node.id) {
                errors.push(format!("Duplicate node ID: {}", node.id));
            }
        }

        // Check edges for orphan references
        for edge in &self.edges {
            if !node_ids.contains(&edge.source) {
                errors.push(format!(
                    "Orphan edge source: {} (referencing non-existent node)",
                    edge.source
                ));
            }
            if !node_ids.contains(&edge.target) {
                errors.push(format!(
                    "Orphan edge target: {} (referencing non-existent node)",
                    edge.target
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl SerializedNode {
    /// Create a new serialized node.
    pub fn new(
        id: Uuid,
        object_type: impl Into<String>,
        title: Option<String>,
        content_hint: impl Into<String>,
    ) -> Self {
        Self {
            id,
            object_type: object_type.into(),
            title,
            content_hint: content_hint.into(),
            tags: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

impl SerializedEdge {
    /// Create a new serialized edge.
    pub fn new(source: Uuid, target: Uuid, relationship: impl Into<String>) -> Self {
        Self {
            source,
            target,
            relationship: relationship.into(),
            weight: 1.0,
            direction: EdgeDirection::Directed,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_direction(mut self, direction: EdgeDirection) -> Self {
        self.direction = direction;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::version::GraphVersion;

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        snapshot.add_node(SerializedNode::new(
            Uuid::new_v4(),
            "note",
            Some("Test Note".to_string()),
            "text/markdown",
        ));

        snapshot.set_metadata("test_key", "test_value");

        let json = snapshot.to_json_string().unwrap();
        let deserialized = GraphSnapshot::from_json_string(&json).unwrap();

        assert_eq!(snapshot.node_count(), deserialized.node_count());
        assert_eq!(
            metadata_value(&snapshot, "test_key"),
            metadata_value(&deserialized, "test_key")
        );
    }

    #[test]
    fn test_validate_structure_valid() {
        let snapshot = create_test_snapshot();
        assert!(snapshot.validate_structure().is_ok());
    }

    #[test]
    fn test_validate_structure_orphan_edge() {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        let node_id = Uuid::new_v4();
        snapshot.add_node(SerializedNode::new(node_id, "note", None, "text"));
        snapshot.add_edge(SerializedEdge::new(
            node_id,
            Uuid::new_v4(), // non-existent node
            "references",
        ));

        let result = snapshot.validate_structure();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Orphan edge target")));
    }

    #[test]
    fn test_edge_direction_serialization() {
        let edge = SerializedEdge::new(Uuid::new_v4(), Uuid::new_v4(), "references")
            .with_direction(EdgeDirection::Bidirectional);
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("Bidirectional"));

        let deserialized: SerializedEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.direction, EdgeDirection::Bidirectional);
    }

    fn create_test_snapshot() -> GraphSnapshot {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let node3 = Uuid::new_v4();

        snapshot.add_node(SerializedNode::new(
            node1,
            "note",
            Some("Node A".into()),
            "text",
        ));
        snapshot.add_node(SerializedNode::new(
            node2,
            "note",
            Some("Node B".into()),
            "text",
        ));
        snapshot.add_node(SerializedNode::new(
            node3,
            "article",
            Some("Node C".into()),
            "text",
        ));

        snapshot.add_edge(SerializedEdge::new(node1, node2, "references"));
        snapshot.add_edge(SerializedEdge::new(node2, node3, "related"));

        snapshot
    }

    fn metadata_value(snapshot: &GraphSnapshot, key: &str) -> Option<String> {
        snapshot.metadata.get(key).cloned()
    }
}
