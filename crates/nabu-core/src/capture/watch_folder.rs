//! Watch folder handler for automatic inbox ingestion.
//!
//! The [`WatchFolderHandler`] implements the [`CaptureHandler`] trait to process
//! files that appear in configured import folders. It reuses the existing
//! [`Normaliser`] for file validation, MIME detection, and byte reading.
//!
//! # Supported Inputs
//!
//! - PDF (`application/pdf`)
//! - Images (`image/jpeg`, `image/png`, `image/gif`, `image/webp`, `image/svg+xml`)
//! - Markdown (`text/markdown`)
//! - Plain text (`text/plain`)
//! - Office documents (`.doc`, `.docx`, `.xls`, `.xlsx`, `.ppt`, `.pptx`)
//! - HTML (`text/html`)
//!
//! Unsupported MIME types are silently ignored.
//!
//! # Architecture
//!
//! ```text
//! Folder change (notify)
//!     ↓
//! WatchFolderHandler
//!     ↓
//! CaptureEngine::ingest
//!     ↓
//! ItemCaptured (event)
//!     ↓
//! IngestionPipeline → ProcessingPipeline → StorageManager
//! ```

use std::path::Path;

use crate::capture::{
    CaptureHandler, CaptureRequest, CaptureResult, IngestionOptions, Normaliser,
};

/// Supported MIME types for watch folder ingestion.
///
/// Files with MIME types not in this set are silently ignored.
const SUPPORTED_MIME_TYPES: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/html",
    "application/pdf",
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

/// Handles files detected in configured watch folders.
///
/// This handler is registered with the [`CaptureEngine`] under the source type
/// `"watch_folder"`. It reuses the [`Normaliser`] for file processing and
/// silently ignores unsupported file types.
///
/// # Payload Format
///
/// The handler expects the `payload` field of [`CaptureRequest`] to contain:
///
/// ```json
/// {
///   "file_path": "/absolute/path/to/file",
///   "folder_id": "optional-folder-identifier"
/// }
/// ```
///
/// The `folder_id` is optional and allows tracking which watch folder
/// triggered the capture.
pub struct WatchFolderHandler {
    normaliser: Normaliser,
}

impl WatchFolderHandler {
    /// Creates a new `WatchFolderHandler`.
    pub fn new() -> Self {
        Self {
            normaliser: Normaliser,
        }
    }

    /// Returns the list of supported MIME types.
    ///
    /// Useful for logging and configuration validation.
    pub fn supported_mime_types() -> &'static [&'static str] {
        SUPPORTED_MIME_TYPES
    }

    /// Checks whether the given MIME type is supported for watch folder ingestion.
    pub fn is_supported_mime(mime: &str) -> bool {
        SUPPORTED_MIME_TYPES.contains(&mime)
    }

    /// Checks whether the file at the given path has a supported MIME type.
    ///
    /// Returns `true` if the file's extension maps to a supported MIME type.
    /// This is a lightweight check that does not read the file.
    pub fn is_supported_file(path: &Path) -> bool {
        let normaliser = Normaliser;
        let mime = normaliser.detect_mime_type(path);
        Self::is_supported_mime(&mime)
    }
}

impl Default for WatchFolderHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for WatchFolderHandler {
    fn source_type(&self) -> &'static str {
        "watch_folder"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "watch_folder"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let file_path = match request.payload.get("file_path") {
            Some(serde_json::Value::String(path)) => std::path::PathBuf::from(path),
            _ => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some("Missing or invalid 'file_path' in payload".to_string()),
                    message: "Watch folder capture failed: invalid payload".to_string(),
                    payload: None,
                };
            }
        };

        // Silently ignore unsupported file types
        if !Self::is_supported_file(&file_path) {
            return CaptureResult {
                success: false,
                knowledge_object_id: None,
                error: None,
                message: format!(
                    "Unsupported file type: {}",
                    file_path.display()
                ),
                payload: None,
            };
        }

        let source_file = file_path.to_str().map(|s| s.to_string());
        let options = IngestionOptions {
            create_knowledge_object: true,
            extract_metadata: true,
            custom: request.context,
        };

        match self.normaliser.normalize(
            self.source_type(),
            &file_path,
            &request.vault_id,
            source_file,
            options,
        ) {
            Ok(ingestion_request) => {
                let payload = match serde_json::to_value(&ingestion_request) {
                    Ok(p) => p,
                    Err(e) => {
                        return CaptureResult {
                            success: false,
                            knowledge_object_id: None,
                            error: Some(format!(
                                "Failed to serialize ingestion request: {}",
                                e
                            )),
                            message: "Watch folder capture failed: serialization error"
                                .to_string(),
                            payload: None,
                        };
                    }
                };
                CaptureResult {
                    success: true,
                    knowledge_object_id: None,
                    error: None,
                    message: format!(
                        "File '{}' captured from watch folder",
                        file_path.display()
                    ),
                    payload: Some(payload),
                }
            }
            Err(e) => CaptureResult {
                success: false,
                knowledge_object_id: None,
                error: Some(e.to_string()),
                message: format!("Watch folder capture failed: {}", e),
                payload: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::IngestionRequest;
    use std::collections::HashMap;
    use std::fs;

    fn create_temp_file(name: &str, content: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("nabu_watch_folder_test");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join(name);
        fs::write(&file_path, content).unwrap();
        (dir, file_path)
    }

    #[test]
    fn source_type_is_watch_folder() {
        let handler = WatchFolderHandler::new();
        assert_eq!(handler.source_type(), "watch_folder");
    }

    #[test]
    fn can_handle_filters_by_source_type() {
        let handler = WatchFolderHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn capture_supported_text_file() {
        let (dir, file_path) = create_temp_file("inbox.txt", b"Hello, Nabu!");

        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.source, "watch_folder");
        assert_eq!(ingestion.mime_type, "text/plain");
        assert_eq!(ingestion.raw_bytes, b"Hello, Nabu!");
        assert_eq!(ingestion.vault_id, "vault-1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_supported_markdown_file() {
        let (dir, file_path) = create_temp_file("note.md", b"# Title\n\nBody");

        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.mime_type, "text/markdown");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_supported_pdf_file() {
        let (dir, file_path) = create_temp_file("doc.pdf", b"%PDF-1.4");

        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.mime_type, "application/pdf");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_supported_image_file() {
        let (dir, file_path) = create_temp_file("photo.png", b"\x89PNG\r\n\x1a\n");

        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.mime_type, "image/png");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_unsupported_file_type_is_ignored() {
        let (dir, file_path) = create_temp_file("data.zip", b"PK\x03\x04");

        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        // Unsupported types return success=false with no error (silently ignored)
        assert!(!result.success);
        assert!(result.error.is_none());
        assert!(result.message.contains("Unsupported file type"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_missing_file_path() {
        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("file_path"));
    }

    #[test]
    fn capture_nonexistent_file() {
        let handler = WatchFolderHandler::new();
        let request = CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({ "file_path": "/nonexistent/file.txt" }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn is_supported_mime_returns_true_for_supported_types() {
        assert!(WatchFolderHandler::is_supported_mime("text/plain"));
        assert!(WatchFolderHandler::is_supported_mime("text/markdown"));
        assert!(WatchFolderHandler::is_supported_mime("application/pdf"));
        assert!(WatchFolderHandler::is_supported_mime("image/png"));
        assert!(WatchFolderHandler::is_supported_mime("image/jpeg"));
    }

    #[test]
    fn is_supported_mime_returns_false_for_unsupported_types() {
        assert!(!WatchFolderHandler::is_supported_mime("application/zip"));
        assert!(!WatchFolderHandler::is_supported_mime("audio/mpeg"));
        assert!(!WatchFolderHandler::is_supported_mime("video/mp4"));
        assert!(!WatchFolderHandler::is_supported_mime("application/octet-stream"));
    }

    #[test]
    fn is_supported_file_checks_by_extension() {
        let dir = std::env::temp_dir().join("nabu_wf_support_test");
        let _ = fs::create_dir_all(&dir);

        let txt = dir.join("test.txt");
        fs::write(&txt, "hello").unwrap();
        assert!(WatchFolderHandler::is_supported_file(&txt));

        let zip = dir.join("data.zip");
        fs::write(&zip, "PK").unwrap();
        assert!(!WatchFolderHandler::is_supported_file(&zip));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn supported_mime_types_contains_expected_types() {
        let types = WatchFolderHandler::supported_mime_types();
        assert!(types.contains(&"text/plain"));
        assert!(types.contains(&"text/markdown"));
        assert!(types.contains(&"application/pdf"));
        assert!(types.contains(&"image/jpeg"));
        assert!(types.contains(&"image/png"));
        assert!(types.contains(&"application/msword"));
        assert!(types.contains(
            &"application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        ));
    }
}