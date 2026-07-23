use std::collections::{HashMap, HashSet};

use petgraph::stable_graph::{NodeIndex, StableGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    File,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Wikilink,
    Embed,
    Backlink,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Wikilink => "wikilink",
            EdgeKind::Embed => "embed",
            EdgeKind::Backlink => "backlink",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub snippet: String,
}

#[derive(Debug, Default)]
pub struct GraphEngine {
    nodes: HashMap<String, GraphNode>,
    adjacency: HashMap<String, HashSet<String>>,
    outgoing: HashMap<String, Vec<(String, EdgeKind)>>,
    forward: StableGraph<String, EdgeKind>,
    node_index: HashMap<String, NodeIndex>,
}

impl GraphEngine {
    pub fn build(files: &[crate::vault::FileEntry], alias_index: &HashMap<String, Vec<String>>) -> Self {
        let mut engine = Self::default();
        engine.rebuild(files, alias_index);
        engine
    }

    pub fn rebuild(&mut self, files: &[crate::vault::FileEntry], alias_index: &HashMap<String, Vec<String>>) {
        self.nodes.clear();
        self.adjacency.clear();
        self.outgoing.clear();
        self.forward = StableGraph::default();
        self.node_index.clear();

        for file in files {
            let title = file.name.strip_suffix(".md").unwrap_or(file.name).to_string();
            self.nodes.insert(
                file.path.clone(),
                GraphNode {
                    id: file.path.clone(),
                    kind: NodeKind::File,
                    title,
                    path: Some(file.path.clone()),
                },
            );
        }

        let mut path_by_lowercase = HashMap::new();
        for file in files {
            path_by_lowercase.insert(file.path.to_lowercase(), file.path.clone());
        }

        for file in files {
            let idx = self.ensure_node(file.path.clone(), NodeKind::File);
            let raw = std::fs::read_to_string(&file.path).unwrap_or_default();
            for target in extract_wikilinks(&raw) {
                let Some(target_path) = resolve_target(&target, &path_by_lowercase, alias_index) else { continue };
                let target_idx = self.ensure_node(target_path.clone(), NodeKind::File);
                self.adjacency.entry(file.path.clone()).or_default().insert(target_path.clone());
                self.outgoing.entry(file.path.clone()).or_default().push((target_path.clone(), EdgeKind::Wikilink));
                self.forward.add_edge(idx, target_idx, EdgeKind::Wikilink);
            }
        }

        for edge in self.forward.edge_references().copied().collect::<Vec<_>>() {
            let target_idx = edge.target();
            let source_idx = edge.source();
            let target = self.forward[target_idx].clone();
            let source = self.forward[source_idx].clone();
            self.outgoing.entry(target.clone()).or_default().push((source, EdgeKind::Backlink));
            self.forward.add_edge(source_idx, target_idx, EdgeKind::Backlink);
        }
    }

    pub fn nodes(&self) -> &HashMap<String, GraphNode> {
        &self.nodes
    }

    pub fn edges(&self) -> Vec<GraphEdge> {
        let mut collected = Vec::new();
        for edge in self.forward.edge_references() {
            let source = self.forward[edge.source()].clone();
            let target = self.forward[edge.target()].clone();
            collected.push(GraphEdge {
                source,
                target,
                kind: *edge.weight(),
                snippet: String::new(),
            });
        }
        collected
    }

    pub fn neighbors(&self, node_id: &str) -> Vec<GraphNode> {
        let Some(&idx) = self.node_index.get(node_id) else { return Vec::new() };
        self.forward
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .filter_map(|neighbor| self.nodes.get(&self.forward[neighbor]).cloned())
            .collect()
    }

    pub fn connected_components(&self) -> Vec<Vec<GraphNode>> {
        let mut seen = HashSet::new();
        let mut components = Vec::new();
        for node_id in self.nodes.keys() {
            if seen.contains(node_id.as_str()) {
                continue;
            }
            let mut component = Vec::new();
            let mut queue = vec![node_id.clone()];
            seen.insert(node_id.clone());
            while let Some(current) = queue.pop() {
                let node = self.nodes[&current].clone();
                component.push(node);
                for neighbor_id in self.adjacency.get(&current).into_iter().flatten().cloned() {
                    if seen.insert(neighbor_id.clone()) {
                        queue.push(neighbor_id);
                    }
                }
                for neighbor_id in self.outgoing.get(&current).into_iter().flatten().filter_map(|(target, kind)| (*kind == EdgeKind::Backlink).then_some(target)).cloned() {
                    if seen.insert(neighbor_id.clone()) {
                        queue.push(neighbor_id);
                    }
                }
            }
            components.push(component);
        }
        components
    }

    pub fn degree(&self, node_id: &str) -> usize {
        let Some(&idx) = self.node_index.get(node_id) else { return 0 };
        self.forward.edges(idx).count()
    }

    fn ensure_node(&mut self, id: String, kind: NodeKind) -> NodeIndex {
        *self.node_index.entry(id.clone()).or_insert_with(|| {
            let mut title = id.clone();
            if let Some(name) = std::path::Path::new(&id).file_name().and_then(|v| v.to_str()) {
                title = name.strip_suffix(".md").unwrap_or(name).to_string();
            }
            let node = GraphNode { id: id.clone(), kind, title, path: Some(id.clone()) };
            let idx = self.forward.add_node(id.clone());
            self.nodes.insert(id.clone(), node);
            idx
        })
    }
}

#[derive(Debug, Default)]
pub struct IncrementalGraphUpdate {
    pub added_paths: Vec<String>,
    pub updated_paths: Vec<String>,
    pub removed_paths: Vec<String>,
}

impl GraphEngine {
    pub fn apply_incremental(&mut self, files: &[crate::vault::FileEntry], delta: &IncrementalGraphUpdate) {
        for path in &delta.removed_paths {
            self.nodes.remove(path);
            self.node_index.remove(path);
            self.adjacency.remove(path);
            self.outgoing.remove(path);
        }

        for path in &delta.added_paths {
            if let Some(name) = std::path::Path::new(path).file_name().and_then(|v| v.to_str()) {
                self.ensure_node(path.clone(), NodeKind::File);
            }
        }

        self.rebuild(files, &HashMap::default());
    }
}

fn title_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or(path)
        .to_string()
}

fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut capture = false;
    let mut current = String::new();
    for token in content.split("") {
        if token == "[" && content.starts_with("[[") {
            capture = true;
            current.clear();
            continue;
        }
        if capture {
            if token == "]" {
                capture = false;
                links.push(current.clone());
                current.clear();
                continue;
            }
            current.push_str(token);
        }
    }
    links
}

fn resolve_target(
    target: &str,
    path_by_lowercase: &HashMap<String, String>,
    alias_index: &HashMap<String, Vec<String>>,
) -> Option<String> {
    if let Some(path) = path_by_lowercase.get(&target.to_lowercase()) {
        return Some(path.clone());
    }
    alias_index.get(&target.to_lowercase()).and_then(|paths| paths.first().cloned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::vault::FileEntry;

    use super::*;

    #[test]
    fn graph_engine_preserves_current_contract() {
        let files = vec![
            FileEntry {
                path: "/vaults/a.md".into(),
                name: "a.md".into(),
                mtime: 1.0,
            },
            FileEntry {
                path: "/vaults/b.md".into(),
                name: "b.md".into(),
                mtime: 2.0,
            },
        ];

        std::fs::write("/vaults/a.md", "[[b]]").ok();
        let engine = GraphEngine::build(&files, &HashMap::default());

        assert_eq!(engine.nodes().len(), 2);
        let edges = engine.edges();
        assert!(edges.iter().any(|edge| edge.source == "/vaults/a.md" && edge.target == "/vaults/b.md" && edge.kind == EdgeKind::Wikilink));
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    File,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Wikilink,
    Embed,
    Backlink,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Wikilink => "wikilink",
            EdgeKind::Embed => "embed",
            EdgeKind::Backlink => "backlink",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub snippet: String,
}

#[derive(Debug, Default)]
pub struct GraphEngine {
    nodes: HashMap<String, GraphNode>,
    adjacency: HashMap<String, HashSet<String>>,
    forward: StableGraph<String, EdgeKind>,
    node_index: HashMap<String, NodeIndex>,
}

impl GraphEngine {
    pub fn build(files: &[crate::vault::FileEntry], alias_index: &HashMap<String, Vec<String>>) -> Self {
        let mut engine = Self::default();
        engine.rebuild(files, alias_index);
        engine
    }

    pub fn rebuild(&mut self, files: &[crate::vault::FileEntry], alias_index: &HashMap<String, Vec<String>>) {
        self.nodes.clear();
        self.adjacency.clear();
        self.forward = StableGraph::default();
        self.node_index.clear();

        let mut path_by_lowercase = HashMap::new();
        for file in files {
            let title = file.name.strip_suffix(".md").unwrap_or(file.name);
            self.nodes.insert(
                file.path.clone(),
                GraphNode {
                    id: file.path.clone(),
                    kind: NodeKind::File,
                    title: title.to_string(),
                    path: Some(file.path.clone()),
                },
            );
            path_by_lowercase.insert(file.path.to_lowercase(), file.path.clone());
        }

        for file in files {
            let source_path = &file.path;
            let idx = self.ensure_node(source_path.clone(), NodeKind::File, title_from_path(source_path));

            let raw = std::fs::read_to_string(source_path).unwrap_or_default();
            for target in extract_wikilinks(&raw) {
                let Some(target_path) = resolve_target(&target, &path_by_lowercase, alias_index) else { continue };
                let target_idx = self.ensure_node(target_path.clone(), NodeKind::File, title_from_path(&target_path));
                self.adjacency.entry(source_path.clone()).or_default().insert(target_path.clone());
                self.forward.add_edge(idx, target_idx, EdgeKind::Wikilink);
            }
        }

        for target_path in self.nodes.keys().cloned().collect::<Vec<_>>() {
            if let Some(sources) = self.sources_for(&target_path) {
                let target_idx = *self.node_index.get(&target_path).unwrap();
                for source in sources {
                    let source_idx = *self.node_index.get(&source).unwrap();
                    self.forward.add_edge(source_idx, target_idx, EdgeKind::Backlink);
                }
            }
        }
    }

    pub fn nodes(&self) -> &HashMap<String, GraphNode> {
        &self.nodes
    }

    pub fn edges(&self) -> Vec<GraphEdge> {
        let mut collected = Vec::new();
        for edge in self.forward.edge_references() {
            let source = self.forward[edge.source()].clone();
            let target = self.forward[edge.target()].clone();
            collected.push(GraphEdge {
                source,
                target,
                kind: *edge.weight(),
                snippet: String::new(),
            });
        }
        collected
    }

    pub fn neighbors(&self, node_id: &str) -> Vec<GraphNode> {
        let Some(&idx) = self.node_index.get(node_id) else { return Vec::new() };
        self.forward
            .neighbors_directed(idx, Direction::Outgoing)
            .filter_map(|neighbor| self.nodes.get(&self.forward[neighbor]).cloned())
            .collect()
    }

    pub fn connected_components(&self) -> Vec<Vec<GraphNode>> {
        let mut seen = HashSet::new();
        let mut components = Vec::new();
        for node_id in self.nodes.keys() {
            if seen.contains(node_id.as_str()) {
                continue;
            }
            let mut component = Vec::new();
            let mut queue = vec![node_id.clone()];
            seen.insert(node_id.clone());
            while let Some(current) = queue.pop() {
                let node = self.nodes[&current].clone();
                component.push(node);
                for neighbor_id in self.adjacency.get(&current).into_iter().flatten().cloned() {
                    if seen.insert(neighbor_id.clone()) {
                        queue.push(neighbor_id);
                    }
                }
                for neighbor_id in self.sources_for(&current).into_iter().flatten().cloned() {
                    if seen.insert(neighbor_id.clone()) {
                        queue.push(neighbor_id);
                    }
                }
            }
            components.push(component);
        }
        components
    }

    pub fn degree(&self, node_id: &str) -> usize {
        let Some(&idx) = self.node_index.get(node_id) else { return 0 };
        self.forward.edges(idx).count()
    }

    fn ensure_node(&mut self, id: String, kind: NodeKind, title: String) -> NodeIndex {
        *self.node_index.entry(id.clone()).or_insert_with(|| {
            let node = GraphNode { id: id.clone(), kind, title, path: Some(id.clone()) };
            let idx = self.forward.add_node(id.clone());
            self.nodes.insert(id.clone(), node);
            idx
        })
    }

    fn sources_for(&self, target_path: &str) -> Option<&HashSet<String>> {
        self.adjacency.iter().find_map(|(source, targets)| {
            if targets.contains(target_path) { Some(source) } else { None }
        })
    }
}

#[derive(Debug, Default)]
pub struct IncrementalGraphUpdate {
    pub added_paths: Vec<String>,
    pub updated_paths: Vec<String>,
    pub removed_paths: Vec<String>,
}

impl GraphEngine {
    pub fn apply_incremental(&mut self, files: &[crate::vault::FileEntry], alias_index: &HashMap<String, Vec<String>>, delta: &IncrementalGraphUpdate) {
        for path in &delta.removed_paths {
            self.nodes.remove(path);
            self.node_index.remove(path);
        }
        let all_files = files
            .iter()
            .map(|file| (file.path.clone(), file.name.clone()))
            .collect::<Vec<_>>();
        let mut rebuild_paths = delta.added_paths.clone();
        rebuild_paths.extend(delta.updated_paths.iter().cloned());
        for (path, name) in all_files {
            if rebuild_paths.contains(&path) {
                if let Some(node) = self.nodes.get_mut(&path) {
                    node.title = title_from_path(&path);
                }
            }
        }
        let reload_paths: Vec<_> = files.iter().map(|file| file.path.clone()).collect();
        self.rebuild_from_paths(&reload_paths, alias_index);
    }

    fn rebuild_from_paths(&mut self, paths: &[String], alias_index: &HashMap<String, Vec<String>>) {
        let mut path_by_lowercase = HashMap::new();
        for source_path in paths {
            if let Some(file) = crate::vault::FileEntry::from_path(source_path) {
                let title = file.name.strip_suffix(".md").unwrap_or(file.name);
                self.nodes.insert(
                    file.path.clone(),
                    GraphNode {
                        id: file.path.clone(),
                        kind: NodeKind::File,
                        title: title.to_string(),
                        path: Some(file.path.clone()),
                    },
                );
                path_by_lowercase.insert(file.path.to_lowercase(), file.path.clone());
                let raw = std::fs::read_to_string(source_path).unwrap_or_default();
                for target in extract_wikilinks(&raw) {
                    if let Some(target_path) = resolve_target(&target, &path_by_lowercase, alias_index) {
                        self.adjacency.entry(source_path.clone()).or_default().insert(target_path.clone());
                    }
                }
            }
            let _ = std::fs::read_to_string(source_path);
        }
        self.update_forward_edges(paths, alias_index);
    }

    fn update_forward_edges(&mut self, paths: &[String], alias_index: &HashMap<String, Vec<String>>) {
        let mut to_remove = Vec::new();
        for source_path in paths {
            if !self.nodes.contains_key(source_path) {
                to_remove.push(source_path.clone());
                continue;
            }
            let source_idx = *self.node_index.entry(source_path.clone()).or_insert_with(|| {
                self.forward.add_node(source_path.clone())
            });
            self.forward.retain_edges(|edge| {
                !(self.forward[edge.source()] == *source_path && edge.weight() == &EdgeKind::Wikilink)
            });
            let raw = std::fs::read_to_string(source_path).unwrap_or_default();
            for target in extract_wikilinks(&raw) {
                let Some(target_path) = resolve_target(&target, &alias_index.iter().flat_map(|(k, v)| v.iter().map(|v| (k.clone(), v.clone()))).collect(), alias_index) else { continue };
                let target_idx = *self.node_index.entry(target_path.clone()).or_insert_with(|| {
                    self.forward.add_node(target_path.clone())
                });
                self.forward.update_edge(source_idx, target_idx, EdgeKind::Wikilink).unwrap();
            }
        }
    }
}

impl GraphEngine {
    pub fn degree(&self, node_id: &str) -> usize {
        let Some(&idx) = self.node_index.get(node_id) else { return 0 };
        self.forward.edges(idx).count()
    }
}



