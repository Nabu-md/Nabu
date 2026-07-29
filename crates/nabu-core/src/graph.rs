use crate::event_bus::{EventBus, GraphOperation, GraphUpdatedEvent, PipelineEvent};
use crate::event_bus::kinds::GRAPH_UPDATED;
use crate::models::KnowledgeObject;
use std::collections::{HashMap, HashSet};
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
/// No duplicate graph systems exist.
/// All graph operations go through VaultGraph.
///
/// In production, this would use petgraph for efficient graph algorithms.
/// Currently uses an adjacency list as a stub.
pub struct VaultGraph {
    nodes: RwLock<HashMap<Uuid, KnowledgeObject>>,
    edges: RwLock<Vec<GraphEdge>>,
    adjacency: RwLock<HashMap<Uuid, HashSet<Uuid>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adjacency: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&self, object: &KnowledgeObject) -> Result<(), String> {
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
        nodes.insert(object.id, object.clone());

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

        Ok(())
    }

    /// Remove a node from the graph.
    pub fn remove_node(&self, object_id: Uuid) -> Result<(), String> {
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
        nodes.remove(&object_id);

        // Remove associated edges
        let mut edges = self.edges.write().map_err(|e| e.to_string())?;
        edges.retain(|e| e.source != object_id && e.target != object_id);

        let mut adj = self.adjacency.write().map_err(|e| e.to_string())?;
        adj.remove(&object_id);
        for neighbors in adj.values_mut() {
            neighbors.remove(&object_id);
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

        let mut edges = self.edges.write().map_err(|e| e.to_string())?;
        edges.push(edge);

        let mut adj = self.adjacency.write().map_err(|e| e.to_string())?;
        adj.entry(source).or_default().insert(target);
        adj.entry(target).or_default().insert(source);

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

        Ok(())
    }

    /// Get connected nodes (neighbors) of a given node.
    pub fn neighbors(&self, object_id: Uuid) -> Vec<Uuid> {
        let adj = self.adjacency.read().ok();
        match adj {
            Some(adj) => adj.get(&object_id).cloned().map(|s| s.into_iter().collect()).unwrap_or_default(),
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

    /// Clear the entire graph (for rebuild).
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
        self.nodes.read().map(|n| n.values().cloned().collect()).unwrap_or_default()
    }
}

impl Default for VaultGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ObjectContent;

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
}
