use std::path::PathBuf;

use tauri::State;
use thiserror::Error;

use crate::models::{FileEntry, VaultScanResult};
use crate::settings::{SettingsError, SettingsStore};
use crate::vault::VaultService;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("invalid payload: {0}")]
    Payload(String),
    #[error("vault error: {0}")]
    Vault(String),
    #[error("settings error: {0}")]
    Settings(String),
    #[error("markdown error: {0}")]
    Markdown(String),
}

impl CommandError {
    fn payload<E: std::fmt::Display>(err: E) -> Self {
        CommandError::Payload(err.to_string())
    }

    fn vault<E: std::fmt::Display>(err: E) -> Self {
        CommandError::Vault(err.to_string())
    }

    fn settings<E: std::fmt::Display>(err: E) -> Self {
        CommandError::Settings(err.to_string())
    }
}

impl From<VaultError> for CommandError {
    fn from(err: VaultError) -> Self {
        CommandError::vault(err)
    }
    CommandError::Vault(e) => CommandError::Vault(e),
}

impl From<SettingsError> for CommandError {
    fn from(err: SettingsError) -> Self {
        CommandError::settings(err)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VaultOpenPayload {
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultOpenResponse {
    pub vault: VaultScanResult,
    pub settings: crate::settings::AppSettings,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotePathPayload {
    pub path: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RenameNotePayload {
    pub path: String,
    pub target_path: String,
}

#[tauri::command]
pub fn vault_open(
    payload: VaultOpenPayload,
    service: State<'_, VaultService>,
    settings: State<'_, SettingsStore>,
) -> Result<VaultOpenResponse, CommandError> {
    ensure_path(&payload.path)?;
    let path = PathBuf::from(&payload.path);
    service.open(path.clone(), settings.get()).map_err(CommandError::vault)?;
    let result = service.scan()?;
    Ok(VaultOpenResponse {
        vault: result,
        settings: settings.get(),
    })
}

#[tauri::command]
pub fn vault_close(
    service: State<'_, VaultService>,
) -> Result<crate::settings::AppSettings, CommandError> {
    let restored = service.close();
    settings.set(restored.clone());
    Ok(restored);
}

#[tauri::command]
pub fn vault_scan(
    service: State<'_, VaultService>,
) -> Result<VaultScanResult, CommandError> {
    service.scan().map_err(CommandError::vault)
}

#[tauri::command]
pub fn note_read(
    payload: NotePathPayload,
    service: State<'_, VaultService>,
) -> Result<String, CommandError> {
    ensure_path(&payload.path)?;
    service.read_file(&payload.path).map_err(CommandError::vault)
}

#[tauri::command]
pub fn note_write(
    payload: NotePathPayload,
    contents: String,
    service: State<'_, VaultService>,
) -> Result<FileEntry, CommandError> {
    ensure_path(&payload.path)?;
    service
        .update_file(&payload.path, &contents)
        .map_err(CommandError::vault)
}

#[tauri::command]
pub fn note_delete(
    payload: NotePathPayload,
    service: State<'_, VaultService>,
) -> Result<(), CommandError> {
    ensure_path(&payload.path)?;
    service.delete_file(&payload.path).map_err(CommandError::vault)
}

#[tauri::command]
pub fn note_rename(
    payload: RenameNotePayload,
    service: State<'_, VaultService>,
) -> Result<FileEntry, CommandError> {
    ensure_path(&payload.path)?;
    ensure_path(&payload.target_path)?;
    service
        .rename_file(&payload.path, &payload.target_path)
        .map_err(CommandError::vault)
}

#[tauri::command]
pub fn folder_create(
    payload: NotePathPayload,
    service: State<'_, VaultService>,
) -> Result<(), CommandError> {
    ensure_path(&payload.path)?;
    service
        .create_folder(&payload.path)
        .map_err(CommandError::vault)
}

#[tauri::command]
pub fn folder_delete(
    payload: NotePathPayload,
    service: State<'_, VaultService>,
) -> Result<(), CommandError> {
    ensure_path(&payload.path)?;
    service
        .delete_folder(&payload.path)
        .map_err(CommandError::vault)
}

fn ensure_path(path: &str) -> Result<(), CommandError> {
    if path.trim().is_empty() {
        return Err(CommandError::payload("path is empty"));
    }
    Ok(())
}
#[tauri::command]
pub fn markdown_parse(markdown: String) -> Result<serde_json::Value, CommandError> {
    let doc = crate::markdown::parse(&markdown).map_err(|e| CommandError::Markdown(e.to_string()))?;
    Ok(crate::markdown::model::normalize(doc.ast))
}

#[tauri::command]
pub fn settings_get(key: String, store: State<'_, SettingsStore>) -> Result<serde_json::Value, CommandError> {
    Ok(store.get_value(&key))
}

#[tauri::command]
pub fn settings_set(key: String, value: serde_json::Value, store: State<'_, SettingsStore>) -> Result<(), CommandError> {
    store.set_value(&key, value);
    Ok(())
}

#[tauri::command]
pub fn settings_get_feature_toggles(store: State<'_, SettingsStore>) -> Result<serde_json::Value, CommandError> {
    Ok(store.get_feature_toggles())
}

#[tauri::command]
pub fn settings_set_feature_toggle(id: String, enabled: bool, store: State<'_, SettingsStore>) -> Result<serde_json::Value, CommandError> {
    Ok(store.set_feature_toggle(id, enabled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::VaultService;
    use crate::settings::SettingsStore;

    fn setup_service() -> (VaultService, SettingsStore, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes", "test.md"), b"hello").unwrap();

        let settings = SettingsStore::default();
        (VaultService::open(root, settings).unwrap(), settings, dir)
    }

    #[test]
    fn vault_open_returns_scan() {
        let (service, settings, _dir) = setup_service();
        let payload = VaultOpenPayload {
            path: service.root().display().to_string(),
        };

        let response = vault_open(payload, tauri::State::new(service), tauri::State::new(settings)).unwrap();
        assert_eq!(response.vault.files.len(), 1);
    }

    #[test]
    fn note_write_and_read_roundtrip() {
        let (service, settings, _dir) = setup_service();
        let write_payload = NotePathPayload { path: "/tmp/note.md".into() };
        note_write(write_payload.clone(), "updated".into(), tauri::State::new(service.clone()), tauri::State::new(settings)).unwrap();
        let contents = note_read(write_payload, tauri::State::new(service), tauri::State::new(settings)).unwrap();
        assert_eq!(contents, "updated");
    }
}
