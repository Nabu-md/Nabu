//! Browser capture handler for Safari Web Extension integration.
//!
//! The [`BrowserCaptureHandler`] implements the [`CaptureHandler`] trait to process
//! capture requests from the Safari extension. It converts browser capture data
//! into canonical [`IngestionRequest`] objects that flow through the existing
//! [`CaptureEngine`] and [`IngestionPipeline`].
//!
//! # Architecture
//!
//! ```text
//! Safari Extension
//!     ↓
//! Native Messaging Host
//!     ↓
//! Tauri Command
//!     ↓
//! BrowserCaptureHandler
//!     ↓
//! CaptureEngine::ingest
//!     ↓
//! ItemCaptured (event)
//!     ↓
//! IngestionPipeline → ProcessingPipeline → StorageManager
//! ```
//!
//! # Supported Capture Types
//!
//! - **Bookmark**: URL, page title, favicon (when available)
//! - **Note**: Selected text, source URL, page title
//! - **Document**: Complete HTML, page URL, title
//!
//! The browser only transfers raw data. Conversion into Markdown or further
//! processing occurs inside Nabu's processing pipeline.

use std::collections::HashMap;

use crate::capture::{CaptureHandler, CaptureRequest, CaptureResult, IngestionOptions};

/// Handler for browser capture requests from the Safari extension.
///
/// This handler is registered with the [`CaptureEngine`] under the source type
/// `"browser"`. It processes three capture modes:
///
/// - `bookmark`: Captures URL, title, and favicon
/// - `note`: Captures selected text with source URL and title
/// - `document`: Captures complete HTML with URL and title
///
/// # Payload Format
///
/// The handler expects the `payload` field of [`CaptureRequest`] to contain:
///
/// ```json
/// {
///   "captureType": "bookmark|note|document",
///   "url": "https://example.com",
///   "title": "Page Title",
///   "html": "<html>...</html>",
///   "selectedText": "Selected text content",
///   "favicon": "https://example.com/favicon.ico"
/// }
/// ```
///
/// Fields vary by capture type:
/// - `bookmark`: requires `url` and `title`
/// - `note`: requires `selectedText`, `url`, and `title`
/// - `document`: requires `html`, `url`, and `title`
pub struct BrowserCaptureHandler;

impl BrowserCaptureHandler {
    /// Creates a new browser capture handler.
    pub fn new() -> Self {
        Self
    }

    /// Extracts the capture type from the payload.
    fn get_capture_type(payload: &serde_json::Value) -> Result<&str, String> {
        payload
            .get("captureType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing or invalid 'captureType' in payload".to_string())
    }

    /// Creates an ingestion request for a bookmark capture.
    fn create_bookmark_request(
        &self,
        payload: &serde_json::Value,
        vault_id: &str,
    ) -> Result<crate::capture::IngestionRequest, String> {
        let url = payload
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'url' for bookmark capture".to_string())?;
        
        let title = payload
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'title' for bookmark capture".to_string())?;
        
        let favicon = payload
            .get("favicon")
            .and_then(|v| v.as_str());

        // Create a structured representation of the bookmark
        let mut bookmark_data = serde_json::json!({
            "url": url,
            "title": title,
        });
        
        if let Some(favicon_url) = favicon {
            bookmark_data["favicon"] = serde_json::json!(favicon_url);
        }

        let raw_bytes = serde_json::to_vec(&bookmark_data)
            .map_err(|e| format!("Failed to serialize bookmark data: {}", e))?;

        let mut custom = HashMap::new();
        custom.insert("capture_type".to_string(), serde_json::json!("bookmark"));
        custom.insert("source_url".to_string(), serde_json::json!(url));
        if let Some(favicon_url) = favicon {
            custom.insert("favicon".to_string(), serde_json::json!(favicon_url));
        }

        Ok(crate::capture::IngestionRequest {
            source: "browser".to_string(),
            raw_bytes,
            mime_type: "application/x-nabu-bookmark".to_string(),
            vault_id: vault_id.to_string(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        })
    }

    /// Creates an ingestion request for a note capture.
    fn create_note_request(
        &self,
        payload: &serde_json::Value,
        vault_id: &str,
    ) -> Result<crate::capture::IngestionRequest, String> {
        let selected_text = payload
            .get("selectedText")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'selectedText' for note capture".to_string())?;
        
        let url = payload
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'url' for note capture".to_string())?;
        
        let title = payload
            .get("title")
            .and_then(|v| v.as_str());

        let raw_bytes = selected_text.as_bytes().to_vec();

        let mut custom = HashMap::new();
        custom.insert("capture_type".to_string(), serde_json::json!("note"));
        custom.insert("source_url".to_string(), serde_json::json!(url));
        if let Some(page_title) = title {
            custom.insert("page_title".to_string(), serde_json::json!(page_title));
        }

        Ok(crate::capture::IngestionRequest {
            source: "browser".to_string(),
            raw_bytes,
            mime_type: "text/plain".to_string(),
            vault_id: vault_id.to_string(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        })
    }

    /// Creates an ingestion request for a document capture.
    fn create_document_request(
        &self,
        payload: &serde_json::Value,
        vault_id: &str,
    ) -> Result<crate::capture::IngestionRequest, String> {
        let html = payload
            .get("html")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'html' for document capture".to_string())?;
        
        let url = payload
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'url' for document capture".to_string())?;
        
        let title = payload
            .get("title")
            .and_then(|v| v.as_str());

        let raw_bytes = html.as_bytes().to_vec();

        let mut custom = HashMap::new();
        custom.insert("capture_type".to_string(), serde_json::json!("document"));
        custom.insert("source_url".to_string(), serde_json::json!(url));
        if let Some(page_title) = title {
            custom.insert("page_title".to_string(), serde_json::json!(page_title));
        }

        Ok(crate::capture::IngestionRequest {
            source: "browser".to_string(),
            raw_bytes,
            mime_type: "text/html".to_string(),
            vault_id: vault_id.to_string(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        })
    }
}

impl Default for BrowserCaptureHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for BrowserCaptureHandler {
    fn source_type(&self) -> &'static str {
        "browser"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "browser"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let capture_type = match Self::get_capture_type(&request.payload) {
            Ok(ct) => ct,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    knowledge_object: None,
                    error: Some(e),
                    message: "Browser capture failed: invalid payload".to_string(),
                    payload: None,
                };
            }
        };

        let ingestion_request = match capture_type {
            "bookmark" => self.create_bookmark_request(&request.payload, &request.vault_id),
            "note" => self.create_note_request(&request.payload, &request.vault_id),
            "document" => self.create_document_request(&request.payload, &request.vault_id),
            _ => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    knowledge_object: None,
                    error: Some(format!("Unsupported capture type: {}", capture_type)),
                    message: format!("Browser capture failed: unsupported type '{}'", capture_type),
                    payload: None,
                };
            }
        };

        match ingestion_request {
            Ok(req) => {
                let payload = match serde_json::to_value(&req) {
                    Ok(p) => p,
                    Err(e) => {
                        return CaptureResult {
                            success: false,
                            knowledge_object_id: None,
                            knowledge_object: None,
                            error: Some(format!("Failed to serialize ingestion request: {}", e)),
                            message: "Browser capture failed: serialization error".to_string(),
                            payload: None,
                        };
                    }
                };

                CaptureResult {
                    success: true,
                    knowledge_object_id: None,
                    knowledge_object: None,
                    error: None,
                    message: format!("Browser {} captured successfully", capture_type),
                    payload: Some(payload),
                }
            }
            Err(e) => CaptureResult {
                success: false,
                knowledge_object_id: None,
                knowledge_object: None,
                error: Some(e),
                message: format!("Browser {} capture failed", capture_type),
                payload: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn source_type_is_browser() {
        let handler = BrowserCaptureHandler::new();
        assert_eq!(handler.source_type(), "browser");
    }

    #[test]
    fn can_handle_filters_by_source_type() {
        let handler = BrowserCaptureHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "browser".to_string(),
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
    fn capture_bookmark_creates_ingestion_request() {
        let handler = BrowserCaptureHandler::new();
        let request = CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({
                "captureType": "bookmark",
                "url": "https://example.com",
                "title": "Example Domain",
                "favicon": "https://example.com/favicon.ico"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: crate::capture::IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.source, "browser");
        assert_eq!(ingestion.mime_type, "application/x-nabu-bookmark");
        assert_eq!(ingestion.vault_id, "vault-1");
        assert_eq!(ingestion.options.custom.get("capture_type").unwrap(), "bookmark");
        assert_eq!(ingestion.options.custom.get("source_url").unwrap(), "https://example.com");
    }

    #[test]
    fn capture_note_creates_ingestion_request() {
        let handler = BrowserCaptureHandler::new();
        let request = CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({
                "captureType": "note",
                "selectedText": "This is selected text",
                "url": "https://example.com",
                "title": "Example Page"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: crate::capture::IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.source, "browser");
        assert_eq!(ingestion.mime_type, "text/plain");
        assert_eq!(ingestion.raw_bytes, b"This is selected text");
        assert_eq!(ingestion.vault_id, "vault-1");
        assert_eq!(ingestion.options.custom.get("capture_type").unwrap(), "note");
    }

    #[test]
    fn capture_document_creates_ingestion_request() {
        let handler = BrowserCaptureHandler::new();
        let request = CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({
                "captureType": "document",
                "html": "<html><body>Hello</body></html>",
                "url": "https://example.com",
                "title": "Example Page"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(result.success);
        assert!(result.payload.is_some());

        let payload = result.payload.unwrap();
        let ingestion: crate::capture::IngestionRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(ingestion.source, "browser");
        assert_eq!(ingestion.mime_type, "text/html");
        assert_eq!(ingestion.raw_bytes, b"<html><body>Hello</body></html>");
        assert_eq!(ingestion.vault_id, "vault-1");
        assert_eq!(ingestion.options.custom.get("capture_type").unwrap(), "document");
    }

    #[test]
    fn capture_missing_capture_type_fails() {
        let handler = BrowserCaptureHandler::new();
        let request = CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({
                "url": "https://example.com"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn capture_invalid_capture_type_fails() {
        let handler = BrowserCaptureHandler::new();
        let request = CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({
                "captureType": "invalid",
                "url": "https://example.com"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = handler.capture(request);
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
