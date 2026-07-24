use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub theme: String,
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<crate::models::RecentVaultEntry>,
}

/// Metadata describing a single file in a vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultScanResult {
    pub path: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("no vault is currently open")]
    NoOpenVault,
    #[error("failed to read file: {0}")]
    ReadFile(String),
    #[error("failed to write file: {0}")]
    WriteFile(String),
    #[error("failed to create item: {0}")]
    Create(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("conflict")]
    Conflict,
}

pub struct VaultService {
    pub sessions: std::collections::HashMap<PathBuf, crate::vault::VaultSession>,
    pub graph: crate::graph::GraphEngine,
}
    pub fn start_watching(&mut self, path: PathBuf) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let _watcher = crate::watcher::Watcher::new(path.clone(), tx)?;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Ok(path) = match event {
                    crate::watcher::WatchEvent::Created(p) | crate::watcher::WatchEvent::Changed(p) => Ok(p),
                    _ => Err(()),
                } {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        // Simplified lookup for demo: assuming only one session or handle path resolution
                        // sessions.values_mut().for_each(|s| { s.indexer.index_document(&path.to_string_lossy(), &content).ok(); });
                    }
                }
            }
        });
        Ok(())
    }

    pub fn search(&self, vault_path: &Path, query: &str) -> Result<Vec<String>> {
        let session = self.sessions.get(vault_path).context("Vault not open")?;
        Ok(session.indexer.search(query)?)
    }
    pub fn get_backlinks(&self, vault_path: &Path, note_path: &str) -> Vec<String> {
        self.sessions.get(vault_path)
            .map(|s| s.graph.get_backlinks(note_path))
            .unwrap_or_default()
impl Default for VaultService {
    fn default() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
            graph: crate::graph::GraphEngine::default(),
        }
    }
}

impl VaultService {
    pub fn get_graph_data(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.graph.nodes(),
            "edges": self.graph.edges()
        })
    }
}
}
pub struct SettingsStore {
    pub theme: String,
    pub last_vault_path: String,
impl VaultService {
    pub fn open(&mut self, path: PathBuf, _settings: AppSettings) -> Result<()> {
        let session = crate::vault::VaultSession::new("temp-id".into(), path.clone());
        self.sessions.insert(path, session);
        Ok(())
    }
    // ... other methods updated similarly
}
        })
    }
}

fn ensure_open(root: &Path) -> Result<()> {
    if root.as_os_str().is_empty() || !root.exists() {
        anyhow::bail!(VaultError::NoOpenVault);
    }
    Ok(())
}

fn resolve(root: &Path, relative: &str) -> Result<PathBuf, VaultError> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    Ok(match root.join(relative).canonicalize() {
        Ok(path) => path,
        Err(_) => root.join(relative),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("nabu-vault-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn open_and_scan_vault_succeeds() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("note.md"), b"hello").unwrap();

        let settings = SettingsStore::default();
        let service = VaultService::open(&root, settings).unwrap();
        let scan = service.scan().unwrap();

        assert_eq!(scan.path, root.display().to_string());
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.files[0].name, "note.md");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_vault_returns_error() {
        let settings = SettingsStore::default();
        let err = VaultService::open(temp_root(), settings).unwrap_err();
        assert!(matches!(err.downcast_ref::<VaultError>().unwrap(), crate::VaultError::InvalidPath(_)));
    }

    #[test]
    fn create_read_update_delete_file_roundtrip() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();

        let settings = SettingsStore::default();
    pub fn update_file(&self, path: &Path, contents: &str) -> Result<FileEntry> {
        if contents.len() > 10 * 1024 * 1024 {
            return Err(VaultError::InvalidPath("File too large".into()).into());
        }
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, contents)?;
        std::fs::rename(&temp_path, path)?;
        
        Ok(FileEntry {
            path: path.to_string_lossy().into(),
            name: path.file_name().unwrap().to_string_lossy().into(),
            mtime: 0.0, // Placeholder
        })
    }

        let settings = SettingsStore::default();
        let service = VaultService::open(&root, settings).unwrap();

        std::fs::write(root.join("root.md"), b"root").unwrap();
        service.create_folder("work").unwrap();
        service.move_item("root.md", "work/inbox.md").unwrap();

        let scan = service.scan().unwrap();
        assert!(scan.files.iter().any(|item| item.name == "inbox.md"));

        service.delete_folder("work").unwrap();
        let scan_after = service.scan().unwrap();
        assert!(scan_after.files.is_empty());

        std::fs::remove_dir_all(&root).unwrap();
    }
}
