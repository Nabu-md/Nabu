use crate::graph::serializer::{SerializedEdge, SerializedNode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

/// An append-only change log for incremental graph updates.
///
/// Each change is recorded as a JSONL line (one JSON object per line).
/// The log can be replayed to reconstruct graph state, and compacted
/// periodically to avoid unbounded growth.
///
/// Stored under `.nabu/graph/changes.log` alongside the main graph snapshot.
pub struct ChangeLog {
    log_path: PathBuf,
    file: Mutex<fs::File>,
    #[allow(dead_code)]
    /// Number of entries written since last compaction
    entries_since_compact: u64,
    /// Total entries in the current log
    total_entries: u64,
}

/// A single change entry in the change log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ChangeEntry {
    /// A new node was added to the graph
    NodeAdded(SerializedNode),
    /// An existing node was modified
    NodeModified(SerializedNode),
    /// A node was removed from the graph
    NodeRemoved { node_id: Uuid },
    /// A new edge was added
    EdgeAdded(SerializedEdge),
    /// An edge was removed
    EdgeRemoved {
        source: Uuid,
        target: Uuid,
        relationship: String,
    },
    /// Full graph checkpoint (for log compaction)
    Checkpoint(CheckpointData),
}

/// A full graph checkpoint, written during log compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    pub nodes: Vec<SerializedNode>,
    pub edges: Vec<SerializedEdge>,
    pub generation: u64,
}

impl ChangeLog {
    /// Open or create the change log file.
    pub fn new(graph_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let graph_dir: PathBuf = graph_dir.into();
        let log_path = graph_dir.join("changes.log");

        // Create parent directory if needed
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create change log directory: {}", e))?;
        }

        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open change log: {}", e))?;

        let total_entries = Self::count_entries(&log_path)?;

        Ok(Self {
            log_path,
            file: Mutex::new(file),
            entries_since_compact: 0,
            total_entries,
        })
    }

    /// Append a change entry to the log.
    pub fn append(&self, entry: &ChangeEntry) -> Result<(), String> {
        let mut file = self.file.lock().map_err(|e| e.to_string())?;

        let json =
            serde_json::to_string(entry).map_err(|e| format!("Serialization error: {}", e))?;

        writeln!(file, "{}", json).map_err(|e| format!("Write error: {}", e))?;

        file.flush().map_err(|e| format!("Flush error: {}", e))?;

        Ok(())
    }

    /// Replay all entries from the change log, calling the provided function
    /// for each entry.
    pub fn replay<F>(&self, mut callback: F) -> Result<u64, String>
    where
        F: FnMut(ChangeEntry) -> Result<(), String>,
    {
        let file = fs::File::open(&self.log_path)
            .map_err(|e| format!("Failed to open change log for replay: {}", e))?;

        let reader = BufReader::new(file);
        let mut count = 0u64;

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: ChangeEntry = serde_json::from_str(&line)
                .map_err(|e| format!("Parse error at line {}: {}", count + 1, e))?;

            callback(entry)?;
            count += 1;
        }

        Ok(count)
    }

    /// Compact the change log by writing a full checkpoint and truncating.
    ///
    /// This should be called periodically (e.g., every 10,000 changes or
    /// after every full rebuild).
    pub fn compact<F>(&self, snapshot_fn: F) -> Result<(), String>
    where
        F: Fn() -> Result<CheckpointData, String>,
    {
        let file = self.file.lock().map_err(|e| e.to_string())?;

        let checkpoint_data = snapshot_fn()?;
        let checkpoint = ChangeEntry::Checkpoint(checkpoint_data);

        // Write checkpoint as first entry of new log
        // Close old file, create new one
        drop(file);

        let temp_path = self.log_path.with_extension("log.tmp");

        let mut new_file = fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp log: {}", e))?;

        let json = serde_json::to_string(&checkpoint)
            .map_err(|e| format!("Serialization error: {}", e))?;

        writeln!(new_file, "{}", json).map_err(|e| format!("Write error: {}", e))?;

        new_file
            .flush()
            .map_err(|e| format!("Flush error: {}", e))?;

        // Atomically replace old log with new
        fs::rename(&temp_path, &self.log_path)
            .map_err(|e| format!("Failed to replace change log: {}", e))?;

        // Reopen
        let new_handle = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.log_path)
            .map_err(|e| format!("Failed to reopen change log: {}", e))?;

        *self.file.lock().map_err(|e| e.to_string())? = new_handle;

        Ok(())
    }

    /// Whether the change log is empty.
    pub fn is_empty(&self) -> bool {
        self.total_entries == 0
    }

    /// Number of entries in the change log.
    pub fn entry_count(&self) -> u64 {
        self.total_entries
    }

    /// Path to the change log file.
    pub fn path(&self) -> &Path {
        &self.log_path
    }

    /// Delete the change log file.
    pub fn delete(&self) -> Result<(), String> {
        if self.log_path.exists() {
            fs::remove_file(&self.log_path)
                .map_err(|e| format!("Failed to delete change log: {}", e))?;
        }
        Ok(())
    }

    /// Count entries in the change log file.
    fn count_entries(path: &Path) -> Result<u64, String> {
        if !path.exists() {
            return Ok(0);
        }

        let file = fs::File::open(path).map_err(|e| format!("Failed to open change log: {}", e))?;

        let reader = BufReader::new(file);
        let mut count = 0u64;

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Read error: {}", e))?;
            if !line.trim().is_empty() {
                count += 1;
            }
        }

        Ok(count)
    }
}

/// Auto-compaction configuration.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Compact after this many entries (0 = never)
    pub max_entries_before_compact: u64,
    /// Minimum entries before first compaction
    pub min_entries_for_compact: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            max_entries_before_compact: 10_000,
            min_entries_for_compact: 1_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_append_and_replay() {
        let dir = tempdir().unwrap();
        let log = ChangeLog::new(dir.path()).unwrap();

        let node_id = Uuid::new_v4();
        log.append(&ChangeEntry::NodeAdded(SerializedNode::new(
            node_id,
            "note",
            Some("Test".to_string()),
            "text/markdown",
        )))
        .unwrap();
        log.append(&ChangeEntry::NodeRemoved { node_id }).unwrap();

        let mut count = 0u64;
        log.replay(|entry| {
            count += 1;
            match entry {
                ChangeEntry::NodeAdded(_) => {}
                ChangeEntry::NodeRemoved { node_id: _ } => {}
                _ => panic!("Unexpected entry type"),
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(count, 2);
    }

    #[test]
    fn test_compaction() {
        let dir = tempdir().unwrap();
        let log = ChangeLog::new(dir.path()).unwrap();

        // Write some entries, then compact
        log.append(&ChangeEntry::NodeAdded(SerializedNode::new(
            Uuid::new_v4(),
            "note",
            None,
            "text",
        )))
        .unwrap();

        log.compact(|| {
            Ok(CheckpointData {
                nodes: vec![],
                edges: vec![],
                generation: 1,
            })
        })
        .unwrap();

        // After compaction, the log should have exactly 1 entry (the checkpoint)
        let mut count = 0u64;
        log.replay(|entry| {
            count += 1;
            assert!(matches!(entry, ChangeEntry::Checkpoint(_)));
            Ok(())
        })
        .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_empty_log() {
        let dir = tempdir().unwrap();
        let log = ChangeLog::new(dir.path()).unwrap();
        assert!(log.is_empty());
    }
}
