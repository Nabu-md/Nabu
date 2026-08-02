use crate::graph::incremental::change_log::{ChangeEntry, ChangeLog, CheckpointData};
use crate::graph::incremental::dependency_tracker::DependencyTracker;
use crate::graph::incremental::region::RegionEngine;
use crate::graph::incremental::update_tracker::{EdgeKey, UpdateTracker};
use crate::graph::persistence::GraphStore;
use crate::graph::serializer::{GraphSnapshot, SerializedEdge, SerializedNode};

use crate::models::KnowledgeObject;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The IncrementalUpdateEngine is the single entry point for all incremental
/// graph updates. It coordinates:
///
/// 1. **Update tracking** — Record what changed
/// 2. **Dependency tracking** — Determine what else is affected
/// 3. **Region identification** — Find which graph regions need recalculation
/// 4. **Change logging** — Append changes for persistence
/// 5. **Snapshot application** — Apply changes to the in-memory graph
/// 6. **Compaction** — Periodically compact the change log
///
/// Instead of rewriting the entire graph on every change, this engine only
/// updates the affected regions, achieving O(changed documents) performance.
pub struct IncrementalUpdateEngine {
    /// Tracks all pending changes
    pub tracker: UpdateTracker,

    /// Tracks dependencies between graph elements
    pub dependencies: DependencyTracker,

    /// Region management for targeted rebuilds
    pub regions: RegionEngine,

    /// Append-only change log
    pub change_log: Option<ChangeLog>,

    /// Graph store for persistence
    pub store: Option<GraphStore>,

    /// Whether the engine is currently in a transaction
    in_transaction: bool,

    /// Changes accumulated during the current transaction
    transaction_tracker: Option<UpdateTracker>,
}

impl IncrementalUpdateEngine {
    pub fn new() -> Self {
        Self {
            tracker: UpdateTracker::new(),
            dependencies: DependencyTracker::new(),
            regions: RegionEngine::new(),
            change_log: None,
            store: None,
            in_transaction: false,
            transaction_tracker: None,
        }
    }

    /// Initialize the engine with a persisted graph.
    pub fn initialize(
        &mut self,
        snapshot: &GraphSnapshot,
        store: GraphStore,
    ) -> Result<(), String> {
        // Build dependency tracking from snapshot
        self.build_dependencies_from_snapshot(snapshot);

        // Discover regions
        self.regions.discover_regions(snapshot);

        // Initialize change log
        let graph_dir = store.graph_dir().to_path_buf();
        self.store = Some(store);
        self.change_log = Some(
            ChangeLog::new(graph_dir)?,
        );

        Ok(())
    }

    /// Build dependency tracking from an existing snapshot.
    fn build_dependencies_from_snapshot(&mut self, snapshot: &GraphSnapshot) {
        for edge in &snapshot.edges {
            self.dependencies.add_dependency(edge.source, edge.target);
        }
    }

    /// Returns the tracker mutations should be recorded into.
    ///
    /// While a transaction is active, changes accumulate in the transaction
    /// tracker and are only merged into the main tracker on commit; rolling
    /// back discards them entirely.
    fn active_tracker(&mut self) -> &mut UpdateTracker {
        if self.in_transaction {
            self.transaction_tracker
                .get_or_insert_with(UpdateTracker::new)
        } else {
            &mut self.tracker
        }
    }

    /// Record that a node was added (new KnowledgeObject).
    pub fn node_added(&mut self, object: &KnowledgeObject) {
        self.active_tracker().record_node_added(object.id);

        // Register in dependency tracker
        for relation in &object.relations {
            self.dependencies
                .add_dependency(object.id, relation.target_id);
            self.dependencies
                .register_relationship_type(object.id, format!("{:?}", relation.relation_type));
        }

        // Log
        if let Some(ref log) = self.change_log {
            let node = crate::graph::recovery::object_to_node(object);
            let _ = log.append(&ChangeEntry::NodeAdded(node));
        }
    }

    /// Record that a node was modified.
    pub fn node_modified(&mut self, object: &KnowledgeObject, old_object: Option<&KnowledgeObject>) {
        self.active_tracker().record_node_modified(object.id);

        // Detect relationship changes
        if let Some(old) = old_object {
            let old_relations: HashSet<_> = old.relations.iter().map(|r| r.target_id).collect();
            let new_relations: HashSet<_> = object.relations.iter().map(|r| r.target_id).collect();

            // Removed relationships
            for removed in old_relations.difference(&new_relations) {
                self.dependencies.remove_dependency(object.id, *removed);
                self.active_tracker().record_edge_removed(EdgeKey::new(
                    object.id,
                    *removed,
                    "references",
                ));
            }

            // Added relationships
            for added in new_relations.difference(&old_relations) {
                self.dependencies.add_dependency(object.id, *added);
                self.active_tracker().record_edge_added(EdgeKey::new(
                    object.id,
                    *added,
                    "references",
                ));
            }
        }

        // Log
        if let Some(ref log) = self.change_log {
            let node = crate::graph::recovery::object_to_node(object);
            let _ = log.append(&ChangeEntry::NodeModified(node));
        }
    }

    /// Record that a node was removed.
    pub fn node_removed(&mut self, node_id: Uuid) {
        self.active_tracker().record_node_removed(node_id);
        self.dependencies.remove_all_dependencies(node_id);

        // Log
        if let Some(ref log) = self.change_log {
            let _ = log.append(&ChangeEntry::NodeRemoved { node_id });
        }
    }

    /// Record that an edge was added.
    pub fn edge_added(&mut self, source: Uuid, target: Uuid, relationship: &str) {
        let key = EdgeKey::new(source, target, relationship);
        self.active_tracker().record_edge_added(key);
        self.dependencies.add_dependency(source, target);

        if let Some(ref log) = self.change_log {
            let _ = log.append(&ChangeEntry::EdgeAdded(SerializedEdge::new(
                source, target, relationship,
            )));
        }
    }

    /// Record that an edge was removed.
    pub fn edge_removed(&mut self, source: Uuid, target: Uuid, relationship: &str) {
        let key = EdgeKey::new(source, target, relationship);
        self.active_tracker().record_edge_removed(key);
        self.dependencies.remove_dependency(source, target);

        if let Some(ref log) = self.change_log {
            let _ = log.append(&ChangeEntry::EdgeRemoved {
                source,
                target,
                relationship: relationship.to_string(),
            });
        }
    }

    /// Begin a transactional update.
    /// All changes recorded until commit() are treated as a single atomic update.
    pub fn begin_transaction(&mut self) {
        self.in_transaction = true;
        self.transaction_tracker = Some(UpdateTracker::new());
    }

    /// Commit the current transaction.
    /// Merges all transactional changes into the main tracker.
    pub fn commit_transaction(&mut self) -> Result<(), String> {
        if !self.in_transaction {
            return Err("No active transaction".to_string());
        }

        // Merge transaction tracker into main tracker
        if let Some(tx_tracker) = self.transaction_tracker.take() {
            for node_id in tx_tracker.added_nodes {
                self.tracker.record_node_added(node_id);
            }
            for node_id in tx_tracker.modified_nodes {
                self.tracker.record_node_modified(node_id);
            }
            for node_id in tx_tracker.removed_nodes {
                self.tracker.record_node_removed(node_id);
            }
            for edge in tx_tracker.added_edges {
                self.tracker.record_edge_added(edge);
            }
            for edge in tx_tracker.removed_edges {
                self.tracker.record_edge_removed(edge);
            }
        }

        self.in_transaction = false;
        Ok(())
    }

    /// Rollback the current transaction.
    pub fn rollback_transaction(&mut self) {
        self.in_transaction = false;
        self.transaction_tracker = None;
    }

    /// Apply all pending updates to the given current nodes and edges.
    ///
    /// Returns the updated nodes and edges.
    pub fn apply_updates(
        &self,
        current_nodes: &mut HashMap<Uuid, SerializedNode>,
        current_edges: &mut Vec<SerializedEdge>,
    ) -> Result<(), String> {
        // Remove deleted nodes and their edges
        for node_id in &self.tracker.removed_nodes {
            current_nodes.remove(node_id);
            current_edges.retain(|e| e.source != *node_id && e.target != *node_id);
        }

        // Add new nodes (they should already be in current_nodes if caller passed them)
        // Modified nodes need their metadata refreshed (handled by caller)

        // Remove deleted edges
        for edge_key in &self.tracker.removed_edges {
            current_edges.retain(|e| {
                !(e.source == edge_key.source
                    && e.target == edge_key.target
                    && e.relationship == edge_key.relationship)
            });
        }

        // Add new edges (will be appended by the caller after adding fresh ones)

        Ok(())
    }

    /// Get the set of nodes that need recalculation (transitive closure).
    pub fn nodes_to_recalculate(&self) -> HashSet<Uuid> {
        let mut to_recalculate = HashSet::new();

        for node_id in &self.tracker.modified_nodes {
            to_recalculate.extend(self.dependencies.transitive_affected(*node_id));
        }

        for node_id in &self.tracker.removed_nodes {
            to_recalculate.extend(self.dependencies.transitive_affected(*node_id));
        }

        for edge_key in &self.tracker.removed_edges {
            to_recalculate.insert(edge_key.source);
            to_recalculate.insert(edge_key.target);
        }

        to_recalculate
    }

    /// Whether there are pending updates.
    pub fn has_pending_updates(&self) -> bool {
        self.tracker.has_pending_changes
    }

    /// Reset the tracker after updates have been applied and persisted.
    pub fn reset(&mut self) {
        self.tracker.reset();
    }

    /// Compact the change log (should be called periodically).
    pub fn compact_log(&self, snapshot: &GraphSnapshot) -> Result<(), String> {
        if let Some(ref log) = self.change_log {
            let checkpoint = CheckpointData {
                nodes: snapshot.nodes.clone(),
                edges: snapshot.edges.clone(),
                generation: snapshot.version.generation,
            };
            log.compact(|| Ok(checkpoint.clone()))?;
        }
        Ok(())
    }

    /// Summary of pending changes.
    pub fn summary(&self) -> String {
        self.tracker.summary()
    }
}

impl Default for IncrementalUpdateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ObjectContent, ObjectType};
    use tempfile::tempdir;

    #[test]
    fn test_node_lifecycle() {
        let mut engine = IncrementalUpdateEngine::new();

        let obj = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Hello".to_string()));
        engine.node_added(&obj);
        assert!(engine.tracker.added_nodes.contains(&obj.id));

        // Modifying a just-added node keeps it tracked as "added" (it is
        // already new — no separate modified entry is created).
        engine.node_modified(&obj, None);
        assert!(engine.tracker.added_nodes.contains(&obj.id));
        assert!(!engine.tracker.modified_nodes.contains(&obj.id));

        // Removing a just-added node cancels out entirely.
        engine.node_removed(obj.id);
        assert!(!engine.tracker.added_nodes.contains(&obj.id));
        assert!(!engine.tracker.removed_nodes.contains(&obj.id));
    }

    #[test]
    fn test_transaction_commit() {
        let mut engine = IncrementalUpdateEngine::new();
        engine.begin_transaction();

        let obj = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Tx".to_string()));
        engine.node_added(&obj);

        // Before commit, tracker should be empty (changes in transaction_tracker)
        assert!(!engine.tracker.has_pending_changes);

        engine.commit_transaction().unwrap();

        // After commit, changes should be in the main tracker
        assert!(engine.tracker.has_pending_changes);
        assert!(engine.tracker.added_nodes.contains(&obj.id));
    }

    #[test]
    fn test_transaction_rollback() {
        let mut engine = IncrementalUpdateEngine::new();
        engine.begin_transaction();

        let obj = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown("Rollback".to_string()));
        engine.node_added(&obj);

        engine.rollback_transaction();

        // After rollback, no changes should have been applied
        assert!(!engine.tracker.has_pending_changes);
    }

    #[test]
    fn test_apply_updates() {
        let mut engine = IncrementalUpdateEngine::new();
        let mut nodes: HashMap<Uuid, SerializedNode> = HashMap::new();
        let mut edges: Vec<SerializedEdge> = Vec::new();

        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();

        nodes.insert(n1, SerializedNode::new(n1, "note", None, "text"));
        nodes.insert(n2, SerializedNode::new(n2, "note", None, "text"));
        edges.push(SerializedEdge::new(n1, n2, "references"));

        // Remove one node
        engine.node_removed(n1);
        engine.apply_updates(&mut nodes, &mut edges).unwrap();

        assert_eq!(nodes.len(), 1);
        assert!(edges.is_empty());
    }

    #[test]
    fn test_nodes_to_recalculate() {
        let mut engine = IncrementalUpdateEngine::new();

        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        engine.dependencies.add_dependency(a, b);
        engine.tracker.record_node_modified(b);

        let to_recalc = engine.nodes_to_recalculate();
        assert!(to_recalc.contains(&a)); // a depends on b, so a needs recalc too
    }
}
