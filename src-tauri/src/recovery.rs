//! # Recovery & Data Protection — Tauri Command Layer
//!
//! Completes Nabu's data-protection story with user-facing recovery tooling:
//!
//! - **Version history** — every save of a note captures an immutable snapshot
//!   under `.nabu/versions/<note-hash>/` (content files + a manifest). Versions
//!   can be listed, previewed, diffed, restored and duplicated.
//! - **Manual snapshots** — `snapshot_create` captures a version on demand.
//! - **Autosave feedback** — `note_save` persists note content and records a
//!   version; the frontend shows Saving…/Saved/Failed/Retrying.
//! - **Session restore** — workspace state (view mode, active note, cursor,
//!   scroll, sidebar toggles) is persisted to `.nabu/session.json`.
//! - **Crash recovery** — a `.running` marker is written at startup and removed
//!   on graceful exit. If it is still present on the next launch, a
//!   `.recovery_pending` marker is written so the UI can offer to restore the
//!   previous session instead of silently discarding it.
//!
//! ## Safety
//!
//! Every destructive flow snapshots the note *before* mutating it, and every
//! restore pushes an undoable [`HistoryEntry`] so a restore can be reversed.
//! Snapshot retention is bounded (`MAX_VERSIONS_PER_NOTE`); the newest versions
//! are always kept.

use nabu_core::history::HistoryOp;
use nabu_core::registry::context::ApplicationContext;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

use crate::settings::SettingsStore;

/// Snapshots retained per note (oldest pruned beyond this).
pub const MAX_VERSIONS_PER_NOTE: usize = 50;
/// Cap diff input size (lines per side) to bound the DP table memory.
const DIFF_MAX_LINES: usize = 1000;

// ── Paths & hashing ─────────────────────────────────────────────────

/// Resolves the configured vault path from the settings store.
fn vault_path(store: &SettingsStore) -> PathBuf {
    PathBuf::from(store.get().last_vault_path.trim())
}

fn nabu_dir(vault: &Path) -> PathBuf {
    vault.join(".nabu")
}

fn versions_root(vault: &Path) -> PathBuf {
    nabu_dir(vault).join("versions")
}

fn session_path(vault: &Path) -> PathBuf {
    nabu_dir(vault).join("session.json")
}

fn running_marker(vault: &Path) -> PathBuf {
    nabu_dir(vault).join(".running")
}

fn pending_marker(vault: &Path) -> PathBuf {
    nabu_dir(vault).join(".recovery_pending")
}

/// Deterministic FNV-1a 64-bit hash rendered as hex — used to build a stable
/// per-note version directory without external dependencies.
fn stable_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// Resolves a vault-relative path *without* requiring the target to exist
/// (unlike `validate_path_within_vault`, which canonicalises). Rejects any
/// traversal components so writes can never escape the vault.
fn resolve_in_vault(vault: &Path, user_path: &str) -> Result<PathBuf, String> {
    let trimmed = user_path.trim().trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Err("Empty path".to_string());
    }
    for component in Path::new(trimmed).components() {
        if matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        ) {
            return Err(format!("Path traversal detected: {}", user_path));
        }
    }
    Ok(vault.join(trimmed))
}

// ── Version model ───────────────────────────────────────────────────

/// Metadata describing one captured snapshot of a note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    /// Snapshot id — the millisecond timestamp used as the file stem.
    pub id: String,
    /// When the snapshot was captured (RFC 3339).
    pub created_at: String,
    /// Raw byte size of the content.
    pub size: usize,
    /// Character count of the content.
    pub char_count: usize,
    /// First heading / first non-empty line, when available.
    pub summary: Option<String>,
    /// True when created via the manual "Snapshot" action.
    pub manual: bool,
    /// Future-compatible author attribution.
    #[serde(default)]
    pub author: Option<String>,
}

/// Manifest for one note: the original (vault-relative) path plus its
/// snapshots, newest last.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub path: String,
    pub versions: Vec<VersionMeta>,
}

fn note_versions_dir(vault: &Path, rel_path: &str) -> PathBuf {
    versions_root(vault).join(stable_hash(rel_path))
}

fn version_file(vault: &Path, rel_path: &str, id: &str) -> PathBuf {
    note_versions_dir(vault, rel_path).join(format!("{id}.md"))
}

fn read_manifest(vault: &Path, rel_path: &str) -> VersionManifest {
    std::fs::read(note_versions_dir(vault, rel_path).join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| VersionManifest {
            path: rel_path.to_string(),
            versions: vec![],
        })
}

fn write_manifest(vault: &Path, manifest: &VersionManifest) -> Result<(), String> {
    let dir = note_versions_dir(vault, &manifest.path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("manifest.json"), json).map_err(|e| e.to_string())
}

/// Records a snapshot of a note's current on-disk content.
///
/// Returns `Ok(true)` when a new version was captured, `Ok(false)` when the
/// content is unchanged (or the file is missing). Retention prunes the oldest
/// snapshots beyond [`MAX_VERSIONS_PER_NOTE`].
pub fn snapshot_note(vault: &Path, rel_path: &str) -> Result<bool, String> {
    let abs = resolve_in_vault(vault, rel_path)?;
    if !abs.is_file() {
        return Ok(false);
    }
    let content = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;

    let mut manifest = read_manifest(vault, rel_path);
    // Skip when identical to the most recent snapshot (avoids autosave spam).
    if let Some(last) = manifest.versions.last() {
        if let Ok(prev) = std::fs::read_to_string(version_file(vault, rel_path, &last.id)) {
            if prev == content {
                return Ok(false);
            }
        }
    }

    let id = chrono::Utc::now().timestamp_millis().to_string();
    let summary = content
        .trim()
        .lines()
        .next()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(80).collect());

    let meta = VersionMeta {
        id: id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        size: content.len(),
        char_count: content.chars().count(),
        summary,
        manual: false,
        author: None,
    };

    // Persist content first, then the manifest (a crash between the two leaves
    // a manifest that simply points at a missing file, which is handled
    // gracefully by versions_get).
    if let Some(parent) = version_file(vault, rel_path, &id).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(version_file(vault, rel_path, &id), &content).map_err(|e| e.to_string())?;

    manifest.versions.push(meta);
    while manifest.versions.len() > MAX_VERSIONS_PER_NOTE {
        let old = manifest.versions.remove(0);
        let _ = std::fs::remove_file(version_file(vault, rel_path, &old.id));
    }
    write_manifest(vault, &manifest)?;
    Ok(true)
}

// ── Diff ────────────────────────────────────────────────────────────

/// Kind of a single diff row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

/// One row of a side-by-side line diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffRow {
    pub kind: DiffKind,
    /// Line number in the older document (None for additions).
    pub old_line: Option<u32>,
    /// Line number in the newer document (None for removals).
    pub new_line: Option<u32>,
    pub text: String,
}

/// Longest-common-subsequence line diff between two documents.
fn line_diff(old: &str, new: &str) -> Vec<DiffRow> {
    let a: Vec<&str> = old.lines().take(DIFF_MAX_LINES).collect();
    let b: Vec<&str> = new.lines().take(DIFF_MAX_LINES).collect();
    let (n, m) = (a.len(), b.len());

    // DP table of LCS lengths.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            rows.push(DiffRow {
                kind: DiffKind::Same,
                old_line: Some(i as u32 + 1),
                new_line: Some(j as u32 + 1),
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            rows.push(DiffRow {
                kind: DiffKind::Removed,
                old_line: Some(i as u32 + 1),
                new_line: None,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            rows.push(DiffRow {
                kind: DiffKind::Added,
                old_line: None,
                new_line: Some(j as u32 + 1),
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        rows.push(DiffRow {
            kind: DiffKind::Removed,
            old_line: Some(i as u32 + 1),
            new_line: None,
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        rows.push(DiffRow {
            kind: DiffKind::Added,
            old_line: None,
            new_line: Some(j as u32 + 1),
            text: b[j].to_string(),
        });
        j += 1;
    }
    rows
}

// ── Session model ───────────────────────────────────────────────────

/// Workspace state persisted between sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Schema version for forward compatibility.
    pub version: u32,
    /// When the session was saved (RFC 3339).
    #[serde(default)]
    pub saved_at: Option<String>,
    /// Active view mode (editor / graph / trash / history / …).
    #[serde(default)]
    pub view_mode: Option<String>,
    /// The note that was open.
    #[serde(default)]
    pub active_note: Option<String>,
    /// Open tab paths.
    #[serde(default)]
    pub open_tabs: Vec<String>,
    /// Simple split-pane layout descriptor (future: window layout).
    #[serde(default)]
    pub split_panes: Vec<String>,
    /// Cursor offset in the active editor.
    #[serde(default)]
    pub cursor_pos: Option<u32>,
    /// Scroll offset of the active editor.
    #[serde(default)]
    pub scroll_top: Option<u32>,
    /// Whether the left sidebar was visible.
    #[serde(default)]
    pub left_sidebar: Option<bool>,
    /// Whether the right inspector was visible.
    #[serde(default)]
    pub right_inspector: Option<bool>,
    /// Future: serialized window layout.
    #[serde(default)]
    pub window_layout: Option<String>,
}

/// Result of a crash check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatus {
    /// True when the previous run did not exit cleanly.
    pub crashed: bool,
    /// True when a saved session exists to restore.
    pub has_session: bool,
    /// The saved session, when present.
    pub session: Option<SessionState>,
}

fn read_session(vault: &Path) -> Option<SessionState> {
    std::fs::read(session_path(vault))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

// ── Crash lifecycle (called from lib.rs) ────────────────────────────

/// Called at app startup. When a `.running` marker already exists the previous
/// run crashed; write a `.recovery_pending` marker so the UI can offer
/// restoration, then (re)write the running marker.
pub fn mark_running(vault: &Path) {
    if !vault.is_dir() {
        return;
    }
    let nabu = nabu_dir(vault);
    if std::fs::create_dir_all(&nabu).is_err() {
        return;
    }
    if running_marker(vault).exists() {
        let _ = std::fs::write(
            pending_marker(vault),
            chrono::Utc::now().to_rfc3339(),
        );
    }
    let _ = std::fs::write(running_marker(vault), "");
}

/// Called on graceful app exit — removes the running marker so the next
/// launch knows the previous session ended cleanly.
pub fn mark_clean_exit(vault: &Path) {
    let _ = std::fs::remove_file(running_marker(vault));
}

// ── Commands ────────────────────────────────────────────────────────

/// Saves note content to disk and records a version snapshot. The autosave
/// path — never pushes a history entry (typing would flood the undo stack).
#[tauri::command]
pub fn note_save(
    path: String,
    content: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let vault = vault_path(&store);
    let abs = resolve_in_vault(&vault, &path)?;
    if let Some(parent) = abs.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(&abs, &content).map_err(|e| e.to_string())?;
    let _ = snapshot_note(&vault, &path);
    Ok(())
}

/// Reads a note's current content (empty string when the note does not exist).
#[tauri::command]
pub fn note_read(path: String, store: State<'_, SettingsStore>) -> Result<String, String> {
    let vault = vault_path(&store);
    let abs = resolve_in_vault(&vault, &path)?;
    if !abs.is_file() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&abs).map_err(|e| e.to_string())
}

/// Lists the snapshots for a note, oldest first.
#[tauri::command]
pub fn versions_list(
    path: String,
    store: State<'_, SettingsStore>,
) -> Result<Vec<VersionMeta>, String> {
    let vault = vault_path(&store);
    let mut manifest = read_manifest(&vault, &path);
    manifest.versions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(manifest.versions)
}

/// Returns the content of a specific snapshot.
#[tauri::command]
pub fn versions_get(
    path: String,
    id: String,
    store: State<'_, SettingsStore>,
) -> Result<String, String> {
    let vault = vault_path(&store);
    let file = version_file(&vault, &path, &id);
    if !file.is_file() {
        return Err(format!("Snapshot {} not found", id));
    }
    std::fs::read_to_string(&file).map_err(|e| e.to_string())
}

/// Restores a snapshot over the live note. The current content is snapshotted
/// first (so it is never lost) and an undoable history entry is pushed.
#[tauri::command]
pub fn versions_restore(
    path: String,
    id: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let vault = vault_path(&store);
    let _ = snapshot_note(&vault, &path);

    let file = version_file(&vault, &path, &id);
    if !file.is_file() {
        return Err(format!("Snapshot {} not found", id));
    }
    let version_content = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;

    let abs = resolve_in_vault(&vault, &path)?;
    let previous = std::fs::read_to_string(&abs).unwrap_or_default();
    if let Some(parent) = abs.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(&abs, &version_content).map_err(|e| e.to_string())?;

    let undo_abs = abs.clone();
    let redo_abs = abs.clone();
    let undo_prev = previous.clone();
    let redo_content = version_content.clone();
    crate::history::push_history(
        &ctx,
        HistoryOp::Editor,
        format!("Restore '{}' from snapshot", path),
        vec![path.clone()],
        serde_json::json!({ "path": path, "content": previous }),
        serde_json::json!({ "path": path, "content": version_content }),
        Arc::new(move || {
            std::fs::write(&undo_abs, &undo_prev).map_err(|e| e.to_string())?;
            Ok(())
        }),
        Arc::new(move || {
            std::fs::write(&redo_abs, &redo_content).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Copies a snapshot to a new note path.
#[tauri::command]
pub fn versions_duplicate(
    path: String,
    id: String,
    dest: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let vault = vault_path(&store);
    let file = version_file(&vault, &path, &id);
    if !file.is_file() {
        return Err(format!("Snapshot {} not found", id));
    }
    let content = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;

    let dest_abs = resolve_in_vault(&vault, &dest)?;
    if dest_abs.exists() {
        return Err(format!("Destination already exists: {}", dest));
    }
    if let Some(parent) = dest_abs.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(&dest_abs, &content).map_err(|e| e.to_string())?;

    let undo_dest = dest_abs.clone();
    let redo_dest = dest_abs.clone();
    let redo_content = content.clone();
    crate::history::push_history(
        &ctx,
        HistoryOp::NoteDuplicate,
        format!("Duplicate '{}' snapshot to '{}'", path, dest),
        vec![dest.clone()],
        serde_json::json!({ "dest": dest, "exists": false }),
        serde_json::json!({ "dest": dest, "exists": true }),
        Arc::new(move || {
            let _ = std::fs::remove_file(&undo_dest);
            Ok(())
        }),
        Arc::new(move || {
            std::fs::write(&redo_dest, &redo_content).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Computes a line diff between two snapshots (or a snapshot and the live
/// note when `id_b` is `None`).
#[tauri::command]
pub fn versions_diff(
    path: String,
    id_a: Option<String>,
    id_b: Option<String>,
    store: State<'_, SettingsStore>,
) -> Result<Vec<DiffRow>, String> {
    let vault = vault_path(&store);
    let read = |id: Option<String>| -> Result<String, String> {
        match id {
            Some(id) => {
                let file = version_file(&vault, &path, &id);
                if !file.is_file() {
                    return Err(format!("Snapshot {} not found", id));
                }
                std::fs::read_to_string(&file).map_err(|e| e.to_string())
            }
            None => {
                let abs = resolve_in_vault(&vault, &path)?;
                if abs.is_file() {
                    std::fs::read_to_string(&abs).map_err(|e| e.to_string())
                } else {
                    Ok(String::new())
                }
            }
        }
    };
    let a = read(id_a)?;
    let b = read(id_b)?;
    Ok(line_diff(&a, &b))
}

/// Captures a manual snapshot of a note.
#[tauri::command]
pub fn snapshot_create(
    path: String,
    store: State<'_, SettingsStore>,
) -> Result<VersionMeta, String> {
    let vault = vault_path(&store);
    let abs = resolve_in_vault(&vault, &path)?;
    if !abs.is_file() {
        return Err(format!("Note does not exist: {}", path));
    }
    let content = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    let mut manifest = read_manifest(&vault, &path);

    let id = chrono::Utc::now().timestamp_millis().to_string();
    let meta = VersionMeta {
        id: id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        size: content.len(),
        char_count: content.chars().count(),
        summary: content
            .trim()
            .lines()
            .next()
            .map(|l| l.trim().trim_start_matches('#').trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.chars().take(80).collect()),
        manual: true,
        author: None,
    };
    if let Some(parent) = version_file(&vault, &path, &id).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(version_file(&vault, &path, &id), &content).map_err(|e| e.to_string())?;
    manifest.versions.push(meta.clone());
    while manifest.versions.len() > MAX_VERSIONS_PER_NOTE {
        let old = manifest.versions.remove(0);
        let _ = std::fs::remove_file(version_file(&vault, &path, &old.id));
    }
    write_manifest(&vault, &manifest)?;
    Ok(meta)
}

/// Summary of one note's snapshot set — used by the Snapshot Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub path: String,
    pub version_count: usize,
    pub last_snapshot_at: Option<String>,
}

/// Lists every note that has snapshots (Snapshot Browser).
#[tauri::command]
pub fn versions_all(store: State<'_, SettingsStore>) -> Result<Vec<NoteSummary>, String> {
    let vault = vault_path(&store);
    let root = versions_root(&vault);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(vec![]);
    };
    let mut summaries = Vec::new();
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("manifest.json");
        let Ok(bytes) = std::fs::read(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_slice::<VersionManifest>(&bytes) else {
            continue;
        };
        let last = manifest
            .versions
            .iter()
            .max_by(|a, b| a.id.cmp(&b.id))
            .map(|v| v.created_at.clone());
        summaries.push(NoteSummary {
            path: manifest.path,
            version_count: manifest.versions.len(),
            last_snapshot_at: last,
        });
    }
    summaries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(summaries)
}

/// Persists the current workspace session.
#[tauri::command]
pub fn session_save(state: SessionState, store: State<'_, SettingsStore>) -> Result<(), String> {
    let vault = vault_path(&store);
    if vault.as_os_str().is_empty() {
        return Ok(());
    }
    let nabu = nabu_dir(&vault);
    std::fs::create_dir_all(&nabu).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    std::fs::write(session_path(&vault), json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Loads the persisted session, if any.
#[tauri::command]
pub fn session_load(store: State<'_, SettingsStore>) -> Result<Option<SessionState>, String> {
    let vault = vault_path(&store);
    Ok(read_session(&vault))
}

/// Clears the persisted session.
#[tauri::command]
pub fn session_clear(store: State<'_, SettingsStore>) -> Result<(), String> {
    let vault = vault_path(&store);
    let _ = std::fs::remove_file(session_path(&vault));
    Ok(())
}

/// Reports whether the previous run crashed and whether a session is available.
#[tauri::command]
pub fn recovery_check(store: State<'_, SettingsStore>) -> Result<RecoveryStatus, String> {
    let vault = vault_path(&store);
    let session = read_session(&vault);
    Ok(RecoveryStatus {
        crashed: pending_marker(&vault).exists(),
        has_session: session.is_some(),
        session,
    })
}

/// Clears the recovery-pending marker after the user restored or discarded
/// the previous session.
#[tauri::command]
pub fn recovery_discard(store: State<'_, SettingsStore>) -> Result<(), String> {
    let vault = vault_path(&store);
    let _ = std::fs::remove_file(pending_marker(&vault));
    Ok(())
}

// ── Public re-exports (Phase 13.3) ──────────────────────────────────
//
// The Comparison View's `notes_diff` command reuses the LCS line diff and
// vault-path helpers defined above. These thin public wrappers expose them
// to other modules without changing their internal visibility or callers.

/// Public wrapper around [`vault_path`] for cross-module reuse.
pub fn vault_path_pub(store: &SettingsStore) -> PathBuf {
    vault_path(store)
}

/// Public wrapper around [`resolve_in_vault`] for cross-module reuse.
pub fn resolve_in_vault_pub(vault: &Path, user_path: &str) -> Result<PathBuf, String> {
    resolve_in_vault(vault, user_path)
}

/// Public wrapper around [`line_diff`] for cross-module reuse.
pub fn line_diff_pub(old: &str, new: &str) -> Vec<DiffRow> {
    line_diff(old, new)
}
