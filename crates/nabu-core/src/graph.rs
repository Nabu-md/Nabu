use petgraph::visit::EdgeRef;
use petgraph::graph::{Graph, NodeIndex};
use std::collections::HashMap;
use regex::Regex;

pub struct VaultGraph {
    pub graph: Graph<String, String>,
    node_map: HashMap<String, NodeIndex>,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Extracts `[[wiki-links]]` from markdown content and updates graph.
    pub fn add_note(&mut self, note_path: String, content: &str) {
        let node_index = *self.node_map.entry(note_path.clone()).or_insert_with(|| {
            self.graph.add_node(note_path.clone())
        });

        // Simple regex to find [[wiki-links]]
        let re = Regex::new(r"\[\[(.*?)\]\]").unwrap();
        
        for cap in re.captures_iter(content) {
            let target = cap[1].to_string();
            let target_node_index = *self.node_map.entry(target.clone()).or_insert_with(|| {
                self.graph.add_node(target)
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
            .map(|edge| self.graph[edge.source()].clone())
            .collect()
    }
    pub fn filter_by_tag(&self, tag: &str) -> Vec<String> {
        // In a real impl, look up notes containing this tag
        // and return the subgraph of these notes
        vec![]
    }
}
