use tauri::State;
use crate::settings::SettingsStore;
use tauri::Manager;

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    let settings_window = app.get_webview_window("settings").ok_or("Settings window not found")?;
    settings_window.show().map_err(|e| e.to_string())?;
    Ok(())
}
