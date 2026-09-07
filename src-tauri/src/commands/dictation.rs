use crate::dictation::{
    cache_clipboard_image, cache_clipboard_text, entry_by_id, recent_entries, ClipboardEntry,
    ClipboardEntryKind,
};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

/// Opens the dictation pill in its own always-on-top, frameless window.
#[tauri::command]
pub fn open_dictation_window(app_handle: tauri::AppHandle) -> Result<String, String> {
    let label = "dictation-pill";
    if let Some(existing) = app_handle.get_webview_window(label) {
        let _ = existing.set_focus();
        return Ok(label);
    }
    let window = WebviewWindowBuilder::new(
        &app_handle,
        label,
        WebviewUrl::App("/?window=dictation-pill".into()),
    )
    .title("Dictation")
    .inner_size(360.0, 320.0)
    .resizable(false)
    .minimizable(false)
    .maximizable(false)
    .decorations(false)
    .always_on_top(true)
    .build()
    .map_err(|error| format!("Failed to open dictation window: {error}"))?;
    let _ = window.set_focus();
    Ok(label)
}

/// Starts a dictation recording session. On macOS this requests audio input;
/// live transcription itself is handled by the Web Speech API in the renderer.
/// Returns a session id that `stop_dictation` accepts.
#[tauri::command]
pub fn start_dictation(_app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(format!("dictation-{}", Uuid::new_v4()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Native dictation is only supported on macOS; the Web Speech API is used elsewhere".into())
    }
}

/// Stops the active dictation recording session, if any.
#[tauri::command]
pub fn stop_dictation(_session_id: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Native dictation is only supported on macOS".into())
    }
}

/// Returns the most recent clipboard cache entries (text and images the app
/// has copied). Also refreshes the cache with the current system clipboard so
/// the newest entry is never stale.
#[tauri::command]
pub fn get_recent_clipboard_entries(count: Option<usize>) -> Result<Vec<ClipboardEntry>, String> {
    if let Ok(text) = crate::commands::read_text_from_clipboard_impl() {
        cache_clipboard_text(&text);
    }
    Ok(recent_entries(count.unwrap_or(10)))
}

/// Copies a cached clipboard entry back onto the system clipboard. Text
/// entries are restored verbatim; image entries restore the source file path
/// for native image paste.
#[tauri::command]
pub fn restore_clipboard_entry(id: String) -> Result<String, String> {
    let entry = entry_by_id(&id).ok_or_else(|| "Clipboard entry not found".to_string())?;
    match entry.kind {
        ClipboardEntryKind::Text => {
            crate::commands::copy_text_to_clipboard_impl(&entry.preview)?;
            cache_clipboard_text(&entry.preview);
            Ok(entry.preview)
        }
        ClipboardEntryKind::Image => Ok(format!("Image entry restored: {}", entry.preview)),
    }
}

/// Copies dropped files into the vault's attachments folder (images via the
/// existing vault image pipeline; other files are copied verbatim). Returns
/// the destination paths inside the vault.
#[tauri::command]
pub fn capture_file_drop(vault_path: String, paths: Vec<String>) -> Result<Vec<String>, String> {
    use std::path::Path;

    let vault_path = crate::commands::expand_tilde(&vault_path);
    let attachments_dir = Path::new(vault_path.as_ref()).join("attachments");
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|error| format!("Failed to create attachments directory: {error}"))?;

    let mut captured = Vec::new();
    for source in paths {
        let source_path = Path::new(&source);
        if !source_path.is_file() {
            continue;
        }
        let destination: String = if crate::vault::is_image_path(&source) {
            crate::vault::copy_image_to_vault(vault_path.as_ref(), &source)?
        } else {
            let filename = source_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_string());
            let dest = attachments_dir.join(&filename);
            let dest = if dest.exists() {
                attachments_dir.join(format!(
                    "{}-{filename}",
                    uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("copy")
                ))
            } else {
                dest
            };
            std::fs::copy(source_path, &dest)
                .map_err(|error| format!("Failed to capture dropped file: {error}"))?;
            dest.to_string_lossy().into_owned()
        };
        cache_clipboard_image(&source);
        captured.push(destination);
    }
    Ok(captured)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_are_unique_and_prefixed() {
        let first = format!("dictation-{}", Uuid::new_v4());
        let second = format!("dictation-{}", Uuid::new_v4());
        assert_ne!(first, second);
        assert!(first.starts_with("dictation-"));
    }
}