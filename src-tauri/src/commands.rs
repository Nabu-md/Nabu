use tauri::{AppHandle, Manager, State};
use crate::settings::{AppSettings, SettingsStore};

#[tauri::command]
pub fn complete_setup(app: AppHandle) -> Result<(), String> {
    let main_window = app.get_webview_window("main").ok_or("Main window not found")?;
    main_window.show().map_err(|e| e.to_string())?;
    let wizard_window = app.get_webview_window("wizard").ok_or("Wizard window not found")?;
    wizard_window.close().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    let settings_window = app.get_webview_window("settings").ok_or("Settings window not found")?;
    settings_window.show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn note_create_file(path: String, content: String) -> Result<(), String> {
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
pub fn settings_set_all(settings: AppSettings, store: State<'_, SettingsStore>) -> Result<(), String> {
    store.save(&settings).map_err(|e| e.to_string())?;
    Ok(())
}
