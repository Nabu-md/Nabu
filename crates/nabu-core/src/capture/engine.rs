use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::capture::{
    CaptureHandler, CaptureRequest, CaptureResult, IngestionRequest, IngestionResult,
    IngestionStatus, utils::current_timestamp,
};
use crate::event_bus::{
    EVENT_ITEM_CAPTURED, EVENT_ITEM_PROCESSED, EventBus, ItemCaptured, ItemProcessed,
};

/// The central capture engine responsible for routing ingestion requests.
///
/// The engine maintains a registry of [`CaptureHandler`] implementations and
/// dispatches [`CaptureRequest`] instances to the appropriate handler based on
/// `source_type`.
///
/// This is the permanent ingestion gateway for Nabu. Future capture sources
/// should register handlers with this engine rather than introducing new
/// entry points.
///
/// # Thread Safety
///
/// `CaptureEngine` is `Send + Sync` and may be shared across threads. The
/// internal handler registry uses `RwLock` for concurrent reads and exclusive
/// writes. Lock poisoning is handled gracefully: a poisoned read lock will
/// still return the handler map.
///
/// # Lifecycle
///
/// 1. Create a `CaptureEngine` with [`CaptureEngine::new`].
/// 2. Register handlers with [`CaptureEngine::register`].
/// 3. Dispatch requests with [`CaptureEngine::dispatch`] or run the full
///    ingestion flow with [`CaptureEngine::ingest`].
/// 4. Unregister handlers with [`CaptureEngine::unregister`] when they are no
///    longer needed.
pub struct CaptureEngine {
    handlers: RwLock<HashMap<String, Arc<dyn CaptureHandler>>>,
    event_bus: Arc<EventBus>,
}

impl CaptureEngine {
    /// Creates a new capture engine with no registered handlers.
    ///
    /// The event bus is used to publish [`ItemCaptured`] events after successful
    /// capture, decoupling the engine from downstream processing.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            event_bus,
        }
    }

    /// Registers a capture handler.
    ///
    /// If a handler for the same `source_type` is already registered, it will
    /// be replaced.
    ///
    /// Handlers are stored as `Arc<dyn CaptureHandler>` so they can be shared
    /// across threads without cloning the handler itself.
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
    ///
    /// Returns `None` if no handler is registered for the given source type.
    pub fn lookup(&self, source_type: &str) -> Option<Arc<dyn CaptureHandler>> {
        let handlers = self.handlers.read().unwrap();
        handlers.get(source_type).cloned()
    }

    /// Dispatches a capture request to the appropriate handler.
    ///
    /// If no handler is registered for the request's `source_type`, or if the
    /// registered handler cannot handle the request, a failed [`CaptureResult`]
    /// is returned.
    ///
    /// This method only performs dispatch; it does not run the ingestion
    /// pipeline. Use [`CaptureEngine::ingest`] for the full flow.
    pub fn dispatch(&self, request: CaptureRequest) -> CaptureResult {
        let handlers = match self.handlers.read() {
            Ok(h) => h,
            Err(poisoned) => poisoned.into_inner(),
        };
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

    /// Runs the full ingestion flow: dispatch → publish ItemCaptured → pipeline → storage.
    ///
    /// Returns an [`IngestionResult`] regardless of whether dispatch succeeds
    /// or fails, so callers always have a single result type to handle.
    ///
    /// # Flow
    ///
    /// 1. Dispatch the request to the appropriate handler.
    /// 2. If dispatch fails, return a failed `IngestionResult`.
    /// 3. Deserialize the handler's payload into an `IngestionRequest`.
    /// 4. Publish an [`ItemCaptured`] event to the event bus.
    /// 5. Wait for the [`ItemProcessed`] event from the pipeline (with timeout).
    /// 6. Return the pipeline's `IngestionResult`, with `knowledge_object_id`
    ///    populated from the created `KnowledgeObject`.
    ///
    /// # Timeout
    ///
    /// If no [`ItemProcessed`] response is received within 30 seconds, the
    /// ingestion is considered failed. This prevents indefinite blocking if
    /// no pipeline subscriber is registered.
    pub fn ingest(&self, request: CaptureRequest) -> IngestionResult {
        let capture_result = self.dispatch(request.clone());

        if !capture_result.success {
            return self.build_failed_result(
                request.source_type,
                capture_result
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
                Vec::new(),
            );
        }

        let payload = match capture_result.payload {
            Some(p) => p,
            None => {
                return self.build_failed_result(
                    request.source_type,
                    "Capture succeeded but produced no payload".to_string(),
                    Vec::new(),
                );
            }
        };

        let ingestion_request: IngestionRequest = match serde_json::from_value(payload) {
            Ok(req) => req,
            Err(e) => {
                return self.build_failed_result(
                    request.source_type,
                    format!("Failed to deserialize ingestion request: {}", e),
                    Vec::new(),
                );
            }
        };

        let id = uuid::Uuid::new_v4();
        let (tx, rx) = std::sync::mpsc::channel();

        let event = ItemCaptured {
            id,
            source: request.source_type.clone(),
            vault_id: request.vault_id.clone(),
            timestamp: current_timestamp(),
            raw_bytes: ingestion_request.raw_bytes.clone(),
            mime_type: ingestion_request.mime_type.clone(),
            source_file: ingestion_request.source_file.clone(),
        };

        let bus = self.event_bus.clone();
        let unsub = bus.subscribe(EVENT_ITEM_PROCESSED, move |processed: &ItemProcessed| {
            if processed.id == id {
                let _ = tx.send(processed.clone());
            }
        });

        self.event_bus.publish(EVENT_ITEM_CAPTURED, &event);

        // Keep subscription alive until we get response or timeout
        let _ = unsub;

        // Wait for the pipeline response with a timeout to prevent indefinite blocking.
        match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(processed) => {
                let knowledge_object = processed.knowledge_object;
                IngestionResult {
                    knowledge_object: Some(knowledge_object.clone()),
                    knowledge_object_id: Some(processed.id),
                    source: event.source,
                    timestamp: event.timestamp,
                    status: IngestionStatus::Success,
                    warnings: processed.warnings,
                }
            }
            Err(_) => self.build_failed_result(
                request.source_type,
                "No response from processing pipeline within timeout".to_string(),
                Vec::new(),
            ),
        }
    }

    /// Builds a failed [`IngestionResult`] with the given source, error, and warnings.
    fn build_failed_result(
        &self,
        source: String,
        error: String,
        warnings: Vec<String>,
    ) -> IngestionResult {
        IngestionResult {
            knowledge_object: None,
            knowledge_object_id: None,
            source,
            timestamp: current_timestamp(),
            status: IngestionStatus::Failed(error),
            warnings,
        }
    }
}

// Note: CaptureEngine does not implement Default because it requires an
// EventBus to function correctly. Tests should use:
//   CaptureEngine::new(Arc::new(EventBus::new()))

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{IngestionOptions, IngestionPipeline};
    use crate::event_bus::EventBus;
    use crate::models::knowledge_object::{ObjectContent, ObjectType};
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
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
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
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
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
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
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
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
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

    #[test]
    fn ingest_full_flow_creates_knowledge_object() {
        let bus = Arc::new(EventBus::new());
        let _pipeline = IngestionPipeline::new(bus.clone());
        let engine = CaptureEngine::new(bus.clone());
        let ingestion_request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"Hello, world!".to_vec(),
            mime_type: "text/plain".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/hello.txt".to_string()),
            options: IngestionOptions::default(),
        };
        let payload = serde_json::to_value(&ingestion_request).unwrap();

        let handler = Arc::new(MockHandler {
            source: "file_drop",
            can_handle: true,
            result: CaptureResult {
                success: true,
                knowledge_object_id: None,
                error: None,
                message: "Captured".to_string(),
                payload: Some(payload),
            },
        });
        engine.register(handler);

        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({"file_path": "/path/to/hello.txt"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = engine.ingest(request);
        assert_eq!(result.status, IngestionStatus::Success);
        assert!(result.knowledge_object.is_some());
        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.vault_id, "vault-1");
        assert_eq!(obj.object_type, ObjectType::Note);
        assert_eq!(obj.content, ObjectContent::PlainText);
        assert_eq!(result.source, "file_drop");
        assert!(result.knowledge_object_id.is_some());
        assert_eq!(result.knowledge_object_id, Some(obj.id));
    }

    #[test]
    fn ingest_handles_failed_capture() {
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
        let request = CaptureRequest {
            source_type: "missing".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = engine.ingest(request);
        match result.status {
            IngestionStatus::Failed(_) => {}
            IngestionStatus::Success => panic!("Expected failed status"),
        }
        assert!(result.knowledge_object.is_none());
        assert_eq!(result.source, "missing");
    }

    #[test]
    fn ingest_handles_missing_payload() {
        let bus = Arc::new(EventBus::new());
        let engine = CaptureEngine::new(bus);
        let handler = Arc::new(MockHandler {
            source: "file_drop",
            can_handle: true,
            result: CaptureResult {
                success: true,
                knowledge_object_id: None,
                error: None,
                message: "Captured".to_string(),
                payload: None,
            },
        });
        engine.register(handler);

        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({"file_path": "/path/to/file.txt"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = engine.ingest(request);
        match result.status {
            IngestionStatus::Failed(_) => {}
            IngestionStatus::Success => panic!("Expected failed status"),
        }
        assert!(result.knowledge_object.is_none());
        assert_eq!(result.source, "file_drop");
    }

    #[test]
    fn ingest_times_out_when_no_pipeline_subscriber() {
        let bus = Arc::new(EventBus::new());
        // No pipeline registered — no one will publish ItemProcessed
        let engine = CaptureEngine::new(bus);
        let handler = Arc::new(MockHandler {
            source: "file_drop",
            can_handle: true,
            result: CaptureResult {
                success: true,
                knowledge_object_id: None,
                error: None,
                message: "Captured".to_string(),
                payload: Some(serde_json::json!({
                    "source": "file_drop",
                    "raw_bytes": [116, 101, 115, 116],
                    "mime_type": "text/plain",
                    "vault_id": "vault-1",
                    "source_file": null,
                    "options": {
                        "create_knowledge_object": true,
                        "extract_metadata": false,
                        "custom": {}
                    }
                })),
            },
        });
        engine.register(handler);

        let request = CaptureRequest {
            source_type: "file_drop".to_string(),
            payload: serde_json::json!({"file_path": "/path/to/file.txt"}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        };

        let result = engine.ingest(request);
        match result.status {
            IngestionStatus::Failed(e) => {
                assert!(e.contains("timeout"), "Expected timeout error, got: {}", e);
            }
            IngestionStatus::Success => panic!("Expected failed status due to timeout"),
        }
        assert!(result.knowledge_object.is_none());
    }
}
