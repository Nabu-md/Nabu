use nabu_core::storage::StorageManager;
use nabu_core::event_bus::EventBus;
use nabu_core::models::knowledge_object::KnowledgeObject;
use std::sync::Arc;

use crate::settings::{AppSettings, SettingsStore};
use std::path::Path;
use tauri::{AppHandle, Manager, State};
use serde::{Deserialize, Serialize};

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
pub fn note_create_file(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    std::fs::write(path, content).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn inbox_subscribe() -> Result<Vec<InboxItem>, String> {
    Ok(vec![])
}

#[tauri::command]
pub fn inbox_get_queue() -> Result<Vec<InboxItem>, String> {
    Ok(vec![])
}

#[tauri::command]
pub fn inbox_approve(id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn inbox_reject(id: String, reason: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn inbox_retry(id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn inbox_delete(id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_approve(ids: Vec<String>) -> Result<(), String> {
    for id in ids { let _ = inbox_approve(id); }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_reject(ids: Vec<String>, reason: String) -> Result<(), String> {
    for id in ids { let _ = inbox_reject(id, reason.clone()); }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_delete(ids: Vec<String>) -> Result<(), String> {
    for id in ids { let _ = inbox_delete(id); }
    Ok(())
}

#[tauri::command]
pub fn inbox_batch_retry(ids: Vec<String>) -> Result<(), String> {
    for id in ids { let _ = inbox_retry(id); }
    Ok(())
}

#[tauri::command]
pub fn inbox_edit_metadata(
    id: String,
    title: Option<String>,
    author: Option<String>,
    language: Option<String>,
    tags: Vec<String>,
    custom: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn inbox_move(id: String, destination: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn fetch_objects(store: State<'_, SettingsStore>) -> Result<Vec<KnowledgeObject>, String> {
    let settings = store.get();
    let vault_path = std::path::PathBuf::from(settings.vault_path);
    let event_bus = Arc::new(EventBus::new());
    let manager = StorageManager::new(vault_path, event_bus);
    
    // Assuming SQLiteStorage needs initialization
    if !manager.is_initialized() {
        manager.initialize().map_err(|e| e.to_string())?;
    }
    
    // Need a way to fetch ALL objects.
    // Let me check StorageManager API for fetching all objects.
    todo!("Implement fetch_all")
}
