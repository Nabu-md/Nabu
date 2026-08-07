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
    // ── Appearance ────────────────────────────────────────────────────
    pub theme: String,
    pub main_window_opacity: f32,
    pub floating_pill_opacity: f32,
    pub pill_hover_boost_opacity: bool,
    pub sidebar_width: f32,
    pub inspector_width: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub reduced_motion: bool,
    pub high_contrast: bool,

    // ── Editor ───────────────────────────────────────────────────────
    pub editor_mode: String,
    pub auto_pair_brackets: bool,
    pub show_line_numbers: bool,
    pub convert_pasted_html_to_markdown: bool,
    pub enable_notion_slash_menu: bool,
    pub auto_format_filler_words: bool,
    pub tab_size: u32,
    pub word_wrap: bool,
    pub spell_check: bool,
    pub auto_save_interval_secs: u32,

    // ── Markdown ─────────────────────────────────────────────────────
    pub markdown_gfm: bool,
    pub markdown_preserve_line_breaks: bool,
    pub markdown_smart_quotes: bool,
    pub markdown_math_rendering: bool,
    pub markdown_diagram_rendering: bool,

    // ── Search ───────────────────────────────────────────────────────
    pub search_index_on_startup: bool,
    pub search_max_results: u32,
    pub search_highlight_matches: bool,
    pub search_fuzzy_matching: bool,

    // ── Graph ────────────────────────────────────────────────────────
    pub include_folders_in_graph: bool,
    pub folder_click_behavior: String,
    pub graph_node_physics_gravity: f32,
    pub graph_node_physics_spacing: f32,
    pub graph_show_tags_as_badges: bool,

    // ── Files & Vaults ───────────────────────────────────────────────
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<RecentVaultEntry>,
    pub default_new_note_path: String,
    pub trash_retention_policy: String,
    pub enable_daily_notes: bool,
    pub confirm_before_delete: bool,
    pub show_hidden_files: bool,
    pub sort_files_alphabetically: bool,

    // ── Import & Export ──────────────────────────────────────────────
    pub default_export_format: String,
    pub export_include_metadata: bool,
    pub export_include_attachments: bool,
    pub import_duplicate_strategy: String,

    // ── OCR ──────────────────────────────────────────────────────────
    pub ocr_language: String,
    pub ocr_auto_process_scanned_pdfs: bool,
    pub ocr_confidence_threshold: f32,

    // ── Accessibility ────────────────────────────────────────────────
    pub screen_reader_support: bool,
    pub keyboard_navigation: bool,
    pub focus_ring_visible: bool,

    // ── Performance ──────────────────────────────────────────────────
    pub max_undo_history: u32,
    pub worker_pool_size: u32,
    pub index_on_startup: bool,
    pub background_processing: bool,

    // ── Privacy ──────────────────────────────────────────────────────
    pub launch_at_startup: bool,
    pub analytics_enabled: bool,
    pub crash_reporting_enabled: bool,
    pub auto_lock_on_idle: bool,
    pub auto_lock_timeout_mins: u32,

    // ── Keyboard Shortcuts ───────────────────────────────────────────
    pub voice_hotkey: String,
    pub quick_capture_hotkey: String,
    pub toggle_sidebar_hotkey: String,

    // ── Advanced ─────────────────────────────────────────────────────
    pub force_sandbox_for_web_snippets: bool,
    pub debug_mode: bool,
    pub developer_tools: bool,
    pub experimental_features: bool,

    // ── Experimental ─────────────────────────────────────────────────
    pub whisper_model: String,
    pub enable_ai_summarization: bool,
    pub enable_semantic_search: bool,

    // ── Extras (backwards-compat) ────────────────────────────────────
    #[serde(default)]
    pub extra_settings: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Appearance
            theme: "system".to_string(),
            main_window_opacity: 1.0,
            floating_pill_opacity: 0.8,
            pill_hover_boost_opacity: true,
            sidebar_width: 280.0,
            inspector_width: 320.0,
            font_size: 14.0,
            line_height: 1.6,
            reduced_motion: false,
            high_contrast: false,

            // Editor
            editor_mode: "Live Preview".to_string(),
            auto_pair_brackets: true,
            show_line_numbers: true,
            convert_pasted_html_to_markdown: true,
            enable_notion_slash_menu: true,
            auto_format_filler_words: true,
            tab_size: 4,
            word_wrap: true,
            spell_check: true,
            auto_save_interval_secs: 30,

            // Markdown
            markdown_gfm: true,
            markdown_preserve_line_breaks: false,
            markdown_smart_quotes: true,
            markdown_math_rendering: true,
            markdown_diagram_rendering: true,

            // Search
            search_index_on_startup: true,
            search_max_results: 100,
            search_highlight_matches: true,
            search_fuzzy_matching: false,

            // Graph
            include_folders_in_graph: true,
            folder_click_behavior: "Open Folder Table View".to_string(),
            graph_node_physics_gravity: 0.5,
            graph_node_physics_spacing: 1.0,
            graph_show_tags_as_badges: true,

            // Files & Vaults
            last_vault_path: "".to_string(),
            recent_vaults: Vec::new(),
            default_new_note_path: "Vault Root".to_string(),
            trash_retention_policy: "30 Days".to_string(),
            enable_daily_notes: true,
            confirm_before_delete: true,
            show_hidden_files: false,
            sort_files_alphabetically: true,

            // Import & Export
            default_export_format: "markdown".to_string(),
            export_include_metadata: true,
            export_include_attachments: true,
            import_duplicate_strategy: "skip".to_string(),

            // OCR
            ocr_language: "eng".to_string(),
            ocr_auto_process_scanned_pdfs: true,
            ocr_confidence_threshold: 0.7,

            // Accessibility
            screen_reader_support: false,
            keyboard_navigation: true,
            focus_ring_visible: true,

            // Performance
            max_undo_history: 100,
            worker_pool_size: 4,
            index_on_startup: true,
            background_processing: true,

            // Privacy
            launch_at_startup: false,
            analytics_enabled: false,
            crash_reporting_enabled: true,
            auto_lock_on_idle: false,
            auto_lock_timeout_mins: 15,

            // Keyboard Shortcuts
            voice_hotkey: "Cmd+Shift+D".to_string(),
            quick_capture_hotkey: "Cmd+Shift+Space".to_string(),
            toggle_sidebar_hotkey: "Cmd+B".to_string(),

            // Advanced
            force_sandbox_for_web_snippets: true,
            debug_mode: false,
            developer_tools: false,
            experimental_features: false,

            // Experimental
            whisper_model: "ggml-base.en.bin".to_string(),
            enable_ai_summarization: false,
            enable_semantic_search: false,

            // Extras
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

/// Phase 15.1 — import/export settings in a versioned envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsExport {
    pub version: String,
    pub exported_at: String,
    pub platform: String,
    pub settings: AppSettings,
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
        &self.path
    }

    pub fn get_settings(&self) -> AppSettings {
        self.inner.lock().unwrap().clone()
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

    /// Serialize settings to JSON for export.
    pub fn export_settings(&self) -> Result<SettingsExport, SettingsError> {
        let settings = self.get();
        Ok(SettingsExport {
            version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            platform: std::env::consts::OS.to_string(),
            settings,
        })
    }

    /// Import settings from a [`SettingsExport`] envelope, validating the
    /// version field to prevent loading incompatible exports.
    pub fn import_settings(&self, payload: &[u8]) -> Result<AppSettings, SettingsError> {
        let export: SettingsExport = serde_json::from_slice(payload)
            .map_err(|e| SettingsError::Malformed(e.to_string()))?;
        // We accept any 0.x version for now; tighten as we evolve schema.
        if !export.version.starts_with('0') {
            return Err(SettingsError::Malformed(format!(
                "Unsupported settings export version: {}",
                export.version
            )));
        }
        self.save(&export.settings)
    }

    fn persist(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        if !self.path.is_absolute() {
            return Err(SettingsError::PathNotAbsolute);
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(SettingsError::write)?;
        }
        let payload = serde_json::to_vec_pretty(settings)
            .map_err(|e| SettingsError::Malformed(e.to_string()))?;
        std::fs::write(&self.path, payload).map_err(SettingsError::write)?;
        Ok(())
    }
    pub fn get_value(&self, key: &str) -> serde_json::Value {
        self.inner
            .lock()
            .unwrap()
            .extra_settings
            .get(key)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    pub fn set_value(&self, key: &str, value: serde_json::Value) {
        self.inner
            .lock()
            .unwrap()
            .extra_settings
            .insert(key.to_string(), value);
    }

    pub fn get_feature_toggles(&self) -> serde_json::Value {
        self.get_value("featureToggles")
    }

    pub fn set_feature_toggle(&self, id: String, enabled: bool) -> serde_json::Value {
        let mut settings = self.inner.lock().unwrap();
        let toggles = settings
            .extra_settings
            .entry("featureToggles".to_string())
            .or_insert(serde_json::json!({}));
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
    let mut settings: AppSettings =
        serde_json::from_str(&payload).map_err(|err| SettingsError::Malformed(err.to_string()))?;
    // Phase 11.2: the trash retention policy changed from legacy values
    // ("Move to System Trash" / ".trash Vault Folder" / "Permanently Delete")
    // to retention periods. Normalize old values so the settings UI shows a
    // valid option and retention stays safe (never auto-purge by default).
    if !matches!(
        settings.trash_retention_policy.as_str(),
        "7 Days" | "30 Days" | "90 Days" | "Never"
    ) {
        settings.trash_retention_policy = "Never".to_string();
    }
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
        let saved = store
            .lock()
            .unwrap()
            .save(&AppSettings {
                theme: String::from("dark"),
                last_vault_path: String::new(),
                recent_vaults: vec![RecentVaultEntry {
                    path: "/vaults/alpha".into(),
                    name: "Alpha".into(),
                }],
                ..Default::default()
            })
            .unwrap();
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
