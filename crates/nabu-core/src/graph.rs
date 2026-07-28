use crate::models::graph::RelationType;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphNode {
    File(NodeMetadata),
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
pub struct NodeMetadata {
    pub path: String,
    pub is_folder: bool,
    pub parent_folder: Option<String>,
}

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

    pub fn add_folder(&mut self, folder_path: String, parent_folder: Option<String>) {
        let metadata = NodeMetadata {
            path: folder_path.clone(),
            is_folder: true,
            parent_folder: parent_folder.clone(),
        };
        let node_index = *self
            .node_map
            .entry(folder_path.clone())
            .or_insert_with(|| self.graph.add_node(GraphNode::File(metadata)));

        if let Some(parent) = parent_folder {
            if let Some(&parent_index) = self.node_map.get(&parent) {
                self.graph.add_edge(parent_index, node_index, GraphEdgeType::WikiLink);
            }
        }
    }

    /// Extracts `[[wiki-links]]` from markdown content and updates graph.
    pub fn add_note(&mut self, note_path: String, content: &str) {
        let metadata = NodeMetadata {
            path: note_path.clone(),
            is_folder: false,
            parent_folder: std::path::Path::new(&note_path)
                .parent()
                .map(|p| p.to_string_lossy().into()),
        };
        let node_index = *self
            .node_map
            .entry(note_path.clone())
            .or_insert_with(|| self.graph.add_node(GraphNode::File(metadata)));

        let re = Regex::new(r"\[\[(.*?)\]\]").unwrap();

        for cap in re.captures_iter(content) {
            let target = cap[1].to_string();
            let target_metadata = NodeMetadata {
                path: target.clone(),
                is_folder: false,
                parent_folder: None,
            };
            let target_node_index = *self
                .node_map
                .entry(target.clone())
                .or_insert_with(|| self.graph.add_node(GraphNode::File(target_metadata)));
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
                if let GraphNode::File(metadata) = &self.graph[source] {
                    Some(metadata.path.clone())
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
                if let GraphNode::File(metadata) = &self.graph[idx] {
                    let content = std::fs::read_to_string(&metadata.path).unwrap_or_default();
                    crate::parser::extract_tags(&content).contains(&tag.to_string())
                } else {
                    false
                }
            })
            .filter_map(|idx| {
                if let GraphNode::File(metadata) = &self.graph[idx] {
                    Some(metadata.path.clone())
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
}
