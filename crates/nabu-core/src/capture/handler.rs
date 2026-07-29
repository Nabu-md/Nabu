use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

/// The result of capturing content from a source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    /// Whether the capture was successful.
    pub success: bool,
    /// The type of content captured (e.g., "article", "image", "pdf").
    pub content_type: String,
    /// The raw content bytes, serialized as a base64 string for portability.
    pub content: Vec<u8>,
    /// Metadata extracted by the capture handler.
    pub metadata: HashMap<String, String>,
    /// Error message if capture failed.
    pub error: Option<String>,
}

impl CaptureResult {
    /// Creates a successful capture result.
    pub fn success(content_type: impl Into<String>, content: Vec<u8>) -> Self {
        CaptureResult {
            success: true,
            content_type: content_type.into(),
            content,
            metadata: HashMap::new(),
            error: None,
        }
    }

    /// Creates a failed capture result.
    pub fn failure(error: impl Into<String>) -> Self {
        CaptureResult {
            success: false,
            content_type: String::new(),
            content: Vec::new(),
            metadata: HashMap::new(),
            error: Some(error.into()),
        }
    }

    /// Adds metadata to this result.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// The `CaptureHandler` trait — implemented by all capture sources.
///
/// Each handler knows how to capture content from a specific source
/// (browser, clipboard, screenshot, file, etc.) and return it as a
/// structured `CaptureResult`.
///
/// Handlers do NOT process or store the captured content — they only
/// extract it. Processing is handled by the `ProcessingPipeline` running
/// inside workers.
pub trait CaptureHandler: Debug + Send + Sync {
    /// Returns a unique identifier for this handler (e.g., "browser", "clipboard").
    fn source_type(&self) -> &str;

    /// Returns the job type string used when enqueuing captured content.
    /// This maps to a registered executor in the `ExecutorRegistry`.
    fn job_type(&self) -> &str {
        &format!("capture:{}", self.source_type())
    }

    /// Captures content from the given request.
    ///
    /// The `request` is a generic key-value map interpreted by each handler.
    fn capture(&self, request: HashMap<String, String>) -> CaptureResult;
}

/// A test handler that returns fixed content.
#[derive(Debug)]
pub struct TestCaptureHandler {
    pub source: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

impl TestCaptureHandler {
    pub fn new(source: impl Into<String>, content_type: impl Into<String>, content: Vec<u8>) -> Self {
        TestCaptureHandler {
            source: source.into(),
            content_type: content_type.into(),
            content,
        }
    }
}

impl CaptureHandler for TestCaptureHandler {
    fn source_type(&self) -> &str {
        &self.source
    }

    fn capture(&self, _request: HashMap<String, String>) -> CaptureResult {
        CaptureResult::success(&self.content_type, self.content.clone())
    }
}

/// A handler that always fails — useful for testing error paths.
#[derive(Debug)]
pub struct FailingCaptureHandler {
    pub source: String,
}

impl FailingCaptureHandler {
    pub fn new(source: impl Into<String>) -> Self {
        FailingCaptureHandler {
            source: source.into(),
        }
    }
}

impl CaptureHandler for FailingCaptureHandler {
    fn source_type(&self) -> &str {
        &self.source
    }

    fn capture(&self, _request: HashMap<String, String>) -> CaptureResult {
        CaptureResult::failure(format!("{} capture failed", self.source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_result_success() {
        let result = CaptureResult::success("article", b"hello".to_vec());
        assert!(result.success);
        assert_eq!(result.content_type, "article");
        assert_eq!(result.content, b"hello");
        assert!(result.error.is_none());
    }

    #[test]
    fn test_capture_result_failure() {
        let result = CaptureResult::failure("something broke");
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_capture_result_with_metadata() {
        let result = CaptureResult::success("test", Vec::new())
            .with_metadata("url", "https://example.com")
            .with_metadata("title", "Example");
        assert_eq!(result.metadata.get("url"), Some(&"https://example.com".to_string()));
        assert_eq!(result.metadata.get("title"), Some(&"Example".to_string()));
    }

    #[test]
    fn test_test_handler() {
        let handler = TestCaptureHandler::new("test", "text/plain", b"data".to_vec());
        assert_eq!(handler.source_type(), "test");

        let result = handler.capture(HashMap::new());
        assert!(result.success);
        assert_eq!(result.content, b"data");
    }

    #[test]
    fn test_failing_handler() {
        let handler = FailingCaptureHandler::new("broken");
        let result = handler.capture(HashMap::new());
        assert!(!result.success);
        assert_eq!(result.error, Some("broken capture failed".into()));
    }
}
