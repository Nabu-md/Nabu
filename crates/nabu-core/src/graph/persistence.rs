use crate::graph::integrity::{check_integrity, compute_graph_checksum, quick_check, IntegrityReport};
use crate::graph::serializer::GraphSnapshot;
use crate::graph::version::GraphVersion;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Default graph data filename.
const GRAPH_DATA_FILENAME: &str = "graph.json";

/// Default graph metadata filename.
const GRAPH_META_FILENAME: &str = "graph.meta.json";

/// The file-backed graph persistence layer.
///
/// All graph data is persisted under `.nabu/graph/`.
/// Never stores canonical data — only derived graph state.
pub struct GraphStore {
    /// Root path (contains `.nabu/graph/`)
    base_path: PathBuf,

    /// Path to the graph data directory
    graph_dir: PathBuf,

    /// Path to the graph data file
    data_path: PathBuf,

    /// Path to the graph metadata file
    meta_path: PathBuf,

    /// Internal lock for file I/O safety
    lock: Mutex<()>,
}

impl GraphStore {
    /// Create a new GraphStore rooted at the given base path.
    ///
    /// Creates the `.nabu/graph/` directory structure if it doesn't exist.
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self, String> {
        let base_path: PathBuf = base_path.into();
        let graph_dir = base_path.join(".nabu").join("graph");

        fs::create_dir_all(&graph_dir)
            .map_err(|e| format!("Failed to create graph directory: {}", e))?;

        Ok(Self {
            base_path,
            graph_dir: graph_dir.clone(),
            data_path: graph_dir.join(GRAPH_DATA_FILENAME),
            meta_path: graph_dir.join(GRAPH_META_FILENAME),
            lock: Mutex::new(()),
        })
    }

    /// Save a graph snapshot to disk.
    ///
    /// Writes both the graph data file and the metadata file.
    /// Computes and stores a checksum for integrity verification.
    pub fn save(&self, snapshot: &GraphSnapshot) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;

        let mut snapshot = snapshot.clone();

        // Compute and store checksum
        let checksum = compute_graph_checksum(&snapshot);
        snapshot.version.mark_saved(Some(checksum.clone()));

        // Write graph data
        let json = snapshot
            .to_json_bytes()
            .map_err(|e| format!("Serialization error: {}", e))?;

        let mut file = fs::File::create(&self.data_path)
            .map_err(|e| format!("Failed to create graph file: {}", e))?;
        file.write_all(&json)
            .map_err(|e| format!("Failed to write graph file: {}", e))?;

        // Write metadata (lightweight, just version info)
        let meta_json = serde_json::to_string_pretty(&snapshot.version)
            .map_err(|e| format!("Metadata serialization error: {}", e))?;
        let mut meta_file = fs::File::create(&self.meta_path)
            .map_err(|e| format!("Failed to create metadata file: {}", e))?;
        meta_file
            .write_all(meta_json.as_bytes())
            .map_err(|e| format!("Failed to write metadata file: {}", e))?;

        tracing::info!(
            "Graph saved: {} nodes, {} edges, checksum: {}",
            snapshot.nodes.len(),
            snapshot.edges.len(),
            &checksum[..12]
        );

        Ok(())
    }

    /// Load a graph snapshot from disk.
    ///
    /// Returns `None` if no graph file exists (first run).
    pub fn load(&self) -> Result<Option<GraphSnapshot>, String> {
        if !self.data_path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&self.data_path)
            .map_err(|e| format!("Failed to read graph file: {}", e))?;

        let snapshot = GraphSnapshot::from_json_bytes(&bytes)
            .map_err(|e| format!("Failed to parse graph file: {}", e))?;

        // Quick structural validation
        if !quick_check(&snapshot) {
            return Err("Graph file failed quick integrity check".to_string());
        }

        Ok(Some(snapshot))
    }

    /// Load just the graph version metadata (lightweight, fast).
    pub fn load_metadata(&self) -> Result<Option<GraphVersion>, String> {
        if !self.meta_path.exists() {
            // Fall back to extracting version from full graph data
            return self.load_version_from_data();
        }

        let json = fs::read_to_string(&self.meta_path)
            .map_err(|e| format!("Failed to read metadata file: {}", e))?;

        serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| format!("Failed to parse metadata: {}", e))
    }

    /// Check if a persisted graph exists.
    pub fn exists(&self) -> bool {
        self.data_path.exists()
    }

    /// Delete the persisted graph files.
    /// Useful before a full rebuild.
    pub fn delete(&self) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;

        if self.data_path.exists() {
            fs::remove_file(&self.data_path)
                .map_err(|e| format!("Failed to delete graph file: {}", e))?;
        }
        if self.meta_path.exists() {
            fs::remove_file(&self.meta_path)
                .map_err(|e| format!("Failed to delete metadata file: {}", e))?;
        }

        Ok(())
    }

    /// Get the size of the graph data file in bytes.
    pub fn data_file_size(&self) -> Result<u64, String> {
        fs::metadata(&self.data_path)
            .map(|m| m.len())
            .map_err(|e| format!("Failed to read graph file metadata: {}", e))
    }

    /// Path to the graph data directory.
    pub fn graph_dir(&self) -> &Path {
        &self.graph_dir
    }

    /// Perform a full integrity check on the persisted graph.
    pub fn verify_integrity(&self) -> Result<IntegrityReport, String> {
        match self.load()? {
            Some(snapshot) => Ok(check_integrity(&snapshot)),
            None => Err("No graph file to verify".to_string()),
        }
    }

    /// Load version from the full graph data (fallback if meta file missing).
    fn load_version_from_data(&self) -> Result<Option<GraphVersion>, String> {
        match self.load()? {
            Some(snapshot) => Ok(Some(snapshot.version)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::serializer::{SerializedEdge, SerializedNode};
    use crate::graph::version::GraphVersion;
    use tempfile::tempdir;

    fn create_test_snapshot() -> GraphSnapshot {
        let version = GraphVersion::new();
        let mut snapshot = GraphSnapshot::new(version);

        let n1 = uuid::Uuid::new_v4();
        let n2 = uuid::Uuid::new_v4();

        snapshot.add_node(SerializedNode::new(n1, "note", Some("Test A".into()), "text"));
        snapshot.add_node(SerializedNode::new(n2, "note", Some("Test B".into()), "text"));
        snapshot.add_edge(SerializedEdge::new(n1, n2, "references"));

        snapshot
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();

        assert!(!store.exists());

        let snapshot = create_test_snapshot();
        store.save(&snapshot).unwrap();

        assert!(store.exists());

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.node_count(), 2);
        assert_eq!(loaded.edge_count(), 1);
    }

    #[test]
    fn test_survives_restart() {
        let dir = tempdir().unwrap();

        // Save in one store instance
        {
            let store = GraphStore::new(dir.path()).unwrap();
            store.save(&create_test_snapshot()).unwrap();
        }

        // Load in another (simulating app restart)
        {
            let store = GraphStore::new(dir.path()).unwrap();
            let loaded = store.load().unwrap().unwrap();
            assert_eq!(loaded.node_count(), 2);
        }
    }

    #[test]
    fn test_metadata_storage() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();

        store.save(&create_test_snapshot()).unwrap();

        let meta = store.load_metadata().unwrap().unwrap();
        assert!(meta.schema_version > 0);
        assert_eq!(meta.build_source, BuildSource::Initial);
    }

    #[test]
    fn test_delete() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();

        store.save(&create_test_snapshot()).unwrap();
        assert!(store.exists());

        store.delete().unwrap();
        assert!(!store.exists());
    }

    #[test]
    fn test_no_file_on_first_run() {
        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path()).unwrap();
        assert!(!store.exists());

        let loaded = store.load().unwrap();
        assert!(loaded.is_none());
    }
}
