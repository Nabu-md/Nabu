use std::path::PathBuf;
use tauri::State;
use thiserror::Error;
use crate::models::{FileEntry, VaultScanResult};
use crate::settings::{SettingsError, SettingsStore};
use crate::vault::{VaultService, VaultError};

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
    fn payload<E: std::fmt::Display>(err: E) -> Self { CommandError::Payload(err.to_string()) }
    fn vault<E: std::fmt::Display>(err: E) -> Self { CommandError::Vault(err.to_string()) }
    fn settings<E: std::fmt::Display>(err: E) -> Self { CommandError::Settings(err.to_string()) }
}

impl From<VaultError> for CommandError {
    fn from(err: VaultError) -> Self { CommandError::vault(err) }
}

impl From<SettingsError> for CommandError {
    fn from(err: SettingsError) -> Self { CommandError::settings(err) }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct VaultOpenPayload { pub path: String }
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultOpenResponse { pub vault: VaultScanResult, pub settings: crate::settings::AppSettings }
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotePathPayload { pub path: String }
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RenameNotePayload { pub path: String, pub target_path: String }
#[tauri::command]
pub fn vault_open(payload: VaultOpenPayload, service: State<'_, VaultService>, settings: State<'_, SettingsStore>) -> Result<VaultOpenResponse, CommandError> {
    let path = PathBuf::from(&payload.path);
    service.open(path.clone()).map_err(CommandError::vault)?;
    Ok(VaultOpenResponse { vault: service.scan(&path)?, settings: settings.get() })
}

#[tauri::command]
pub fn note_create_file(vault_path: String, name: String, service: State<'_, VaultService>) -> Result<FileEntry, CommandError> {
    let path = PathBuf::from(vault_path).join(name);
    std::fs::File::create(&path).map_err(CommandError::vault)?;
    Ok(FileEntry { path: path.to_string_lossy().into(), name: path.file_name().unwrap().to_string_lossy().into(), mtime: 0.0 })
}

#[tauri::command]
pub fn note_delete_file(path: String) -> Result<(), CommandError> {
    std::fs::remove_file(path).map_err(CommandError::vault)?;
    Ok(())
}

#[tauri::command]
pub fn note_rename_file(path: String, new_path: String) -> Result<(), CommandError> {
    std::fs::rename(path, new_path).map_err(CommandError::vault)?;
    Ok(())
}
pub fn note_daily(path: String, service: State<'_, VaultService>) -> Result<String, CommandError> {
    // Implementation of daily note logic...
    Ok("daily-note-content".into())
}


#[tauri::command]
pub fn start_dictation() -> Result<String, CommandError> {
    Ok("Dictation started".to_string())
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
#[tauri::command]
pub fn settings_get_all(store: State<'_, SettingsStore>) -> Result<crate::settings::AppSettings, CommandError> {
    Ok(store.get())
}

#[tauri::command]
pub fn settings_set_all(settings: crate::settings::AppSettings, store: State<'_, SettingsStore>) -> Result<(), CommandError> {
    store.save(&settings).map_err(CommandError::settings)?;
    Ok(())
}

#[tauri::command]
pub fn stage_files(paths: Vec<String>) -> Result<(), CommandError> {
    // Placeholder for file staging logic
    println!("Staged files: {:?}", paths);
    Ok(())
}
#[tauri::command]
pub fn search(vault_path: String, query: String, service: State<'_, VaultService>) -> Result<Vec<String>, CommandError> {
    service.search(&PathBuf::from(vault_path), &query).map_err(CommandError::vault)
}

#[tauri::command]
pub fn get_graph_data(service: State<'_, VaultService>) -> Result<serde_json::Value, CommandError> {
    Ok(service.get_graph_data())
}

#[tauri::command]
pub fn note_daily(vault_path: String, service: State<'_, VaultService>) -> Result<FileEntry, CommandError> {
    let path = PathBuf::from(&vault_path);
    let date_name = chrono::Local::now().format("%Y-%m-%d").to_string();
    let content = crate::template_manager::TemplateManager::new(&path)
        .get_template("Daily Note").unwrap_or_default();
    service.create_note(&vault_path, &format!("{}.md", date_name), &content).map_err(CommandError::vault)
}

#[tauri::command]
pub fn export_note(
    vault_path: String,
    note_path: String,
    output_path: String,
    template_name: String,
    _service: State<'_, VaultService>
) -> Result<(), CommandError> {
    let root = std::path::PathBuf::from(vault_path);
    let engine = crate::export_engine::ExportEngine::new(root);
    engine.export_to_html(
        &std::path::PathBuf::from(note_path),
        &std::path::PathBuf::from(output_path),
        &template_name
    ).map_err(CommandError::vault)
}
#[tauri::command]
pub fn filter_graph_by_tag(tag: String, service: State<'_, VaultService>) -> Result<Vec<String>, CommandError> {
    Ok(service.graph.filter_by_tag(&tag))
}

#[tauri::command]
pub fn annotate_pdf(pdf_path: String, page: u32, content: String, service: State<'_, VaultService>) -> Result<(), CommandError> {
    let root = std::path::PathBuf::from(&pdf_path).parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
    let annotator = crate::native::pdf::PdfAnnotator::new(&root);
    annotator.annotate(&pdf_path, page, &content).map_err(CommandError::vault)
}

#[tauri::command]
pub fn run_ocr(path: String) -> Result<String, CommandError> {
    let engine = crate::native::ocr::OcrEngine::new();
    engine.extract_text(&path).map_err(CommandError::vault)
}

#[tauri::command]
pub fn run_dictation(audio_data: Vec<f32>, settings: State<'_, SettingsStore>) -> Result<String, CommandError> {
    let model_name = settings.get().whisper_model;
    let model_path = format!("resources/whisper-models/{}", model_name);
    let engine = crate::native::audio::AudioEngine::new(&model_path).map_err(CommandError::vault)?;
    engine.transcribe(&audio_data).map_err(CommandError::vault)
}

#[tauri::command]
pub fn annotate_pdf(path: String, page: u32, content: String) -> Result<(), CommandError> {
    let root = std::path::PathBuf::from(&path).parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
    let annotator = crate::native::pdf::PdfAnnotator::new(&root);
    annotator.annotate(&path, page, &content).map_err(CommandError::vault)
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
