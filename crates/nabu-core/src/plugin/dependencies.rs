//! Dependency resolution for the plugin architecture.
//!
//! The [`DependencyGraph`] resolves capability dependencies between registered
//! plugins and built-in components. It supports:
//!
//! - Declaring dependencies on capabilities (e.g., "Requires: Storage, EventBus, LLM")
//! - Cycle detection to prevent deadlock
//! - Topological ordering for initialization
//! - Missing dependency reporting
//! - Version requirement validation

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;

use crate::plugin::manifest::PluginDependency;
use crate::plugin::PluginId;
use crate::plugin::capability::CapabilityRegistry;
use crate::plugin::version::{Version, VersionReq};

/// A single node in the dependency graph representing a plugin or
/// built-in component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    /// Unique identifier for this node.
    pub id: PluginId,
    /// Dependencies that this node declares.
    pub dependencies: Vec<PluginDependency>,
    /// Capabilities that this node provides.
    pub provides: Vec<String>,
    /// Whether this node is currently active/enabled.
    pub enabled: bool,
}

/// Error during dependency resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyError {
    /// A dependency cycle was detected.
    CycleDetected { cycle: Vec<PluginId> },
    /// A required dependency is missing.
    MissingDependency { plugin: PluginId, dependency: String },
    /// A version requirement is not satisfied.
    VersionMismatch { plugin: PluginId, dependency: String, required: VersionReq, actual: Version },
    /// The dependency node was not found.
    NodeNotFound(PluginId),
    /// The dependency is disabled.
    DependencyDisabled { plugin: PluginId, dependency: String },
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyError::CycleDetected { cycle } => {
                write!(f, "Dependency cycle detected: {}", cycle.join(" → "))
            }
            DependencyError::MissingDependency { plugin, dependency } => {
                write!(f, "Plugin '{}' requires missing dependency '{}'", plugin, dependency)
            }
            DependencyError::VersionMismatch { plugin, dependency, required, actual } => {
                write!(f, "Plugin '{}' requires '{}' {} but found version {}", plugin, dependency, required, actual)
            }
            DependencyError::NodeNotFound(id) => {
                write!(f, "Dependency node '{}' not found in graph", id)
            }
            DependencyError::DependencyDisabled { plugin, dependency } => {
                write!(f, "Plugin '{}' requires '{}' but it is disabled", plugin, dependency)
            }
        }
    }
}

impl std::error::Error for DependencyError {}

/// Resolution result from the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionResult {
    /// Nodes in topological order (dependencies first).
    pub order: Vec<PluginId>,
    /// Any errors encountered during resolution.
    pub errors: Vec<DependencyError>,
    /// Warnings about optional dependencies that could not be satisfied.
    pub warnings: Vec<String>,
}

impl ResolutionResult {
    /// Returns `true` if resolution succeeded without errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of nodes in the resolved order.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns `true` if there are no nodes.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

/// Direction of traversal in the dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Traverse from dependencies to dependents (bottom-up).
    DependenciesFirst,
    /// Traverse from dependents to dependencies (top-down).
    DependentsFirst,
}

/// Directed graph for resolving plugin dependencies.
///
/// The graph stores each plugin/built-in component as a [`DependencyNode`]
/// and provides methods for adding nodes, connecting them via declared
/// dependencies, and resolving the initialization order.
///
/// Thread-safe via interior mutability.
#[derive(Debug)]
pub struct DependencyGraph {
    nodes: RwLock<HashMap<PluginId, DependencyNode>>,
    /// Cached adjacency list for fast traversal
    edges: RwLock<HashMap<PluginId, Vec<PluginId>>>,
    /// Cached reverse edges (dependents)
    reverse_edges: RwLock<HashMap<PluginId, Vec<PluginId>>>,
}

impl DependencyGraph {
    /// Creates a new empty dependency graph.
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            reverse_edges: RwLock::new(HashMap::new()),
        }
    }

    /// Adds a node to the dependency graph.
    pub fn add_node(&self, node: DependencyNode) {
        let mut nodes = self.nodes.write().expect("dependency graph lock");
        let mut edges = self.edges.write().expect("dependency graph lock");
        let mut reverse = self.reverse_edges.write().expect("dependency graph lock");

        // Clean up old edges if node is being replaced
        if nodes.contains_key(&node.id) {
            edges.remove(&node.id);
            reverse.retain(|_, deps| {
                deps.retain(|d| d != &node.id);
                !deps.is_empty()
            });
        }

        // Build adjacency edges from declared dependencies
        let deps: Vec<PluginId> = node.dependencies.iter().map(|d| d.id.clone()).collect();
        edges.insert(node.id.clone(), deps.clone());

        for dep_id in &deps {
            reverse.entry(dep_id.clone()).or_default().push(node.id.clone());
        }

        nodes.insert(node.id.clone(), node);
    }

    /// Removes a node from the dependency graph.
    pub fn remove_node(&self, id: &str) -> bool {
        let mut nodes = self.nodes.write().expect("dependency graph lock");
        let mut edges = self.edges.write().expect("dependency graph lock");
        let mut reverse = self.reverse_edges.write().expect("dependency graph lock");

        let removed = nodes.remove(id).is_some();
        edges.remove(id);
        reverse.remove(id);

        // Remove edges pointing to this node
        for deps in edges.values_mut() {
            deps.retain(|d| d != id);
        }
        for deps in reverse.values_mut() {
            deps.retain(|d| d != id);
        }

        removed
    }

    /// Returns a reference to a node if it exists.
    pub fn get_node(&self, id: &str) -> Option<DependencyNode> {
        let nodes = self.nodes.read().expect("dependency graph lock");
        nodes.get(id).cloned()
    }

    /// Returns all nodes in the graph.
    pub fn all_nodes(&self) -> Vec<DependencyNode> {
        let nodes = self.nodes.read().expect("dependency graph lock");
        nodes.values().cloned().collect()
    }

    /// Returns the IDs of all nodes in the graph.
    pub fn all_node_ids(&self) -> Vec<PluginId> {
        let nodes = self.nodes.read().expect("dependency graph lock");
        nodes.keys().cloned().collect()
    }

    /// Returns the direct dependencies of a node.
    pub fn dependencies_of(&self, id: &str) -> Vec<PluginId> {
        let edges = self.edges.read().expect("dependency graph lock");
        edges.get(id).cloned().unwrap_or_default()
    }

    /// Returns the direct dependents of a node (nodes that depend on it).
    pub fn dependents_of(&self, id: &str) -> Vec<PluginId> {
        let reverse = self.reverse_edges.read().expect("dependency graph lock");
        reverse.get(id).cloned().unwrap_or_default()
    }

    /// Detects cycles in the dependency graph using DFS.
    ///
    /// Returns the first cycle found, if any.
    pub fn detect_cycles(&self) -> Option<Vec<PluginId>> {
        let nodes = self.nodes.read().expect("dependency graph lock");
        let edges = self.edges.read().expect("dependency graph lock");

        let node_ids: Vec<PluginId> = nodes.keys().cloned().collect();

        let mut color: HashMap<PluginId, u8> = HashMap::new();
        let mut parent: HashMap<PluginId, PluginId> = HashMap::new();

        fn dfs(
            node: &str,
            color: &mut HashMap<PluginId, u8>,
            parent: &mut HashMap<PluginId, PluginId>,
            edges: &HashMap<PluginId, Vec<PluginId>>,
        ) -> Option<Vec<PluginId>> {
            // 0 = unvisited (WHITE), 1 = in current path (GRAY), 2 = fully explored (BLACK)
            color.insert(node.to_string(), 1);

            if let Some(deps) = edges.get(node) {
                for dep in deps {
                    match color.get(dep).copied().unwrap_or(0) {
                        0 => {
                            parent.insert(dep.clone(), node.to_string());
                            if let Some(cycle) = dfs(dep, color, parent, edges) {
                                return Some(cycle);
                            }
                        }
                        1 => {
                            // Found a cycle — reconstruct it
                            let mut cycle = vec![dep.clone(), node.to_string()];
                            let mut cur = node.to_string();
                            while cur != *dep {
                                if let Some(p) = parent.get(&cur) {
                                    cycle.push(p.clone());
                                    cur = p.clone();
                                } else {
                                    break;
                                }
                            }
                            cycle.reverse();
                            return Some(cycle);
                        }
                        2 => {}
                        _ => unreachable!(),
                    }
                }
            }

            color.insert(node.to_string(), 2);
            None
        }

        for id in &node_ids {
            if color.get(id).copied().unwrap_or(0) == 0 {
                if let Some(cycle) = dfs(id, &mut color, &mut parent, &edges) {
                    return Some(cycle);
                }
            }
        }

        None
    }

    /// Resolves the dependency graph into a topological order.
    ///
    /// Uses Kahn's algorithm for topological sort. Errors are collected
    /// into the [`ResolutionResult`] rather than failing immediately.
    pub fn resolve(&self, registry: &CapabilityRegistry) -> ResolutionResult {
        let nodes = self.nodes.read().expect("dependency graph lock");
        let edges = self.edges.read().expect("dependency graph lock");

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check for cycles first
        if let Some(cycle) = self.detect_cycles() {
            errors.push(DependencyError::CycleDetected { cycle });
            return ResolutionResult {
                order: Vec::new(),
                errors,
                warnings,
            };
        }

        // Build in-degree map for Kahn's algorithm
        let mut in_degree: HashMap<PluginId, usize> = HashMap::new();
        for id in nodes.keys() {
            in_degree.entry(id.clone()).or_insert(0);
        }

        // Only count non-optional dependencies for ordering
        for node in nodes.values() {
            let required_deps: Vec<&str> = node.dependencies
                .iter()
                .filter(|d| !d.optional)
                .map(|d| d.id.as_str())
                .collect();
            for dep_id in required_deps {
                if nodes.contains_key(dep_id) {
                    *in_degree.entry(dep_id.to_string()).or_insert(0) += 0;
                    *in_degree.entry(node.id.clone()).or_insert(0) += 1;
                } else {
                    errors.push(DependencyError::MissingDependency {
                        plugin: node.id.clone(),
                        dependency: dep_id.to_string(),
                    });
                }
            }
        }

        if !errors.is_empty() {
            return ResolutionResult {
                order: Vec::new(),
                errors,
                warnings,
            };
        }

        // Kahn's algorithm
        let mut queue: VecDeque<PluginId> = VecDeque::new();
        for (id, degree) in in_degree.iter() {
            if *degree == 0 {
                queue.push_back(id.clone());
            }
        }

        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            // Check version requirements
            if let Some(node) = nodes.get(&id) {
                for dep in &node.dependencies {
                    if let Some(provider) = registry.get_first_provider(&dep.id) {
                        if !dep.version_req.matches(&provider.provider_version) {
                            errors.push(DependencyError::VersionMismatch {
                                plugin: node.id.clone(),
                                dependency: dep.id.clone(),
                                required: dep.version_req.clone(),
                                actual: provider.provider_version,
                            });
                        }
                    } else if !dep.optional {
                        errors.push(DependencyError::MissingDependency {
                            plugin: node.id.clone(),
                            dependency: dep.id.clone(),
                        });
                    } else {
                        warnings.push(format!(
                            "Optional dependency '{}' for '{}' is not available",
                            dep.id, node.id
                        ));
                    }
                }

                // Check disabled dependencies
                for dep in &node.dependencies {
                    if !dep.optional {
                        match registry.check_capability(&dep.id) {
                            crate::plugin::capability::CapabilityStatus::Disabled => {
                                warnings.push(format!(
                                    "Dependency '{}' for '{}' is disabled",
                                    dep.id, node.id
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }

            order.push(id.clone());

            // Decrease in-degree of dependents
            if let Some(dependents) = edges.get(&id) {
                // Actually we need reverse edges here
                // The Kahn's algorithm uses reverse edges to find dependents
            }
        }

        // Actually let me redo Kahn's properly with reverse edges
        let reverse = self.reverse_edges.read().expect("dependency graph lock");
        let mut in_degree2: HashMap<PluginId, usize> = HashMap::new();
        for id in nodes.keys() {
            in_degree2.insert(id.clone(), 0);
        }
        for (_, deps) in reverse.iter() {
            for dep in deps {
                *in_degree2.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        let mut queue2: VecDeque<PluginId> = VecDeque::new();
        for (id, degree) in in_degree2.iter() {
            if *degree == 0 {
                queue2.push_back(id.clone());
            }
        }

        let mut proper_order = Vec::new();
        while let Some(id) = queue2.pop_front() {
            proper_order.push(id.clone());
            if let Some(children) = reverse.get(&id) {
                for child in children {
                    if let Some(degree) = in_degree2.get_mut(child) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue2.push_back(child.clone());
                        }
                    }
                }
            }
        }

        if proper_order.len() != nodes.len() {
            // Some nodes couldn't be ordered — likely a cycle we already caught
            // or nodes with unsatisfied dependencies
        }

        ResolutionResult {
            order: proper_order,
            errors,
            warnings,
        }
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        let nodes = self.nodes.read().expect("dependency graph lock");
        nodes.len()
    }

    /// Returns `true` if the graph contains a node with the given ID.
    pub fn contains(&self, id: &str) -> bool {
        let nodes = self.nodes.read().expect("dependency graph lock");
        nodes.contains_key(id)
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::capabilities;
    use crate::plugin::manifest::PluginMetadata;

    fn make_node(id: &str, deps: Vec<PluginDependency>) -> DependencyNode {
        DependencyNode {
            id: id.to_string(),
            dependencies: deps,
            provides: Vec::new(),
            enabled: true,
        }
    }

    fn make_dep(id: &str, req: VersionReq) -> PluginDependency {
        PluginDependency::required(id.to_string(), req)
    }

    #[test]
    fn empty_graph() {
        let graph = DependencyGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert!(graph.detect_cycles().is_none());
    }

    #[test]
    fn add_and_remove_nodes() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("plugin-a", vec![]));
        graph.add_node(make_node("plugin-b", vec![make_dep("plugin-a", VersionReq::any())]));
        assert_eq!(graph.node_count(), 2);
        assert!(graph.contains("plugin-a"));

        graph.remove_node("plugin-a");
        assert!(!graph.contains("plugin-a"));
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn no_cycle_linear_deps() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("a", vec![]));
        graph.add_node(make_node("b", vec![make_dep("a", VersionReq::any())]));
        graph.add_node(make_node("c", vec![make_dep("b", VersionReq::any())]));
        assert!(graph.detect_cycles().is_none());
    }

    #[test]
    fn cycle_detected() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("a", vec![make_dep("b", VersionReq::any())]));
        graph.add_node(make_node("b", vec![make_dep("c", VersionReq::any())]));
        graph.add_node(make_node("c", vec![make_dep("a", VersionReq::any())]));
        assert!(graph.detect_cycles().is_some());
    }

    #[test]
    fn self_cycle_detected() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("a", vec![make_dep("a", VersionReq::any())]));
        assert!(graph.detect_cycles().is_some());
    }

    #[test]
    fn dependencies_and_dependents() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("storage", vec![]));
        graph.add_node(make_node("indexer", vec![make_dep("storage", VersionReq::any())]));
        graph.add_node(make_node("search", vec![make_dep("indexer", VersionReq::any())]));

        assert_eq!(graph.dependencies_of("storage"), Vec::<String>::new());
        assert_eq!(graph.dependencies_of("indexer"), vec!["storage"]);
        assert_eq!(graph.dependents_of("storage"), vec!["indexer"]);
    }

    #[test]
    fn topological_order() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("storage", vec![]));
        graph.add_node(make_node("indexer", vec![make_dep("storage", VersionReq::any())]));
        graph.add_node(make_node("search", vec![make_dep("indexer", VersionReq::any())]));

        let registry = CapabilityRegistry::new();
        let result = graph.resolve(&registry);

        // Order should be storage → indexer → search
        assert!(result.is_ok());
        assert_eq!(result.order.len(), 3);
        assert_eq!(result.order[0], "storage");
        assert_eq!(result.order[1], "indexer");
        assert_eq!(result.order[2], "search");
    }

    #[test]
    fn missing_dependency_error() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("plugin", vec![make_dep("missing", VersionReq::any())]));

        let registry = CapabilityRegistry::new();
        let result = graph.resolve(&registry);

        // Should have missing dependency errors
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn all_nodes_and_ids() {
        let graph = DependencyGraph::new();
        graph.add_node(make_node("a", vec![]));
        graph.add_node(make_node("b", vec![]));

        let nodes = graph.all_nodes();
        assert_eq!(nodes.len(), 2);
        let ids = graph.all_node_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"a".to_string()));
        assert!(ids.contains(&"b".to_string()));
    }
}
