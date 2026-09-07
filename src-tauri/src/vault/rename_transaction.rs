use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct RenameTransaction {
    old_path: String,
    new_path: String,
    backup_path: String,
}

pub(super) struct RenameWorkspace {
    dir: PathBuf,
}

impl RenameWorkspace {
    pub(super) fn new(vault: &Path) -> Result<Self, String> {
        let dir = vault.join(".nabu-rename-txn");
        fs::create_dir_all(&dir).map_err(|e| {
            format!(
                "Failed to create rename transaction dir {}: {}",
                dir.display(),
                e
            )
        })?;
        Ok(Self { dir })
    }

    pub(super) fn stage_note_content(&self, content: &str) -> Result<NamedTempFile, String> {
        let mut staged = NamedTempFile::new_in(&self.dir)
            .map_err(|e| format!("Failed to create staged rename file: {}", e))?;
        staged
            .write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write staged rename file: {}", e))?;
        staged
            .as_file_mut()
            .sync_all()
            .map_err(|e| format!("Failed to sync staged rename file: {}", e))?;
        Ok(staged)
    }

    pub(super) fn operation<'a>(
        &self,
        old_path: &'a str,
        old_file: &'a Path,
    ) -> RenameOperation<'a> {
        RenameOperation {
            old_path,
            old_file,
            transaction_dir: self.dir.clone(),
            backup_path: self.dir.join(format!("{}.bak", Uuid::new_v4())),
            manifest_path: self.dir.join(format!("{}.json", Uuid::new_v4())),
        }
    }
}

impl Drop for RenameWorkspace {
    fn drop(&mut self) {
        remove_empty_transaction_dir(&self.dir);
    }
}

pub(super) struct CommittedRename {
    new_file: PathBuf,
    manifest_path: PathBuf,
    backup_path: PathBuf,
    transaction_dir: PathBuf,
}

impl CommittedRename {
    pub(super) fn new_file(&self) -> &Path {
        &self.new_file
    }
}

impl Drop for CommittedRename {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.backup_path);
        let _ = fs::remove_file(&self.manifest_path);
        remove_empty_transaction_dir(&self.transaction_dir);
    }
}

pub(super) struct RenameOperation<'a> {
    old_path: &'a str,
    old_file: &'a Path,
    transaction_dir: PathBuf,
    backup_path: PathBuf,
    manifest_path: PathBuf,
}

impl<'a> RenameOperation<'a> {
    pub(super) fn rename_with_candidates(
        &self,
        staged: NamedTempFile,
        desired_filename: &str,
        parent_dir: &Path,
    ) -> Result<CommittedRename, String> {
        let mut staged = staged;
        for attempt in 0.. {
            let candidate = parent_dir.join(candidate_filename(desired_filename, attempt));
            self.prepare(&candidate)?;

            match staged.persist_noclobber(&candidate) {
                Ok(_) => return Ok(self.committed(candidate)),
                Err(err) if err.error.kind() == ErrorKind::AlreadyExists => {
                    staged = err.file;
                    self.rollback()?;
                }
                Err(err) => {
                    self.rollback()?;
                    return Err(format!(
                        "Failed to create {}: {}",
                        candidate.display(),
                        err.error
                    ));
                }
            }
        }
        unreachable!()
    }

    pub(super) fn rename_exact(
        &self,
        staged: NamedTempFile,
        new_file: &Path,
    ) -> Result<CommittedRename, String> {
        self.prepare(new_file)?;
        match staged.persist_noclobber(new_file) {
            Ok(_) => Ok(self.committed(new_file.to_path_buf())),
            Err(err) if err.error.kind() == ErrorKind::AlreadyExists => {
                self.rollback()?;
                Err("A note with that name already exists".to_string())
            }
            Err(err) => {
                self.rollback()?;
                Err(format!(
                    "Failed to rename {} to {}: {}",
                    self.old_path,
                    new_file.to_string_lossy(),
                    err.error
                ))
            }
        }
    }

    fn prepare(&self, new_file: &Path) -> Result<(), String> {
        self.write_manifest(new_file)?;
        self.move_into_backup()
    }

    fn write_manifest(&self, new_file: &Path) -> Result<(), String> {
        let transaction = RenameTransaction {
            old_path: self.old_path.to_string(),
            new_path: new_file.to_string_lossy().to_string(),
            backup_path: self.backup_path.to_string_lossy().to_string(),
        };
        let data = serde_json::to_string(&transaction)
            .map_err(|e| format!("Failed to serialize rename transaction: {}", e))?;
        fs::write(&self.manifest_path, data).map_err(|e| {
            format!(
                "Failed to write rename transaction {}: {}",
                self.manifest_path.display(),
                e
            )
        })
    }

    fn move_into_backup(&self) -> Result<(), String> {
        fs::rename(self.old_file, &self.backup_path).map_err(|e| {
            format!(
                "Failed to move {} into rename backup {}: {}",
                self.old_file.display(),
                self.backup_path.display(),
                e
            )
        })
    }

    fn rollback(&self) -> Result<(), String> {
        if self.backup_path.exists() {
            if let Some(parent) = self.old_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&self.backup_path, self.old_file).map_err(|e| {
                format!(
                    "Failed to restore {} from {}: {}",
                    self.old_file.display(),
                    self.backup_path.display(),
                    e
                )
            })?;
        }
        let _ = fs::remove_file(&self.manifest_path);
        remove_empty_transaction_dir(&self.transaction_dir);
        Ok(())
    }

    fn committed(&self, new_file: PathBuf) -> CommittedRename {
        CommittedRename {
            new_file,
            manifest_path: self.manifest_path.clone(),
            backup_path: self.backup_path.clone(),
            transaction_dir: self.transaction_dir.clone(),
        }
    }
}

fn candidate_filename(filename: &str, attempt: usize) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = Path::new(filename)
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    if attempt == 0 {
        return filename.to_string();
    }
    format!("{}-{}{}", stem, attempt + 1, ext)
}

fn transaction_dir(vault: &Path) -> PathBuf {
    vault.join(".nabu-rename-txn")
}

fn remove_empty_transaction_dir(dir: &Path) {
    let _ = fs::remove_dir(dir);
}

pub(super) fn recover_pending_rename_transactions(vault: &Path) -> Result<(), String> {
    let txn_dir = transaction_dir(vault);
    if !txn_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(&txn_dir).map_err(|e| {
        format!(
            "Failed to read rename transaction dir {}: {}",
            txn_dir.display(),
            e
        )
    })?;

    for entry in entries {
        let path = entry
            .map_err(|e| format!("Failed to read rename transaction entry: {}", e))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let Ok(data) = fs::read_to_string(&path) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let Ok(transaction) = serde_json::from_str::<RenameTransaction>(&data) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        recover_rename_transaction(&path, transaction)?;
    }

    remove_empty_transaction_dir(&txn_dir);
    Ok(())
}

fn recover_rename_transaction(
    manifest_path: &Path,
    transaction: RenameTransaction,
) -> Result<(), String> {
    let old_path = Path::new(&transaction.old_path);
    let new_path = Path::new(&transaction.new_path);
    let backup_path = Path::new(&transaction.backup_path);

    if !backup_path.exists() {
        let _ = fs::remove_file(manifest_path);
        return Ok(());
    }

    if new_path.exists() || old_path.exists() {
        let _ = fs::remove_file(backup_path);
        let _ = fs::remove_file(manifest_path);
        return Ok(());
    }

    if let Some(parent) = old_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::rename(backup_path, old_path).map_err(|e| {
        format!(
            "Failed to recover {} from {}: {}",
            old_path.display(),
            backup_path.display(),
            e
        )
    })?;
    let _ = fs::remove_file(manifest_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn committed_rename_removes_empty_transaction_directory() {
        let vault = TempDir::new().unwrap();
        let old_file = vault.path().join("old.md");
        let new_file = vault.path().join("new.md");
        fs::write(&old_file, "# Old\n").unwrap();

        let workspace = RenameWorkspace::new(vault.path()).unwrap();
        let staged = workspace.stage_note_content("# New\n").unwrap();
        let operation = workspace.operation("old.md", &old_file);
        let committed = operation.rename_exact(staged, &new_file).unwrap();
        drop(committed);

        assert!(!transaction_dir(vault.path()).exists());
    }

    #[test]
    fn recovery_removes_empty_transaction_directory() {
        let vault = TempDir::new().unwrap();
        let txn_dir = transaction_dir(vault.path());
        fs::create_dir(&txn_dir).unwrap();

        recover_pending_rename_transactions(vault.path()).unwrap();

        assert!(!txn_dir.exists());
    }

    #[test]
    fn unused_workspace_removes_empty_transaction_directory() {
        let vault = TempDir::new().unwrap();
        let txn_dir = transaction_dir(vault.path());

        let workspace = RenameWorkspace::new(vault.path()).unwrap();
        drop(workspace);

        assert!(!txn_dir.exists());
    }
}
