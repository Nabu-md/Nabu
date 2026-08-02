use nabu_core::models::{CustomPropertyValue, KnowledgeObject};
use nabu_core::registry::context::ApplicationContext;
use nabu_core::storage::StorageManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

use crate::settings::{AppSettings, SettingsStore};

// ── Security Utilities ──────────────────────────────────────

/// Validates that a file path is within the vault directory.
/// Prevents path traversal attacks by ensuring the resolved path
/// is a descendant of the vault path.
pub(crate) fn validate_path_within_vault(
    vault_path: &Path,
    user_path: &str,
) -> Result<PathBuf, String> {
    let resolved = vault_path.join(user_path);
    let canonical = resolved
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    let canonical_vault = vault_path
        .canonicalize()
        .map_err(|e| format!("Invalid vault path: {}", e))?;

    if !canonical.starts_with(&canonical_vault) {
        return Err(format!(
            "Path traversal detected: {} is outside vault directory {}",
            user_path,
            canonical_vault.display()
        ));
    }

    Ok(canonical)
}

/// Validates that a string input does not contain dangerous characters
/// or patterns that could be used for injection attacks.
fn validate_input_safe(input: &str, max_length: usize) -> Result<(), String> {
    if input.len() > max_length {
        return Err(format!(
            "Input exceeds maximum length of {} characters",
            max_length
        ));
    }

    // Check for null bytes
    if input.contains('\0') {
        return Err("Input contains null bytes".to_string());
    }

    // Check for path traversal patterns
    if input.contains("..") {
        return Err("Input contains path traversal pattern '..'".to_string());
    }

    // Check for null byte injection
    if input.contains('%') && input.contains('0') {
        return Err("Input contains potential URL encoding injection".to_string());
    }

    Ok(())
}

// ── Queue Types ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Unread,
    Reading,
    Completed,
    Archived,
}

impl Default for QueueStatus {
    fn default() -> Self {
        Self::Unread
    }
}

impl QueueStatus {
    fn label(self) -> &'static str {
        match self {
            QueueStatus::Unread => "unread",
            QueueStatus::Reading => "reading",
            QueueStatus::Completed => "completed",
            QueueStatus::Archived => "archived",
        }
    }

    fn from_label(label: &str) -> Self {
        match label {
            "reading" => QueueStatus::Reading,
            "completed" => QueueStatus::Completed,
            "archived" => QueueStatus::Archived,
            _ => QueueStatus::Unread,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePriority {
    Low,
    Normal,
    High,
}

impl Default for QueuePriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl QueuePriority {
    fn label(self) -> &'static str {
        match self {
            QueuePriority::Low => "low",
            QueuePriority::Normal => "normal",
            QueuePriority::High => "high",
        }
    }

    fn from_label(label: &str) -> Self {
        match label {
            "low" => QueuePriority::Low,
            "high" => QueuePriority::High,
            _ => QueuePriority::Normal,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub title: String,
    pub object_type: String,
    pub status: QueueStatus,
    pub priority: QueuePriority,
    pub progress: f32,
    pub source: String,
    pub modified_at: String,
    pub tags: Vec<String>,
    pub selected: bool,
}

// ── Inbox Types ──────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxStatus {
    Pending,
    Processing,
    Ready,
    Approved,
    Rejected,
    Failed,
}

impl Default for InboxStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub title: String,
    pub object_type: String,
    pub source: String,
    pub status: InboxStatus,
    pub mime_type: Option<String>,
    pub source_file: Option<String>,
    pub metadata: InboxMetadata,
    pub duplicate_info: Option<DuplicateInfo>,
    pub timeline_info: Option<TimelineInfo>,
    pub ocr_info: Option<OcrInfo>,
    pub processing_history: Vec<ProcessingHistoryEntry>,
    pub warnings: Vec<String>,
    pub selected: bool,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct InboxMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DuplicateInfo {
    pub confidence: String,
    pub candidate_ids: Vec<String>,
    pub reason: Option<String>,
    pub duplicate_source: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct TimelineInfo {
    pub document_date: Option<String>,
    pub created_date: Option<String>,
    pub modified_date: Option<String>,
    pub detected_event_date: Option<String>,
    pub extraction_confidence: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct OcrInfo {
    pub extracted_text: Option<String>,
    pub confidence: Option<f64>,
    pub recognition_language: Option<String>,
    pub page_count: Option<u32>,
    pub processing_duration_ms: Option<u64>,
    pub is_scanned: Option<bool>,
    pub warning: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProcessingHistoryEntry {
    pub processor_name: String,
    pub timestamp: String,
    pub duration_ms: u64,
    pub success: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

// ── Canonical storage access ────────────────────────────────────────

/// Resolves the single canonical StorageManager from the ApplicationContext.
///
/// R1: nothing constructs its own StorageManager / EventBus — every command
/// resolves the one registered at startup through dependency injection.
fn get_storage_manager(ctx: &ApplicationContext) -> Result<Arc<StorageManager>, String> {
    ctx.storage_manager()
        .ok_or_else(|| "StorageManager is not registered in the application context".to_string())
}

/// Reads a plain-text custom property from the canonical model.
fn custom_text(obj: &KnowledgeObject, key: &str) -> Option<String> {
    match obj.custom_properties.get(key) {
        Some(CustomPropertyValue::Text(s))
        | Some(CustomPropertyValue::Select(s))
        | Some(CustomPropertyValue::Url(s))
        | Some(CustomPropertyValue::Date(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Reads a JSON-encoded custom property (stored as a serialized Text value).
fn custom_json(obj: &KnowledgeObject, key: &str) -> Option<serde_json::Value> {
    custom_text(obj, key).and_then(|s| serde_json::from_str(&s).ok())
}

/// Writes an arbitrary JSON value into a custom property as serialized Text.
fn set_custom_json(obj: &mut KnowledgeObject, key: &str, value: &serde_json::Value) {
    obj.custom_properties.insert(
        key.to_string(),
        CustomPropertyValue::Text(serde_json::to_string(value).unwrap_or_default()),
    );
}

/// Sets a plain text custom property.
fn set_custom_text(obj: &mut KnowledgeObject, key: &str, value: &str) {
    obj.custom_properties.insert(
        key.to_string(),
        CustomPropertyValue::Text(value.to_string()),
    );
}

/// Reads a numeric custom property.
fn custom_number(obj: &KnowledgeObject, key: &str) -> Option<f64> {
    match obj.custom_properties.get(key) {
        Some(CustomPropertyValue::Number(n)) => Some(*n),
        _ => None,
    }
}

// ── File Tree Commands ────────────────────────────────────────────

/// One entry of the vault file tree returned to the frontend. `path` is
/// vault-relative (forward slashes) so the frontend can round-trip it to the
/// other file commands; hidden entries (`.nabu`, dotfiles) are skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_folder: bool,
    pub children: Vec<TreeEntry>,
}

/// Recursively scans `dir`, producing vault-relative [`TreeEntry`]s.
/// Hidden entries (leading `.`) are skipped so metadata dirs like `.nabu`
/// never appear in the user-facing tree.
fn scan_tree(dir: &Path, prefix: &str) -> Vec<TreeEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items: Vec<TreeEntry> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let is_folder = e.file_type().map(|t| t.is_dir()).unwrap_or_else(|_| {
                std::fs::metadata(e.path()).map(|m| m.is_dir()).unwrap_or(false)
            });
            let children = if is_folder {
                scan_tree(&e.path(), &path)
            } else {
                Vec::new()
            };
            TreeEntry {
                name,
                path,
                is_folder,
                children,
            }
        })
        .collect();
    // Folders first, then notes, each alphabetically — matches Finder/Explorer.
    items.sort_by(|a, b| {
        b.is_folder
            .cmp(&a.is_folder)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    items
}

/// Returns the vault's file tree (vault-relative paths). The left sidebar
/// renders this and drives drag-and-drop, context menus and inline rename.
#[tauri::command]
pub fn tree_list(store: State<'_, SettingsStore>) -> Result<Vec<TreeEntry>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    if vault_path.as_os_str().is_empty() || !vault_path.exists() {
        return Ok(Vec::new());
    }
    Ok(scan_tree(&vault_path, ""))
}

/// Reveals a vault-relative path in the operating system's file manager
/// (Finder / Explorer / the default file manager).
#[cfg(target_os = "macos")]
#[tauri::command]
pub fn reveal_in_file_manager(
    path: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let full = validate_path_within_vault(&vault_path, &path)?;
    std::process::Command::new("open")
        .arg("-R")
        .arg(&full)
        .status()
        .map_err(|e| format!("Could not reveal in Finder: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn reveal_in_file_manager(
    path: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let full = validate_path_within_vault(&vault_path, &path)?;
    std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&full)
        .spawn()
        .map_err(|e| format!("Could not reveal in Explorer: {e}"))?;
    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
#[tauri::command]
pub fn reveal_in_file_manager(
    path: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    use std::path::Path as P;
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);
    let full = validate_path_within_vault(&vault_path, &path)?;
    // `xdg-open` opens the parent directory for folders; for files, open the
    // containing directory (there is no portable "select" flag).
    let target = if full.is_dir() {
        full
    } else {
        full.parent().map(P::to_path_buf).unwrap_or(vault_path)
    };
    std::process::Command::new("xdg-open")
        .arg(target)
        .spawn()
        .map_err(|e| format!("Could not open file manager: {e}"))?;
    Ok(())
}

// ── Vault Commands ─────────────────────────────────────────────

#[tauri::command]
pub fn check_vault_exists(store: State<'_, SettingsStore>) -> Result<Option<String>, String> {
    let settings = store.get();
    let path = settings.last_vault_path.trim();
    if !path.is_empty() && Path::new(path).exists() {
        // Retention runs at app startup with a previously-configured vault, so
        // expired trashed items are purged even if the Trash screen is never
        // opened (matches `trash_purge_expired`'s "on vault load" contract).
        let _ = crate::history::trash_purge_expired(store.clone());
        Ok(Some(path.to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn get_current_vault(store: State<'_, SettingsStore>) -> Result<Option<String>, String> {
    check_vault_exists(store)
}

#[tauri::command]
pub fn select_vault_dialog(store: State<'_, SettingsStore>) -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("Select Vault Directory")
        .pick_folder();

    if let Some(path) = folder {
        let path_str = path.display().to_string();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        store
            .update(|s| {
                s.last_vault_path = path_str.clone();
                crate::settings::update_recent_vaults(s, path_str.clone(), name);
            })
            .map_err(|e| e.to_string())?;
        // Retention runs as soon as a vault becomes active, so expired trashed
        // items are purged even if the user never opens the Trash screen.
        let _ = crate::history::trash_purge_expired(store.clone());
        Ok(Some(path_str))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn create_vault_dialog(store: State<'_, SettingsStore>) -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("Select Directory for New Vault")
        .pick_folder();

    if let Some(path) = folder {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        let path_str = path.display().to_string();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        store
            .update(|s| {
                s.last_vault_path = path_str.clone();
                crate::settings::update_recent_vaults(s, path_str.clone(), name);
            })
            .map_err(|e| e.to_string())?;
        let _ = crate::history::trash_purge_expired(store.clone());
        Ok(Some(path_str))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn open_dictation_pill(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("dictation-pill") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        tauri::WebviewWindowBuilder::new(
            &app,
            "dictation-pill",
            tauri::WebviewUrl::App("dictation-pill.html".into()),
        )
        .title("Dictation Pill")
        .inner_size(260.0, 64.0)
        .resizable(false)
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn close_dictation_pill(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("dictation-pill") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_dictation_pill(app: AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("dictation-pill") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
            Ok(false)
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            Ok(true)
        }
    } else {
        open_dictation_pill(app)?;
        Ok(true)
    }
}

#[tauri::command]
pub fn start_dictation() -> Result<String, String> {
    Ok("Dictation started".to_string())
}

#[tauri::command]
pub fn stop_dictation() -> Result<String, String> {
    Ok("Dictation stopped".to_string())
}

#[tauri::command]
pub fn complete_setup(app: AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
    }
    if let Some(wizard_window) = app.get_webview_window("wizard") {
        let _ = wizard_window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(settings_window) = app.get_webview_window("settings") {
        let _ = settings_window.show();
    }
    Ok(())
}

#[tauri::command]
pub fn note_create_file(
    path: String,
    content: Option<String>,
    store: State<'_, SettingsStore>,
    ctx: State<'_, ApplicationContext>,
) -> Result<(), String> {
    let content = content.unwrap_or_default();
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.last_vault_path);

    // Validate path is within vault
    let safe_path = validate_path_within_vault(&vault_path, &path)?;

    // Validate input safety
    validate_input_safe(&path, 500)?;
    validate_input_safe(&content, 1_000_000)?; // 1MB max content

    // Phase 11.3: if this write overwrites an existing note (import / bulk
    // edit), snapshot the previous content first so it is never lost.
    let _ = crate::recovery::snapshot_note(&vault_path, &path);

    if let Some(parent) = safe_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
    }
    std::fs::write(&safe_path, &content).map_err(|e| e.to_string())?;

    // Register an undoable history entry so creation can be reversed.
    let undo_path = safe_path.clone();
    let redo_path = safe_path.clone();
    let redo_content = content.clone();
    crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::NoteCreate,
        format!("Create Note '{}'", path),
        vec![path.clone()],
        serde_json::json!({ "path": path, "exists": false }),
        serde_json::json!({ "path": path, "exists": true }),
        std::sync::Arc::new(move || {
            if undo_path.exists() {
                std::fs::remove_file(&undo_path).map_err(|e| e.to_string())?;
            }
            Ok(())
        }),
        std::sync::Arc::new(move || {
            if let Some(parent) = redo_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            std::fs::write(&redo_path, &redo_content).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn note_daily() -> Result<String, String> {
    let date_name = chrono::Local::now().format("%Y-%m-%d").to_string();
    Ok(format!("{}.md", date_name))
}

#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> Result<AppSettings, String> {
    Ok(store.get())
}

#[tauri::command]
pub fn settings_set(
    key: String,
    value: serde_json::Value,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    store
        .update(|s| {
            s.extra_settings.insert(key, value);
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(
    key: String,
    store: State<'_, SettingsStore>,
) -> Result<serde_json::Value, String> {
    Ok(store.get_value(&key))
}

#[tauri::command]
pub fn settings_set_all(
    settings: AppSettings,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    store.save(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Inbox Commands ──────────────────────────────────────────────────

/// Helper: convert a canonical KnowledgeObject to the frontend InboxItem.
fn knowledge_object_to_inbox_item(obj: &KnowledgeObject) -> InboxItem {
    let inbox_status_str =
        custom_text(obj, "inbox_status").unwrap_or_else(|| "pending".to_string());
    let status = match inbox_status_str.as_str() {
        "processing" => InboxStatus::Processing,
        "ready" => InboxStatus::Ready,
        "approved" => InboxStatus::Approved,
        "rejected" => InboxStatus::Rejected,
        "failed" => InboxStatus::Failed,
        _ => InboxStatus::Pending,
    };

    let duplicate_info = custom_json(obj, "duplicate_info")
        .and_then(|v| serde_json::from_value::<DuplicateInfo>(v).ok());

    let timeline_info = custom_json(obj, "timeline_info")
        .and_then(|v| serde_json::from_value::<TimelineInfo>(v).ok());

    let ocr_info =
        custom_json(obj, "ocr_info").and_then(|v| serde_json::from_value::<OcrInfo>(v).ok());

    let processing_history = custom_json(obj, "processing_history")
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<ProcessingHistoryEntry>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let warnings = custom_json(obj, "processing_warnings")
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let custom: std::collections::HashMap<String, serde_json::Value> = obj
        .custom_properties
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or_default()))
        .collect();

    InboxItem {
        id: obj.id.to_string(),
        title: obj.metadata.title.clone().unwrap_or_default(),
        object_type: obj.object_type.variant_name().to_string(),
        source: obj.metadata.source_url.clone().unwrap_or_default(),
        status,
        mime_type: obj.metadata.mime_type.clone(),
        source_file: obj.metadata.original_filename.clone(),
        metadata: InboxMetadata {
            title: obj.metadata.title.clone(),
            author: obj.metadata.authors.first().cloned(),
            language: obj.metadata.language.clone(),
            source_url: obj.metadata.source_url.clone(),
            tags: obj.tags.clone(),
            custom,
        },
        duplicate_info,
        timeline_info,
        ocr_info,
        processing_history,
        warnings,
        selected: false,
    }
}

#[tauri::command]
pub fn inbox_subscribe(ctx: State<'_, ApplicationContext>) -> Result<Vec<InboxItem>, String> {
    inbox_get_queue(ctx)
}

#[tauri::command]
pub fn inbox_get_queue(ctx: State<'_, ApplicationContext>) -> Result<Vec<InboxItem>, String> {
    let manager = get_storage_manager(&ctx)?;
    let objects = manager
        .list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let inbox_items: Vec<InboxItem> = objects
        .into_iter()
        .filter(|obj| {
            obj.custom_properties.contains_key("inbox_status")
                || obj.custom_properties.contains_key("suggested_folder")
                || obj.custom_properties.contains_key("classification")
        })
        .map(|obj| knowledge_object_to_inbox_item(&obj))
        .collect();

    Ok(inbox_items)
}

#[tauri::command]
pub fn inbox_approve(ctx: State<'_, ApplicationContext>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    let previous = custom_text(&obj, "inbox_status").unwrap_or_else(|| "pending".to_string());
    set_custom_text(&mut obj, "inbox_status", "approved");
    manager.save(&obj).map_err(|e| e.to_string())?;

    // Undo flips back to the previous status; redo re-approves.
    let manager_undo = manager.clone();
    let manager_redo = manager.clone();
    crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::Metadata,
        "Approve Inbox Item".to_string(),
        vec![id.clone()],
        serde_json::json!({ "inbox_status": previous }),
        serde_json::json!({ "inbox_status": "approved" }),
        std::sync::Arc::new(move || {
            let mut o = manager_undo
                .load(object_id)
                .ok_or_else(|| "Object not found during undo".to_string())?;
            set_custom_text(&mut o, "inbox_status", &previous);
            manager_undo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
        std::sync::Arc::new(move || {
            let mut o = manager_redo
                .load(object_id)
                .ok_or_else(|| "Object not found during redo".to_string())?;
            set_custom_text(&mut o, "inbox_status", "approved");
            manager_redo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn inbox_reject(
    ctx: State<'_, ApplicationContext>,
    id: String,
    reason: String,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    let previous_status =
        custom_text(&obj, "inbox_status").unwrap_or_else(|| "pending".to_string());
    let previous_reason = custom_text(&obj, "rejection_reason").unwrap_or_default();
    set_custom_text(&mut obj, "inbox_status", "rejected");
    set_custom_text(&mut obj, "rejection_reason", &reason);
    manager.save(&obj).map_err(|e| e.to_string())?;

    let manager_undo = manager.clone();
    let manager_redo = manager.clone();
    crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::Metadata,
        "Reject Inbox Item".to_string(),
        vec![id.clone()],
        serde_json::json!({ "inbox_status": previous_status }),
        serde_json::json!({ "inbox_status": "rejected", "rejection_reason": reason }),
        std::sync::Arc::new(move || {
            let mut o = manager_undo
                .load(object_id)
                .ok_or_else(|| "Object not found during undo".to_string())?;
            set_custom_text(&mut o, "inbox_status", &previous_status);
            set_custom_text(&mut o, "rejection_reason", &previous_reason);
            manager_undo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
        std::sync::Arc::new(move || {
            let mut o = manager_redo
                .load(object_id)
                .ok_or_else(|| "Object not found during redo".to_string())?;
            set_custom_text(&mut o, "inbox_status", "rejected");
            manager_redo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn inbox_retry(ctx: State<'_, ApplicationContext>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    set_custom_text(&mut obj, "inbox_status", "pending");
    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_delete(ctx: State<'_, ApplicationContext>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    manager.delete(object_id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_approve(
    ctx: State<'_, ApplicationContext>,
    ids: Vec<String>,
) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_approve(ctx.clone(), id) {
            eprintln!("Failed to approve inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_reject(
    ctx: State<'_, ApplicationContext>,
    ids: Vec<String>,
    reason: String,
) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_reject(ctx.clone(), id, reason.clone()) {
            eprintln!("Failed to reject inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_delete(
    ctx: State<'_, ApplicationContext>,
    ids: Vec<String>,
) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_delete(ctx.clone(), id) {
            eprintln!("Failed to delete inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_retry(
    ctx: State<'_, ApplicationContext>,
    ids: Vec<String>,
) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_retry(ctx.clone(), id) {
            eprintln!("Failed to retry inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_edit_metadata(
    ctx: State<'_, ApplicationContext>,
    id: String,
    title: Option<String>,
    author: Option<String>,
    language: Option<String>,
    tags: Vec<String>,
    custom: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    if let Some(t) = title {
        obj.metadata.title = Some(t);
    }
    if let Some(a) = author {
        obj.metadata.authors = vec![a];
    }
    if let Some(l) = language {
        obj.metadata.language = Some(l);
    }
    if !tags.is_empty() {
        obj.tags = tags;
    }
    for (key, value) in custom {
        set_custom_json(&mut obj, &key, &value);
    }

    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_move(
    ctx: State<'_, ApplicationContext>,
    id: String,
    destination: String,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    set_custom_text(&mut obj, "destination_folder", &destination);
    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Reading Queue Commands ────────────────────────────────────────

/// Helper: convert a canonical KnowledgeObject to the frontend QueueItem.
fn knowledge_object_to_queue_item(obj: &KnowledgeObject) -> QueueItem {
    let status = custom_text(obj, "reading_status")
        .map(|s| QueueStatus::from_label(&s))
        .unwrap_or_default();
    let priority = custom_text(obj, "reading_priority")
        .map(|s| QueuePriority::from_label(&s))
        .unwrap_or_default();
    let progress = custom_number(obj, "reading_progress").unwrap_or(0.0) as f32;

    QueueItem {
        id: obj.id.to_string(),
        title: obj.metadata.title.clone().unwrap_or_default(),
        object_type: obj.object_type.variant_name().to_string(),
        status,
        priority,
        progress,
        source: obj.metadata.source_url.clone().unwrap_or_default(),
        modified_at: obj.updated_at.to_rfc3339(),
        tags: obj.tags.clone(),
        selected: false,
    }
}

#[tauri::command]
pub fn queue_get_all(ctx: State<'_, ApplicationContext>) -> Result<Vec<QueueItem>, String> {
    let manager = get_storage_manager(&ctx)?;
    let objects = manager
        .list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let queue_items = objects
        .into_iter()
        .map(|obj| knowledge_object_to_queue_item(&obj))
        .collect();

    Ok(queue_items)
}

#[tauri::command]
pub fn queue_set_status(
    ctx: State<'_, ApplicationContext>,
    id: String,
    status: String,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Object not found: {}", id))?;

    let status = QueueStatus::from_label(&status);
    let previous =
        custom_text(&obj, "reading_status").unwrap_or_else(|| QueueStatus::Unread.label().to_string());
    let new_label = status.label().to_string();
    set_custom_text(&mut obj, "reading_status", &new_label);
    manager.save(&obj).map_err(|e| e.to_string())?;

    let manager_undo = manager.clone();
    let manager_redo = manager.clone();
    crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::Metadata,
        format!("Mark '{}' {}", obj.metadata.title.as_deref().unwrap_or("item"), new_label),
        vec![id.clone()],
        serde_json::json!({ "reading_status": previous }),
        serde_json::json!({ "reading_status": new_label }),
        std::sync::Arc::new(move || {
            let mut o = manager_undo
                .load(object_id)
                .ok_or_else(|| "Object not found during undo".to_string())?;
            set_custom_text(&mut o, "reading_status", &previous);
            manager_undo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
        std::sync::Arc::new(move || {
            let mut o = manager_redo
                .load(object_id)
                .ok_or_else(|| "Object not found during redo".to_string())?;
            set_custom_text(&mut o, "reading_status", &new_label);
            manager_redo.save(&o).map_err(|e| e.to_string())?;
            Ok(())
        }),
    )?;
    Ok(())
}

#[tauri::command]
pub fn queue_set_priority(
    ctx: State<'_, ApplicationContext>,
    id: String,
    priority: String,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Object not found: {}", id))?;

    let priority = QueuePriority::from_label(&priority);
    set_custom_text(&mut obj, "reading_priority", priority.label());
    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_set_progress(
    ctx: State<'_, ApplicationContext>,
    id: String,
    progress: f32,
) -> Result<(), String> {
    let manager = get_storage_manager(&ctx)?;
    let object_id = uuid::Uuid::parse_str(&id).map_err(|e| format!("Invalid object id: {}", e))?;
    let mut obj = manager
        .load(object_id)
        .ok_or_else(|| format!("Object not found: {}", id))?;

    obj.custom_properties.insert(
        "reading_progress".to_string(),
        CustomPropertyValue::Number(progress.clamp(0.0, 1.0) as f64),
    );
    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_batch_set_status(
    ctx: State<'_, ApplicationContext>,
    ids: Vec<String>,
    status: String,
) -> Result<(), String> {
    for id in ids {
        if let Err(e) = queue_set_status(ctx.clone(), id, status.clone()) {
            eprintln!("Failed to set queue status: {}", e);
        }
    }
    Ok(())
}

// ── Navigation & Discovery Commands ────────────────────────────────

/// One note in the vault index — powers the dashboard (recently modified),
/// the quick switcher and the search page's folder filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteIndexEntry {
    /// Vault-relative path (forward slashes).
    pub path: String,
    /// Display title (file name without `.md`).
    pub title: String,
    /// Parent folder ("" for the vault root).
    pub folder: String,
    /// Last modification time as an RFC 3339 string.
    pub modified_at: String,
    /// Whether the note is pinned (reserved for future use).
    #[serde(default)]
    pub pinned: bool,
}

/// Scans the vault and returns every note as a flat, sorted index.
///
/// The index is used by the dashboard's "Recently Modified" section, the
/// Quick Switcher's note list and the Search page's folder filter. Hidden
/// entries (leading `.`) are skipped, matching `tree_list`.
#[tauri::command]
pub fn notes_index(store: State<'_, SettingsStore>) -> Result<Vec<NoteIndexEntry>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(Vec::new());
    }

    fn walk(dir: &Path, prefix: &str, out: &mut Vec<NoteIndexEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let full = entry.path();
            if full.is_dir() {
                walk(&full, &path, out);
            } else if name.ends_with(".md") {
                let modified = std::fs::metadata(&full)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|t| {
                        // RFC 3339 via SystemTime → seconds since epoch.
                        let secs = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        chrono::DateTime::from_timestamp(secs as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let title = name.trim_end_matches(".md").to_string();
                let folder = match path.rfind('/') {
                    Some(i) => path[..i].to_string(),
                    None => String::new(),
                };
                out.push(NoteIndexEntry {
                    path,
                    title,
                    folder,
                    modified_at: modified,
                    pinned: false,
                });
            }
        }
    }

    let mut notes = Vec::new();
    walk(&vault_path, "", &mut notes);
    // Most recently modified first.
    notes.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(notes)
}

/// One full-text search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Vault-relative path of the matching note.
    pub path: String,
    /// Display title.
    pub title: String,
    /// Parent folder ("" for vault root).
    pub folder: String,
    /// Text snippet around the first match.
    pub snippet: String,
    /// Character offset of the first match within `snippet`.
    pub match_start: usize,
    /// Character offset one past the end of the first match in `snippet`.
    pub match_end: usize,
    /// Last modification time (RFC 3339) for sorting.
    pub modified_at: String,
}

/// Returns a case-insensitive byte index of the first occurrence of `needle`
/// in `haystack`, or `None`.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    lower.find(&needle_lower)
}

/// Builds a short context snippet around a byte offset, returning the snippet
/// and the (recomputed) match range within it.
fn make_snippet(content: &str, byte_idx: usize, match_len: usize) -> (String, usize, usize) {
    const CONTEXT: usize = 60;
    let chars: Vec<char> = content.chars().collect();
    let char_idx = content[..byte_idx].chars().count();
    let match_chars = content[byte_idx..byte_idx + match_len.min(content.len() - byte_idx)]
        .chars()
        .count();
    let start = char_idx.saturating_sub(CONTEXT);
    let end = (char_idx + match_chars + CONTEXT).min(chars.len());
    let mut snippet: String = chars[start..end].iter().collect();
    snippet = snippet.replace('\n', " ");
    snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    // Recompute the match range inside the whitespace-collapsed snippet. We
    // reconstruct by scanning the original slice for the query's character
    // count — simpler: return offsets relative to the collapsed string by
    // locating the match again.
    (snippet, char_idx - start, char_idx - start + match_chars)
}

/// Full-text search across note contents.
///
/// Returns up to `limit` hits (default 50) with a snippet and match offsets
/// so the frontend can highlight the matched text. Case-insensitive substring
/// matching, files read on demand — no index to maintain.
#[tauri::command]
pub fn notes_search(
    query: String,
    store: State<'_, SettingsStore>,
) -> Result<Vec<SearchHit>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(Vec::new());
    }

    let mut hits = Vec::new();

    fn walk(
        dir: &Path,
        prefix: &str,
        q: &str,
        out: &mut Vec<SearchHit>,
        limit: usize,
    ) {
        if out.len() >= limit {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            if out.len() >= limit {
                return;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let full = entry.path();
            if full.is_dir() {
                walk(&full, &path, q, out, limit);
            } else if name.ends_with(".md") {
                let Ok(content) = std::fs::read_to_string(&full) else {
                    continue;
                };
                let title = name.trim_end_matches(".md").to_string();
                let folder = match path.rfind('/') {
                    Some(i) => path[..i].to_string(),
                    None => String::new(),
                };
                let modified = std::fs::metadata(&full)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs() as i64)
                    })
                    .and_then(|secs| {
                        chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.to_rfc3339())
                    })
                    .unwrap_or_default();

                // Title match (no snippet needed — highlight the title).
                if let Some(idx) = find_ci(&title, q) {
                    let _ = idx;
                    let (snippet, s, e) = make_snippet(&content, 0, 0);
                    let _ = (s, e);
                    out.push(SearchHit {
                        path: path.clone(),
                        title: title.clone(),
                        folder: folder.clone(),
                        snippet,
                        match_start: 0,
                        match_end: 0,
                        modified_at: modified.clone(),
                    });
                    continue;
                }

                if let Some(idx) = find_ci(&content, q) {
                    let (snippet, s, e) = make_snippet(&content, idx, q.len());
                    out.push(SearchHit {
                        path,
                        title,
                        folder,
                        snippet,
                        match_start: s,
                        match_end: e,
                        modified_at: modified,
                    });
                }
            }
        }
    }

    let limit = 50;
    walk(&vault_path, "", q, &mut hits, limit);
    Ok(hits)
}

#[tauri::command]
pub fn queue_archive_completed(ctx: State<'_, ApplicationContext>) -> Result<usize, String> {
    let manager = get_storage_manager(&ctx)?;
    let objects = manager
        .list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let mut archived = 0;
    for mut obj in objects {
        let status = custom_text(&obj, "reading_status")
            .map(|s| QueueStatus::from_label(&s))
            .unwrap_or_default();
        if status == QueueStatus::Completed {
            set_custom_text(&mut obj, "reading_status", "archived");
            if manager.save(&obj).is_ok() {
                archived += 1;
            }
        }
    }
    Ok(archived)
}

// ── Knowledge Graph & Connected Knowledge ────────────────────────────
//
// Phase 13.1: the UI-facing knowledge graph. Wikilinks (`[[Title]]`) are
// extracted from note markdown on demand and resolved against the vault's
// note titles — reusing the existing tree/scan conventions (hidden entries
// skipped, vault-relative forward-slash paths). No new indexing system is
// introduced; the graph data is derived state, rebuilt on each call.

/// One note collected from the vault with its raw content.
#[derive(Clone)]
struct NoteEntry {
    path: String,
    title: String,
    folder: String,
    modified_at: String,
    content: String,
}

/// Scans the vault for `.md` notes (hidden entries skipped, matching
/// `tree_list` / `notes_index`).
fn collect_notes(vault_path: &Path) -> Vec<NoteEntry> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<NoteEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let full = entry.path();
            if full.is_dir() {
                walk(&full, &path, out);
            } else if name.ends_with(".md") {
                if let Ok(content) = std::fs::read_to_string(&full) {
                    let title = name.trim_end_matches(".md").to_string();
                    let folder = match path.rfind('/') {
                        Some(i) => path[..i].to_string(),
                        None => String::new(),
                    };
                    let modified = std::fs::metadata(&full)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .ok()
                                .map(|d| d.as_secs() as i64)
                        })
                        .and_then(|secs| {
                            chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.to_rfc3339())
                        })
                        .unwrap_or_default();
                    out.push(NoteEntry {
                        path,
                        title,
                        folder,
                        modified_at: modified,
                        content,
                    });
                }
            }
        }
    }
    let mut notes = Vec::new();
    walk(vault_path, "", &mut notes);
    notes
}

/// Extracts `[[...]]` wikilink targets from markdown, handling the alias
/// (`[[Title|Alias]]`), heading (`[[Title#Heading]]`) and block (`[[Title^id]]`)
/// suffixes. Returns the resolved target name (alias/heading/block stripped).
fn extract_wikilinks(content: &str) -> Vec<String> {
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(rel) = content[i + 2..].find("]]") {
                let raw = &content[i + 2..i + 2 + rel];
                let target = raw.split('|').next().unwrap_or(raw);
                let target = target.split('#').next().unwrap_or(target);
                let target = target.split('^').next().unwrap_or(target).trim();
                if !target.is_empty() {
                    out.push(target.to_string());
                }
                i += 2 + rel + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Builds a `lowercased title → paths` index for link resolution.
fn build_title_index(notes: &[NoteEntry]) -> std::collections::HashMap<String, Vec<String>> {
    let mut index: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for note in notes {
        index
            .entry(note.title.to_lowercase())
            .or_default()
            .push(note.path.clone());
        // Also index the full path (folder/title) so `[[Folder/Note]]` works.
        index
            .entry(note.path.to_lowercase())
            .or_default()
            .push(note.path.clone());
    }
    index
}

/// Resolves a wikilink target to a note path, or `None` (broken link).
fn resolve_note(
    index: &std::collections::HashMap<String, Vec<String>>,
    target: &str,
) -> Option<String> {
    let key = target.trim().to_lowercase();
    index
        .get(&key)
        .and_then(|paths| paths.first().cloned())
        .or_else(|| {
            // Allow trailing `.md` in the link.
            index
                .get(&format!("{key}.md"))
                .and_then(|paths| paths.first().cloned())
        })
}

/// Extracts `tags:` from YAML frontmatter — inline array (`tags: [a, b]`),
/// comma list (`tags: a, b`) or block list (`tags:\n  - a\n  - b`).
fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let fm = &rest[..end];
            let lines: Vec<&str> = fm.lines().collect();
            let mut in_block = false;
            for line in lines {
                let trimmed = line.trim();
                if in_block {
                    if let Some(item) = trimmed.strip_prefix("-") {
                        let p = item.trim().trim_matches('"').trim_matches('\'').to_string();
                        if !p.is_empty() {
                            tags.push(p);
                        }
                        continue;
                    }
                    in_block = false;
                }
                if let Some(value) = trimmed.strip_prefix("tags:") {
                    let inner = value.trim();
                    if inner.is_empty() {
                        in_block = true;
                        continue;
                    }
                    let inner = inner.trim_start_matches('[').trim_end_matches(']');
                    for part in inner.split(',') {
                        let p = part.trim().trim_matches('"').trim_matches('\'').to_string();
                        if !p.is_empty() {
                            tags.push(p);
                        }
                    }
                }
            }
        }
    }
    tags
}

/// One node in the knowledge graph (a markdown note).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Vault-relative path.
    pub path: String,
    /// Display title (file name without `.md`).
    pub title: String,
    /// Parent folder ("" for the vault root).
    pub folder: String,
    /// Last modification time (RFC 3339).
    pub modified_at: String,
    /// Tags from frontmatter.
    pub tags: Vec<String>,
    /// Incoming link count (other notes linking to this one).
    pub backlink_count: usize,
    /// Outgoing link count.
    pub outgoing_count: usize,
    /// Total degree (backlinks + outgoing).
    pub degree: usize,
}

/// One edge in the knowledge graph (a resolved wikilink).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdgeData {
    /// Path of the note containing the link.
    pub source: String,
    /// Resolved target path (or the raw link text when `broken`).
    pub target: String,
    /// True when the target does not resolve to a note.
    pub broken: bool,
}

/// Full graph payload for the graph view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdgeData>,
    /// Notes with zero connections (orphans).
    pub orphan_count: usize,
    /// Number of disconnected components.
    pub cluster_count: usize,
}

/// Returns the full knowledge graph: every note as a node plus every resolved
/// wikilink as an edge, with degree counts and cluster statistics.
#[tauri::command]
pub fn graph_data(store: State<'_, SettingsStore>) -> Result<GraphData, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(GraphData {
            nodes: Vec::new(),
            edges: Vec::new(),
            orphan_count: 0,
            cluster_count: 0,
        });
    }

    let notes = collect_notes(&vault_path);
    let index = build_title_index(&notes);

    let mut nodes: Vec<GraphNode> = notes
        .iter()
        .map(|n| GraphNode {
            path: n.path.clone(),
            title: n.title.clone(),
            folder: n.folder.clone(),
            modified_at: n.modified_at.clone(),
            tags: extract_tags(&n.content),
            backlink_count: 0,
            outgoing_count: 0,
            degree: 0,
        })
        .collect();

    let mut edges = Vec::new();
    for note in &notes {
        for target in extract_wikilinks(&note.content) {
            match resolve_note(&index, &target) {
                Some(tpath) => {
                    if tpath != note.path {
                        edges.push(GraphEdgeData {
                            source: note.path.clone(),
                            target: tpath,
                            broken: false,
                        });
                    }
                }
                None => {
                    edges.push(GraphEdgeData {
                        source: note.path.clone(),
                        target: target.clone(),
                        broken: true,
                    });
                }
            }
        }
    }

    // Degree / backlink counts (O(nodes + edges) via a path → index map with
    // owned String keys so `nodes` isn't held borrowed while mutated).
    let mut node_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        node_index.insert(n.path.clone(), i);
    }
    for edge in &edges {
        if let Some(&si) = node_index.get(&edge.source) {
            nodes[si].outgoing_count += 1;
            nodes[si].degree += 1;
        }
        if !edge.broken {
            if let Some(&di) = node_index.get(&edge.target) {
                nodes[di].backlink_count += 1;
                nodes[di].degree += 1;
            }
        }
    }

    // Cluster count via union-find over the resolved edges. Owned String keys
    // keep the borrows short so `nodes`/`edges` can be moved into the result.
    let mut parent: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    fn find(parent: &mut std::collections::HashMap<String, String>, x: &str) -> String {
        let px = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
        if px != x {
            let root = find(parent, &px);
            parent.insert(x.to_string(), root.clone());
            root
        } else {
            px
        }
    }
    for edge in &edges {
        if edge.broken {
            continue;
        }
        let a = find(&mut parent, &edge.source);
        let b = find(&mut parent, &edge.target);
        if a != b {
            parent.insert(a, b);
        }
    }
    let mut roots = std::collections::HashSet::new();
    for n in &nodes {
        roots.insert(find(&mut parent, &n.path));
    }
    let orphan_count = nodes.iter().filter(|n| n.degree == 0).count();

    Ok(GraphData {
        nodes,
        edges,
        orphan_count,
        cluster_count: roots.len(),
    })
}

/// One backlink hit: another note linking to the inspected note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkEntry {
    /// Path of the linking note.
    pub path: String,
    /// Title of the linking note.
    pub title: String,
    /// Parent folder of the linking note.
    pub folder: String,
    /// Context snippet around the first link.
    pub snippet: String,
    /// Character offset of the match within the snippet.
    pub match_start: usize,
    /// Character offset one past the match within the snippet.
    pub match_end: usize,
    /// How many times the linking note references this note.
    pub count: usize,
}

/// One outgoing link from the inspected note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingLink {
    /// `internal` (resolves to a note), `broken`, or `external` (URL).
    pub kind: String,
    /// Raw link text or URL.
    pub target: String,
    /// Resolved note path when `internal`.
    pub path: Option<String>,
    /// How many times this target is linked.
    pub count: usize,
}

/// One unlinked mention: another note's title appearing as plain text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionEntry {
    /// The note that would be linked (matched by title).
    pub title: String,
    /// Path of the matched note.
    pub path: String,
    /// Context snippet around the first occurrence.
    pub snippet: String,
    /// Character offset of the match within the snippet.
    pub match_start: usize,
    /// Character offset one past the match within the snippet.
    pub match_end: usize,
    /// Match strength (longer titles rank higher).
    pub score: u32,
}

/// Backlinks, outgoing links and unlinked mentions for one note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteLinks {
    pub backlinks: Vec<BacklinkEntry>,
    pub outgoing: Vec<OutgoingLink>,
    pub mentions: Vec<MentionEntry>,
    /// Frontmatter tags of the inspected note.
    pub tags: Vec<String>,
}

/// Lowercased chars of `content` with their original byte offsets. Each char
/// maps to its first lowercase form (1:1 for all common scripts; multi-char
/// lowercase expansions like `İ` are not handled).
fn lc_chars(content: &str) -> Vec<(usize, char)> {
    content
        .char_indices()
        .map(|(i, c)| (i, c.to_lowercase().next().unwrap_or(c)))
        .collect()
}

/// Returns every word-boundary, case-insensitive occurrence of `needle` in
/// `content` as a byte range into the ORIGINAL `content` (char-safe).
///
/// Word boundaries: the chars immediately before/after must not be
/// alphanumeric or `_`. Reuses the canonical snippet builder (`make_snippet`)
/// semantics — this is the single place plain-text mention matching happens.
fn ci_word_ranges(content: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    ci_word_ranges_in_lc(content, &lc_chars(content), needle)
}

/// Like [`ci_word_ranges`], but matches against a precomputed lowercase char
/// list so callers that scan many needles (unlinked mentions) only build the
/// lowercase list once — O(content) setup + O(needle × content) per needle
/// instead of O(content) per needle.
fn ci_word_ranges_in_lc(
    content: &str,
    lc: &[(usize, char)],
    needle: &str,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let needle_lc: Vec<char> = needle
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    if needle_lc.is_empty() || needle_lc.len() > lc.len() {
        return out;
    }
    let mut i = 0usize;
    while i + needle_lc.len() <= lc.len() {
        let matched = needle_lc
            .iter()
            .enumerate()
            .all(|(k, &c)| lc[i + k].1 == c);
        if matched {
            let before_ok = i == 0 || {
                let p = lc[i - 1].1;
                !(p.is_alphanumeric() || p == '_')
            };
            let after_idx = i + needle_lc.len();
            let after_ok = after_idx >= lc.len() || {
                let p = lc[after_idx].1;
                !(p.is_alphanumeric() || p == '_')
            };
            if before_ok && after_ok {
                let start = lc[i].0;
                let end = if after_idx < lc.len() {
                    lc[after_idx].0
                } else {
                    content.len()
                };
                out.push((start, end));
            }
        }
        i += 1;
    }
    out
}

/// Returns backlinks, outgoing links and unlinked mentions for a note.
///
/// `min_title_len` (default 3) is the mention-detection sensitivity: titles
/// shorter than this are never suggested as unlinked mentions.
#[tauri::command]
pub fn note_links(
    path: String,
    min_title_len: Option<usize>,
    store: State<'_, SettingsStore>,
) -> Result<NoteLinks, String> {
    let min_len = min_title_len.unwrap_or(3).max(2);
    if path.trim().is_empty() {
        return Ok(NoteLinks {
            backlinks: Vec::new(),
            outgoing: Vec::new(),
            mentions: Vec::new(),
            tags: Vec::new(),
        });
    }
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(NoteLinks {
            backlinks: Vec::new(),
            outgoing: Vec::new(),
            mentions: Vec::new(),
            tags: Vec::new(),
        });
    }

    let notes = collect_notes(&vault_path);
    let index = build_title_index(&notes);
    let this = notes
        .iter()
        .find(|n| n.path == path)
        .cloned()
        .ok_or_else(|| "Note not found".to_string())?;

    // Titles the user has chosen to ignore (persisted under `nabu.mention_ignored`).
    let ignored: std::collections::HashSet<String> = store
        .get_value("nabu.mention_ignored")
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    // Hoisted outside the per-note loops (they were re-extracted per note).
    let this_links = extract_wikilinks(&this.content);

    // ── Backlinks ──
    let mut backlinks = Vec::new();
    for note in &notes {
        if note.path == this.path {
            continue;
        }
        let links = extract_wikilinks(&note.content);
        let mut count = 0usize;
        let mut first: Option<(usize, usize)> = None;
        let lc = lc_chars(&note.content);
        for target in links {
            let resolved = resolve_note(&index, &target)
                .or_else(|| (target.eq_ignore_ascii_case(&this.title)).then(|| this.path.clone()));
            if resolved.as_deref() == Some(this.path.as_str()) {
                count += 1;
                if first.is_none() {
                    // Locate the first `[[target` span char-safely.
                    for (start, end) in ci_word_ranges_in_lc(&note.content, &lc, &target) {
                        if start > 0 && note.content[..start].ends_with('[') {
                            first = Some((start, end - start));
                            break;
                        }
                    }
                }
            }
        }
        if count > 0 {
            let (snippet, s, e) = first
                .map(|(off, len)| make_snippet(&note.content, off, len))
                .unwrap_or_else(|| ("…".to_string(), 0, 0));
            backlinks.push(BacklinkEntry {
                path: note.path.clone(),
                title: note.title.clone(),
                folder: note.folder.clone(),
                snippet,
                match_start: s,
                match_end: e,
                count,
            });
        }
    }
    backlinks.sort_by(|a, b| b.count.cmp(&a.count));

    // ── Outgoing ──
    let mut outgoing: Vec<OutgoingLink> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for target in &this_links {
        let kind;
        let resolved_path;
        if target.starts_with("http://")
            || target.starts_with("https://")
            || target.starts_with("www.")
        {
            kind = "external";
            resolved_path = None;
        } else if let Some(tpath) = resolve_note(&index, target) {
            kind = "internal";
            resolved_path = Some(tpath);
        } else {
            kind = "broken";
            resolved_path = None;
        }
        let key = resolved_path
            .clone()
            .unwrap_or_else(|| target.to_lowercase());
        let entry = seen.entry(key).or_insert(0);
        *entry += 1;
        if *entry == 1 {
            outgoing.push(OutgoingLink {
                kind: kind.to_string(),
                target: target.clone(),
                path: resolved_path,
                count: 1,
            });
        }
    }
    // Update counts for duplicates.
    for link in outgoing.iter_mut() {
        let key = link
            .path
            .clone()
            .unwrap_or_else(|| link.target.to_lowercase());
        link.count = seen.get(&key).copied().unwrap_or(1);
    }
    outgoing.sort_by(|a, b| b.count.cmp(&a.count));

    // ── Unlinked mentions ──
    // Only notes with a title at least `min_len` chars count (avoids noise
    // from short common words). Longer titles rank higher. Word-boundary
    // matching is char-safe; the lowercase list is built ONCE for this note so
    // the scan is O(needles × content) rather than rebuilding per needle.
    let this_lc = lc_chars(&this.content);
    // Paths already linked from this note (O(1) lookup per candidate).
    let linked_paths: std::collections::HashSet<String> = this_links
        .iter()
        .filter_map(|t| resolve_note(&index, t))
        .collect();
    let mut mentions: Vec<MentionEntry> = Vec::new();
    for note in &notes {
        if note.path == this.path {
            continue;
        }
        let title = note.title.trim();
        if title.len() < min_len {
            continue;
        }
        let title_lower = title.to_lowercase();
        if ignored.contains(&title_lower) {
            continue;
        }
        // Skip titles that are already wikilinked anywhere in this note.
        if linked_paths.contains(&note.path) {
            continue;
        }
        // Word-boundary occurrence count in the plain text.
        let ranges = ci_word_ranges_in_lc(&this.content, &this_lc, title);
        if ranges.is_empty() {
            continue;
        }
        // Exclude matches inside an existing `[[...]]` span (rare: a title
        // could match the link text itself).
        let mut count = 0usize;
        let mut first: Option<(usize, usize)> = None;
        for (start, end) in ranges {
            let inside_link = this.content[..start]
                .rfind("[[")
                .map(|open| {
                    this.content[open..]
                        .find("]]")
                        .map(|close| open + close + 2 > start)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if inside_link {
                continue;
            }
            count += 1;
            if first.is_none() {
                first = Some((start, end - start));
            }
        }
        if count > 0 {
            if let Some((off, len)) = first {
                let (snippet, s, e) = make_snippet(&this.content, off, len);
                mentions.push(MentionEntry {
                    title: title.to_string(),
                    path: note.path.clone(),
                    snippet,
                    match_start: s,
                    match_end: e,
                    score: (title.len() as u32) * (count as u32),
                });
            }
        }
    }
    mentions.sort_by(|a, b| b.score.cmp(&a.score));

    Ok(NoteLinks {
        backlinks,
        outgoing,
        mentions,
        tags: extract_tags(&this.content),
    })
}

/// Converts the first plain-text occurrence of `title` in the note at `path`
/// into a `[[wikilink]]` and writes the note back. Returns the new content.
#[tauri::command]
pub fn link_mention(
    path: String,
    title: String,
    store: State<'_, SettingsStore>,
) -> Result<String, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    let abs = validate_path_within_vault(&vault_path, &path)?;
    let content = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    // Char-safe word-boundary matching on the ORIGINAL bytes (no
    // `to_lowercase()` length shift → slicing can never panic).
    let mut replacement: Option<(usize, usize)> = None;
    for (start, end) in ci_word_ranges(&content, &title) {
        let inside_link = content[..start]
            .rfind("[[")
            .map(|open| {
                content[open..]
                    .find("]]")
                    .map(|close| open + close + 2 > start)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !inside_link {
            replacement = Some((start, end - start));
            break;
        }
    }
    let Some((idx, len)) = replacement else {
        return Err("No matching plain-text mention found".to_string());
    };
    let new_content = format!(
        "{}[[{}]]{}",
        &content[..idx],
        title,
        &content[idx + len..]
    );
    std::fs::write(&abs, &new_content).map_err(|e| e.to_string())?;
    Ok(new_content)
}

/// Reads the persisted list of mention titles the user chose to ignore.
#[tauri::command]
pub fn mention_ignore_list(
    store: State<'_, SettingsStore>,
) -> Result<Vec<String>, String> {
    let value = store.get_value("nabu.mention_ignored");
    if value.is_null() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Adds a mention title to the ignore list (it stops appearing in the
/// unlinked-mentions panel).
#[tauri::command]
pub fn mention_ignore(
    title: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    store
        .update(|s| {
            let mut list: Vec<String> = s
                .extra_settings
                .get("nabu.mention_ignored")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if !list.contains(&title) {
                list.push(title.clone());
            }
            s.extra_settings
                .insert("nabu.mention_ignored".to_string(), serde_json::json!(list));
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
