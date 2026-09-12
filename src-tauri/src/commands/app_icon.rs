#[cfg(desktop)]
#[tauri::command]
pub fn update_app_icon(app_handle: tauri::AppHandle, theme_mode: String) -> Result<(), String> {
    crate::app_icon::update_app_icon_for_theme(&app_handle, &theme_mode)
}

#[cfg(mobile)]
#[tauri::command]
pub fn update_app_icon(_theme_mode: String) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
pub fn set_dock_icon_variant(app_handle: tauri::AppHandle, variant: String) -> Result<(), String> {
    crate::app_icon::update_app_icon_for_theme(&app_handle, &variant)
}

#[cfg(mobile)]
#[tauri::command]
pub fn set_dock_icon_variant(_variant: String) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
#[tauri::command]
pub fn open_vault_in_new_window(
    vault_path: String,
    vault_color: Option<String>,
) -> Result<(), String> {
    crate::vault_instance::open_vault_in_new_window(
        std::path::Path::new(&vault_path),
        vault_color.as_deref(),
    )
}

#[cfg(mobile)]
#[tauri::command]
pub fn open_vault_in_new_window(
    _vault_path: String,
    _vault_color: Option<String>,
) -> Result<(), String> {
    Err("Separate vault windows are not available on mobile".to_string())
}
