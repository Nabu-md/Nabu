use std::collections::HashMap;
use std::sync::Arc;

use crate::event_bus::EventBus;
use crate::jobs::job::{Job, JobPayload, Priority};
use crate::jobs::queue::{DurableJobQueue, Queue};

use super::handler::{CaptureHandler, CaptureResult};

/// Events emitted by the CaptureEngine.
#[derive(Debug, Clone)]
pub enum CaptureEvent {
    /// A capture request was received and is being processed.
    CaptureRequested {
        source_type: String,
        content_type: String,
    },
    /// A job has been enqueued for the captured content.
    JobEnqueued {
        source_type: String,
        job_id: String,
        priority: String,
    },
    /// Capture failed before enqueuing.
    CaptureFailed {
        source_type: String,
        error: String,
    },
}

/// The capture engine routes incoming content from capture sources
/// to the durable job queue for async processing.
///
/// ## Flow
///
/// ```text
/// Capture source (browser, clipboard, etc.)
///     │
///     ▼
/// CaptureEngine.ingest(source_type, request)
///     │
///     ├── Finds handler for source_type
///     ├── Calls handler.capture(request)
///     ├── Creates JobPayload from CaptureResult
///     ├── Enqueues job in DurableJobQueue
///     └── Returns job_id to caller
/// ```
#[derive(Debug)]
pub struct CaptureEngine {
    /// Registered capture handlers, keyed by source type.
    handlers: HashMap<String, Box<dyn CaptureHandler>>,

    /// The durable job queue for async processing.
    queue: Arc<DurableJobQueue>,

    /// Optional event bus for publishing capture events.
    event_bus: Option<EventBus<CaptureEvent>>,
}

impl CaptureEngine {
    /// Creates a new capture engine connected to the given queue.
    pub fn new(queue: Arc<DurableJobQueue>) -> Self {
        CaptureEngine {
            handlers: HashMap::new(),
            queue,
            event_bus: None,
        }
    }

    /// Creates a new capture engine with an event bus.
    pub fn with_event_bus(queue: Arc<DurableJobQueue>, event_bus: EventBus<CaptureEvent>) -> Self {
        CaptureEngine {
            handlers: HashMap::new(),
            queue,
            event_bus: Some(event_bus),
        }
    }

    /// Registers a capture handler.
    pub fn register<H: CaptureHandler + 'static>(&mut self, handler: H) {
        self.handlers
            .insert(handler.source_type().to_string(), Box::new(handler));
    }

    /// Returns the number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// Returns the source types of all registered handlers.
    pub fn registered_sources(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Checks if a handler is registered for the given source.
    pub fn has_handler(&self, source_type: &str) -> bool {
        self.handlers.contains_key(source_type)
    }

    /// Captures content from the given source and enqueues it for processing.
    ///
    /// This is the primary entry point for all capture sources.
    /// It:
    /// 1. Looks up the handler for the source type.
    /// 2. Calls the handler to capture content.
    /// 3. Creates a `JobPayload` with the captured content.
    /// 4. Enqueues the job in the durable job queue.
    /// 5. Returns the `JobId` to the caller.
    ///
    /// The caller receives the job ID immediately — processing happens
    /// asynchronously in the worker pool.
    pub async fn ingest(
        &self,
        source_type: &str,
        request: HashMap<String, String>,
        priority: Priority,
    ) -> Result<String, String> {
        // Find the handler
        let handler = self
            .handlers
            .get(source_type)
            .ok_or_else(|| format!("no handler for source type: {}", source_type))?;

        // Publish capture requested event
        if let Some(ref bus) = self.event_bus {
            bus.publish(&CaptureEvent::CaptureRequested {
                source_type: source_type.to_string(),
                content_type: String::new(), // will be updated after capture
            });
        }

        // Capture the content
        let result = handler.capture(request);

        if !result.success {
            let error = result.error.unwrap_or_else(|| "unknown capture error".into());
            if let Some(ref bus) = self.event_bus {
                bus.publish(&CaptureEvent::CaptureFailed {
                    source_type: source_type.to_string(),
                    error: error.clone(),
                });
            }
            return Err(error);
        }

        // Build job payload from the capture result
        let mut payload = JobPayload::new();
        payload.insert("content_type".into(), serde_json::Value::String(result.content_type.clone()));
        payload.insert("content".into(), serde_json::Value::String(
            // Store content as base64 for safe JSON transport
            base64_encode(&result.content),
        ));

        // Add metadata
        let metadata_json = serde_json::to_value(&result.metadata)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        payload.insert("metadata".into(), metadata_json);

        // Add source type
        payload.insert("source_type".into(), serde_json::Value::String(source_type.to_string()));

        // Create and enqueue the job
        let job_type = handler.job_type();
        let job = Job::new(job_type, payload).with_priority(priority);

        let job_id = self
            .queue
            .enqueue(job)
            .await
            .map_err(|e| format!("failed to enqueue capture job: {}", e))?;

        // Publish job enqueued event
        if let Some(ref bus) = self.event_bus {
            bus.publish(&CaptureEvent::JobEnqueued {
                source_type: source_type.to_string(),
                job_id: job_id.to_string(),
                priority: format!("{:?}", priority),
            });
        }

        Ok(job_id.to_string())
    }

    /// Synchronous version for testing (creates a new runtime internally).
    #[cfg(test)]
    pub fn ingest_sync(
        &self,
        source_type: &str,
        request: HashMap<String, String>,
        priority: Priority,
    ) -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(self.ingest(source_type, request, priority))
    }
}

/// Simple base64 encoding for binary content serialization.
fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::queue::DurableJobQueue as DJQ;

    async fn setup_engine() -> (tempfile::TempDir, CaptureEngine) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".nabu").join("jobs");
        let queue = Arc::new(DJQ::new(&path).await.unwrap());
        let mut engine = CaptureEngine::new(queue);
        engine.register(TestCaptureHandler::new("test_source", "text/plain", b"hello".to_vec()));
        (dir, engine)
    }

    #[tokio::test]
    async fn test_engine_register_and_ingest() {
        let (_dir, engine) = setup_engine().await;
        assert_eq!(engine.handler_count(), 1);
        assert!(engine.has_handler("test_source"));

        let result = engine
            .ingest("test_source", HashMap::new(), Priority::Normal)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().len() > 0); // should be a valid job ID
    }

    #[tokio::test]
    async fn test_engine_unknown_source() {
        let (_dir, engine) = setup_engine().await;
        let result = engine
            .ingest("unknown", HashMap::new(), Priority::Normal)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_registered_sources() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (_dir, engine) = setup_engine().await;
            let sources = engine.registered_sources();
            assert_eq!(sources, vec!["test_source"]);
        });
    }
}
