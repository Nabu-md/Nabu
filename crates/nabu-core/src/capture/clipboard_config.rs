//! Configuration model for clipboard monitoring.
//!
//! This module provides the data structures needed to configure clipboard
//! capture behaviour. The configuration is designed to be future-compatible:
//! new fields can be added without breaking existing configurations.
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
//! - The `options` field allows handler-specific customization.
//! - The `ClipboardMonitorConfig` struct can be embedded in a larger config.

use serde::{Deserialize, Serialize};

/// Monitoring modes for clipboard capture.
///
/// Controls how the clipboard monitor reacts to pasteboard changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardMonitorMode {
    /// Clipboard monitoring is disabled. No automatic captures occur.
    Disabled,
    /// Clipboard content is captured only when explicitly triggered
    /// (e.g., via a user action or Tauri command).
    Manual,
    /// Clipboard content is automatically captured whenever a new
    /// item is copied to the pasteboard.
    Automatic,
}

impl Default for ClipboardMonitorMode {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Top-level configuration for clipboard monitoring.
///
/// This struct is designed to be embedded in the vault configuration.
///
/// # Example
///
/// ```json
/// {
///   "mode": "automatic",
///   "poll_interval_ms": 500,
///   "capture_urls": true,
///   "capture_text": true,
///   "capture_images": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct ClipboardMonitorConfig {
    /// Monitoring mode.
    #[serde(default)]
    pub mode: ClipboardMonitorMode,

    /// Polling interval in milliseconds for detecting pasteboard changes.
    /// Only used when mode is Automatic.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,

    /// Whether to capture URLs from the clipboard.
    #[serde(default = "default_true")]
    pub capture_urls: bool,

    /// Whether to capture plain text from the clipboard.
    #[serde(default = "default_true")]
    pub capture_text: bool,

    /// Whether to capture images from the clipboard.
    #[serde(default = "default_true")]
    pub capture_images: bool,

    /// Optional handler-specific options.
    #[serde(default)]
    pub options: std::collections::HashMap<String, serde_json::Value>,
}

fn default_poll_interval() -> u64 {
    500
}

fn default_true() -> bool {
    true
}

impl Default for ClipboardMonitorConfig {
    fn default() -> Self {
        Self {
            mode: ClipboardMonitorMode::Disabled,
            poll_interval_ms: default_poll_interval(),
            capture_urls: default_true(),
            capture_text: default_true(),
            capture_images: default_true(),
            options: std::collections::HashMap::new(),
        }
    }
}

impl ClipboardMonitorConfig {
    /// Creates a new clipboard monitor configuration with the given mode.
    pub fn new(mode: ClipboardMonitorMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Returns whether the given content type should be captured based on
    /// the current configuration.
    pub fn should_capture_url(&self) -> bool {
        self.capture_urls && self.mode != ClipboardMonitorMode::Disabled
    }

    pub fn should_capture_text(&self) -> bool {
        self.capture_text && self.mode != ClipboardMonitorMode::Disabled
    }

    pub fn should_capture_image(&self) -> bool {
        self.capture_images && self.mode != ClipboardMonitorMode::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_disabled() {
        let config = ClipboardMonitorConfig::default();
        assert_eq!(config.mode, ClipboardMonitorMode::Disabled);
    }

    #[test]
    fn automatic_mode_allows_capture() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        assert!(config.should_capture_url());
        assert!(config.should_capture_text());
        assert!(config.should_capture_image());
    }

    #[test]
    fn disabled_mode_prevents_capture() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Disabled);
        assert!(!config.should_capture_url());
        assert!(!config.should_capture_text());
        assert!(!config.should_capture_image());
    }

    #[test]
    fn manual_mode_allows_capture() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Manual);
        assert!(config.should_capture_url());
        assert!(config.should_capture_text());
        assert!(config.should_capture_image());
    }

    #[test]
    fn custom_poll_interval() {
        let mut config = ClipboardMonitorConfig::default();
        config.poll_interval_ms = 1000;
        assert_eq!(config.poll_interval_ms, 1000);
    }

    #[test]
    fn capture_flags_can_be_disabled() {
        let mut config = ClipboardMonitorConfig::default();
        config.capture_urls = false;
        assert!(!config.should_capture_url());
        assert!(config.should_capture_text());
        assert!(config.should_capture_image());
    }

    #[test]
    fn serialization_round_trip() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: ClipboardMonitorConfig =
            serde_json::from_str(&serialized).expect("Failed to deserialize");
        assert_eq!(config, deserialized);
    }
}