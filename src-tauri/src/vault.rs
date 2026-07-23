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
    root: PathBuf,
    settings: std::sync::Arc<std::sync::Mutex<SettingsStore>>,
}

#[derive(Debug, Default)]
pub struct SettingsStore {
    pub theme: String,
    pub last_vault_path: String,
    pub recent_vaults: Vec<crate::models::RecentVaultEntry>,
}

impl VaultService {
    pub fn open<P: Into<PathBuf>>(root: P, settings: SettingsStore) -> Result<Self> {
        let root = root.into();
        if !root.exists() {
            anyhow::bail!(VaultError::InvalidPath(format!("vault missing: {}", root.display())));
        }
        if !root.is_dir() {
            anyhow::bail!(VaultError::InvalidPath(format!("vault not a directory: {}", root.display())));
        }

        Ok(Self {
            root,
            settings: std::sync::Arc::new(std::sync::Mutex::new(settings)),
        })
    }

    pub fn close(mut self) -> SettingsStore {
        let settings = self.settings.lock().unwrap().clone();
        self.root = PathBuf::new();
        settings
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn scan(&self) -> Result<VaultScanResult> {
        ensure_open(&self.root)?;

        let mut files = Vec::new();
        for entry in walkdir::WalkDir::new(&self.root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let metadata = match std::fs::metadata(path) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let mtime = match metadata.modified() {
                Ok(value) => value.duration_since(UNIX_EPOCH).map(|v| v.as_secs_f64()).unwrap_or(0.0),
                Err(_) => 0.0,
            };

            files.push(FileEntry {
                path: path.display().to_string(),
                name: entry.file_name().to_string_lossy().into_owned(),
                mtime,
            });
        }

        Ok(VaultScanResult {
            path: self.root.display().to_string(),
            files,
        })
    }

    pub fn create_file(&self, relative: &str, contents: &str) -> Result<FileEntry> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;

        if target.exists() {
            anyhow::bail!(VaultError::Conflict);
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&target, contents)?;

        Self::entry_from_path(target)
    }

    pub fn read_file(&self, relative: &str) -> Result<String> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;
        std::fs::read_to_string(target).map_err(|err| VaultError::ReadFile(err.to_string()).into())
    }

    pub fn update_file(&self, relative: &str, contents: &str) -> Result<FileEntry> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;

        if !target.exists() {
            anyhow::bail!(VaultError::InvalidPath(format!("file missing: {}", target.display())));
        }

        std::fs::write(&target, contents)?;

        Self::entry_from_path(target)
    }

    pub fn delete_file(&self, relative: &str) -> Result<()> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;
        std::fs::remove_file(target)?;
        Ok(())
    }

    pub fn rename_file(&self, relative: &str, target_relative: &str) -> Result<FileEntry> {
        ensure_open(&self.root)?;
        let source = resolve(&self.root, relative)?;
        let target = resolve(&self.root, target_relative)?;

        if !source.exists() {
            anyhow::bail!(VaultError::InvalidPath(format!("source missing: {}", source.display())));
        }
        if target.exists() {
            anyhow::bail!(VaultError::Conflict);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&source, &target)?;

        Self::entry_from_path(target)
    }

    pub fn create_folder(&self, relative: &str) -> Result<()> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;
        std::fs::create_dir_all(target)?;
        Ok(())
    }

    pub fn delete_folder(&self, relative: &str) -> Result<()> {
        ensure_open(&self.root)?;
        let target = resolve(&self.root, relative)?;
        std::fs::remove_dir_all(target)?;
        Ok(())
    }

    pub fn move_item(&self, source_relative: &str, target_relative: &str) -> Result<()> {
        ensure_open(&self.root)?;
        let source = resolve(&self.root, source_relative)?;
        let target = resolve(&self.root, target_relative)?;

        if !source.exists() {
            anyhow::bail!(VaultError::InvalidPath(format!("move source missing: {}", source.display())));
        }
        if target.exists() {
            anyhow::bail!(VaultError::Conflict);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&source, &target)?;
        Ok(())
    }

    pub fn settings(&self) -> std::sync::Arc<std::sync::Mutex<SettingsStore>> {
        self.settings.clone()
    }

    fn entry_from_path(path: PathBuf) -> Result<FileEntry> {
        let metadata = std::fs::metadata(&path).context("failed to stat file")?;
        let mtime = metadata
            .modified()
            .context("failed to read modified time")?
            .duration_since(UNIX_EPOCH)
            .map(|v| v.as_secs_f64())
            .unwrap_or(0.0);

        Ok(FileEntry {
            path: path.display().to_string(),
            name: path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default(),
            mtime,
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
        let service = VaultService::open(&root, settings).unwrap();

        service.create_file("notes/test.md", "hello").unwrap();
        service.update_file("notes/test.md", "world").unwrap();
        let contents = service.read_file("notes/test.md").unwrap();
        assert_eq!(contents, "world");

        let scan = service.scan().unwrap();
        assert_eq!(scan.files.len(), 1);

        service.delete_file("notes/test.md").unwrap();
        let scan_after = service.scan().unwrap();
        assert!(scan_after.files.is_empty());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn folders_and_move_success() {
        let root = temp_root();
        std::fs::create_dir_all(&root).unwrap();

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
