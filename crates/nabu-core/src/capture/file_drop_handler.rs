use std::path::PathBuf;

use crate::capture::{CaptureHandler, CaptureRequest, CaptureResult, IngestionOptions, Normaliser};

/// Handles file drop capture events.
///
/// Validates the dropped file, detects its MIME type, and normalizes it into
/// an [`IngestionRequest`].
pub struct FileDropHandler {
    normaliser: Normaliser,
}

impl FileDropHandler {
    /// Creates a new `FileDropHandler` with a default normaliser.
    pub fn new() -> Self {
        Self {
            normaliser: Normaliser,
        }
    }
}

impl Default for FileDropHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for FileDropHandler {
    fn source_type(&self) -> &'static str {
        "file_drop"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "file_drop"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let file_path = match request.payload.get("file_path") {
            Some(serde_json::Value::String(path)) => PathBuf::from(path),
            _ => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some("Missing or invalid 'file_path' in payload".to_string()),
                    message: "Capture failed: invalid payload".to_string(),
                    payload: None,
                };
            }
        };

        let source_file = file_path.to_str().map(|s| s.to_string());
        let options = IngestionOptions {
            create_knowledge_object: true,
            extract_metadata: true,
            custom: request.context,
        };

        match self
            .normaliser
            .normalize(&file_path, &request.vault_id, source_file, options)
        {
            Ok(ingestion_request) => {
                let payload = serde_json::to_value(&ingestion_request)
                    .expect("IngestionRequest should always be serializable");
                CaptureResult {
                    success: true,
                    knowledge_object_id: None,
                    error: None,
                    message: format!("File '{}' captured successfully", file_path.display()),
                    payload: Some(payload),
                }
            }
            Err(e) => CaptureResult {
                success: false,
                knowledge_object_id: None,
                error: Some(e.to_string()),
                message: format!("Capture failed: {}", e),
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
        let dir = std::env::temp_dir().join("nabu_capture_filedrop_test");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join(name);
        fs::write(&file_path, content).unwrap();
        (dir, file_path)
    }

    #[test]
    fn capture_successful_text_file() {
        let (dir, file_path) = create_temp_file("drop.txt", b"Hello, world!");

        let handler = FileDropHandler::new();
        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({ "file_path": file_path.to_str() }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());
        assert!(result.message.contains("captured successfully"));

        let payload = result.payload.unwrap();
        let ingestion: IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.source, "file_drop");
        assert_eq!(ingestion.mime_type, "text/plain");
        assert_eq!(ingestion.raw_bytes, b"Hello, world!");
        assert_eq!(ingestion.vault_id, "vault-1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_missing_file_path() {
        let handler = FileDropHandler::new();
        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
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
        let handler = FileDropHandler::new();
        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({ "file_path": "/nonexistent/file.txt" }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn can_handle_filters_by_source_type() {
        let handler = FileDropHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn source_type_is_file_drop() {
        let handler = FileDropHandler::new();
        assert_eq!(handler.source_type(), "file_drop");
    }
}
