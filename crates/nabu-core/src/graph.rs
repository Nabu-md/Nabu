use crate::models::knowledge_object::KnowledgeObject;

use crate::models::graph::RelationType;
use uuid::Uuid;

pub enum GraphNode {
    Object(KnowledgeObject),
    Entity(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphEdgeType {
    WikiLink,
    Semantic(RelationType),
}

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use regex::Regex;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct VaultGraph {
    pub graph: Graph<GraphNode, GraphEdgeType>,
    node_map: HashMap<String, NodeIndex>,
    entity_map: HashMap<Uuid, NodeIndex>,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            node_map: HashMap::new(),
            entity_map: HashMap::new(),
        }
    }

    pub fn add_folder(&mut self, folder_object: KnowledgeObject) {
        let path = folder_object.metadata.source_file.clone().unwrap_or_default();
        let node_index = *self
            .node_map
            .entry(path.clone())
            .or_insert_with(|| self.graph.add_node(GraphNode::Object(folder_object)));
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
            // Note: In a real system, we'd look up the KnowledgeObject for the target.
            // For now, this is a placeholder to get it to compile.
            let target_node_index = *self
                .node_map
                .entry(target.clone())
                .or_insert_with(|| {
                    self.graph.add_node(GraphNode::Object(KnowledgeObject::default()))
                });
            self.graph
                .add_edge(node_index, target_node_index, GraphEdgeType::WikiLink);
        }
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

    pub fn filter_by_tag(&self, tag: &str) -> Vec<String> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                if let GraphNode::Object(obj) = &self.graph[idx] {
                    // This is inefficient but necessary for now
                    let content = std::fs::read_to_string(obj.metadata.source_file.as_ref().unwrap_or(&"".to_string())).unwrap_or_default();
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
    }

    pub fn add_semantic_relation(&mut self, source: Uuid, target: Uuid, relation: RelationType) {
        if let (Some(&s_idx), Some(&t_idx)) = (self.entity_map.get(&source), self.entity_map.get(&target)) {
            self.graph.add_edge(s_idx, t_idx, GraphEdgeType::Semantic(relation));
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
    }

    /// Remove a node from the graph by its source file path.
    /// Also removes all edges connected to this node.
    pub fn remove_node(&mut self, path: &str) {
        if let Some(&node_idx) = self.node_map.get(path) {
            // Remove all edges connected to this node
            let edges: Vec<_> = self.graph.edges(node_idx).map(|e| e.id()).collect();
            for edge_id in edges {
                self.graph.remove_edge(edge_id);
            }
            self.graph.remove_node(node_idx);
            self.node_map.remove(path);
        }
    }

    /// Update a semantic relation between two entities.
    /// If the relation already exists, it is replaced.
    /// If either entity doesn't exist, it is created.
    pub fn update_semantic_relation(&mut self, source: Uuid, target: Uuid, relation: RelationType) {
        // Ensure both entities exist
        let s_idx = *self.entity_map.entry(source).or_insert_with(|| {
            self.graph.add_node(GraphNode::Entity(source))
        });
        let t_idx = *self.entity_map.entry(target).or_insert_with(|| {
            self.graph.add_node(GraphNode::Entity(target))
        });

        // Remove existing edge between these entities (if any) to avoid duplicates
        let edges: Vec<_> = self.graph.edges_connecting(s_idx, t_idx).map(|e| e.id()).collect();
        for edge_id in edges {
            self.graph.remove_edge(edge_id);
        }

        // Add the new relation
        self.graph.add_edge(s_idx, t_idx, GraphEdgeType::Semantic(relation));
    }

    /// Remove a semantic relation between two entities.
    pub fn remove_semantic_relation(&mut self, source: Uuid, target: Uuid) {
        if let (Some(&s_idx), Some(&t_idx)) = (self.entity_map.get(&source), self.entity_map.get(&target)) {
            let edges: Vec<_> = self.graph.edges_connecting(s_idx, t_idx).map(|e| e.id()).collect();
            for edge_id in edges {
                self.graph.remove_edge(edge_id);
            }
        }
    }

    /// Get the number of nodes in the graph (useful for monitoring and debugging).
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get the number of edges in the graph (useful for monitoring and debugging).
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}
