

use crate::graph::serializer::{GraphSnapshot, SerializedEdge, SerializedNode};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// A graph region is a subset of the graph that can be independently invalidated
/// and rebuilt without touching unrelated portions.
#[derive(Debug, Clone)]
pub struct GraphRegion {
    /// Unique region identifier
    pub id: String,

    /// Nodes in this region
    pub nodes: HashSet<Uuid>,

    /// Edges fully contained within this region
    pub edges: HashSet<(Uuid, Uuid, String)>,

    /// Nodes on the boundary of this region (connected to nodes outside)
    pub boundary_nodes: HashSet<Uuid>,

    /// Region-level metadata
    pub metadata: HashMap<String, String>,
}

impl GraphRegion {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            nodes: HashSet::new(),
            edges: HashSet::new(),
            boundary_nodes: HashSet::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node_id: Uuid) {
        self.nodes.insert(node_id);
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid, relationship: impl Into<String>) {
        self.edges.insert((source, target, relationship.into()));
    }

    pub fn mark_boundary(&mut self, node_id: Uuid) {
        self.boundary_nodes.insert(node_id);
    }
}

/// Identifies and manages graph regions for targeted incremental rebuilds.
///
/// Regions can be:
/// - Folder-based: all notes in the same vault folder
/// - Tag-based: all notes sharing a common tag
/// - Type-based: all notes of the same object type
/// - Manual: user-defined collections
/// - Proximity-based: nodes within N hops of a changed node
pub struct RegionEngine {
    regions: HashMap<String, GraphRegion>,
    node_to_region: HashMap<Uuid, Vec<String>>,
}

impl RegionEngine {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
            node_to_region: HashMap::new(),
        }
    }

    /// Register a region.
    pub fn register_region(&mut self, region: GraphRegion) {
        for node_id in &region.nodes {
            self.node_to_region
                .entry(*node_id)
                .or_default()
                .push(region.id.clone());
        }
        self.regions.insert(region.id.clone(), region);
    }

    /// Remove a region.
    pub fn remove_region(&mut self, region_id: &str) {
        if let Some(region) = self.regions.remove(region_id) {
            for node_id in region.nodes {
                if let Some(regions) = self.node_to_region.get_mut(&node_id) {
                    regions.retain(|r| r != region_id);
                }
            }
        }
    }

    /// Get all regions for a given node.
    pub fn regions_for_node(&self, node_id: Uuid) -> Vec<&GraphRegion> {
        self.node_to_region
            .get(&node_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.regions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all regions affected by a set of changed nodes.
    /// Returns the union of regions containing any changed node.
    pub fn affected_regions<'a>(
        &'a self,
        changed_nodes: &HashSet<Uuid>,
    ) -> Vec<&'a GraphRegion> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for node_id in changed_nodes {
            if let Some(region_ids) = self.node_to_region.get(node_id) {
                for region_id in region_ids {
                    if seen.insert(region_id.clone()) {
                        if let Some(region) = self.regions.get(region_id) {
                            result.push(region);
                        }
                    }
                }
            }
        }

        result
    }

    /// Determine the proximity region around a changed node (within N hops).
    pub fn proximity_region(
        &self,
        root: Uuid,
        snapshot: &GraphSnapshot,
        max_hops: usize,
    ) -> GraphRegion {
        let mut region = GraphRegion::new(format!("proximity:{}", root));
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((root, 0usize));

        // Build adjacency from snapshot
        let adjacency = build_adjacency(snapshot);

        while let Some((current, depth)) = queue.pop_front() {
            if depth > max_hops || !visited.insert(current) {
                continue;
            }

            region.add_node(current);

            // Add neighbors
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    region.add_edge(current, *neighbor, "related");
                    if depth + 1 <= max_hops {
                        queue.push_back((*neighbor, depth + 1));
                    } else {
                        region.mark_boundary(*neighbor);
                    }
                }
            }
        }

        region
    }

    /// Identify regions from the snapshot's graph structure.
    /// This auto-discovers regions based on folder structure (from node titles).
    pub fn discover_regions(&mut self, snapshot: &GraphSnapshot) {
        // Folder-based regions: group by first path segment of title
        let mut folder_nodes: HashMap<String, Vec<Uuid>> = HashMap::new();

        for node in &snapshot.nodes {
            if let Some(ref title) = node.title {
                if title.contains('/') {
                    let folder = title.split('/').next().unwrap_or("Inbox").to_string();
                    folder_nodes.entry(folder).or_default().push(node.id);
                } else {
                    folder_nodes.entry("Inbox".to_string()).or_default().push(node.id);
                }
            } else {
                folder_nodes.entry("Inbox".to_string()).or_default().push(node.id);
            }
        }

        // Type-based regions
        let mut type_nodes: HashMap<String, Vec<Uuid>> = HashMap::new();
        for node in &snapshot.nodes {
            type_nodes
                .entry(node.object_type.clone())
                .or_default()
                .push(node.id);
        }

        // Register folder regions
        for (folder, node_ids) in &folder_nodes {
            let mut region = GraphRegion::new(format!("folder:{}", folder));
            for node_id in node_ids {
                region.add_node(*node_id);
            }
            self.register_region(region);
        }

        // Register type-based regions
        for (obj_type, node_ids) in &type_nodes {
            let mut region = GraphRegion::new(format!("type:{}", obj_type));
            for node_id in node_ids {
                region.add_node(*node_id);
            }
            self.register_region(region);
        }
    }

    /// Rebuild only the nodes and edges in the affected regions.
    ///
    /// Returns the set of rebuilt node snapshots.
    pub fn rebuild_regions(
        &self,
        affected_regions: &[&GraphRegion],
        current_nodes: &HashMap<Uuid, SerializedNode>,
        _current_edges: &[SerializedEdge],
    ) -> (Vec<SerializedNode>, Vec<SerializedEdge>) {
        let mut rebuilt_nodes = Vec::new();
        let mut rebuilt_edges_set = HashSet::new();

        let mut seen_nodes = HashSet::new();

        for region in affected_regions {
            for node_id in &region.nodes {
                if seen_nodes.insert(*node_id) {
                    if let Some(node) = current_nodes.get(node_id) {
                        rebuilt_nodes.push(node.clone());
                    }
                }
            }

            for edge in &region.edges {
                rebuilt_edges_set.insert(edge.clone());
            }
        }

        let rebuilt_edges: Vec<SerializedEdge> = rebuilt_edges_set
            .into_iter()
            .map(|(source, target, relationship)| {
                SerializedEdge::new(source, target, relationship)
            })
            .collect();

        (rebuilt_nodes, rebuilt_edges)
    }

    /// Number of registered regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// List all region IDs.
    pub fn region_ids(&self) -> Vec<String> {
        self.regions.keys().cloned().collect()
    }
}

impl Default for RegionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a simple adjacency map from a graph snapshot.
fn build_adjacency(snapshot: &GraphSnapshot) -> HashMap<Uuid, HashSet<Uuid>> {
    let mut adjacency: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();

    for edge in &snapshot.edges {
        adjacency.entry(edge.source).or_default().insert(edge.target);
        adjacency.entry(edge.target).or_default().insert(edge.source);
    }

    adjacency
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::version::GraphVersion;

    fn create_test_snapshot() -> GraphSnapshot {
        let mut snapshot = GraphSnapshot::new(GraphVersion::new());
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let n3 = Uuid::new_v4();

        snapshot.add_node(SerializedNode::new(n1, "note", Some("Inbox/Note A".into()), "text"));
        snapshot.add_node(SerializedNode::new(n2, "note", Some("Work/Note B".into()), "text"));
        snapshot.add_node(SerializedNode::new(n3, "article", Some("Inbox/Article C".into()), "text"));

        snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));
        snapshot.add_edge(SerializedEdge::new(n2, n3, "related"));

        snapshot
    }

    #[test]
    fn test_discover_regions() {
        let snapshot = create_test_snapshot();
        let mut engine = RegionEngine::new();
        engine.discover_regions(&snapshot);

        // Should have discovered folder and type-based regions
        assert!(engine.region_count() >= 4); // Inbox, Work, note, article
    }

    #[test]
    fn test_proximity_region() {
        let snapshot = create_test_snapshot();
        let engine = RegionEngine::new();

        let root = snapshot.nodes[0].id;
        let region = engine.proximity_region(root, &snapshot, 1);

        assert!(region.nodes.contains(&root));
        assert!(region.nodes.len() >= 1);
    }

    #[test]
    fn test_affected_regions() {
        let snapshot = create_test_snapshot();
        let mut engine = RegionEngine::new();
        engine.discover_regions(&snapshot);

        let changed = {
            let mut s = HashSet::new();
            s.insert(snapshot.nodes[0].id);
            s
        };

        let affected = engine.affected_regions(&changed);
        assert!(!affected.is_empty());
    }

    #[test]
    fn test_region_rebuild() {
        let snapshot = create_test_snapshot();
        let mut engine = RegionEngine::new();
        engine.discover_regions(&snapshot);

        let affected = engine.affected_regions(&snapshot.nodes.iter().map(|n| n.id).collect());
        let current_nodes: HashMap<Uuid, SerializedNode> = snapshot
            .nodes
            .iter()
            .map(|n| (n.id, n.clone()))
            .collect();

        let (rebuilt_nodes, rebuilt_edges) =
            engine.rebuild_regions(&affected, &current_nodes, &snapshot.edges);

        assert!(!rebuilt_nodes.is_empty());
    }
}
