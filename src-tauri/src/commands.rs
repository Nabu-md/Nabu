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
        .filter(|e| {
            let name = e.file_name().to_string_lossy();
            // The reserved `archive/` folder is hidden from normal navigation.
            !name.starts_with('.') && !(prefix.is_empty() && name == ARCHIVE_FOLDER)
        })
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
            // Archived content stays searchable but is hidden from navigation.
            if prefix.is_empty() && name == ARCHIVE_FOLDER {
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
            // Archived content stays searchable but is hidden from navigation.
            if prefix.is_empty() && name == ARCHIVE_FOLDER {
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

// ── Knowledge Organisation & Workflow (Phase 13.2) ──────────────────────
//
// Virtual organisational overlays that leave the filesystem untouched:
// - Archive: notes move into a reserved `archive/` folder at the vault root.
//   They stay searchable (notes_search scans everything) but are hidden from
//   normal navigation (tree / index / graph skip the folder) until the
//   Archive view explicitly lists them for restore.
// - Smart Folders: persisted query definitions evaluated on demand against
//   the vault (tags / folders / dates / full text).
// - Calendar: date-indexed note listing for the calendar workspace.
// - Templates: CRUD persisted in settings (browse / edit / duplicate /
//   favourite).
// - Quick capture: create an inbox KnowledgeObject from the palette.

// ── Archive ─────────────────────────────────────────────────────────────

/// One archived note (its path inside `archive/` plus the original location).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Path inside the archive folder (vault-relative, `archive/...`).
    pub archive_path: String,
    /// Original vault-relative path (where restore puts it back).
    pub original_path: String,
    /// Display title (file name without `.md`).
    pub title: String,
    /// Original parent folder ("" for vault root).
    pub folder: String,
    /// Last modification time (RFC 3339).
    pub modified_at: String,
}

/// The reserved archive folder name at the vault root. Notes moved here are
/// hidden from normal navigation but remain full-text searchable.
pub const ARCHIVE_FOLDER: &str = "archive";

fn archive_dir(vault_path: &Path) -> PathBuf {
    vault_path.join(ARCHIVE_FOLDER)
}

/// Moves a note (or folder) into the reserved `archive/` folder, preserving
/// its relative layout, and returns the new archive-relative path. Non-
/// destructive: the original content is untouched and restore is trivial.
#[tauri::command]
pub fn archive_note(
    path: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if path.trim().is_empty() || path == ARCHIVE_FOLDER || path.starts_with("archive/") {
        return Err("Invalid path for archiving".to_string());
    }
    let full = validate_path_within_vault(&vault_path, &path)?;
    if !full.exists() {
        return Err(format!("Not found: {path}"));
    }
    let dest = archive_dir(&vault_path).join(&path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&full, &dest).map_err(|e| format!("Could not archive: {e}"))?;
    // Persist a reversible history entry so Archive → Undo restores the note.
    let src = full.clone();
    let dst = dest.clone();
    let _ = crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::Metadata,
        format!("Archive '{path}'"),
        vec![path.clone()],
        serde_json::json!({ "archived": false }),
        serde_json::json!({ "archived": true }),
        std::sync::Arc::new(move || {
            std::fs::rename(&dst, &src).map_err(|e| e.to_string())
        }),
        std::sync::Arc::new(move || {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&src, &dst).map_err(|e| e.to_string())
        }),
    );
    Ok(())
}

/// Restores an archived note to its original location.
#[tauri::command]
pub fn archive_restore(
    archive_path: String,
    ctx: State<'_, ApplicationContext>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if !archive_path.starts_with("archive/") {
        return Err("Not an archived path".to_string());
    }
    let full = validate_path_within_vault(&vault_path, &archive_path)?;
    if !full.exists() {
        return Err(format!("Not found: {archive_path}"));
    }
    let original_rel = archive_path
        .strip_prefix("archive/")
        .map(|s| s.to_string())
        .unwrap_or_default();
    let original = validate_path_within_vault(&vault_path, &original_rel)?;
    if let Some(parent) = original.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&full, &original).map_err(|e| format!("Could not restore: {e}"))?;
    let src = full.clone();
    let dst = original.clone();
    let _ = crate::history::push_history(
        &ctx,
        nabu_core::history::HistoryOp::Metadata,
        format!("Restore '{original_rel}'"),
        vec![original_rel.clone()],
        serde_json::json!({ "archived": true }),
        serde_json::json!({ "archived": false }),
        std::sync::Arc::new(move || {
            if let Some(parent) = src.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&dst, &src).map_err(|e| e.to_string())
        }),
        std::sync::Arc::new(move || {
            std::fs::rename(&src, &dst).map_err(|e| e.to_string())
        }),
    );
    Ok(())
}

/// Lists every note inside the reserved `archive/` folder with its original
/// location, so the Archive view can offer restore.
#[tauri::command]
pub fn archive_list(store: State<'_, SettingsStore>) -> Result<Vec<ArchiveEntry>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    let dir = archive_dir(&vault_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<ArchiveEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let archive_rel = format!("archive/{path}");
            let full = entry.path();
            if full.is_dir() {
                walk(&full, &path, out);
            } else if name.ends_with(".md") {
                let original_path = path.clone();
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
                out.push(ArchiveEntry {
                    archive_path: archive_rel,
                    original_path,
                    title,
                    folder,
                    modified_at: modified,
                });
            }
        }
    }
    walk(&dir, "", &mut out);
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(out)
}

// ── Smart Folders ────────────────────────────────────────────────────────

/// A persisted smart-folder definition (a named query shown in the sidebar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub query: String,
    #[serde(default)]
    pub pinned: bool,
}

const K_SMART_FOLDERS: &str = "nabu.smart_folders";

/// Lists all saved smart folders (persisted in settings).
#[tauri::command]
pub fn smart_folders_list(store: State<'_, SettingsStore>) -> Result<Vec<SmartFolder>, String> {
    Ok(store
        .get_value(K_SMART_FOLDERS)
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<SmartFolder>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
}

/// Saves (creates or updates) a smart folder definition.
#[tauri::command]
pub fn smart_folder_save(
    folder: SmartFolder,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let mut list = smart_folders_list(store.clone()).unwrap_or_default();
    if let Some(existing) = list.iter_mut().find(|f| f.id == folder.id) {
        *existing = folder.clone();
    } else {
        list.push(folder);
    }
    store
        .update(|s| {
            s.extra_settings.insert(K_SMART_FOLDERS.to_string(), serde_json::to_value(&list).unwrap());
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Deletes a smart folder by id.
#[tauri::command]
pub fn smart_folder_delete(id: String, store: State<'_, SettingsStore>) -> Result<(), String> {
    let list = smart_folders_list(store.clone()).unwrap_or_default();
    let filtered: Vec<SmartFolder> = list.into_iter().filter(|f| f.id != id).collect();
    store
        .update(|s| {
            s.extra_settings.insert(K_SMART_FOLDERS.to_string(), serde_json::to_value(&filtered).unwrap());
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Reads `date:` / `created:` / `modified:` from YAML frontmatter (YYYY-MM-DD).
fn frontmatter_date(content: &str) -> Option<String> {
    let rest = content.strip_prefix("---")?;
    let fm = rest.split("\n---").next()?;
    for line in fm.lines() {
        let trimmed = line.trim();
        for key in ["date:", "created:", "modified:"] {
            if let Some(value) = trimmed.strip_prefix(key) {
                let v = value.trim().trim_matches('"').trim_matches('\'');
                if v.len() >= 10 {
                    let d: String = v.chars().take(10).collect();
                    if d.chars().filter(|c| *c == '-').count() == 2 {
                        return Some(d);
                    }
                }
            }
        }
    }
    None
}

/// Evaluates a smart-folder query against the vault and returns matching
/// notes. Mini query language (whitespace-separated, ANDed):
///   `tag:name`     — frontmatter tag contains `name`
///   `folder:path`  — note lives in `path` (or a subfolder of it)
///   `date:YYYY-MM-DD` / `before:...` / `after:...` — frontmatter date or mtime
///   anything else  — case-insensitive full-text search (title + content)
#[tauri::command]
/// Evaluate a smart-folder query against the vault and return matching notes.
///
/// Query syntax (space-separated tokens, all optional):
/// - `tag:<name>`       — note must carry the tag (substring match, case-insensitive)
/// - `folder:<path>`    — note must live in the folder or a subfolder (case-insensitive)
/// - `date:YYYY-MM-DD`  — frontmatter date must equal the value
/// - `before:YYYY-MM-DD`— frontmatter date must be earlier (notes without a date pass)
/// - `after:YYYY-MM-DD` — frontmatter date must be later (notes without a date pass)
/// - any other token    — full-text term, must appear in title or body (case-insensitive)
///
/// An empty query matches every note in the vault.
pub fn smart_folder_evaluate(
    query: String,
    store: State<'_, SettingsStore>,
) -> Result<Vec<NoteIndexEntry>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(Vec::new());
    }
    let notes = collect_notes(&vault_path);
    let q = query.trim();
    if q.is_empty() {
        return Ok(notes
            .into_iter()
            .map(|n| NoteIndexEntry {
                path: n.path,
                title: n.title,
                folder: n.folder,
                modified_at: n.modified_at,
                pinned: false,
            })
            .collect());
    }

    let mut tag_filters: Vec<String> = Vec::new();
    let mut folder_filters: Vec<String> = Vec::new();
    let mut before: Option<String> = None;
    let mut after: Option<String> = None;
    let mut exact_date: Option<String> = None;
    let mut text_terms: Vec<String> = Vec::new();

    for token in q.split_whitespace() {
        if let Some(t) = token.strip_prefix("tag:") {
            tag_filters.push(t.to_lowercase());
        } else if let Some(f) = token.strip_prefix("folder:") {
            folder_filters.push(f.trim_end_matches('/').to_lowercase());
        } else if let Some(d) = token.strip_prefix("date:") {
            exact_date = Some(d.to_string());
        } else if let Some(d) = token.strip_prefix("before:") {
            before = Some(d.to_string());
        } else if let Some(d) = token.strip_prefix("after:") {
            after = Some(d.to_string());
        } else {
            text_terms.push(token.to_lowercase());
        }
    }

    let mut out = Vec::new();
    for note in notes {
        let tags = extract_tags(&note.content);
        if !tag_filters.is_empty()
            && !tag_filters.iter().all(|f| tags.iter().any(|t| t.to_lowercase().contains(f)))
        {
            continue;
        }
        let folder_lc = note.folder.to_lowercase();
        if !folder_filters.is_empty()
            && !folder_filters.iter().all(|f| folder_lc == *f || folder_lc.starts_with(&format!("{f}/")))
        {
            continue;
        }
        let date = frontmatter_date(&note.content)
            .or_else(|| note.modified_at.chars().take(10).collect::<String>().into())
            .unwrap_or_default();
        if let Some(d) = &exact_date {
            if date != *d {
                continue;
            }
        }
        if let Some(b) = &before {
            if !date.is_empty() && date >= *b {
                continue;
            }
        }
        if let Some(a) = &after {
            if !date.is_empty() && date <= *a {
                continue;
            }
        }
        if !text_terms.is_empty() {
            let hay = format!("{} {}", note.title.to_lowercase(), note.content.to_lowercase());
            if !text_terms.iter().all(|t| hay.contains(t)) {
                continue;
            }
        }
        out.push(NoteIndexEntry {
            path: note.path.clone(),
            title: note.title.clone(),
            folder: note.folder.clone(),
            modified_at: note.modified_at.clone(),
            pinned: false,
        });
    }
    Ok(out)
}

// ── Calendar ─────────────────────────────────────────────────────────────

/// One dated note for the calendar workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub path: String,
    pub title: String,
    pub folder: String,
    /// The date this note is shown under (YYYY-MM-DD, frontmatter date or mtime).
    pub date: String,
    pub modified_at: String,
}

/// Returns notes dated within `month` ("YYYY-MM"), using the frontmatter
/// `date:`/`created:` when present, else the file's modification date.
#[tauri::command]
pub fn calendar_notes(
    month: String,
    store: State<'_, SettingsStore>,
) -> Result<Vec<CalendarEntry>, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(Vec::new());
    }
    let month = month.trim().to_string();
    let notes = collect_notes(&vault_path);
    let mut out = Vec::new();
    for note in notes {
        let mtime: String = note.modified_at.chars().take(10).collect();
        let date = frontmatter_date(&note.content).unwrap_or_else(|| mtime.clone());
        if !month.is_empty() && !date.starts_with(&month) {
            continue;
        }
        out.push(CalendarEntry {
            path: note.path.clone(),
            title: note.title.clone(),
            folder: note.folder.clone(),
            date,
            modified_at: note.modified_at.clone(),
        });
    }
    out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(out)
}

/// Returns the vault-relative path of the daily note for `date` (YYYY-MM-DD),
/// opening an existing one or creating it on first write.
#[tauri::command]
pub fn daily_note_for(date: String, store: State<'_, SettingsStore>) -> Result<String, String> {
    let d = date.trim();
    if d.len() < 10 || d.chars().filter(|c| *c == '-').count() != 2 {
        return Err("Invalid date (expected YYYY-MM-DD)".to_string());
    }
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    let path = format!("{d}.md");
    let full = validate_path_within_vault(&vault_path, &path)?;
    if !full.exists() {
        let content = format!("# {d}\n");
        std::fs::write(&full, content).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

// ── Templates ────────────────────────────────────────────────────────────

/// A persisted note template (mirrors the UI `Template` model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRecord {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_folder: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub favourite: bool,
    pub frontmatter_defaults: std::collections::HashMap<String, String>,
    pub property_presets: std::collections::HashMap<String, serde_json::Value>,
    pub body: String,
    pub object_type: Option<String>,
}

const K_TEMPLATES: &str = "nabu.templates";

#[tauri::command]
pub fn template_list(store: State<'_, SettingsStore>) -> Result<Vec<TemplateRecord>, String> {
    Ok(store
        .get_value(K_TEMPLATES)
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<TemplateRecord>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default())
}

fn template_persist(store: &SettingsStore, list: &[TemplateRecord]) -> Result<(), String> {
    store
        .update(|s| {
            s.extra_settings.insert(K_TEMPLATES.to_string(), serde_json::to_value(list).unwrap());
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn template_save(template: TemplateRecord, store: State<'_, SettingsStore>) -> Result<(), String> {
    let mut list = template_list(store.clone()).unwrap_or_default();
    if let Some(existing) = list.iter_mut().find(|t| t.name == template.name) {
        *existing = template.clone();
    } else {
        list.push(template);
    }
    template_persist(&store, &list)
}

#[tauri::command]
pub fn template_delete(name: String, store: State<'_, SettingsStore>) -> Result<(), String> {
    let list = template_list(store.clone()).unwrap_or_default();
    let filtered: Vec<TemplateRecord> = list.into_iter().filter(|t| t.name != name).collect();
    template_persist(&store, &filtered)
}

#[tauri::command]
pub fn template_duplicate(name: String, store: State<'_, SettingsStore>) -> Result<TemplateRecord, String> {
    let list = template_list(store.clone()).unwrap_or_default();
    let source = list
        .iter()
        .find(|t| t.name == name)
        .cloned()
        .ok_or_else(|| "Template not found".to_string())?;
    let mut copy = source;
    let base = format!("{} Copy", copy.name);
    copy.favourite = false;
    // Ensure a unique name if a copy already exists (…Copy, …Copy 1, …Copy 2, …).
    let mut n = 1;
    let mut candidate = base.clone();
    while list.iter().any(|t| t.name == candidate) {
        candidate = format!("{base} {n}");
        n += 1;
    }
    copy.name = candidate;
    list.push(copy.clone());
    template_persist(&store, &list)?;
    Ok(copy)
}

#[tauri::command]
pub fn template_set_favourite(
    name: String,
    favourite: bool,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let mut list = template_list(store.clone()).unwrap_or_default();
    if let Some(t) = list.iter_mut().find(|t| t.name == name) {
        t.favourite = favourite;
    }
    template_persist(&store, &list)
}

// ── Quick capture ────────────────────────────────────────────────────────

/// Captures a quick note into the Inbox (a pending KnowledgeObject) from the
/// command palette or navbar, without touching the filesystem.
#[tauri::command]
pub fn inbox_quick_capture(
    ctx: State<'_, ApplicationContext>,
    title: String,
    content: String,
) -> Result<(), String> {
    use nabu_core::models::knowledge_object::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
    let manager = get_storage_manager(&ctx)?;
    let mut obj = KnowledgeObject::new(ObjectType::Note, ObjectContent::Markdown(content));
    let mut metadata = ObjectMetadata::default();
    metadata.title = Some(if title.trim().is_empty() {
        "Quick capture".to_string()
    } else {
        title
    });
    metadata.description = Some("Captured via Quick Capture".to_string());
    obj.metadata = metadata;
    set_custom_text(&mut obj, "inbox_status", "pending");
    set_custom_text(&mut obj, "source", "quick_capture");
    manager.save(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Canvas Commands (Phase 13.3) ───────────────────────────────────
//
// Canvases are infinite visual workspaces that *reference* existing notes
// rather than duplicating content. A canvas definition is a JSON document
// stored in the settings store under `nabu.canvases` — no proprietary
// storage format, no content duplication. Each node carries a vault-relative
// note path plus an (x, y) position; edges are visual connectors between
// nodes. Groups are bounding boxes that label a region of the canvas.

/// One positioned node on a canvas. The `note_path` references an existing
/// note — the canvas never owns content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasNode {
    /// Stable client-generated id (uuid).
    pub id: String,
    /// Vault-relative path of the referenced note.
    pub note_path: String,
    /// Display title (cached from the note for the sidebar list).
    pub title: String,
    /// X position in canvas coordinates.
    pub x: f64,
    /// Y position in canvas coordinates.
    pub y: f64,
    /// Optional width override (px). `None` = default card width.
    #[serde(default)]
    pub width: Option<f64>,
    /// Optional height override (px).
    #[serde(default)]
    pub height: Option<f64>,
    /// Node kind: note, image, pdf, link, group, annotation.
    #[serde(default = "default_node_kind")]
    pub kind: String,
    /// For image/pdf/link nodes: the source URL or vault-relative path.
    #[serde(default)]
    pub source: String,
    /// For annotation nodes: the text content.
    #[serde(default)]
    pub text: String,
}

fn default_node_kind() -> String {
    "note".to_string()
}

/// One visual connector between two canvas nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    /// Optional label on the connector (e.g. "references").
    #[serde(default)]
    pub label: String,
}

/// One labelled group (bounding box) on a canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasGroup {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Node ids contained in this group.
    #[serde(default)]
    pub members: Vec<String>,
}

/// A complete canvas definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub groups: Vec<CanvasGroup>,
    /// Pan offset (x, y) — the canvas viewport origin.
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    /// Zoom level (1.0 = 100%).
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 {
    1.0
}

const CANVAS_KEY: &str = "nabu.canvases";

fn load_canvases(store: &SettingsStore) -> Vec<CanvasDef> {
    let value = store.get_value(CANVAS_KEY);
    serde_json::from_value::<Vec<CanvasDef>>(value).unwrap_or_default()
}

fn save_canvases(store: &SettingsStore, canvases: &[CanvasDef]) -> Result<(), String> {
    store
        .update(|s| {
            s.extra_settings
                .insert(CANVAS_KEY.to_string(), serde_json::to_value(canvases).unwrap());
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Lists every saved canvas (id + name only — nodes/edges omitted for speed).
#[tauri::command]
pub fn canvas_list(store: State<'_, SettingsStore>) -> Result<Vec<CanvasDef>, String> {
    Ok(load_canvases(&store))
}

/// Returns the full canvas definition (nodes, edges, groups).
#[tauri::command]
pub fn canvas_get(
    id: String,
    store: State<'_, SettingsStore>,
) -> Result<Option<CanvasDef>, String> {
    Ok(load_canvases(&store).into_iter().find(|c| c.id == id))
}

/// Creates or updates a canvas (deduped by id) and persists it.
#[tauri::command]
pub fn canvas_save(
    canvas: CanvasDef,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let mut canvases = load_canvases(&store);
    if let Some(existing) = canvases.iter_mut().find(|c| c.id == canvas.id) {
        *existing = canvas;
    } else {
        canvases.push(canvas);
    }
    save_canvases(&store, &canvases)
}

/// Deletes a canvas by id.
#[tauri::command]
pub fn canvas_delete(id: String, store: State<'_, SettingsStore>) -> Result<(), String> {
    let mut canvases = load_canvases(&store);
    canvases.retain(|c| c.id != id);
    save_canvases(&store, &canvases)
}

// ── Comparison View (Phase 13.3) ───────────────────────────────────

/// Computes a line diff between two arbitrary notes (by vault-relative path).
/// Reuses the same LCS diff engine as `versions_diff` so the Comparison View
/// can compare any two notes — not just revisions of the same note.
#[tauri::command]
pub fn notes_diff(
    path_a: String,
    path_b: String,
    store: State<'_, SettingsStore>,
) -> Result<Vec<crate::recovery::DiffRow>, String> {
    let vault = crate::recovery::vault_path_pub(&store);
    let read_note = |p: &str| -> Result<String, String> {
        let abs = crate::recovery::resolve_in_vault_pub(&vault, p)?;
        if !abs.is_file() {
            return Ok(String::new());
        }
        std::fs::read_to_string(&abs).map_err(|e| e.to_string())
    };
    let a = read_note(&path_a)?;
    let b = read_note(&path_b)?;
    // Reuse the LCS diff from recovery.rs via the public re-export.
    Ok(crate::recovery::line_diff_pub(&a, &b))
}

// ── Statistics & Insights (Phase 13.3) ─────────────────────────────

/// One tag with its usage count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagStat {
    pub tag: String,
    pub count: usize,
}

/// One day in the vault-growth histogram (notes created that day).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthPoint {
    pub date: String,
    pub count: usize,
}

/// One recently-active note (created or modified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentNoteStat {
    pub path: String,
    pub title: String,
    pub folder: String,
    pub modified_at: String,
    pub created_at: Option<String>,
    pub size: usize,
}

/// The complete vault statistics payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStatistics {
    pub note_count: usize,
    pub folder_count: usize,
    pub tag_count: usize,
    pub total_tags: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub graph_orphans: usize,
    pub graph_clusters: usize,
    pub tags: Vec<TagStat>,
    pub recently_created: Vec<RecentNoteStat>,
    pub recently_modified: Vec<RecentNoteStat>,
    pub growth: Vec<GrowthPoint>,
    pub storage_bytes: u64,
    pub writing_streak_days: usize,
    pub active_days_last_30: usize,
}

/// Recursively counts folders (directories) in the vault, skipping hidden
/// entries and the reserved `archive/` folder.
fn count_folders(vault_path: &Path) -> usize {
    fn walk(dir: &Path, prefix: &str) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut count = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if prefix.is_empty() && name == ARCHIVE_FOLDER {
                continue;
            }
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if entry.path().is_dir() {
                count += 1 + walk(&entry.path(), &path);
            }
        }
        count
    }
    walk(vault_path, "")
}

/// Computes the total size of all `.md` files in the vault (bytes).
fn vault_storage_usage(vault_path: &Path) -> u64 {
    fn walk(dir: &Path) -> u64 {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut total = 0u64;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                total += walk(&path);
            } else if name.ends_with(".md") {
                total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            }
        }
        total
    }
    walk(vault_path)
}

/// Computes the writing streak: the number of consecutive days (ending today
/// or yesterday) on which at least one note was modified. Also returns the
/// count of active days in the last 30 days.
fn writing_streak(vault_path: &Path) -> (usize, usize) {
    let notes = collect_notes(vault_path);
    let today = chrono::Local::now().date_naive();
    let mut active_days: std::collections::HashSet<chrono::NaiveDate> = notes
        .iter()
        .filter_map(|n| {
            chrono::DateTime::parse_from_rfc3339(&n.modified_at)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Local).date_naive())
        })
        .collect();

    let streak = {
        let mut streak = 0;
        let mut cursor = today;
        // Allow today to be empty (streak counts from yesterday if today has
        // no edits yet).
        if !active_days.contains(&cursor) {
            cursor = cursor.pred_opt().unwrap_or(cursor);
        }
        while active_days.contains(&cursor) {
            streak += 1;
            cursor = match cursor.pred_opt() {
                Some(d) => d,
                None => break,
            };
        }
        streak
    };

    // Active days in the last 30 days.
    let cutoff = today - chrono::Duration::days(30);
    active_days.retain(|d| *d >= cutoff);
    (streak, active_days.len())
}

/// Builds a 30-day vault-growth histogram (notes modified per day).
fn vault_growth(vault_path: &Path) -> Vec<GrowthPoint> {
    let notes = collect_notes(vault_path);
    let today = chrono::Local::now().date_naive();
    let mut buckets: std::collections::BTreeMap<chrono::NaiveDate, usize> =
        std::collections::BTreeMap::new();
    for d in 0..30 {
        let date = today - chrono::Duration::days(d);
        buckets.insert(date, 0);
    }
    for note in &notes {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&note.modified_at) {
            let date = dt.with_timezone(&chrono::Local).date_naive();
            if let Some(count) = buckets.get_mut(&date) {
                *count += 1;
            }
        }
    }
    buckets
        .into_iter()
        .rev()
        .map(|(date, count)| GrowthPoint {
            date: date.format("%Y-%m-%d").to_string(),
            count,
        })
        .collect()
}

/// Returns comprehensive vault statistics for the Statistics dashboard.
#[tauri::command]
pub fn statistics_get(
    store: State<'_, SettingsStore>,
    ctx: State<'_, ApplicationContext>,
) -> Result<VaultStatistics, String> {
    let settings = store.get();
    let vault_path = PathBuf::from(settings.last_vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Ok(VaultStatistics {
            note_count: 0,
            folder_count: 0,
            tag_count: 0,
            total_tags: 0,
            graph_nodes: 0,
            graph_edges: 0,
            graph_orphans: 0,
            graph_clusters: 0,
            tags: vec![],
            recently_created: vec![],
            recently_modified: vec![],
            growth: vec![],
            storage_bytes: 0,
            writing_streak_days: 0,
            active_days_last_30: 0,
        });
    }

    let notes = collect_notes(&vault_path);

    // Tag aggregation.
    let mut tag_map: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for note in &notes {
        for tag in extract_tags(&note.content) {
            *tag_map.entry(tag).or_insert(0) += 1;
        }
    }
    let mut tags: Vec<TagStat> = tag_map
        .into_iter()
        .map(|(tag, count)| TagStat { tag, count })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    let total_tags: usize = tags.iter().map(|t| t.count).sum();
    let tag_count = tags.len();

    // Graph data (reuse the graph_data command logic via the context).
    let graph = graph_data_inner(&vault_path);

    // Recently modified (top 10).
    let mut recent: Vec<RecentNoteStat> = notes
        .iter()
        .map(|n| {
            let size = n.content.len();
            RecentNoteStat {
                path: n.path.clone(),
                title: n.title.clone(),
                folder: n.folder.clone(),
                modified_at: n.modified_at.clone(),
                created_at: None, // creation time not tracked separately
                size,
            }
        })
        .collect();
    recent.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    let recently_modified: Vec<RecentNoteStat> = recent.iter().take(10).cloned().collect();
    // Recently "created" — approximated by the oldest modifications reversed
    // (creation time is not stored separately in markdown files).
    let mut by_oldest = recent.clone();
    by_oldest.sort_by(|a, b| a.modified_at.cmp(&b.modified_at));
    let recently_created: Vec<RecentNoteStat> = by_oldest.into_iter().take(10).collect();

    let growth = vault_growth(&vault_path);
    let storage_bytes = vault_storage_usage(&vault_path);
    let (writing_streak_days, active_days_last_30) = writing_streak(&vault_path);
    let folder_count = count_folders(&vault_path);

    Ok(VaultStatistics {
        note_count: notes.len(),
        folder_count,
        tag_count,
        total_tags,
        graph_nodes: graph.nodes.len(),
        graph_edges: graph.edges.len(),
        graph_orphans: graph.orphan_count,
        graph_clusters: graph.cluster_count,
        tags,
        recently_created,
        recently_modified,
        growth,
        storage_bytes,
        writing_streak_days,
        active_days_last_30,
    })
}

/// Internal helper that computes GraphData without going through Tauri state
/// (used by `statistics_get` which already holds the vault path).
fn graph_data_inner(vault_path: &Path) -> GraphData {
    let notes = collect_notes(vault_path);
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
                        target: target,
                        broken: true,
                    });
                }
            }
        }
    }

    // Degree counts.
    for edge in &edges {
        if let Some(node) = nodes.iter_mut().find(|n| n.path == edge.source) {
            node.outgoing_count += 1;
            node.degree += 1;
        }
        if !edge.broken {
            if let Some(node) = nodes.iter_mut().find(|n| n.path == edge.target) {
                node.backlink_count += 1;
                node.degree += 1;
            }
        }
    }

    let orphan_count = nodes.iter().filter(|n| n.degree == 0).count();

    // Cluster count via union-find.
    let mut uf = UnionFind::new(nodes.len());
    let path_index: std::collections::HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.path.clone(), i))
        .collect();
    for edge in &edges {
        if edge.broken {
            continue;
        }
        if let (Some(&a), Some(&b)) =
            (path_index.get(&edge.source), path_index.get(&edge.target))
        {
            uf.union(a, b);
        }
    }
    let cluster_count = uf.count();

    GraphData {
        nodes,
        edges,
        orphan_count,
        cluster_count,
    }
}

/// Simple union-find for cluster counting.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }
    fn count(&mut self) -> usize {
        let mut roots = std::collections::HashSet::new();
        for i in 0..self.parent.len() {
            roots.insert(self.find(i));
        }
        roots.len()
    }
}
