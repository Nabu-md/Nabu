use petgraph::visit::EdgeRef;
use petgraph::graph::{Graph, NodeIndex};
use std::collections::HashMap;
use regex::Regex;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub path: String,
    pub is_folder: bool,
    pub parent_folder: Option<String>,
}

pub struct VaultGraph {
    pub graph: Graph<NodeMetadata, String>,
    node_map: HashMap<String, NodeIndex>,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            node_map: HashMap::new(),
        }
    }

    pub fn add_folder(&mut self, folder_path: String, parent_folder: Option<String>) {
        let metadata = NodeMetadata {
            path: folder_path.clone(),
            is_folder: true,
            parent_folder: parent_folder.clone(),
        };
        let node_index = *self.node_map.entry(folder_path.clone()).or_insert_with(|| {
            self.graph.add_node(metadata)
        });

        if let Some(parent) = parent_folder {
            if let Some(parent_index) = self.node_map.get(&parent) {
                self.graph.add_edge(*parent_index, node_index, "contains".to_string());
            }
        }
    }

    /// Extracts `[[wiki-links]]` from markdown content and updates graph.
    pub fn add_note(&mut self, note_path: String, content: &str) {
        let metadata = NodeMetadata {
            path: note_path.clone(),
            is_folder: false,
            parent_folder: std::path::Path::new(&note_path).parent().map(|p| p.to_string_lossy().into()),
        };
        let node_index = *self.node_map.entry(note_path.clone()).or_insert_with(|| {
            self.graph.add_node(metadata)
        });

        // Simple regex to find [[wiki-links]]
        let re = Regex::new(r"\[\[(.*?)\]\]").unwrap();
        
        for cap in re.captures_iter(content) {
            let target = cap[1].to_string();
            let target_metadata = NodeMetadata {
                path: target.clone(),
                is_folder: false,
                parent_folder: None,
            };
            let target_node_index = *self.node_map.entry(target.clone()).or_insert_with(|| {
                self.graph.add_node(target_metadata)
            });
            self.graph.add_edge(node_index, target_node_index, "links_to".to_string());
        }
    }
    pub fn get_backlinks(&self, note_path: &str) -> Vec<String> {
        let node_index = match self.node_map.get(note_path) {
            Some(idx) => *idx,
            None => return vec![],
        };

        self.graph
            .edges_directed(node_index, petgraph::Direction::Incoming)
            .map(|edge| self.graph[edge.source()].path.clone())
            .collect()
    }

    pub fn filter_by_tag(&self, tag: &str) -> Vec<String> {
        self.graph.node_indices()
            .filter(|&idx| {
                let metadata = self.graph[idx].clone();
                let content = std::fs::read_to_string(&metadata.path).unwrap_or_default();
                crate::parser::extract_tags(&content).contains(&tag.to_string())
            })
            .map(|idx| self.graph[idx].path.clone())
            .collect()
    }
}
