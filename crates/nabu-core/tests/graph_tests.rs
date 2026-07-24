use nabu_core::graph::VaultGraph;

#[test]
fn test_graph_creation() {
    let graph = VaultGraph::new();
    assert_eq!(graph.graph.node_count(), 0);
}
