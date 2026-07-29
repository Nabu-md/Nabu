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
use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2::ClassType;
use objc2_foundation::{NSObject, NSString};

objc2::extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    pub struct NSPasteboard;
);

objc2::extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    pub struct NSImage;
);

objc2::extern_class!(
    #[derive(Debug, PartialEq)]
    #[unsafe(super(NSObject))]
    pub struct NSData;
);

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
        #[cfg(target_os = "macos")]
        {
            self.read_macos_pasteboard()
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// macOS-specific pasteboard reading using objc2.
    #[cfg(target_os = "macos")]
    fn read_macos_pasteboard(&self) -> Option<(String, Vec<u8>)> {
        use objc2_foundation::NSString;

        // Obtain the general pasteboard via +generalPasteboard
        let pasteboard_class = NSPasteboard::class();
        let pasteboard: objc2::rc::Retained<NSPasteboard> =
            unsafe { objc2::msg_send![pasteboard_class, generalPasteboard] };

        // Try URL first (highest priority)
        if let Some(url_str) = Self::pasteboard_string_for_type(&pasteboard, "public.url") {
            if url_str.trim().starts_with("http") {
                return Some(("text/uri-list".to_string(), url_str.into_bytes()));
            }
        }

        // Try HTML (rich text)
        if let Some(html) = Self::pasteboard_string_for_type(&pasteboard, "public.html") {
            return Some(("text/html".to_string(), html.into_bytes()));
        }

        // Try plain text
        if let Some(text) = Self::pasteboard_string_for_type(&pasteboard, "public.utf8-plain-text")
        {
            return Some(("text/plain".to_string(), text.into_bytes()));
        }

        // Try image (PNG)
        if let Some(image_data) = Self::pasteboard_image_data(&pasteboard) {
            return Some(("image/png".to_string(), image_data));
        }

        None
    }

    /// Reads a string value from the pasteboard for the given type.
    #[cfg(target_os = "macos")]
    fn pasteboard_string_for_type(
        pasteboard: &objc2::rc::Retained<NSPasteboard>,
        type_name: &str,
    ) -> Option<String> {
        use objc2_foundation::NSString;

        let type_str = NSString::from_str(type_name);

        // Check if the pasteboard has this type
        // SAFETY: `pasteboard` is a valid `NSPasteboard`; `types` returns an
        // autoreleased `NSArray`.
        let types: objc2::rc::Retained<objc2_foundation::NSArray<objc2_foundation::NSObject>> =
            unsafe { objc2::msg_send![&**pasteboard, types] };

        let has_type: bool = unsafe {
            let msg = objc2::msg_send![types, containsObject: &*type_str];
            msg
        };

        if !has_type {
            return None;
        }

        // Read the string value
        let value: Option<objc2::rc::Retained<NSString>> = unsafe {
            let msg = objc2::msg_send![pasteboard, stringForType: &*type_str];
            msg
        };

        value.and_then(|v| {
            autoreleasepool(|pool| {
                // SAFETY: `v` is a valid `NSString` and `pool` is the current
                // autorelease pool. `to_str` returns a borrowed `&str` whose
                // lifetime is bounded by the pool; we copy it into an owned
                // `String` before the pool drains.
                let s = unsafe { v.to_str(pool) };
                Some(s.to_string())
            })
        })
    }

    /// Reads image data from the pasteboard as PNG bytes.
    #[cfg(target_os = "macos")]
    fn pasteboard_image_data(
        pasteboard: &objc2::rc::Retained<NSPasteboard>,
    ) -> Option<Vec<u8>> {
        use objc2_foundation::NSString;

        let type_str = NSString::from_str("public.png");

        // Check if the pasteboard has image data
        // SAFETY: `pasteboard` is a valid `NSPasteboard`; `types` returns an
        // autoreleased `NSArray`.
        let types: objc2::rc::Retained<objc2_foundation::NSArray<objc2_foundation::NSObject>> =
            unsafe { objc2::msg_send![&**pasteboard, types] };

        // SAFETY: `types` is a valid `NSArray`; `containsObject:` returns a
        // primitive `BOOL`.
        let has_type: bool = unsafe { objc2::msg_send![&*types, containsObject: &*type_str] };

        if !has_type {
            return None;
        }

        // Read the image
        // SAFETY: `pasteboard` is a valid `NSPasteboard`; `imageForType:`
        // returns an autoreleased `NSImage` (or `nil`).
        let image: Option<objc2::rc::Retained<NSImage>> =
            unsafe { objc2::msg_send![&**pasteboard, imageForType: &*type_str] };

        image.and_then(|img| {
            // Get TIFF representation first (NSImage -> TIFF)
            // SAFETY: `img` is a valid `NSImage`; `TIFFRepresentation` returns
            // an autoreleased `NSData` (or `nil`).
            let tiff_data: Option<objc2::rc::Retained<objc2_foundation::NSData>> =
                unsafe { objc2::msg_send![&*img, TIFFRepresentation] };

            tiff_data.and_then(|data| {
                // SAFETY: `data` is a valid `NSData`; `bytes` returns a raw
                // pointer to the underlying buffer and `length` returns its
                // size in bytes.
                let bytes_ptr: *const u8 = unsafe { objc2::msg_send![&*data, bytes] };
                let length: usize = unsafe { objc2::msg_send![&*data, length] };

                if bytes_ptr.is_null() || length == 0 {
                    return None;
                }

                // SAFETY: `bytes_ptr` is valid for `length` bytes as guaranteed
                // by the `NSData` contract. We immediately copy into a `Vec`.
                Some(unsafe { std::slice::from_raw_parts(bytes_ptr, length).to_vec() })
            })
        })
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