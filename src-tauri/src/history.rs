//! # Universal Undo/Redo — Tauri Command Layer
//!
//! Bridges the frontend to the canonical [`HistoryManager`] registered in the
//! [`ApplicationContext`], and implements reversible filesystem operations
//! (create / rename / move / delete→trash / restore, folder ops).
//!
//! Every command that mutates the vault pushes a [`HistoryEntry`] whose undo /
//! redo closures capture exactly the paths and bytes needed to reverse the
//! operation without ever corrupting user files.

use nabu_core::history::{HistoryAction, HistoryEntry, HistoryManager, HistoryOp};
use nabu_core::registry::context::ApplicationContext;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::State;

use crate::commands::validate_path_within_vault;
use crate::settings::SettingsStore;

/// Resolves the canonical history manager from the application context.
pub(crate) fn get_history_manager(
    ctx: &ApplicationContext,
) -> Result<Arc<RwLock<HistoryManager>>, String> {
    ctx.history_manager()
        .ok_or_else(|| "HistoryManager is not registered in the application context".to_string())
}

/// Pushes a new reversible operation onto the history manager.
pub(crate) fn push_history(
    ctx: &ApplicationContext,
    op: HistoryOp,
    label: impl Into<String>,
    affected: Vec<String>,
    previous_state: serde_json::Value,
    new_state: serde_json::Value,
    undo: HistoryAction,
    redo: HistoryAction,
) -> Result<(), String> {
    let manager = get_history_manager(ctx)?;
    let entry = HistoryEntry::new(op, label, affected, previous_state, new_state, undo, redo);
    manager
        .write()
        .map_err(|e| e.to_string())?
        .push(entry);
    Ok(())
}

// ── History status / navigation ───────────────────────────────────────

/// Current undo/redo state, returned to the frontend for button states and
/// user feedback ("Nothing to undo" / "Nothing to redo").
#[derive(Debug, Clone, Serialize)]
pub struct HistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub undo_len: usize,
    pub redo_len: usize,
    pub max_depth: usize,
}

#[tauri::command]
pub fn history_status(ctx: State<'_, ApplicationContext>) -> Result<HistoryStatus, String> {
    let manager = get_history_manager(&ctx)?;
    let manager = manager.read().map_err(|e| e.to_string())?;
    Ok(HistoryStatus {
        can_undo: manager.can_undo(),
        can_redo: manager.can_redo(),
        undo_label: manager.undo_label(),
        redo_label: manager.redo_label(),
        undo_len: manager.undo_len(),
        redo_len: manager.redo_len(),
        max_depth: manager.max_depth(),
    })
}

/// Undoes the most recent operation. Returns the label of what was undone,
/// or `None` when there is nothing to undo.
#[tauri::command]
pub fn history_undo(ctx: State<'_, ApplicationContext>) -> Result<Option<String>, String> {
    let manager = get_history_manager(&ctx)?;
    manager.write().map_err(|e| e.to_string())?.undo()
}

/// Redoes the most recently undone operation. Returns the label of what was
/// redone, or `None` when there is nothing to redo.
#[tauri::command]
pub fn history_redo(ctx: State<'_, ApplicationContext>) -> Result<Option<String>, String> {
    let manager = get_history_manager(&ctx)?;
    manager.write().map_err(|e| e.to_string())?.redo()
}

/// Clears all history (used on vault switch / workspace invalidation).
#[tauri::command]
pub fn history_clear(ctx: State<'_, ApplicationContext>) -> Result<(), String> {
    let manager = get_history_manager(&ctx)?;
    manager.write().map_err(|e| e.to_string())?.clear();
    Ok(())
}

/// Sets the maximum history depth.
#[tauri::command]
pub fn history_set_depth(
    ctx: State<'_, ApplicationContext>,
    depth: usize,
) -> Result<(), String> {
    let manager = get_history_manager(&ctx)?;
    manager
        .write()
        .map_err(|e| e.to_string())?
        .set_max_depth(depth);
    Ok(())
}

// ── Trash helpers ─────────────────────────────────────────────────────

/// Returns the trash directory inside the vault (`.nabu/trash`).
fn trash_dir(vault_path: &Path) -> PathBuf {
    vault_path.join(".nabu").join("trash")
}

/// A manifest record mapping a trashed file back to its original location.
///
/// Public because it is returned to the frontend by [`trash_list`].
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct TrashRecord {
    /// The current location of the trashed file.
    pub trash_path: String,
    /// The original location the file should be restored to.
    pub original_path: String,
}

fn trash_manifest_path(vault_path: &Path) -> PathBuf {
    trash_dir(vault_path).join("manifest.json")
}

fn read_trash_manifest(vault_path: &Path) -> Vec<TrashRecord> {
    std::fs::read(trash_manifest_path(vault_path))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_trash_manifest(vault_path: &Path, records: &[TrashRecord]) -> Result<(), String> {
    let path = trash_manifest_path(vault_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(records).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Moves `src` into the vault trash, recording the original location so the
/// operation can be reversed (and survives application restart).
fn trash_file(vault_path: &Path, src: &Path) -> Result<PathBuf, String> {
    let dir = trash_dir(vault_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".to_string());
    let stamp = chrono::Utc::now().timestamp_millis();
    let dest = dir.join(format!("{}-{}", stamp, file_name));

    // Avoid collisions by appending a counter if needed.
    let mut candidate = dest.clone();
    let mut i = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{}-{}-{}", stamp, i, file_name));
        i += 1;
    }

    // Record the mapping *before* moving the file so a manifest write failure
    // never strands the file in trash with no record (which would make the
    // note appear deleted and unrecoverable). If the rename itself fails, the
    // just-written record is rolled back.
    let mut records = read_trash_manifest(vault_path);
    records.push(TrashRecord {
        trash_path: candidate.display().to_string(),
        original_path: src.display().to_string(),
    });
    write_trash_manifest(vault_path, &records)?;

    if let Err(e) = std::fs::rename(src, &candidate) {
        // Roll back the record we just wrote — the file was not moved.
        let remaining: Vec<TrashRecord> = read_trash_manifest(vault_path)
            .into_iter()
            .filter(|r| Path::new(&r.trash_path) != candidate)
            .collect();
        let _ = write_trash_manifest(vault_path, &remaining);
        return Err(e.to_string());
    }
    Ok(candidate)
}

/// Restores a trashed file to its original location (looked up by the
/// manifest record whose `trash_path` matches).
fn restore_from_trash(vault_path: &Path, trash_path: &Path) -> Result<PathBuf, String> {
    let records = read_trash_manifest(vault_path);
    let record = records
        .iter()
        .find(|r| Path::new(&r.trash_path) == trash_path)
        .ok_or_else(|| "Trash record not found".to_string())?;

    let original = PathBuf::from(&record.original_path);
    if let Some(parent) = original.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(trash_path, &original).map_err(|e| e.to_string())?;

    let remaining: Vec<TrashRecord> = records
        .into_iter()
        .filter(|r| Path::new(&r.trash_path) != trash_path)
        .collect();
    write_trash_manifest(vault_path, &remaining)?;
    Ok(original)
}

/// Restores a trashed file by its *original* location — resolves the current
/// trash record from the manifest so undo/redo can cycle indefinitely even
/// though each trashing produces a fresh timestamped trash name.
fn restore_by_original(vault_path: &Path, original_path: &Path) -> Result<(), String> {
    let records = read_trash_manifest(vault_path);
    let record = records
        .iter()
        .find(|r| Path::new(&r.original_path) == original_path)
        .cloned()
        .ok_or_else(|| "Trash record not found for original path".to_string())?;

    let trash = PathBuf::from(&record.trash_path);
    if let Some(parent) = original_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&trash, original_path).map_err(|e| e.to_string())?;

    let remaining: Vec<TrashRecord> = records
        .into_iter()
        .filter(|r| Path::new(&r.trash_path) != trash)
        .collect();
    write_trash_manifest(vault_path, &remaining)?;
    Ok(())
}

// ── Reversible filesystem operations ──────────────────────────────────

/// Renames (or moves) a file inside the vault and registers an undoable
/// history entry. `from` and `to` are vault-relative paths.
#[tauri::command]
pub fn note_rename(
    from: String,
    to: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let from_path = validate_path_within_vault(&vault_path, &from)?;
    let to_path = validate_path_within_vault(&vault_path, &to)?;

    if !from_path.exists() {
        return Err(format!("Source file does not exist: {}", from));
    }
    if to_path.exists() {
        return Err(format!("Destination already exists: {}", to));
    }
    if let Some(parent) = to_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::rename(&from_path, &to_path).map_err(|e| e.to_string())?;

    let undo_from = from_path.clone();
    let undo_to = to_path.clone();
    let redo_from = from_path;
    let redo_to = to_path;

    push_history(
        &ctx,
        HistoryOp::NoteRename,
        format!("Rename Note to '{}'", to),
        vec![from.clone(), to.clone()],
        serde_json::json!({ "from": from }),
        serde_json::json!({ "to": to }),
        // Undo: rename back.
        Arc::new(move || {
            std::fs::rename(&undo_to, &undo_from).map_err(|e| e.to_string())?;
            Ok(())
        }),
        // Redo: rename forward again.
        Arc::new(move || {
            std::fs::rename(&redo_from, &redo_to).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Deletes a note by moving it into the vault trash (reversible, never
/// destroys user data). Registers an undoable history entry.
#[tauri::command]
pub fn note_delete(
    path: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let safe_path = validate_path_within_vault(&vault_path, &path)?;
    if !safe_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    let _trash = trash_file(&vault_path, &safe_path)?;
    let undo_vault = vault_path.clone();
    let redo_vault = vault_path;
    let undo_path = safe_path.clone();
    let redo_path = safe_path;

    push_history(
        &ctx,
        HistoryOp::NoteDelete,
        format!("Delete Note '{}'", path),
        vec![path.clone()],
        serde_json::json!({ "path": path, "trashed": false }),
        serde_json::json!({ "path": path, "trashed": true }),
        // Undo: restore the note from trash (resolved by original path).
        Arc::new(move || {
            restore_by_original(&undo_vault, &undo_path)?;
            Ok(())
        }),
        // Redo: trash the note again (fresh timestamped name each time).
        Arc::new(move || {
            trash_file(&redo_vault, &redo_path)?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Restores a trashed file back to its original location. Registers an
/// undoable history entry.
#[tauri::command]
pub fn note_restore(
    trash_path: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let trash = PathBuf::from(&trash_path);
    if !trash.exists() {
        return Err(format!("Trash item does not exist: {}", trash_path));
    }

    let original = restore_from_trash(&vault_path, &trash)?;
    let label = format!(
        "Restore '{}'",
        original
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    );
    let undo_vault = vault_path.clone();
    let redo_vault = vault_path;
    let undo_original = original.clone();
    let redo_original = original;

    push_history(
        &ctx,
        HistoryOp::NoteRestore,
        label,
        vec![trash_path.clone()],
        serde_json::json!({ "restored": false }),
        serde_json::json!({ "restored": true }),
        // Undo: trash it again (fresh timestamped name).
        Arc::new(move || {
            trash_file(&undo_vault, &undo_original)?;
            Ok(())
        }),
        // Redo: restore it again (resolved by original path).
        Arc::new(move || {
            restore_by_original(&redo_vault, &redo_original)?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Lists the current contents of the vault trash.
#[tauri::command]
pub fn trash_list(store: State<'_, SettingsStore>) -> Result<Vec<TrashRecord>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    Ok(read_trash_manifest(&vault_path))
}

/// Permanently empties the vault trash (clears files + manifest).
#[tauri::command]
pub fn trash_empty(store: State<'_, SettingsStore>) -> Result<usize, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let records = read_trash_manifest(&vault_path);
    let count = records.len();
    for record in &records {
        let _ = std::fs::remove_file(Path::new(&record.trash_path));
    }
    write_trash_manifest(&vault_path, &[])?;
    Ok(count)
}

/// Creates a folder inside the vault and registers an undoable entry.
#[tauri::command]
pub fn folder_create(
    path: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let safe_path = validate_path_within_vault(&vault_path, &path)?;
    if safe_path.exists() {
        return Err(format!("Folder already exists: {}", path));
    }
    std::fs::create_dir_all(&safe_path).map_err(|e| e.to_string())?;

    let undo_path = safe_path.clone();
    let redo_path = safe_path;
    push_history(
        &ctx,
        HistoryOp::FolderCreate,
        format!("Create Folder '{}'", path),
        vec![path.clone()],
        serde_json::json!({ "path": path, "exists": false }),
        serde_json::json!({ "path": path, "exists": true }),
        // Undo: remove the (empty) folder.
        Arc::new(move || {
            if undo_path.is_dir() && std::fs::read_dir(&undo_path).map(|mut d| d.next().is_none()).unwrap_or(false) {
                std::fs::remove_dir(&undo_path).map_err(|e| e.to_string())?;
            }
            Ok(())
        }),
        // Redo: recreate the folder.
        Arc::new(move || {
            std::fs::create_dir_all(&redo_path).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Renames a folder and registers an undoable entry.
#[tauri::command]
pub fn folder_rename(
    from: String,
    to: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let from_path = validate_path_within_vault(&vault_path, &from)?;
    let to_path = validate_path_within_vault(&vault_path, &to)?;
    if !from_path.is_dir() {
        return Err(format!("Source folder does not exist: {}", from));
    }
    if to_path.exists() {
        return Err(format!("Destination already exists: {}", to));
    }
    std::fs::rename(&from_path, &to_path).map_err(|e| e.to_string())?;

    let undo_from = from_path.clone();
    let undo_to = to_path.clone();
    let redo_from = from_path;
    let redo_to = to_path;

    push_history(
        &ctx,
        HistoryOp::FolderRename,
        format!("Rename Folder to '{}'", to),
        vec![from.clone(), to.clone()],
        serde_json::json!({ "from": from }),
        serde_json::json!({ "to": to }),
        Arc::new(move || {
            std::fs::rename(&undo_to, &undo_from).map_err(|e| e.to_string())?;
            Ok(())
        }),
        Arc::new(move || {
            std::fs::rename(&redo_from, &redo_to).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

