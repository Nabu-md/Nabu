use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Tracks dependencies between graph elements for targeted invalidation.
///
/// When node A has edges to nodes B and C, and node B changes, this tracker
/// ensures that only edges involving B (and their immediate neighbors) are
/// flagged for recalculation — not the entire graph.
///
/// This enables O(changed documents) graph updates instead of O(entire vault).
#[derive(Debug, Clone)]
pub struct DependencyTracker {
    /// For a given node, which other nodes' edges reference it.
    /// node_dependents[A] = {B, C} means edges involving A affect B and C.
    node_dependents: HashMap<Uuid, HashSet<Uuid>>,

    /// For a given node, which regions it belongs to.
    node_regions: HashMap<Uuid, HashSet<String>>,

    /// For a given region, which nodes are in it.
    region_nodes: HashMap<String, HashSet<Uuid>>,

    /// For a given edge type, which nodes use that relationship type.
    relationship_type_users: HashMap<String, HashSet<Uuid>>,

    /// Cache of computed upstream dependencies:
    /// node_dependencies[A] = {B, C} means A depends on B and C.
    node_dependencies: HashMap<Uuid, HashSet<Uuid>>,
}

impl DependencyTracker {
    pub fn new() -> Self {
        Self {
            node_dependents: HashMap::new(),
            node_regions: HashMap::new(),
            region_nodes: HashMap::new(),
            relationship_type_users: HashMap::new(),
            node_dependencies: HashMap::new(),
        }
    }

    /// Register that node A depends on node B (e.g., A has an edge to B).
    pub fn add_dependency(&mut self, from: Uuid, to: Uuid) {
        self.node_dependents
            .entry(to)
            .or_default()
            .insert(from);

        self.node_dependencies
            .entry(from)
            .or_default()
            .insert(to);
    }

    /// Remove a dependency (edge was deleted).
    pub fn remove_dependency(&mut self, from: Uuid, to: Uuid) {
        if let Some(dependents) = self.node_dependents.get_mut(&to) {
            dependents.remove(&from);
        }
        if let Some(deps) = self.node_dependencies.get_mut(&from) {
            deps.remove(&to);
        }
    }

    /// Remove all dependencies for a node (node was deleted).
    pub fn remove_all_dependencies(&mut self, node_id: Uuid) {
        self.node_dependents.remove(&node_id);
        self.node_dependencies.remove(&node_id);

        // Also remove from all other nodes' dependency lists
        for deps in self.node_dependents.values_mut() {
            deps.remove(&node_id);
        }
        for deps in self.node_dependencies.values_mut() {
            deps.remove(&node_id);
        }
    }

    /// Register a node's membership in a region.
    pub fn add_to_region(&mut self, node_id: Uuid, region: impl Into<String>) {
        let region = region.into();
        self.node_regions
            .entry(node_id)
            .or_default()
            .insert(region.clone());
        self.region_nodes
            .entry(region)
            .or_default()
            .insert(node_id);
    }

    /// Remove a node from a region.
    pub fn remove_from_region(&mut self, node_id: Uuid, region: &str) {
        if let Some(regions) = self.node_regions.get_mut(&node_id) {
            regions.remove(region);
        }
        if let Some(nodes) = self.region_nodes.get_mut(region) {
            nodes.remove(&node_id);
        }
    }

    /// Register a node as using a particular relationship type.
    pub fn register_relationship_type(&mut self, node_id: Uuid, relationship: impl Into<String>) {
        self.relationship_type_users
            .entry(relationship.into())
            .or_default()
            .insert(node_id);
    }

    /// Unregister a node from a relationship type.
    pub fn unregister_relationship_type(&mut self, node_id: Uuid, relationship: &str) {
        if let Some(users) = self.relationship_type_users.get_mut(relationship) {
            users.remove(&node_id);
        }
    }

    /// Get all nodes that depend on the given node.
    /// This is used to propagate invalidation: if `node_id` changes,
    /// all nodes in the returned set are also potentially affected.
    pub fn dependents_of(&self, node_id: Uuid) -> HashSet<Uuid> {
        self.node_dependents
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all nodes that the given node depends on.
    /// This is used to determine which other nodes must be recalculated
    /// if this node changes.
    pub fn dependencies_of(&self, node_id: Uuid) -> HashSet<Uuid> {
        self.node_dependencies
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all nodes in the given region.
    pub fn nodes_in_region(&self, region: &str) -> HashSet<Uuid> {
        self.region_nodes
            .get(region)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all regions the given node belongs to.
    pub fn regions_of(&self, node_id: Uuid) -> HashSet<String> {
        self.node_regions
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Compute the transitive closure of affected nodes starting from the given node.
    /// This is the set of all nodes that need recalculation if `root` changes.
    ///
    /// Uses BFS to traverse the dependency graph.
    pub fn transitive_affected(&self, root: Uuid) -> HashSet<Uuid> {
        let mut affected = HashSet::new();
        let mut queue = vec![root];

        while let Some(current) = queue.pop() {
            if !affected.insert(current) {
                continue; // Already visited
            }

            // All dependents of this node are also affected
            for dependent in self.dependents_of(current) {
                queue.push(dependent);
            }

            // All nodes in the same regions are potentially affected
            for region in self.regions_of(current) {
                for node in self.nodes_in_region(&region) {
                    queue.push(node);
                }
            }
        }

        affected
    }

    /// Number of tracked dependency relationships.
    pub fn dependency_count(&self) -> usize {
        self.node_dependents.values().map(|s| s.len()).sum()
    }

    /// Number of tracked regions.
    pub fn region_count(&self) -> usize {
        self.region_nodes.len()
    }

    /// Clear all dependency tracking (for full rebuild).
    pub fn clear(&mut self) {
        self.node_dependents.clear();
        self.node_regions.clear();
        self.region_nodes.clear();
        self.relationship_type_users.clear();
        self.node_dependencies.clear();
    }
}

impl Default for DependencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_dependency() {
        let mut tracker = DependencyTracker::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        tracker.add_dependency(a, b);

        assert!(tracker.dependents_of(b).contains(&a));
        assert!(tracker.dependencies_of(a).contains(&b));
    }

    #[test]
    fn test_transitive_affected() {
        let mut tracker = DependencyTracker::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        tracker.add_dependency(a, b);
        tracker.add_dependency(b, c);

        let affected = tracker.transitive_affected(c);
        assert!(affected.contains(&b));
        assert!(affected.contains(&a));
    }

    #[test]
    fn test_region_membership() {
        let mut tracker = DependencyTracker::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        tracker.add_to_region(a, "inbox");
        tracker.add_to_region(b, "inbox");

        assert!(tracker.nodes_in_region("inbox").contains(&a));
        assert!(tracker.nodes_in_region("inbox").contains(&b));
        assert_eq!(tracker.regions_of(a).len(), 1);
    }

    #[test]
    fn test_remove_dependency() {
        let mut tracker = DependencyTracker::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        tracker.add_dependency(a, b);
        assert_eq!(tracker.dependency_count(), 1);

        tracker.remove_dependency(a, b);
        assert_eq!(tracker.dependency_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut tracker = DependencyTracker::new();
        tracker.add_dependency(Uuid::new_v4(), Uuid::new_v4());
        tracker.add_to_region(Uuid::new_v4(), "test");

        tracker.clear();
        assert_eq!(tracker.dependency_count(), 0);
        assert_eq!(tracker.region_count(), 0);
    }
}
