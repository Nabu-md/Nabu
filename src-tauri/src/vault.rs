use std::{
    collections::HashMap,
    path::{Path, PathBuf},
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

pub struct VaultService {
    pub sessions: HashMap<PathBuf, crate::vault::VaultSession>,
}

impl VaultService {
    pub fn open(&mut self, path: PathBuf, _settings: crate::settings::SettingsStore) -> Result<()> {
        Ok(())
    }

    pub fn scan(&self, vault_path: &Path) -> Result<VaultScanResult> {
        Ok(VaultScanResult { path: vault_path.display().to_string(), files: vec![] })
    }
}

impl Default for VaultService {
    fn default() -> Self {
        Self { sessions: HashMap::new() }
    }
}
