use petgraph::graph::{Graph, NodeIndex};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::HashMap;

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

    pub fn add_note(&mut self, note_path: String, content: &str) {
        let node_index = *self.node_map.entry(note_path.clone()).or_insert_with(|| {
            self.graph.add_node(note_path)
        });

        let parser = Parser::new(content);
        for event in parser {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                let target = dest_url.to_string();
                let target_node_index = *self.node_map.entry(target.clone()).or_insert_with(|| {
                    self.graph.add_node(target)
                });
                self.graph.add_edge(node_index, target_node_index, "links_to".to_string());
            }
        }
    }
}
