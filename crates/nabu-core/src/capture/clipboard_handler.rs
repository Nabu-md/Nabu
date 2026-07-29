//! Clipboard capture handler for macOS NSPasteboard integration.
//!
//! The [`ClipboardHandler`] implements the [`CaptureHandler`] trait to process
//! clipboard content (URLs, plain text, rich text, and images) and produce
//! canonical [`IngestionRequest`] objects that flow through the existing
//! [`CaptureEngine`] and [`IngestionPipeline`].
//!
//! # Architecture
//!
//! ```text
//! NSPasteboard change
//!     ↓
//! ClipboardHandler (CaptureHandler)
//!     ↓
//! CaptureEngine::ingest
//!     ↓
//! ItemCaptured (event)
//!     ↓
//! IngestionPipeline → ProcessingPipeline → StorageManager
//! ```
//!
//! # Supported Clipboard Types
//!
//! - **URL**: Valid web URLs captured as Bookmark KnowledgeObjects
//! - **Plain text**: Captured as Note KnowledgeObjects
//! - **Rich text (HTML)**: Captured as Note KnowledgeObjects with HTML content
//! - **Images**: Captured as Image KnowledgeObjects
//!
//! Unsupported clipboard contents are silently ignored.
//!
//! # Configuration
//!
//! Clipboard monitoring is controlled by [`ClipboardMonitorConfig`] which
//! supports three modes: Disabled, Manual, and Automatic.
//!
//! # Error Handling
//!
//! Clipboard failures never crash the application. All errors are returned
//! as failed [`CaptureResult`] instances.

use std::collections::HashMap;

use crate::capture::{
    CaptureHandler, CaptureRequest, CaptureResult, ClipboardMonitorConfig, ClipboardMonitorMode,
    IngestionOptions, IngestionRequest,
};
use crate::native::clipboard::{self, ClipboardContent};

/// Handles clipboard capture requests on macOS.
///
/// This handler reads the current NSPasteboard content and produces
/// [`IngestionRequest`] objects for downstream processing. It supports
/// URLs, plain text, rich text (HTML), and images.
///
/// The handler is registered with the [`CaptureEngine`] under the source type
/// `"clipboard"`.
///
/// # Platform Support
///
/// On macOS, the handler reads from NSPasteboard using the `objc2`
/// framework. On non-macOS platforms, the handler returns a failed
/// capture result indicating that clipboard capture is not supported.
pub struct ClipboardHandler {
    config: ClipboardMonitorConfig,
}

impl ClipboardHandler {
    /// Creates a new clipboard handler with default configuration.
    pub fn new() -> Self {
        Self {
            config: ClipboardMonitorConfig::default(),
        }
    }

    /// Creates a new clipboard handler with the given configuration.
    pub fn with_config(config: ClipboardMonitorConfig) -> Self {
        Self { config }
    }

    /// Returns the current clipboard monitor configuration.
    pub fn config(&self) -> &ClipboardMonitorConfig {
        &self.config
    }

    /// Updates the clipboard monitor configuration.
    pub fn set_config(&mut self, config: ClipboardMonitorConfig) {
        self.config = config;
    }

    /// Reads the current pasteboard content and returns the best
    /// available (mime_type, raw_bytes) pair.
    ///
    /// Returns `None` if no supported content is available or if
    /// the pasteboard cannot be accessed.
    fn read_clipboard_content(&self) -> Option<(String, Vec<u8>)> {
        let content = clipboard::read_clipboard();

        match content {
            ClipboardContent::Text(text) => {
                Some(("text/plain".to_string(), text.into_bytes()))
            }
            ClipboardContent::Html(html) => {
                Some(("text/html".to_string(), html.into_bytes()))
            }
            ClipboardContent::Url(url) => {
                Some(("text/uri-list".to_string(), url.into_bytes()))
            }
            ClipboardContent::Image(data) => {
                Some(("image/png".to_string(), data))
            }
            ClipboardContent::None => None,
        }
    }
}

impl Default for ClipboardHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for ClipboardHandler {
    fn source_type(&self) -> &'static str {
        "clipboard"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "clipboard"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        // Check if clipboard monitoring is enabled for this request
        if self.config.mode == ClipboardMonitorMode::Disabled {
                return CaptureResult {
                    success: false,
                    knowledge_object: None,
                    knowledge_object_id: None,
                    payload: None,
                    error: Some("Clipboard monitoring is disabled".to_string()),
                    message: "Clipboard capture skipped: monitoring is disabled".to_string(),
                };
            }

        // Read clipboard content
        let content = match self.read_clipboard_content() {
            Some(c) => c,
            None => {
                return CaptureResult {
                    success: false,
                    knowledge_object: None,
                    knowledge_object_id: None,
                    payload: None,
                    error: None,
                    message: "No supported clipboard content available".to_string(),
                };
            }
        };

        let (mime_type, raw_bytes) = content;

        // Build custom metadata based on content type
        let mut custom = HashMap::new();
        custom.insert("capture_type".to_string(), serde_json::json!("clipboard"));

        let options = IngestionOptions {
            create_knowledge_object: true,
            extract_metadata: true,
            custom,
        };

        let ingestion_request = IngestionRequest {
            source: "clipboard".to_string(),
            raw_bytes,
            mime_type,
            vault_id: request.vault_id.clone(),
            source_file: None,
            options,
        };

        let payload = match serde_json::to_value(&ingestion_request) {
            Ok(p) => p,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object: None,
                    knowledge_object_id: None,
                    payload: None,
                    error: Some(format!(
                        "Failed to serialize ingestion request: {}",
                        e
                    )),
                    message: "Clipboard capture failed: serialization error".to_string(),
                };
            }
        };

        CaptureResult {
            success: true,
            knowledge_object: None,
            knowledge_object_id: None,
            payload: None,
            error: None,
            message: "Clipboard content captured".to_string(),
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureRequest;
    use std::collections::HashMap;

    #[test]
    fn source_type_is_clipboard() {
        let handler = ClipboardHandler::new();
        assert_eq!(handler.source_type(), "clipboard");
    }

    #[test]
    fn can_handle_clipboard_requests() {
        let handler = ClipboardHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "clipboard".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn default_config_is_disabled() {
        let handler = ClipboardHandler::new();
        assert_eq!(handler.config.mode, ClipboardMonitorMode::Disabled);
    }

    #[test]
    fn config_can_be_updated() {
        let mut handler = ClipboardHandler::new();
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        handler.set_config(config);
        assert_eq!(handler.config.mode, ClipboardMonitorMode::Automatic);
    }

    #[test]
    fn disabled_mode_returns_error() {
        let handler = ClipboardHandler::new();
        let request = CaptureRequest {
            source_type: "clipboard".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };
        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result
            .error
            .unwrap()
            .contains("Clipboard monitoring is disabled"));
    }

    #[test]
    fn handler_clone_works() {
        let mut handler = ClipboardHandler::new();
        handler.set_config(ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic));
        let cloned = handler.clone();
        assert_eq!(cloned.config.mode, ClipboardMonitorMode::Automatic);
    }
}