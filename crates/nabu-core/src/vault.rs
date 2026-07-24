use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use anyhow::{Context, Result};
use crate::view_state::ViewStateManager;
use crate::indexer::Indexer;
use crate::watcher::WatchEvent;

pub struct VaultSession {
    pub vault_id: String,
    pub vault_path: PathBuf,
    pub view_state_manager: ViewStateManager,
    pub indexer: Indexer,
    pub is_active: bool,
}

impl VaultSession {
    pub fn handle_event(&mut self, event: crate::watcher::WatchEvent) -> anyhow::Result<()> {
        match event {
            crate::watcher::WatchEvent::Created(path) | crate::watcher::WatchEvent::Changed(path) => {
                let content = std::fs::read_to_string(&path)?;
                self.indexer.index_document(&path.to_string_lossy(), &content)?;
            }
            crate::watcher::WatchEvent::Removed(_) => {}
        }
        Ok(())
    }
}
impl VaultSession {
    pub fn new(vault_id: String, vault_path: PathBuf) -> Self {
        let view_state_manager = ViewStateManager::new(vault_path.clone());
        let indexer = Indexer::new(vault_path.join(".nabu/index")).expect("Failed to initialize indexer");
        Self {
            vault_id,
            vault_path,
            view_state_manager,
            indexer,
            is_active: true,
        }
    }
}

pub struct VaultService {
    pub sessions: HashMap<PathBuf, VaultSession>,
    pub graph: crate::graph::GraphEngine,
}

impl VaultService {
    pub fn open(path: &Path, _settings: crate::settings::SettingsStore) -> Result<Self> {
        let mut service = VaultService::default();
        let session = VaultSession::new("temp-id".into(), path.to_path_buf());
        service.sessions.insert(path.to_path_buf(), session);
        Ok(service)
    }

    pub fn scan(&self) -> Result<VaultScanResult> {
        Ok(VaultScanResult { path: "".into(), files: vec![] })
    }

    pub fn create_folder(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    pub fn move_item(&self, _from: &str, _to: &str) -> Result<()> {
        Ok(())
    }

    pub fn delete_folder(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    pub fn start_watching(&mut self, path: PathBuf) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let _watcher = crate::watcher::Watcher::new(path.clone(), tx)?;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // ...
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
            .map(|s| s.indexer.get_backlinks(note_path))
            .unwrap_or_default()
    }

    pub fn get_graph_data(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.graph.nodes(),
            "edges": self.graph.edges()
        })
    }
}


impl Default for VaultService {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            graph: crate::graph::GraphEngine::default(),
        }
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
