use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Tracks all changes to the graph since the last full rebuild.
///
/// This is the core data structure for incremental updates.
/// It records every add, modify, and delete operation so the
/// IncrementalUpdateEngine can apply only the affected changes.
#[derive(Debug, Clone)]
pub struct UpdateTracker {
    /// Nodes added since last full rebuild
    pub added_nodes: HashSet<Uuid>,

    /// Nodes whose metadata has changed
    pub modified_nodes: HashSet<Uuid>,

    /// Nodes removed since last full rebuild
    pub removed_nodes: HashSet<Uuid>,

    /// Edges added (keyed by (source, target, relationship))
    pub added_edges: HashSet<EdgeKey>,

    /// Edges removed
    pub removed_edges: HashSet<EdgeKey>,

    /// Nodes whose metadata/title has changed
    pub metadata_changed: HashSet<Uuid>,

    /// Nodes whose relationships have changed
    pub relationship_changed: HashSet<Uuid>,

    /// Whether this tracker has any pending changes
    pub has_pending_changes: bool,

    /// Monotonically increasing sequence number
    pub sequence: u64,
}

/// Composite key uniquely identifying an edge.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct EdgeKey {
    pub source: Uuid,
    pub target: Uuid,
    pub relationship: String,
}

impl EdgeKey {
    pub fn new(source: Uuid, target: Uuid, relationship: impl Into<String>) -> Self {
        Self {
            source,
            target,
            relationship: relationship.into(),
        }
    }
}

impl UpdateTracker {
    pub fn new() -> Self {
        Self {
            added_nodes: HashSet::new(),
            modified_nodes: HashSet::new(),
            removed_nodes: HashSet::new(),
            added_edges: HashSet::new(),
            removed_edges: HashSet::new(),
            metadata_changed: HashSet::new(),
            relationship_changed: HashSet::new(),
            has_pending_changes: false,
            sequence: 0,
        }
    }

    /// Record that a node was added.
    pub fn record_node_added(&mut self, node_id: Uuid) {
        // If it was previously removed, it's now a modification instead
        if self.removed_nodes.remove(&node_id) {
            self.modified_nodes.insert(node_id);
        } else {
            self.added_nodes.insert(node_id);
        }
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Record that a node was modified (metadata change).
    pub fn record_node_modified(&mut self, node_id: Uuid) {
        if !self.added_nodes.contains(&node_id) {
            self.modified_nodes.insert(node_id);
            self.metadata_changed.insert(node_id);
        }
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Record that a node was removed.
    pub fn record_node_removed(&mut self, node_id: Uuid) {
        // If it was just added, remove from added set
        if self.added_nodes.remove(&node_id) {
            // Never existed — nothing to do
        } else {
            self.removed_nodes.insert(node_id);
            self.modified_nodes.remove(&node_id);
        }
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Record that a node's relationships changed.
    pub fn record_relationship_changed(&mut self, node_id: Uuid) {
        self.relationship_changed.insert(node_id);
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Record that an edge was added.
    pub fn record_edge_added(&mut self, edge: EdgeKey) {
        if self.removed_edges.remove(&edge) {
            // Was removed then re-added — cancel out
        } else {
            self.added_edges.insert(edge.clone());
        }
        self.relationship_changed.insert(edge.source);
        self.relationship_changed.insert(edge.target);
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Record that an edge was removed.
    pub fn record_edge_removed(&mut self, edge: EdgeKey) {
        if self.added_edges.remove(&edge) {
            // Was added then removed — cancel out
        } else {
            self.removed_edges.insert(edge.clone());
        }
        self.relationship_changed.insert(edge.source);
        self.relationship_changed.insert(edge.target);
        self.has_pending_changes = true;
        self.sequence += 1;
    }

    /// Get all node IDs that are affected by the current changes.
    /// This includes nodes that were added, modified, removed, or had relationship changes.
    pub fn affected_nodes(&self) -> HashSet<Uuid> {
        let mut nodes = HashSet::new();
        nodes.extend(&self.added_nodes);
        nodes.extend(&self.modified_nodes);
        nodes.extend(&self.removed_nodes);
        nodes.extend(&self.relationship_changed);
        nodes
    }

    /// Get all edge keys that are affected by the current changes.
    pub fn affected_edges(&self) -> HashSet<&EdgeKey> {
        self.added_edges.union(&self.removed_edges).collect()
    }

    /// Number of individual changes tracked.
    pub fn change_count(&self) -> usize {
        self.added_nodes.len()
            + self.modified_nodes.len()
            + self.removed_nodes.len()
            + self.added_edges.len()
            + self.removed_edges.len()
    }

    /// Whether there are no pending changes.
    pub fn is_clean(&self) -> bool {
        !self.has_pending_changes
    }

    /// Reset the tracker after applying changes.
    pub fn reset(&mut self) {
        self.added_nodes.clear();
        self.modified_nodes.clear();
        self.removed_nodes.clear();
        self.added_edges.clear();
        self.removed_edges.clear();
        self.metadata_changed.clear();
        self.relationship_changed.clear();
        self.has_pending_changes = false;
    }

    /// Summary of tracked changes as a string.
    pub fn summary(&self) -> String {
        format!(
            "Δ: +{} nodes, ~{} modified, -{} removed; +{} edges, -{} edges",
            self.added_nodes.len(),
            self.modified_nodes.len(),
            self.removed_nodes.len(),
            self.added_edges.len(),
            self.removed_edges.len(),
        )
    }
}

impl Default for UpdateTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_add_and_remove_cancel() {
        let mut tracker = UpdateTracker::new();
        let id = Uuid::new_v4();

        tracker.record_node_added(id);
        assert!(tracker.added_nodes.contains(&id));

        tracker.record_node_removed(id);
        assert!(!tracker.added_nodes.contains(&id)); // cancelled out
        assert!(!tracker.removed_nodes.contains(&id)); // never existed
        assert!(!tracker.has_pending_changes); // cancelled out
    }

    #[test]
    fn test_track_modify_existing() {
        let mut tracker = UpdateTracker::new();
        let id = Uuid::new_v4();

        tracker.record_node_modified(id);
        assert!(tracker.modified_nodes.contains(&id));
        assert!(tracker.metadata_changed.contains(&id));
    }

    #[test]
    fn test_track_remove_then_readd() {
        let mut tracker = UpdateTracker::new();
        let id = Uuid::new_v4();

        tracker.record_node_removed(id);
        assert!(tracker.removed_nodes.contains(&id));

        tracker.record_node_added(id);
        assert!(tracker.modified_nodes.contains(&id)); // became modification
        assert!(!tracker.removed_nodes.contains(&id));
    }

    #[test]
    fn test_edge_tracking() {
        let mut tracker = UpdateTracker::new();
        let edge = EdgeKey::new(Uuid::new_v4(), Uuid::new_v4(), "references");

        tracker.record_edge_added(edge.clone());
        assert!(tracker.added_edges.contains(&edge));

        tracker.record_edge_removed(edge.clone());
        assert!(!tracker.added_edges.contains(&edge)); // cancelled
    }

    #[test]
    fn test_affected_nodes() {
        let mut tracker = UpdateTracker::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        tracker.record_node_added(id1);
        tracker.record_node_modified(id2);

        assert_eq!(tracker.affected_nodes().len(), 2);
    }

    #[test]
    fn test_change_count() {
        let mut tracker = UpdateTracker::new();
        tracker.record_node_added(Uuid::new_v4());
        tracker.record_node_modified(Uuid::new_v4());
        tracker.record_node_removed(Uuid::new_v4());
        tracker.record_edge_added(EdgeKey::new(Uuid::new_v4(), Uuid::new_v4(), "references"));

        assert_eq!(tracker.change_count(), 4);
    }

    #[test]
    fn test_summary() {
        let mut tracker = UpdateTracker::new();
        tracker.record_node_added(Uuid::new_v4());
        let summary = tracker.summary();
        assert!(summary.contains("+1 nodes"));
    }
}
