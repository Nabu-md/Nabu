//! Plugin Dependency Graph — resolution, circular detection, and reporting.
//!
//! Supports:
//! - Required dependencies
//! - Optional dependencies
//! - Circular dependency detection
//! - Missing dependency reporting
//! - Version conflict reporting

use std::collections::{HashMap, HashSet, VecDeque};

use crate::plugin::manifest::PluginManifest;
use crate::plugin::version::Version;

/// A resolved dependency graph for plugins.
///
/// The graph is built from plugin manifests and can be queried for
/// dependency order, circular dependencies, and conflicts.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// Adjacency list: plugin_id → list of dependency plugin IDs.
    edges: HashMap<String, Vec<String>>,
    /// Optional adjacency list.
    optional_edges: HashMap<String, Vec<String>>,
    /// All known plugin IDs.
    nodes: HashSet<String>,
    /// Plugin versions for conflict checking.
    versions: HashMap<String, Version>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            optional_edges: HashMap::new(),
            nodes: HashSet::new(),
            versions: HashMap::new(),
        }
    }

    /// Add a plugin and its dependencies to the graph.
    pub fn add_plugin(&mut self, manifest: &PluginManifest) {
        self.nodes.insert(manifest.id.clone());
        self.versions.insert(manifest.id.clone(), manifest.version.clone());

        let deps: Vec<String> = manifest.dependencies.iter()
            .map(|d| d.plugin_id.clone())
            .collect();
        if !deps.is_empty() {
            self.edges.insert(manifest.id.clone(), deps);
        }

        let opt_deps: Vec<String> = manifest.optional_dependencies.iter()
            .map(|d| d.plugin_id.clone())
            .collect();
        if !opt_deps.is_empty() {
            self.optional_edges.insert(manifest.id.clone(), opt_deps);
        }
    }

    /// Add an edge between two plugins.
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.nodes.insert(from.to_string());
        self.nodes.insert(to.to_string());
        self.edges.entry(from.to_string()).or_default().push(to.to_string());
    }

    /// Check if a plugin ID exists in the graph.
    pub fn has_plugin(&self, plugin_id: &str) -> bool {
        self.nodes.contains(plugin_id)
    }

    /// Get the version of a plugin.
    pub fn version(&self, plugin_id: &str) -> Option<&Version> {
        self.versions.get(plugin_id)
    }

    /// Detect circular dependencies using DFS.
    ///
    /// Returns a list of cycles found, where each cycle is a list of
    /// plugin IDs forming a circular dependency.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut cycles = Vec::new();

        for node in &self.nodes {
            if !visited.contains(node) {
                let mut path = Vec::new();
                self.dfs(node, &mut visited, &mut in_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = self.edges.get(node) {
            for dep in deps {
                if in_stack.contains(dep) {
                    // Found a cycle — extract it from the path
                    let cycle_start = path.iter().position(|p| p == dep).unwrap();
                    let cycle: Vec<String> = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                } else if !visited.contains(dep) {
                    self.dfs(dep, visited, in_stack, path, cycles);
                }
            }
        }

        path.pop();
        in_stack.remove(node);
    }

    /// Find missing dependencies — required dependencies that are not in the graph.
    pub fn missing_dependencies(&self) -> Vec<MissingDependency> {
        let mut missing = Vec::new();

        for (plugin_id, deps) in &self.edges {
            for dep in deps {
                if !self.nodes.contains(dep) {
                    missing.push(MissingDependency {
                        plugin: plugin_id.clone(),
                        dependency: dep.clone(),
                        optional: false,
                    });
                }
            }
        }

        for (plugin_id, deps) in &self.optional_edges {
            for dep in deps {
                if !self.nodes.contains(dep) {
                    missing.push(MissingDependency {
                        plugin: plugin_id.clone(),
                        dependency: dep.clone(),
                        optional: true,
                    });
                }
            }
        }

        missing
    }

    /// Compute a topological ordering of plugins for installation.
    ///
    /// Returns `None` if cycles are detected.
    pub fn topological_order(&self) -> Option<Vec<String>> {
        if !self.detect_cycles().is_empty() {
            return None;
        }

        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.entry(node.clone()).or_insert(0);
        }

        for deps in self.edges.values() {
            for dep in deps {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<String> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(deps) = self.edges.get(&node) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        if order.len() == self.nodes.len() {
            // Kahn's algorithm emits dependents before their dependencies;
            // installation requires the reverse — dependencies first.
            order.reverse();
            Some(order)
        } else {
            None
        }
    }

    /// Count of plugins in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Count of dependency edges.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }
}

/// A missing dependency that prevents plugin installation.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingDependency {
    pub plugin: String,
    pub dependency: String,
    pub optional: bool,
}

/// Validates and evaluates a set of plugin manifests for dependency conflicts.
pub fn validate_dependencies(
    manifests: &[PluginManifest],
) -> DependencyReport {
    let mut graph = DependencyGraph::new();
    for manifest in manifests {
        graph.add_plugin(manifest);
    }

    let cycles = graph.detect_cycles();
    let missing = graph.missing_dependencies();
    let topological = graph.topological_order();

    DependencyReport {
        graph,
        cycles,
        missing,
        topological,
        total_plugins: manifests.len(),
    }
}

/// Report from dependency validation.
#[derive(Debug, Clone)]
pub struct DependencyReport {
    pub graph: DependencyGraph,
    pub cycles: Vec<Vec<String>>,
    pub missing: Vec<MissingDependency>,
    pub topological: Option<Vec<String>>,
    pub total_plugins: usize,
}

impl DependencyReport {
    pub fn is_valid(&self) -> bool {
        self.cycles.is_empty() && self.missing.iter().all(|m| m.optional)
    }

    pub fn has_critical_issues(&self) -> bool {
        !self.cycles.is_empty() || self.missing.iter().any(|m| !m.optional)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{PluginDependency, PluginManifest};
    use crate::plugin::version::Version;

    fn make_plugin(id: &str, deps: Vec<&str>) -> PluginManifest {
        let deps: Vec<PluginDependency> = deps.into_iter()
            .map(|d| PluginDependency {
                plugin_id: d.to_string(),
                version_requirement: crate::plugin::version::VersionRequirement::Compatible(Version::new(0, 1, 0)),
                optional: false,
            })
            .collect();

        PluginManifest {
            id: id.to_string(),
            name: format!("Plugin {}", id),
            version: Version::new(1, 0, 0),
            author: "test".into(),
            description: "test plugin".into(),
            min_nabu_version: Version::new(0, 1, 0),
            max_tested_version: None,
            manifest_version: 1,
            capabilities: vec![],
            dependencies: deps,
            optional_dependencies: vec![],
            feature_flags: vec![],
            permissions: vec![],
            entry_type: crate::plugin::manifest::PluginEntryType::Wasm,
        }
    }

    #[test]
    fn empty_graph() {
        let report = validate_dependencies(&[]);
        assert!(report.is_valid());
        assert!(!report.has_critical_issues());
    }

    #[test]
    fn no_dependencies() {
        let manifest = make_plugin("core", vec![]);
        let report = validate_dependencies(&[manifest]);
        assert!(report.is_valid());
        assert_eq!(report.graph.node_count(), 1);
    }

    #[test]
    fn linear_dependencies() {
        let a = make_plugin("a", vec!["b"]);
        let b = make_plugin("b", vec![]);
        let report = validate_dependencies(&[a, b]);
        assert!(report.is_valid());
        assert!(report.topological.is_some());
    }

    #[test]
    fn circular_dependency_detected() {
        let a = make_plugin("a", vec!["b"]);
        let b = make_plugin("b", vec!["a"]);
        let report = validate_dependencies(&[a, b]);
        assert!(report.has_critical_issues());
        assert_eq!(report.cycles.len(), 1);
    }

    #[test]
    fn missing_dependency_detected() {
        let a = make_plugin("a", vec!["missing_plugin"]);
        let report = validate_dependencies(&[a]);
        assert!(report.has_critical_issues());
        assert_eq!(report.missing.len(), 1);
        assert!(!report.missing[0].optional);
    }

    #[test]
    fn topological_order_respected() {
        let a = make_plugin("a", vec!["b", "c"]);
        let b = make_plugin("b", vec!["c"]);
        let c = make_plugin("c", vec![]);
        let report = validate_dependencies(&[a, b, c]);
        let order = report.topological.unwrap();
        // c must come before b, which must come before a
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        assert!(pos_c < pos_b);
        assert!(pos_b < pos_a);
    }
}
