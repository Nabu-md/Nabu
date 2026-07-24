use petgraph::graph::{Graph, NodeIndex};

pub struct VaultGraph {
    pub graph: Graph<String, String>,
}

impl VaultGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
        }
    }
}
