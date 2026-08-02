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
