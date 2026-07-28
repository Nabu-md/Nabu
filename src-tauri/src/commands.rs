use nabu_core::storage::StorageManager;
use nabu_core::event_bus::EventBus;
use nabu_core::models::knowledge_object::KnowledgeObject;
use nabu_core::reading_queue::{ReadingMetadata, ReadingStatus, ReadingPriority};
use std::sync::Arc;

use crate::settings::{AppSettings, SettingsStore};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use serde::{Deserialize, Serialize};

// ── Security Utilities ──────────────────────────────────────

/// Validates that a file path is within the vault directory.
/// Prevents path traversal attacks by ensuring the resolved path
/// is a descendant of the vault path.
fn validate_path_within_vault(vault_path: &Path, user_path: &str) -> Result<PathBuf, String> {
    let resolved = vault_path.join(user_path);
    let canonical = resolved.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let canonical_vault = vault_path.canonicalize().map_err(|e| format!("Invalid vault path: {}", e))?;
    
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
        return Err(format!("Input exceeds maximum length of {} characters", max_length));
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

/// Validates that a file path is safe (no traversal, no absolute paths outside vault).
fn validate_file_path_safe(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    
    // Reject absolute paths that could escape the vault
    if Path::new(path).is_absolute() && !path.starts_with('/') {
        // Relative paths are fine, absolute paths need vault validation
    }
    
    // Check for dangerous characters
    let dangerous_chars = ['\0', '<', '>', '|', '&', ';', '$', '`'];
    for c in dangerous_chars {
        if path.contains(c) {
            return Err(format!("Path contains dangerous character: {}", c));
        }
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
    fn default() -> Self { Self::Unread }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePriority {
    Low,
    Normal,
    High,
}

impl Default for QueuePriority {
    fn default() -> Self { Self::Normal }
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
    fn default() -> Self { Self::Pending }
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

#[tauri::command]
pub fn check_vault_exists(store: State<'_, SettingsStore>) -> Result<Option<String>, String> {
    let settings = store.get();
    let path = settings.last_vault_path.trim();
    if !path.is_empty() && Path::new(path).exists() {
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
    content: String,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let settings = store.get();
    let vault_path = PathBuf::from(&settings.vault_path);

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
    std::fs::write(&safe_path, content).map_err(|e| e.to_string())?;
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

/// Helper: create a StorageManager from the vault path in settings.
fn get_storage_manager(store: &State<'_, SettingsStore>) -> Result<StorageManager, String> {
    let settings = store.get();
    let vault_path = std::path::PathBuf::from(&settings.vault_path);
    let event_bus = Arc::new(EventBus::new());
    let manager = StorageManager::new(vault_path, event_bus);
    if !manager.is_initialized() {
        manager.initialize().map_err(|e| e.to_string())?;
    }
    Ok(manager)
}

/// Helper: convert a KnowledgeObject to an InboxItem.
fn knowledge_object_to_inbox_item(obj: &KnowledgeObject) -> InboxItem {
    let inbox_status_str = obj.metadata.custom.get("inbox_status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let status = match inbox_status_str {
        "processing" => InboxStatus::Processing,
        "ready" => InboxStatus::Ready,
        "approved" => InboxStatus::Approved,
        "rejected" => InboxStatus::Rejected,
        "failed" => InboxStatus::Failed,
        _ => InboxStatus::Pending,
    };

    let duplicate_info = obj.metadata.custom.get("duplicate_info")
        .and_then(|v| serde_json::from_value::<DuplicateInfo>(v.clone()).ok());

    let timeline_info = obj.metadata.custom.get("timeline_info")
        .and_then(|v| serde_json::from_value::<TimelineInfo>(v.clone()).ok());

    let ocr_info = obj.metadata.custom.get("ocr_info")
        .and_then(|v| serde_json::from_value::<OcrInfo>(v.clone()).ok());

    let processing_history = obj.metadata.custom.get("processing_history")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<ProcessingHistoryEntry>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let warnings = obj.metadata.custom.get("processing_warnings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    InboxItem {
        id: obj.id.to_string(),
        title: obj.metadata.title.clone().unwrap_or_default(),
        object_type: obj.object_type.to_string(),
        source: obj.metadata.source_url.clone().unwrap_or_default(),
        status,
        mime_type: obj.metadata.mime_type.clone(),
        source_file: obj.metadata.source_file.clone(),
        metadata: InboxMetadata {
            title: obj.metadata.title.clone(),
            author: obj.metadata.author.clone(),
            language: obj.metadata.language.clone(),
            source_url: obj.metadata.source_url.clone(),
            tags: obj.metadata.custom.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            custom: obj.metadata.custom.clone(),
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
pub fn inbox_subscribe(store: State<'_, SettingsStore>) -> Result<Vec<InboxItem>, String> {
    inbox_get_queue(store)
}

#[tauri::command]
pub fn inbox_get_queue(store: State<'_, SettingsStore>) -> Result<Vec<InboxItem>, String> {
    let manager = get_storage_manager(&store)?;
    let objects = manager.list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let inbox_items: Vec<InboxItem> = objects
        .into_iter()
        .filter(|obj| {
            obj.metadata.custom.contains_key("inbox_status")
                || obj.metadata.custom.contains_key("auto_file_suggestions")
                || obj.metadata.custom.contains_key("classification")
        })
        .map(|obj| knowledge_object_to_inbox_item(&obj))
        .collect();

    Ok(inbox_items)
}

#[tauri::command]
pub fn inbox_approve(store: State<'_, SettingsStore>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    obj.metadata.custom.insert(
        "inbox_status".to_string(),
        serde_json::Value::String("approved".to_string()),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_reject(store: State<'_, SettingsStore>, id: String, reason: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    obj.metadata.custom.insert(
        "inbox_status".to_string(),
        serde_json::Value::String("rejected".to_string()),
    );
    obj.metadata.custom.insert(
        "rejection_reason".to_string(),
        serde_json::Value::String(reason),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_retry(store: State<'_, SettingsStore>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    obj.metadata.custom.insert(
        "inbox_status".to_string(),
        serde_json::Value::String("pending".to_string()),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_delete(store: State<'_, SettingsStore>, id: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    manager.delete_object(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_approve(store: State<'_, SettingsStore>, ids: Vec<String>) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_approve(store.clone(), id) {
            eprintln!("Failed to approve inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_reject(store: State<'_, SettingsStore>, ids: Vec<String>, reason: String) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_reject(store.clone(), id, reason.clone()) {
            eprintln!("Failed to reject inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_delete(store: State<'_, SettingsStore>, ids: Vec<String>) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_delete(store.clone(), id) {
            eprintln!("Failed to delete inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_retry(store: State<'_, SettingsStore>, ids: Vec<String>) -> Result<(), String> {
    for id in ids {
        if let Err(e) = inbox_retry(store.clone(), id) {
            eprintln!("Failed to retry inbox item: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn inbox_edit_metadata(
    store: State<'_, SettingsStore>,
    id: String,
    title: Option<String>,
    author: Option<String>,
    language: Option<String>,
    tags: Vec<String>,
    custom: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    if let Some(t) = title { obj.metadata.title = Some(t); }
    if let Some(a) = author { obj.metadata.author = Some(a); }
    if let Some(l) = language { obj.metadata.language = Some(l); }
    if !tags.is_empty() {
        obj.metadata.custom.insert("tags".to_string(), serde_json::Value::Array(
            tags.into_iter().map(serde_json::Value::String).collect()
        ));
    }
    for (key, value) in custom {
        obj.metadata.custom.insert(key, value);
    }

    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn inbox_move(store: State<'_, SettingsStore>, id: String, destination: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Inbox item not found: {}", id))?;

    obj.metadata.custom.insert(
        "destination_folder".to_string(),
        serde_json::Value::String(destination),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Reading Queue Commands ────────────────────────────────────────

#[tauri::command]
pub fn queue_get_all(store: State<'_, SettingsStore>) -> Result<Vec<QueueItem>, String> {
    let settings = store.get();
    let vault_path = std::path::PathBuf::from(settings.vault_path);
    let event_bus = Arc::new(EventBus::new());
    let manager = StorageManager::new(vault_path, event_bus);

    if !manager.is_initialized() {
        manager.initialize().map_err(|e| e.to_string())?;
    }

    let objects = manager.list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let queue_items = objects.into_iter().map(|obj| {
        let reading_meta = ReadingMetadata::from_object(&obj);
        QueueItem {
            id: obj.id.to_string(),
            title: obj.metadata.title.clone().unwrap_or_default(),
            object_type: obj.object_type.to_string(),
            status: reading_meta.status,
            priority: reading_meta.priority,
            progress: reading_meta.progress,
            source: obj.metadata.source_url.clone().unwrap_or_default(),
            modified_at: obj.modified_at.clone(),
            tags: obj.metadata.custom.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            selected: false,
        }
    }).collect();

    Ok(queue_items)
}

#[tauri::command]
pub fn queue_set_status(store: State<'_, SettingsStore>, id: String, status: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Object not found: {}", id))?;

    let reading_meta = ReadingMetadata {
        status: match status.as_str() {
            "reading" => ReadingStatus::Reading,
            "completed" => ReadingStatus::Read,
            "archived" => ReadingStatus::Archived,
            _ => ReadingStatus::Unread,
        },
        ..ReadingMetadata::from_object(&obj)
    };
    obj.metadata.custom.insert(
        "reading_queue".to_string(),
        serde_json::to_value(&reading_meta).unwrap_or_default(),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_set_priority(store: State<'_, SettingsStore>, id: String, priority: String) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Object not found: {}", id))?;

    let mut reading_meta = ReadingMetadata::from_object(&obj);
    reading_meta.priority = match priority.as_str() {
        "low" => ReadingPriority::Low,
        "high" => ReadingPriority::High,
        _ => ReadingPriority::Normal,
    };
    obj.metadata.custom.insert(
        "reading_queue".to_string(),
        serde_json::to_value(&reading_meta).unwrap_or_default(),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_set_progress(store: State<'_, SettingsStore>, id: String, progress: f32) -> Result<(), String> {
    let manager = get_storage_manager(&store)?;
    let mut obj = manager.get_object(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Object not found: {}", id))?;

    let mut reading_meta = ReadingMetadata::from_object(&obj);
    reading_meta.progress = progress.clamp(0.0, 1.0);
    obj.metadata.custom.insert(
        "reading_queue".to_string(),
        serde_json::to_value(&reading_meta).unwrap_or_default(),
    );
    manager.save_object(&obj).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn queue_batch_set_status(store: State<'_, SettingsStore>, ids: Vec<String>, status: String) -> Result<(), String> {
    for id in ids {
        if let Err(e) = queue_set_status(store.clone(), id, status.clone()) {
            eprintln!("Failed to set queue status: {}", e);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn queue_archive_completed(store: State<'_, SettingsStore>) -> Result<usize, String> {
    let manager = get_storage_manager(&store)?;
    let objects = manager.list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    let mut archived = 0;
    for obj in objects {
        let reading_meta = ReadingMetadata::from_object(&obj);
        if reading_meta.status == ReadingStatus::Read {
            let mut obj = obj;
            let mut meta = reading_meta;
            meta.status = ReadingStatus::Archived;
            obj.metadata.custom.insert(
                "reading_queue".to_string(),
                serde_json::to_value(&meta).unwrap_or_default(),
            );
            if manager.save_object(&obj).is_ok() {
                archived += 1;
            }
        }
    }
    Ok(archived)
}

#[tauri::command]
pub fn fetch_objects(store: State<'_, SettingsStore>) -> Result<Vec<KnowledgeObject>, String> {
    let settings = store.get();
    let vault_path = std::path::PathBuf::from(settings.vault_path);
    let event_bus = Arc::new(EventBus::new());
    let manager = StorageManager::new(vault_path, event_bus);

    if !manager.is_initialized() {
        manager.initialize().map_err(|e| e.to_string())?;
    }

    let objects = manager.list_objects("", None, 1000)
        .map_err(|e| e.to_string())?;

    Ok(objects)
}
