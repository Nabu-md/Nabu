use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentVaultEntry {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub theme: String,
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<RecentVaultEntry>,
    #[serde(default)]
    pub main_window_opacity: f32,
    #[serde(default)]
    pub floating_pill_opacity: f32,
    #[serde(default)]
    pub whisper_model: String,
    #[serde(default)]
    pub enable_daily_notes: bool,
    #[serde(default)]
    pub extra_settings: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(default)]
    pub live_preview_mode: bool,
    #[serde(default)]
    pub auto_pair_brackets: bool,
    #[serde(default)]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub convert_html_to_md: bool,
    #[serde(default)]
    pub slash_menu_enabled: bool,
    #[serde(default)]
    pub voice_hotkey: String,
    #[serde(default)]
    pub auto_format_transcription: bool,
    #[serde(default)]
    pub pill_hover_focus: bool,
    #[serde(default)]
    pub new_note_path_strategy: String,
    #[serde(default)]
    pub trash_strategy: String,
    #[serde(default)]
    pub force_sandbox: bool,
    #[serde(default)]
    pub graph_include_folders: bool,
    #[serde(default)]
    pub graph_folder_click_behavior: String,
    #[serde(default)]
    pub graph_gravity: f32,
    #[serde(default)]
    pub graph_node_density: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            last_vault_path: "".to_string(),
            recent_vaults: Vec::new(),
            main_window_opacity: 1.0,
            floating_pill_opacity: 0.8,
            whisper_model: "ggml-tiny.en.bin".to_string(),
            enable_daily_notes: false,
            launch_at_startup: false,
            live_preview_mode: true,
            auto_pair_brackets: true,
            show_line_numbers: true,
            convert_html_to_md: true,
            slash_menu_enabled: true,
            voice_hotkey: "Cmd+Shift+D".to_string(),
            auto_format_transcription: true,
            pill_hover_focus: true,
            new_note_path_strategy: "vault_root".to_string(),
            trash_strategy: "system_trash".to_string(),
            force_sandbox: true,
            graph_include_folders: true,
            graph_folder_click_behavior: "table_view".to_string(),
            graph_gravity: 0.5,
            graph_node_density: 0.5,
            extra_settings: std::collections::HashMap::new(),
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("settings path is not absolute")]
    PathNotAbsolute,
    #[error("settings file missing")]
    Missing,
    #[error("malformed settings: {0}")]
    Malformed(String),
    #[error("write failed: {0}")]
    Write(String),
}

impl SettingsError {
    fn write<E: std::fmt::Display>(err: E) -> Self {
        SettingsError::Write(err.to_string())
    }
}

pub struct SettingsStore {
    path: PathBuf,
    #[allow(clippy::mutex_atomic)]
    inner: Mutex<AppSettings>,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let inner = Mutex::new(AppSettings::default());
        Self { path, inner }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, SettingsError> {
        let path = path.into();
        validate_path(&path)?;
        let settings = if path.exists() {
            read_settings(&path)?
        } else {
            AppSettings::default()
        };
        Ok(Self {
            path,
            inner: Mutex::new(settings),
        })
    }

    pub fn path(&self) -> &Path {
    pub fn get_settings(&self) -> AppSettings {
        self.inner.lock().unwrap().clone()
    }
        &self.path
    }

    pub fn get(&self) -> AppSettings {
        self.inner.lock().unwrap().clone()
    }

    pub fn set(&self, settings: AppSettings) {
        *self.inner.lock().unwrap() = settings;
    }

    pub fn update(
        &self,
        updater: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, SettingsError> {
        let mut guard = self.inner.lock().unwrap();
        updater(&mut guard);
        let updated = guard.clone();
        drop(guard);
        self.persist(&updated)?;
        self.set(updated.clone());
        Ok(updated)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<AppSettings, SettingsError> {
        self.persist(settings)?;
        self.set(settings.clone());
        Ok(settings.clone())
    }

    pub fn reset(&self) -> Result<AppSettings, SettingsError> {
        self.update(|settings| *settings = AppSettings::default())
    }

    fn persist(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        if !self.path.is_absolute() {
            return Err(SettingsError::PathNotAbsolute);
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(SettingsError::write)?;
        }

        let payload = serde_json::to_vec_pretty(settings).map_err(SettingsError::Malformed)?;
        std::fs::write(&self.path, payload).map_err(SettingsError::write)?;
        Ok(())
    }
    pub fn get_value(&self, key: &str) -> serde_json::Value {
        self.inner.lock().unwrap().extra_settings.get(key).cloned().unwrap_or(serde_json::Value::Null)
    }

    pub fn set_value(&self, key: &str, value: serde_json::Value) {
        self.inner.lock().unwrap().extra_settings.insert(key.to_string(), value);
    }

    pub fn get_feature_toggles(&self) -> serde_json::Value {
        self.get_value("featureToggles")
    }

    pub fn set_feature_toggle(&self, id: String, enabled: bool) -> serde_json::Value {
        let mut settings = self.inner.lock().unwrap();
        let toggles = settings.extra_settings.entry("featureToggles".to_string()).or_insert(serde_json::json!({}));
        toggles[id] = serde_json::json!(enabled);
        toggles.clone()
    }
}

fn validate_path(path: &Path) -> Result<(), SettingsError> {
    if path.as_os_str().is_empty() {
        return Err(SettingsError::PathNotAbsolute);
    }
    if !path.is_absolute() {
        return Err(SettingsError::PathNotAbsolute);
    }
    Ok(())
}

fn read_settings(path: &Path) -> Result<AppSettings, SettingsError> {
    let payload = std::fs::read_to_string(path).map_err(SettingsError::write)?;
    let settings: AppSettings =
        serde_json::from_str(&payload).map_err(|err| SettingsError::Malformed(err.to_string()))?;
    Ok(settings)
}

const MAX_RECENT_VAULTS: usize = 20;

pub fn update_recent_vaults(settings: &mut AppSettings, path: String, name: String) -> usize {
    let entry = RecentVaultEntry { path, name };
    let mut index = 0;
    settings.recent_vaults.retain(|item| {
        if item.path == entry.path {
            index += 1;
            false
        } else {
            true
        }
    });

    if index == 0 {
        settings.recent_vaults.insert(0, entry);
        index = 1;
    }

    if settings.recent_vaults.len() > MAX_RECENT_VAULTS {
        settings.recent_vaults.truncate(MAX_RECENT_VAULTS);
    }

    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nabu-settings-{name}.json"))
    }

    #[test]
    fn missing_file_returns_defaults() {
        let path = tmp_path("missing-file");
        let _ = std::fs::remove_file(&path);
        let store = SettingsStore::load(&path).unwrap();
        assert_eq!(store.get(), AppSettings::default());
    }

    #[test]
    fn crud_roundtrip_persists() {
        let path = tmp_path("roundtrip");
        let _ = std::fs::remove_file(&path);

        let store = Arc::new(Mutex::new(SettingsStore::load(&path).unwrap()));

        let updated = update_recent_vaults(
            &mut AppSettings {
                theme: String::from("dark"),
                ..Default::default()
            },
            "/vaults/alpha".into(),
            "Alpha".into(),
        );

        assert_eq!(updated, 1);
        let saved = store.lock().unwrap().save(&AppSettings {
            theme: String::from("dark"),
            last_vault_path: String::new(),
            recent_vaults: vec![RecentVaultEntry {
                path: "/vaults/alpha".into(),
                name: "Alpha".into(),
            }],
        }).unwrap();
        assert_eq!(saved.theme, "dark");

        let reloaded = SettingsStore::load(&path).unwrap();
        assert_eq!(reloaded.get().theme, "dark");
        assert_eq!(reloaded.get().recent_vaults.len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_json_returns_defaults() {
        let path = tmp_path("malformed");
        std::fs::write(&path, "not-json").unwrap();

        let result = SettingsStore::load(&path);
        assert!(matches!(result, Err(SettingsError::Malformed(_))));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn update_applies_and_persists() {
        let path = tmp_path("update");
        let _ = std::fs::remove_file(&path);

        let store = SettingsStore::load(&path).unwrap();
        let updated = store
            .update(|settings| {
                settings.theme = String::from("light");
                update_recent_vaults(settings, "/vaults/beta".into(), "Beta".into());
            })
            .unwrap();

        assert_eq!(updated.theme, "light");
        assert_eq!(updated.recent_vaults.len(), 1);

        let reloaded = SettingsStore::load(&path).unwrap();
        assert_eq!(reloaded.get().theme, "light");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reset_restores_defaults() {
        let path = tmp_path("reset");
        let store = SettingsStore::load(&path).unwrap();

        store
            .update(|settings| {
                settings.theme = String::from("dark");
            })
            .unwrap();
        store.reset().unwrap();

        let current = SettingsStore::load(&path).unwrap().get();
        assert_eq!(current.theme, AppSettings::default().theme);
        assert!(current.recent_vaults.is_empty());

        let _ = std::fs::remove_file(path);
    }
}
