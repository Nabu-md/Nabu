use crate::models::knowledge_object::KnowledgeObject;
use crate::models::graph::RelationType;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Architectural note: VaultGraph is the SINGLE canonical graph (Principle 7).
//
// No subsystem may introduce its own graph, relation engine, or graph
// database. All entity relations, wiki-link backlinks, and semantic edges
// flow through this module.
//
// The graph is a *derived* data structure (Principle 9). It is persisted
// under `.nabu/graph/` and is fully rebuildable from Markdown content on
// vault open. Deleting `.nabu/graph/` must never destroy user-authored
// knowledge.
//
// Persistence format:
//   .nabu/graph/
//       graph.json       - Nodes, edges, and maps
//       metadata.json    - Version, integrity checksum, counts
//
// The graph is incrementally updated via the EventBus. Each mutation
// triggers a save to disk, so crash recovery is limited to the last
// completed mutation.
// ---------------------------------------------------------------------------

pub const GRAPH_STORAGE_VERSION: u32 = 1;
pub const GRAPH_DIR: &str = ".nabu/graph";
pub const GRAPH_FILE: &str = "graph.json";
pub const METADATA_FILE: &str = "metadata.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphNode {
    Object(KnowledgeObject),
    Entity(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphEdgeType {
    WikiLink,
    Semantic(RelationType),
}

#[derive(Debug, Clone)]
pub struct VaultGraph {
    pub graph: Graph<GraphNode, GraphEdgeType>,
    node_map: HashMap<String, NodeIndex>,
    entity_map: HashMap<Uuid, NodeIndex>,
    /// Persistence directory (`.nabu/graph/`) — `None` if persistence disabled.
    storage_path: Option<PathBuf>,
    /// Current storage version for migration support.
    storage_version: u32,
    /// Whether a save is pending (avoids redundant writes).
    dirty: bool,
}

// ---------------------------------------------------------------------------
// Serializable representation for persistence
// ---------------------------------------------------------------------------

/// Serializable node — stores data without relying on Petgraph's serde.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedNode {
    node_type: String, // "Object" or "Entity"
    knowledge_object: Option<KnowledgeObject>,
    uuid: Option<Uuid>,
}

/// Serializable edge with source/target as node indices.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEdge {
    source: usize,
    target: usize,
    edge_type: GraphEdgeType,
}

/// Serializable graph data file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedGraph {
    version: u32,
    nodes: Vec<PersistedNode>,
    edges: Vec<PersistedEdge>,
    node_map: HashMap<String, usize>,
    entity_map: HashMap<String, usize>, // UUID strings for JSON compatibility
}

/// Graph metadata file with integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphMetadata {
    version: u32,
    node_count: usize,
    edge_count: usize,
    checksum_sha256: String,
    last_modified_at: String,
}

// ===========================================================================
// Construction
// ===========================================================================

impl VaultGraph {
    /// Create an empty in-memory graph without persistence.
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            node_map: HashMap::new(),
            entity_map: HashMap::new(),
            storage_path: None,
            storage_version: GRAPH_STORAGE_VERSION,
            dirty: false,
        }
    }

    /// Create a VaultGraph with persistence at `.nabu/graph/` inside `vault_path`.
    ///
    /// On creation, attempts to load existing graph data from disk.
    /// If no persisted graph exists, returns an empty graph.
    /// If the persisted data is corrupted or from an incompatible version,
    /// returns an empty graph — the caller should trigger a rebuild from
    /// Markdown source files.
    pub fn with_storage(vault_path: PathBuf) -> Self {
        let storage_path = vault_path.join(GRAPH_DIR);

        // Try to load existing graph
        if let Ok(graph) = Self::load_from_path(&storage_path) {
            return graph;
        }

        // No existing graph found — create empty.
        // The caller (e.g., src-tauri) will detect this and trigger a rebuild.
        Self {
            graph: Graph::new(),
            node_map: HashMap::new(),
            entity_map: HashMap::new(),
            storage_path: Some(storage_path),
            storage_version: GRAPH_STORAGE_VERSION,
            dirty: false,
        }
    }

    /// Returns the storage path if persistence is enabled.
    pub fn storage_path(&self) -> Option<&PathBuf> {
        self.storage_path.as_ref()
    }

    /// Returns whether this graph has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

// ===========================================================================
// Persistence — Save / Load
// ===========================================================================

impl VaultGraph {
    /// Save the graph to its storage path.
    ///
    /// Writes `graph.json` (node/edge data) and `metadata.json` (version,
    /// integrity checksum, counts). Returns an error if no storage path
    /// is configured or if the write fails.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let storage_path = self.storage_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Graph persistence not configured — no storage path set")
        })?;
        self.save_to_path(storage_path)?;
        self.dirty = false;
        Ok(())
    }

    /// Serialise the graph and write it to the given directory.
    fn save_to_path(&self, dir: &Path) -> anyhow::Result<()> {
        // Ensure the directory exists
        std::fs::create_dir_all(dir)?;

        // Convert nodes
        let nodes: Vec<PersistedNode> = self
            .graph
            .node_indices()
            .map(|idx| match &self.graph[idx] {
                GraphNode::Object(ko) => PersistedNode {
                    node_type: "Object".to_string(),
                    knowledge_object: Some(ko.clone()),
                    uuid: None,
                },
                GraphNode::Entity(id) => PersistedNode {
                    node_type: "Entity".to_string(),
                    knowledge_object: None,
                    uuid: Some(*id),
                },
            })
            .collect();

        // Convert edges — map NodeIndex to usize position
        let edges: Vec<PersistedEdge> = self
            .graph
            .edge_indices()
            .map(|edge_idx| {
                let (source, target) = self.graph.edge_endpoints(edge_idx).unwrap();
                PersistedEdge {
                    source: source.index(),
                    target: target.index(),
                    edge_type: self.graph[edge_idx].clone(),
                }
            })
            .collect();

        // Convert node_map (path → usize)
        let node_map: HashMap<String, usize> = self
            .node_map
            .iter()
            .map(|(path, idx)| (path.clone(), idx.index()))
            .collect();

        // Convert entity_map (Uuid → usize)
        let entity_map: HashMap<String, usize> = self
            .entity_map
            .iter()
            .map(|(uuid, idx)| (uuid.to_string(), idx.index()))
            .collect();

        let persisted = PersistedGraph {
            version: GRAPH_STORAGE_VERSION,
            nodes,
            edges,
            node_map,
            entity_map,
        };

        // Serialise to JSON
        let json = serde_json::to_string_pretty(&persisted)?;

        // Compute integrity checksum
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        let checksum = format!("{:x}", hasher.finalize());

        // Write graph data
        std::fs::write(dir.join(GRAPH_FILE), &json)?;

        // Write metadata
        let metadata = GraphMetadata {
            version: GRAPH_STORAGE_VERSION,
            node_count: self.graph.node_count(),
            edge_count: self.graph.edge_count(),
            checksum_sha256: checksum,
            last_modified_at: current_timestamp(),
        };
        std::fs::write(
            dir.join(METADATA_FILE),
            serde_json::to_string_pretty(&metadata)?,
        )?;

        Ok(())
    }

    /// Load a VaultGraph from the given directory.
    ///
    /// Returns an error if:
    /// - The graph files do not exist
    /// - The JSON is malformed
    /// - The integrity checksum does not match
    /// - The version is incompatible
    fn load_from_path(dir: &Path) -> anyhow::Result<Self> {
        // Read metadata and verify integrity
        let metadata_path = dir.join(METADATA_FILE);
        let metadata: GraphMetadata = serde_json::from_str(
            &std::fs::read_to_string(&metadata_path)?,
        )?;

        if metadata.version > GRAPH_STORAGE_VERSION {
            anyhow::bail!(
                "Graph metadata version {} is newer than supported version {}",
                metadata.version,
                GRAPH_STORAGE_VERSION
            );
        }

        // Read graph data
        let graph_path = dir.join(GRAPH_FILE);
        let json = std::fs::read_to_string(&graph_path)?;

        // Verify integrity checksum
        if !metadata.checksum_sha256.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(json.as_bytes());
            let actual_checksum = format!("{:x}", hasher.finalize());
            if actual_checksum != metadata.checksum_sha256 {
                anyhow::bail!(
                    "Graph integrity check failed: expected checksum {}, got {}. \
                     The graph file may be corrupted. Delete `.nabu/graph/` and reopen the vault.",
                    metadata.checksum_sha256,
                    actual_checksum
                );
            }
        }

        // Deserialize
        let persisted: PersistedGraph = serde_json::from_str(&json)?;

        if persisted.version > GRAPH_STORAGE_VERSION {
            anyhow::bail!(
                "Graph data version {} is newer than supported version {}",
                persisted.version,
                GRAPH_STORAGE_VERSION
            );
        }

        // Rebuild Petgraph graph from persisted data
        let mut graph = Graph::new();
        let mut index_mapping: HashMap<usize, NodeIndex> = HashMap::new();

        // Add nodes in order (index position = original index)
        for (i, persisted_node) in persisted.nodes.iter().enumerate() {
            let node = match persisted_node.node_type.as_str() {
                "Object" => {
                    let ko = persisted_node
                        .knowledge_object
                        .clone()
                        .unwrap_or_default();
                    GraphNode::Object(ko)
                }
                "Entity" => {
                    let id = persisted_node.uuid.unwrap_or_default();
                    GraphNode::Entity(id)
                }
                other => {
                    anyhow::bail!("Unknown node type in persisted graph: {}", other);
                }
            };
            let idx = graph.add_node(node);
            index_mapping.insert(i, idx);
        }

        // Add edges
        for persisted_edge in &persisted.edges {
            let source = index_mapping.get(&persisted_edge.source).ok_or_else(|| {
                anyhow::anyhow!(
                    "Corrupted graph: edge references missing source node index {}",
                    persisted_edge.source
                )
            })?;
            let target = index_mapping.get(&persisted_edge.target).ok_or_else(|| {
                anyhow::anyhow!(
                    "Corrupted graph: edge references missing target node index {}",
                    persisted_edge.target
                )
            })?;
            graph.add_edge(*source, *target, persisted_edge.edge_type.clone());
        }

        // Rebuild node_map (path → NodeIndex)
        let node_map: HashMap<String, NodeIndex> = persisted
            .node_map
            .iter()
            .filter_map(|(path, idx)| {
                index_mapping.get(idx).map(|ni| (path.clone(), *ni))
            })
            .collect();

        // Rebuild entity_map (Uuid → NodeIndex)
        let entity_map: HashMap<Uuid, NodeIndex> = persisted
            .entity_map
            .iter()
            .filter_map(|(uuid_str, idx)| {
                let uuid = Uuid::parse_str(uuid_str).ok()?;
                index_mapping.get(idx).map(|ni| (uuid, *ni))
            })
            .collect();

        Ok(Self {
            graph,
            node_map,
            entity_map,
            storage_path: Some(dir.to_path_buf()),
            storage_version: GRAPH_STORAGE_VERSION,
            dirty: false,
        })
    }

    /// Check whether a persisted graph exists at the given vault path.
    ///
    /// Returns `true` if both `graph.json` and `metadata.json` exist
    /// and the metadata looks valid.
    pub fn persisted_exists(vault_path: &Path) -> bool {
        let dir = vault_path.join(GRAPH_DIR);
        let graph_path = dir.join(GRAPH_FILE);
        let metadata_path = dir.join(METADATA_FILE);

        if !graph_path.exists() || !metadata_path.exists() {
            return false;
        }

        // Quick validation — try parsing metadata
        std::fs::read_to_string(&metadata_path)
            .ok()
            .and_then(|s| serde_json::from_str::<GraphMetadata>(&s).ok())
            .map(|m| m.version <= GRAPH_STORAGE_VERSION)
            .unwrap_or(false)
    }

    /// Delete the persisted graph files from disk.
    ///
    /// After calling this, the graph will be rebuilt from source files
    /// on the next vault open. The in-memory graph is untouched.
    pub fn clear_persistence(vault_path: &Path) -> anyhow::Result<()> {
        let dir = vault_path.join(GRAPH_DIR);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}

// ===========================================================================
// Mark dirty and auto-save
// ===========================================================================

impl VaultGraph {
    /// Mark the graph as having unsaved changes.
    /// If persistence is configured, saves immediately.
    fn mark_dirty(&mut self) {
        if self.storage_path.is_some() && !self.dirty {
            self.dirty = true;
            // Attempt save — log failure but don't propagate
            if let Err(e) = self.save() {
                eprintln!("VaultGraph save failed: {}", e);
            }
        }
    }
}

// ===========================================================================
// Graph mutation methods — all auto-save when persistence is enabled
// ===========================================================================

impl VaultGraph {
    pub fn add_folder(&mut self, folder_object: KnowledgeObject) {
        let path = folder_object.metadata.source_file.clone().unwrap_or_default();
        self.node_map
            .entry(path.clone())
            .or_insert_with(|| self.graph.add_node(GraphNode::Object(folder_object)));
        self.mark_dirty();
    }

    pub fn add_note(&mut self, note_object: KnowledgeObject, content: &str) {
        let note_path = note_object.metadata.source_file.clone().unwrap_or_default();
        let node_index = *self
            .node_map
            .entry(note_path.clone())
            .or_insert_with(|| self.graph.add_node(GraphNode::Object(note_object)));

        let re = Regex::new(r"\[\[(.*?)\]\]").unwrap();

        for cap in re.captures_iter(content) {
            let target = cap[1].to_string();
            let target_node_index = *self
                .node_map
                .entry(target.clone())
                .or_insert_with(|| {
                    self.graph.add_node(GraphNode::Object(KnowledgeObject::default()))
                });
            self.graph
                .add_edge(node_index, target_node_index, GraphEdgeType::WikiLink);
        }

        self.mark_dirty();
    }

    pub fn get_backlinks(&self, note_path: &str) -> Vec<String> {
        let node_index = match self.node_map.get(note_path) {
            Some(idx) => *idx,
            None => return Vec::new(),
        };

        self.graph
            .edges_directed(node_index, petgraph::Direction::Incoming)
            .filter(|e| matches!(e.weight(), GraphEdgeType::WikiLink))
            .filter_map(|e| {
                let source = e.source();
                if let GraphNode::Object(obj) = &self.graph[source] {
                    obj.metadata.source_file.clone()
                } else {
                    None
                }
            })
            .collect()
    }

    /// Filter graph nodes by tag.
    ///
    /// # Known debt (Principle 8 — Metadata First)
    ///
    /// This currently reads each node's source file from disk to extract tags.
    pub fn filter_by_tag(&self, tag: &str) -> Vec<String> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                if let GraphNode::Object(obj) = &self.graph[idx] {
                    let path = obj.metadata.source_file.as_deref().unwrap_or("");
                    let content = std::fs::read_to_string(path).unwrap_or_default();
                    crate::markdown::extract_tags(&content).contains(&tag.to_string())
                } else {
                    false
                }
            })
            .filter_map(|idx| {
                if let GraphNode::Object(obj) = &self.graph[idx] {
                    obj.metadata.source_file.clone()
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn add_entity(&mut self, entity_id: Uuid) {
        if self.entity_map.contains_key(&entity_id) {
            return;
        }
        let node_index = self.graph.add_node(GraphNode::Entity(entity_id));
        self.entity_map.insert(entity_id, node_index);
        self.mark_dirty();
    }

    pub fn add_semantic_relation(&mut self, source: Uuid, target: Uuid, relation: RelationType) {
        if let (Some(&s_idx), Some(&t_idx)) =
            (self.entity_map.get(&source), self.entity_map.get(&target))
        {
            self.graph
                .add_edge(s_idx, t_idx, GraphEdgeType::Semantic(relation));
            self.mark_dirty();
        }
    }

    /// Update an existing KnowledgeObject node in the graph.
    /// If the node exists, its content is updated in-place without rebuilding the entire graph.
    /// If the node does not exist, it is added.
    pub fn update_node(&mut self, object: &KnowledgeObject) {
        let path = object.metadata.source_file.clone().unwrap_or_default();
        if let Some(&node_idx) = self.node_map.get(&path) {
            // Update existing node in-place — no full rebuild needed
            self.graph[node_idx] = GraphNode::Object(object.clone());
        } else {
            // Node doesn't exist yet, add it
            let node_index = self.graph.add_node(GraphNode::Object(object.clone()));
            self.node_map.insert(path, node_index);
        }
        self.mark_dirty();
    }

    /// Remove a node from the graph by its source file path.
    /// Also removes all edges connected to this node.
    pub fn remove_node(&mut self, path: &str) {
        if let Some(&node_idx) = self.node_map.get(path) {
            let edges: Vec<_> = self.graph.edges(node_idx).map(|e| e.id()).collect();
            for edge_id in edges {
                self.graph.remove_edge(edge_id);
            }
            self.graph.remove_node(node_idx);
            self.node_map.remove(path);
        }
        self.mark_dirty();
    }

    /// Update a semantic relation between two entities.
    /// If the relation already exists, it is replaced.
    /// If either entity doesn't exist, it is created.
    pub fn update_semantic_relation(&mut self, source: Uuid, target: Uuid, relation: RelationType) {
        let s_idx = *self
            .entity_map
            .entry(source)
            .or_insert_with(|| self.graph.add_node(GraphNode::Entity(source)));
        let t_idx = *self
            .entity_map
            .entry(target)
            .or_insert_with(|| self.graph.add_node(GraphNode::Entity(target)));

        let edges: Vec<_> = self
            .graph
            .edges_connecting(s_idx, t_idx)
            .map(|e| e.id())
            .collect();
        for edge_id in edges {
            self.graph.remove_edge(edge_id);
        }

        self.graph
            .add_edge(s_idx, t_idx, GraphEdgeType::Semantic(relation));
        self.mark_dirty();
    }

    /// Remove a semantic relation between two entities.
    pub fn remove_semantic_relation(&mut self, source: Uuid, target: Uuid) {
        if let (Some(&s_idx), Some(&t_idx)) =
            (self.entity_map.get(&source), self.entity_map.get(&target))
        {
            let edges: Vec<_> = self
                .graph
                .edges_connecting(s_idx, t_idx)
                .map(|e| e.id())
                .collect();
            for edge_id in edges {
                self.graph.remove_edge(edge_id);
            }
        }
        self.mark_dirty();
    }

    /// Get the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for VaultGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    format!("{}.{:03}Z", secs, millis)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{
        KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_note_object(path: &str, title: &str) -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::Markdown,
            metadata: ObjectMetadata {
                title: Some(title.to_string()),
                source_file: Some(path.to_string()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn new_graph_is_empty() {
        let g = VaultGraph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
        assert!(!g.is_dirty());
        assert!(g.storage_path().is_none());
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();

        // Create a graph with some nodes and edges
        let mut graph = VaultGraph::new();
        let obj1 = create_note_object("/path/to/a.md", "Note A");
        let obj2 = create_note_object("/path/to/b.md", "Note B");
        graph.add_note(obj1, "Content with [[Note B]] wiki-link");
        graph.add_note(obj2, "Content");

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        // Save to temp dir
        graph.save_to_path(&storage_path).unwrap();

        // Load from temp dir
        let loaded = VaultGraph::load_from_path(&storage_path).unwrap();
        assert_eq!(loaded.node_count(), 2);
        assert_eq!(loaded.edge_count(), 1);
        assert!(loaded.storage_path().is_some());

        // Verify backlinks work after reload
        let backlinks = loaded.get_backlinks("/path/to/b.md");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0], "/path/to/a.md");
    }

    #[test]
    fn save_and_load_with_entities() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();

        let mut graph = VaultGraph::new();
        let entity_id = Uuid::new_v4();
        let entity_id2 = Uuid::new_v4();

        graph.add_entity(entity_id);
        graph.add_entity(entity_id2);
        graph.add_semantic_relation(
            entity_id,
            entity_id2,
            RelationType::RelatedTo,
        );

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        graph.save_to_path(&storage_path).unwrap();

        let loaded = VaultGraph::load_from_path(&storage_path).unwrap();
        assert_eq!(loaded.node_count(), 2);
        assert_eq!(loaded.edge_count(), 1);
    }

    #[test]
    fn integrity_check_detects_corruption() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();

        let mut graph = VaultGraph::new();
        let obj = create_note_object("/path/to/a.md", "Note A");
        graph.add_note(obj, "Content");
        graph.save_to_path(&storage_path).unwrap();

        // Corrupt the graph file
        let graph_file = storage_path.join(GRAPH_FILE);
        std::fs::write(&graph_file, "corrupted data").unwrap();

        // Loading should fail with integrity error
        let result = VaultGraph::load_from_path(&storage_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("checksum") || err.contains("integrity"));
    }

    #[test]
    fn missing_metadata_file_returns_error() {
        let dir = tempdir().unwrap();
        let result = VaultGraph::load_from_path(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn persisted_exists_returns_true_for_valid_graph() {
        let dir = tempdir().unwrap();

        // No files yet
        assert!(!VaultGraph::persisted_exists(dir.path()));

        // Create and save
        let mut graph = VaultGraph::new();
        let obj = create_note_object("/path/to/a.md", "Note A");
        graph.add_note(obj, "Content");
        graph.save_to_path(dir.path()).unwrap();

        // Should detect the persisted graph
        assert!(VaultGraph::persisted_exists(dir.path()));
    }

    #[test]
    fn clear_persistence_removes_files() {
        let dir = tempdir().unwrap();

        let mut graph = VaultGraph::new();
        let obj = create_note_object("/path/to/a.md", "Note A");
        graph.add_note(obj, "Content");
        graph.save_to_path(dir.path()).unwrap();

        assert!(VaultGraph::persisted_exists(dir.path()));
        VaultGraph::clear_persistence(dir.path()).unwrap();
        assert!(!VaultGraph::persisted_exists(dir.path()));
    }

    #[test]
    fn with_storage_loads_existing_graph() {
        let dir = tempdir().unwrap();

        // Save a graph
        let mut graph = VaultGraph::new();
        let obj = create_note_object("/path/to/a.md", "Note A");
        graph.add_note(obj, "Content");
        graph.save_to_path(dir.path()).unwrap();

        // Re-open with with_storage
        let loaded = VaultGraph::with_storage(dir.path().to_path_buf());
        assert_eq!(loaded.node_count(), 1);
        assert_eq!(loaded.edge_count(), 0);
        assert!(loaded.storage_path().is_some());
    }

    #[test]
    fn with_storage_returns_empty_when_no_graph_exists() {
        let dir = tempdir().unwrap();
        let graph = VaultGraph::with_storage(dir.path().to_path_buf());
        assert_eq!(graph.node_count(), 0);
        assert!(graph.storage_path().is_some());
    }

    #[test]
    fn update_node_triggers_save() {
        let dir = tempdir().unwrap();
        let storage_path = dir.path().to_path_buf();

        // Create graph with storage
        let mut graph = VaultGraph::new();
        let obj = create_note_object("/path/to/a.md", "Note A");
        graph.add_note(obj, "Content");
        graph.save_to_path(&storage_path).unwrap();

        // Verify it's not dirty after save
        assert!(!graph.is_dirty());

        // Further mutations would mark dirty, but since there's no storage_path
        // on this graph object (it was created with new()), let's test the
        // storage path set via load
        drop(graph);

        // Load with storage path
        let mut loaded = VaultGraph::load_from_path(&storage_path).unwrap();
        assert!(!loaded.is_dirty());

        // Update node — should auto-save
        let updated = create_note_object("/path/to/a.md", "Updated A");
        loaded.update_node(&updated);
        // After save, dirty should be false
        assert!(!loaded.is_dirty());
    }
}
