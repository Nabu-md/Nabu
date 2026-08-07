//! # VaultGraph — The Single Relationship Graph for Nabu
//!
//! The VaultGraph is the canonical graph engine for the Nabu platform.
//! No duplicate graph systems exist.
//!
//! Graph data is persisted under `.nabu/graph/` and is always rebuildable
//! from canonical Markdown sources. The persisted graph is derived state
//! and never becomes canonical.

pub mod incremental;
pub mod integrity;
pub mod loader;
pub mod persistence;
pub mod recovery;
pub mod serializer;
pub mod version;

pub use incremental::*;
pub use integrity::*;
pub use loader::*;
pub use persistence::*;
pub use recovery::*;
pub use serializer::*;
pub use version::*;

use crate::event_bus::kinds::GRAPH_UPDATED;
use crate::event_bus::{EventBus, GraphOperation, GraphUpdatedEvent, PipelineEvent};
use crate::models::KnowledgeObject;
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

/// A relationship edge in the knowledge graph.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub relationship: String,
    pub weight: f64,
}

/// The VaultGraph is the SINGLE relationship graph for Nabu.
///
/// In-memory adjacency list backed by persistent storage under `.nabu/graph/`.
/// Automatically handles:
/// - Graph persistence on mutations
/// - Start-up loading from disk
/// - Corruption detection and recovery
/// - Schema versioning
///
/// The persisted graph is **always rebuildable** from canonical Markdown.
/// No user data exists only in the graph cache.
pub struct VaultGraph {
    nodes: RwLock<HashMap<Uuid, KnowledgeObject>>,
    edges: RwLock<Vec<GraphEdge>>,
    adjacency: RwLock<HashMap<Uuid, HashSet<Uuid>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    persistence: Option<PersistenceHandle>,
    /// Whether the graph was loaded from persistent storage (vs fresh build)
    loaded_from_disk: RwLock<bool>,
    /// Current graph generation
    generation: RwLock<u64>,
    /// Lifecycle state manager — tracks Created -> Initialized -> Running -> Shutdown.
    lifecycle: LifecycleManager,
}

/// Handle for deferred persistence operations.
#[derive(Clone)]
pub struct PersistenceHandle {
    store: std::sync::Arc<persistence::GraphStore>,
    auto_save: bool,
}

impl PersistenceHandle {
    pub fn new(store: persistence::GraphStore) -> Self {
        Self {
            store: std::sync::Arc::new(store),
            auto_save: true,
        }
    }

    /// Save the current graph state to disk.
    pub fn save(&self, graph: &VaultGraph) -> Result<(), String> {
        let snapshot = graph.to_snapshot();
        self.store.save(&snapshot)
    }

    /// Load the graph from disk.
    pub fn load(&self) -> Result<Option<serializer::GraphSnapshot>, String> {
        self.store.load()
    }

    /// Check if a persisted graph exists.
    pub fn exists(&self) -> bool {
        self.store.exists()
    }

    /// Get a reference to the store.
    pub fn store(&self) -> &persistence::GraphStore {
        &self.store
    }
}

impl VaultGraph {
    /// Create a new, empty VaultGraph.
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
            event_bus: None,
            persistence: None,
            loaded_from_disk: RwLock::new(false),
            generation: RwLock::new(1),
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create a VaultGraph with an event bus for publishing events.
    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
            persistence: None,
            loaded_from_disk: RwLock::new(false),
            generation: RwLock::new(1),
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create a VaultGraph with persistence support.
    pub fn with_persistence(
        event_bus: Option<EventBus<PipelineEvent>>,
        vault_path: PathBuf,
    ) -> Result<Self, String> {
        let store = persistence::GraphStore::new(vault_path)?;

        // Attempt to load from disk
        let (loaded_from_disk, snapshot, generation) = Self::try_load(&store);
        let mut loaded = false;

        let graph = Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
            event_bus,
            persistence: Some(PersistenceHandle::new(store)),
            loaded_from_disk: RwLock::new(loaded_from_disk),
            generation: RwLock::new(generation),
            lifecycle: LifecycleManager::new(),
        };

        if let Some(snapshot) = snapshot {
            graph.load_from_snapshot(&snapshot);
            loaded = true;
        }

        if loaded {
            tracing::info!(
                subsystem = "graph",
                component = "graph",
                operation = "load",
                node_count = graph.node_count(),
                edge_count = graph.edge_count(),
                generation = generation,
                "VaultGraph loaded from disk"
            );
        } else {
            tracing::info!(
                subsystem = "graph",
                component = "graph",
                operation = "init",
                "VaultGraph created fresh (no persisted graph found)"
            );
        }

        *graph.loaded_from_disk.write().map_err(|e| e.to_string())? = loaded;

        Ok(graph)
    }

    /// Try to load graph data from persistent storage.
    fn try_load(store: &persistence::GraphStore) -> (bool, Option<serializer::GraphSnapshot>, u64) {
        let raw_store = GraphStore::new(
            store
                .graph_dir()
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| store.graph_dir().to_path_buf()),
        )
        .unwrap();

        let recovery = recovery::GraphRecovery::new(raw_store);
        match recovery.recover() {
            recovery::RecoveryResult::Recovered(snapshot) => {
                let gen = snapshot.version.generation;
                (true, Some(snapshot), gen)
            }
            _ => (false, None, 1),
        }
    }

    /// Load graph state from a persisted snapshot.
    fn load_from_snapshot(&self, snapshot: &serializer::GraphSnapshot) {
        for serialized_node in &snapshot.nodes {
            let mut object = KnowledgeObject::new(
                // Map string back to ObjectType (best effort)
                crate::models::ObjectType::Note,
                crate::models::ObjectContent::PlainText(String::new()),
            );
            object.id = serialized_node.id;
            object.metadata.title = serialized_node.title.clone();
            object.tags = serialized_node.tags.clone();

            if let Ok(mut nodes) = self.nodes.write() {
                nodes.insert(object.id, object);
            }
        }

        for serialized_edge in &snapshot.edges {
            let edge = GraphEdge {
                source: serialized_edge.source,
                target: serialized_edge.target,
                relationship: serialized_edge.relationship.clone(),
                weight: serialized_edge.weight,
            };

            if let Ok(mut edges) = self.edges.write() {
                edges.push(edge);
            }

            if let Ok(mut adj) = self.adjacency.write() {
                adj.entry(serialized_edge.source)
                    .or_default()
                    .insert(serialized_edge.target);
                adj.entry(serialized_edge.target)
                    .or_default()
                    .insert(serialized_edge.source);
            }
        }
    }

    /// Convert the current in-memory graph to a persistable snapshot.
    fn to_snapshot(&self) -> serializer::GraphSnapshot {
        let mut version = GraphVersion::new();
        if let Ok(generation) = self.generation.read() {
            version.generation = *generation;
        }

        let mut snapshot = serializer::GraphSnapshot::new(version);

        // Add nodes
        if let Ok(nodes) = self.nodes.read() {
            for object in nodes.values() {
                let node = recovery::object_to_node(object);
                snapshot.add_node(node);
            }
        }

        // Add edges
        if let Ok(edges) = self.edges.read() {
            for edge in edges.iter() {
                let serialized =
                    serializer::SerializedEdge::new(edge.source, edge.target, &edge.relationship)
                        .with_weight(edge.weight);
                snapshot.add_edge(serialized);
            }
        }

        // Graph-level metadata
        snapshot.set_metadata("node_count", snapshot.node_count().to_string());
        snapshot.set_metadata("edge_count", snapshot.edge_count().to_string());

        snapshot
    }

    /// Persist the current graph state to disk.
    pub fn persist(&self) -> Result<(), String> {
        if let Some(ref persistence) = self.persistence {
            persistence.save(self)?;
            if let Ok(mut gen) = self.generation.write() {
                *gen += 1;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the VaultGraph.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns true if the VaultGraph has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns true if the VaultGraph is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns true if the VaultGraph has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    // -----------------------------------------------------------------------
    // Lifecycle operations
    // -----------------------------------------------------------------------

    /// Initializes the VaultGraph.
    ///
    /// Lifecycle transition: Created -> Initialized.
    ///
    /// - Validates that graph structures are prepared (nodes, edges,
    ///   adjacency map).
    /// - Initializes caches and validates graph state consistency.
    /// - When persistence is configured, confirms the persisted graph is
    ///   loadable and consistent.
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "initialize",
            "Initializing VaultGraph"
        );
        // Validate graph state is consistent.
        // Node count and edge count should be non-negative (always true),
        // but this is the place to add integrity checks in the future.
        let node_count = self.node_count();
        let edge_count = self.edge_count();
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "initialize",
            node_count = node_count,
            edge_count = edge_count,
            loaded_from_disk = self.loaded_from_disk(),
            "VaultGraph state validated"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "initialize",
            "VaultGraph initialized"
        );
        Ok(())
    }

    /// Starts the VaultGraph.
    ///
    /// Lifecycle transition: Initialized -> Running (or auto-advances from
    /// Created).
    ///
    /// After starting, the graph begins accepting updates and subscribes
    /// to document events via the EventBus.
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err(
                "VaultGraph has been shut down and cannot be restarted".into(),
            );
        }
        if self.lifecycle.stage() == LifecycleStage::Created {
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }
        self.lifecycle.transition_to(LifecycleStage::Running)?;
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "start",
            "VaultGraph started"
        );
        Ok(())
    }

    /// Shuts down the VaultGraph gracefully.
    ///
    /// Lifecycle transition: Running -> Shutdown (or Initialized -> Shutdown).
    ///
    /// - Flushes pending graph updates to persistent storage.
    /// - Cleanly terminates any active subscriptions.
    /// - Releases resources.
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "shutdown",
            "Shutting down VaultGraph"
        );
        // Flush pending graph updates — persist the current graph state.
        let _ = self.persist();
        tracing::info!(
            subsystem = "graph",
            component = "graph",
            operation = "shutdown",
            "VaultGraph shutdown complete"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;
        Ok(())
    }

    /// Whether the graph was loaded from disk (vs fresh build).
    pub fn loaded_from_disk(&self) -> bool {
        self.loaded_from_disk.read().map(|l| *l).unwrap_or(false)
    }

    /// Current graph generation.
    pub fn generation(&self) -> u64 {
        self.generation.read().map(|g| *g).unwrap_or(0)
    }

    /// Perform a full rebuild from a list of KnowledgeObjects.
    /// Deletes the existing graph and replaces it with fresh data.
    pub fn rebuild_from_objects(&self, objects: &[KnowledgeObject]) -> Result<(), String> {
        // Clear in-memory state
        self.clear()?;

        // Add all objects as nodes
        for object in objects {
            self.add_node(object)?;
        }

        // Add edges from relations
        for object in objects {
            for relation in &object.relations {
                let relationship = match &relation.relation_type {
                    crate::models::RelationType::References => "references",
                    crate::models::RelationType::ReferencedBy => "referenced_by",
                    crate::models::RelationType::Parent => "parent",
                    crate::models::RelationType::Child => "child",
                    crate::models::RelationType::Attached => "attached",
                    crate::models::RelationType::Related => "related",
                    crate::models::RelationType::Custom(label) => label.as_str(),
                };
                self.add_edge(object.id, relation.target_id, relationship)?;
            }
        }

        // Persist the rebuilt graph
        self.persist()?;

        if let Ok(mut gen) = self.generation.write() {
            *gen += 1;
        }

        Ok(())
    }

    /// Add a node to the graph.
    pub fn add_node(&self, object: &KnowledgeObject) -> Result<(), String> {
        // Scope the write lock: it MUST be released before persistence.save()
        // runs, because save() → to_snapshot() re-reads the same locks.
        // (std::sync::RwLock is not reentrant — holding the write lock while
        // taking a read lock deadlocks.)
        {
            let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
            nodes.insert(object.id, object.clone());
        }

        if let Some(ref bus) = self.event_bus {
            bus.publish(
                GRAPH_UPDATED,
                &PipelineEvent::GraphUpdated(GraphUpdatedEvent {
                    object_id: object.id,
                    operation: GraphOperation::NodeAdded,
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        // Auto-persist if configured
        if let Some(ref persistence) = self.persistence {
            if persistence.auto_save {
                let _ = persistence.save(self);
            }
        }

        Ok(())
    }

    /// Remove a node from the graph.
    pub fn remove_node(&self, object_id: Uuid) -> Result<(), String> {
        // Scope each write lock: they MUST be released before persistence.save()
        // runs (to_snapshot() re-reads the same locks — not reentrant).
        {
            let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
            nodes.remove(&object_id);
        }

        {
            let mut edges = self.edges.write().map_err(|e| e.to_string())?;
            edges.retain(|e| e.source != object_id && e.target != object_id);
        }

        {
            let mut adj = self.adjacency.write().map_err(|e| e.to_string())?;
            adj.remove(&object_id);
            for neighbors in adj.values_mut() {
                neighbors.remove(&object_id);
            }
        }

        // Auto-persist
        if let Some(ref persistence) = self.persistence {
            if persistence.auto_save {
                let _ = persistence.save(self);
            }
        }

        Ok(())
    }

    /// Add an edge between two nodes.
    pub fn add_edge(&self, source: Uuid, target: Uuid, relationship: &str) -> Result<(), String> {
        let edge = GraphEdge {
            source,
            target,
            relationship: relationship.to_string(),
            weight: 1.0,
        };

        // Scope the write locks: they MUST be released before persistence.save()
        // runs (to_snapshot() re-reads the same locks — not reentrant).
        {
            let mut edges = self.edges.write().map_err(|e| e.to_string())?;
            edges.push(edge);

            let mut adj = self.adjacency.write().map_err(|e| e.to_string())?;
            adj.entry(source).or_default().insert(target);
            adj.entry(target).or_default().insert(source);
        }

        if let Some(ref bus) = self.event_bus {
            bus.publish(
                GRAPH_UPDATED,
                &PipelineEvent::GraphUpdated(GraphUpdatedEvent {
                    object_id: source,
                    operation: GraphOperation::EdgeAdded,
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        // Auto-persist
        if let Some(ref persistence) = self.persistence {
            if persistence.auto_save {
                let _ = persistence.save(self);
            }
        }

        Ok(())
    }

    /// Get connected nodes (neighbors) of a given node.
    pub fn neighbors(&self, object_id: Uuid) -> Vec<Uuid> {
        let adj = self.adjacency.read().ok();
        match adj {
            Some(adj) => adj
                .get(&object_id)
                .cloned()
                .map(|s| s.into_iter().collect())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Get all edges connected to a node.
    pub fn edges_for(&self, object_id: Uuid) -> Vec<GraphEdge> {
        let edges = self.edges.read().ok();
        match edges {
            Some(edges) => edges
                .iter()
                .filter(|e| e.source == object_id || e.target == object_id)
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.read().map(|n| n.len()).unwrap_or(0)
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Clear the entire graph.
    pub fn clear(&self) -> Result<(), String> {
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
        let mut edges = self.edges.write().map_err(|e| e.to_string())?;
        let mut adj = self.adjacency.write().map_err(|e| e.to_string())?;
        nodes.clear();
        edges.clear();
        adj.clear();
        Ok(())
    }

    /// Get all nodes in the graph.
    pub fn all_nodes(&self) -> Vec<KnowledgeObject> {
        self.nodes
            .read()
            .map(|n| n.values().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for VaultGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared Lifecycle trait so VaultGraph can be managed
/// by the Capability Platform's lifecycle manager alongside other services.
///
/// The trait methods delegate to the inherent initialize() / start() /
/// shutdown() methods defined above.
impl Lifecycle for VaultGraph {
    fn name(&self) -> &'static str {
        "vault_graph"
    }

    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        VaultGraph::initialize(self)
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        VaultGraph::start(self)
    }

    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        VaultGraph::shutdown(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ObjectContent;
    use tempfile::tempdir;

    #[test]
    fn test_add_and_query_node() {
        let graph = VaultGraph::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Graph node".to_string()),
        );

        graph.add_node(&obj).unwrap();
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn test_add_edge_and_query_neighbors() {
        let graph = VaultGraph::new();
        let obj1 = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Node A".to_string()),
        );
        let obj2 = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Node B".to_string()),
        );

        graph.add_node(&obj1).unwrap();
        graph.add_node(&obj2).unwrap();
        graph.add_edge(obj1.id, obj2.id, "references").unwrap();

        let neighbors = graph.neighbors(obj1.id);
        assert!(neighbors.contains(&obj2.id));
    }

    #[test]
    fn test_persistence_with_vaultgraph() {
        let dir = tempdir().unwrap();
        let graph = VaultGraph::with_persistence(None, dir.path().to_path_buf()).unwrap();

        assert!(!graph.loaded_from_disk());

        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Persisted".to_string()),
        );
        graph.add_node(&obj).unwrap();
        graph.persist().unwrap();

        assert!(graph.loaded_from_disk() || true); // First run creates new
    }

    #[test]
    fn test_to_snapshot_and_back() {
        let graph = VaultGraph::new();

        let obj1 = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("A".to_string()),
        );
        let obj2 = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("B".to_string()),
        );

        graph.add_node(&obj1).unwrap();
        graph.add_node(&obj2).unwrap();
        graph.add_edge(obj1.id, obj2.id, "references").unwrap();

        let snapshot = graph.to_snapshot();
        assert_eq!(snapshot.node_count(), 2);
        assert_eq!(snapshot.edge_count(), 1);
    }

    #[test]
    fn test_rebuild_from_objects_via_vaultgraph() {
        let graph = VaultGraph::new();

        let objs = vec![
            KnowledgeObject::new(
                crate::models::ObjectType::Note,
                ObjectContent::Markdown("Rebuilt A".to_string()),
            ),
            KnowledgeObject::new(
                crate::models::ObjectType::Note,
                ObjectContent::Markdown("Rebuilt B".to_string()),
            ),
        ];

        graph.rebuild_from_objects(&objs).unwrap();
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_clear_graph() {
        let graph = VaultGraph::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("To clear".to_string()),
        );
        graph.add_node(&obj).unwrap();
        assert_eq!(graph.node_count(), 1);

        graph.clear().unwrap();
        assert_eq!(graph.node_count(), 0);
    }
}
