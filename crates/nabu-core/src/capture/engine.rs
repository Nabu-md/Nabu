use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::capture::{CaptureHandler, CaptureRequest, CaptureResult};

/// The central capture engine responsible for routing ingestion requests.
///
/// The engine maintains a registry of [`CaptureHandler`] implementations and
/// dispatches [`CaptureRequest`] instances to the appropriate handler based on
/// `source_type`.
///
/// This is the permanent ingestion gateway for Nabu. Future capture sources
/// should register handlers with this engine rather than introducing new
/// entry points.
pub struct CaptureEngine {
    handlers: RwLock<HashMap<String, Arc<dyn CaptureHandler>>>,
}

impl CaptureEngine {
    /// Creates a new capture engine with no registered handlers.
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a capture handler.
    ///
    /// If a handler for the same `source_type` is already registered, it will
    /// be replaced.
    pub fn register(&self, handler: Arc<dyn CaptureHandler>) {
        let mut handlers = self.handlers.write().unwrap();
        handlers.insert(handler.source_type().to_string(), handler);
    }

    /// Unregisters a capture handler by its source type.
    ///
    /// Returns the removed handler if one was registered.
    pub fn unregister(&self, source_type: &str) -> Option<Arc<dyn CaptureHandler>> {
        let mut handlers = self.handlers.write().unwrap();
        handlers.remove(source_type)
    }

    /// Looks up a registered handler by source type.
    pub fn lookup(&self, source_type: &str) -> Option<Arc<dyn CaptureHandler>> {
        let handlers = self.handlers.read().unwrap();
        handlers.get(source_type).cloned()
    }

    /// Dispatches a capture request to the appropriate handler.
    ///
    /// If no handler is registered for the request's `source_type`, or if the
    /// registered handler cannot handle the request, a failed [`CaptureResult`]
    /// is returned.
    pub fn dispatch(&self, request: CaptureRequest) -> CaptureResult {
        let handlers = self.handlers.read().unwrap();
        if let Some(handler) = handlers.get(&request.source_type)
            && handler.can_handle(&request)
        {
            return handler.capture(request);
        }
        CaptureResult {
            success: false,
            knowledge_object_id: None,
            error: Some(format!(
                "No handler available for source type: {}",
                request.source_type
            )),
            message: "Capture failed: no suitable handler".to_string(),
            payload: None,
        }
    }
}

impl Default for CaptureEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MockHandler {
        source: &'static str,
        can_handle: bool,
        result: CaptureResult,
    }

    impl CaptureHandler for MockHandler {
        fn source_type(&self) -> &'static str {
            self.source
        }

        fn can_handle(&self, _request: &CaptureRequest) -> bool {
            self.can_handle
        }

        fn capture(&self, _request: CaptureRequest) -> CaptureResult {
            self.result.clone()
        }
    }

    fn test_request(source_type: &str) -> CaptureRequest {
        CaptureRequest {
            source_type: source_type.to_string(),
            payload: serde_json::json!({"test": true}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }
    }

    #[test]
    fn register_and_dispatch() {
        let engine = CaptureEngine::new();
        let handler = Arc::new(MockHandler {
            source: "test_source",
            can_handle: true,
            result: CaptureResult {
                success: true,
                knowledge_object_id: Some(Uuid::new_v4()),
                error: None,
                message: "Captured".to_string(),
                payload: None,
            },
        });
        engine.register(handler);

        let request = test_request("test_source");
        let result = engine.dispatch(request);
        assert!(result.success);
        assert!(result.knowledge_object_id.is_some());
    }

    #[test]
    fn dispatch_no_handler() {
        let engine = CaptureEngine::new();
        let request = test_request("missing");
        let result = engine.dispatch(request);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(
            result
                .error
                .unwrap()
                .contains("No handler available for source type: missing")
        );
    }

    #[test]
    fn unregister_removes_handler() {
        let engine = CaptureEngine::new();
        let handler = Arc::new(MockHandler {
            source: "removable",
            can_handle: true,
            result: CaptureResult {
                success: true,
                knowledge_object_id: Some(Uuid::new_v4()),
                error: None,
                message: "Captured".to_string(),
                payload: None,
            },
        });
        engine.register(handler);
        assert!(engine.lookup("removable").is_some());

        engine.unregister("removable");
        assert!(engine.lookup("removable").is_none());

        let request = test_request("removable");
        let result = engine.dispatch(request);
        assert!(!result.success);
    }

    #[test]
    fn can_handle_is_checked_before_capture() {
        let engine = CaptureEngine::new();
        let handler = Arc::new(MockHandler {
            source: "conditional",
            can_handle: false,
            result: CaptureResult {
                success: true,
                knowledge_object_id: Some(Uuid::new_v4()),
                error: None,
                message: "Should not reach".to_string(),
                payload: None,
            },
        });
        engine.register(handler);

        let request = test_request("conditional");
        let result = engine.dispatch(request);
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
