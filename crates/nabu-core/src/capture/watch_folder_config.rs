//! Configuration model for import folders (watch folders).
//!
//! This module provides the data structures needed to configure watch folders
//! for automatic inbox ingestion. The configuration is designed to be
//! future-compatible: new fields can be added without breaking existing
//! configurations.
//!
//! # Usage
//!
//! Configuration is stored as part of the vault configuration and is loaded
//! during system initialization. No settings UI is provided yet.
//!
//! # Future Compatibility
//!
//! - New fields can be added with `#[serde(default)]` to preserve backward
//!   compatibility.
//! - The `options` field on each folder allows handler-specific customization.
//! - The `WatchFolderConfig` struct can be embedded in a larger config.

use serde::{Deserialize, Serialize};

/// Top-level configuration for all import folders.
///
/// This struct is designed to be embedded in the vault configuration.
///
/// # Example
///
/// ```json
/// {
///   "folders": [
///     {
///       "path": "/Users/me/Nabu Inbox",
///       "enabled": true,
///       "recursive": true
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct WatchFolderConfig {
    /// List of configured import folders.
    #[serde(default)]
    pub folders: Vec<ImportFolder>,
}

impl Default for WatchFolderConfig {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
        }
    }
}

/// A single import folder configuration.
///
/// Each folder defines a directory that is monitored for new files.
/// When a supported file appears, it is automatically ingested into the inbox.
///
/// # Future Compatibility
///
/// New fields may be added without breaking existing configurations by using
/// `#[serde(default)]`. For example, a `debounce_ms` field could be added
/// to allow per-folder debounce settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct ImportFolder {
    /// Absolute path to the directory to watch.
    pub path: String,
    /// Whether this watch folder is currently active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether to watch subdirectories recursively.
    #[serde(default)]
    pub recursive: bool,
    /// Optional human-readable label for this folder.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional handler-specific options.
    ///
    /// This allows future extensions without schema changes.
    #[serde(default)]
    pub options: std::collections::HashMap<String, serde_json::Value>,
}

fn default_enabled() -> bool {
    true
}

impl ImportFolder {
    /// Creates a new import folder configuration.
    ///
    /// By default, the folder is enabled and non-recursive.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            enabled: true,
            recursive: false,
            label: None,
            options: std::collections::HashMap::new(),
        }
    }

    /// Sets whether this folder is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets whether to watch subdirectories recursively.
    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Sets a human-readable label for this folder.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Inserts a handler-specific option.
    pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_folder_defaults_to_enabled() {
        let folder = ImportFolder::new("/path/to/inbox");
        assert_eq!(folder.path, "/path/to/inbox");
        assert!(folder.enabled);
        assert!(!folder.recursive);
        assert!(folder.label.is_none());
        assert!(folder.options.is_empty());
    }

    #[test]
    fn import_folder_can_be_configured() {
        let folder = ImportFolder::new("/path/to/inbox")
            .with_enabled(false)
            .with_recursive(true)
            .with_label("My Inbox")
            .with_option("debounce_ms", serde_json::json!(500));

        assert_eq!(folder.path, "/path/to/inbox");
        assert!(!folder.enabled);
        assert!(folder.recursive);
        assert_eq!(folder.label, Some("My Inbox".to_string()));
        assert_eq!(
            folder.options.get("debounce_ms"),
            Some(&serde_json::json!(500))
        );
    }

    #[test]
    fn watch_folder_config_defaults_to_empty() {
        let config = WatchFolderConfig::default();
        assert!(config.folders.is_empty());
    }

    #[test]
    fn watch_folder_config_serializes_and_deserializes() {
        let config = WatchFolderConfig {
            folders: vec![
                ImportFolder::new("/inbox/a")
                    .with_recursive(true)
                    .with_label("Work Inbox"),
                ImportFolder::new("/inbox/b")
                    .with_enabled(false),
            ],
        };

        let serialized = serde_json::to_string_pretty(&config)
            .expect("Failed to serialize WatchFolderConfig");
        let deserialized: WatchFolderConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize WatchFolderConfig");

        assert_eq!(config, deserialized);
    }

    #[test]
    fn watch_folder_config_round_trip_preserves_options() {
        let folder = ImportFolder::new("/inbox")
            .with_option("custom_field", serde_json::json!("value"))
            .with_option("max_depth", serde_json::json!(3));

        let config = WatchFolderConfig {
            folders: vec![folder],
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: WatchFolderConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config, deserialized);
        let deserialized_folder = &deserialized.folders[0];
        assert_eq!(
            deserialized_folder.options.get("custom_field"),
            Some(&serde_json::json!("value"))
        );
        assert_eq!(
            deserialized_folder.options.get("max_depth"),
            Some(&serde_json::json!(3))
        );
    }

    #[test]
    fn deserialization_with_missing_fields_uses_defaults() {
        let json = r#"{
            "folders": [
                { "path": "/inbox" }
            ]
        }"#;
        let config: WatchFolderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.folders.len(), 1);
        let folder = &config.folders[0];
        assert_eq!(folder.path, "/inbox");
        // enabled should default to true
        assert!(folder.enabled);
        // recursive should default to false
        assert!(!folder.recursive);
        // label should default to None
        assert!(folder.label.is_none());
    }
}