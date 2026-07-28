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
}
