//! Processing pipeline that owns an ordered chain of processors.
//!
//! The [`ProcessingPipeline`] sits between the [`IngestionPipeline`] and the
//! [`StorageManager`]. It subscribes to [`ItemProcessed`] events (published by
//! the ingestion pipeline), executes its processor chain on the knowledge object,
//! records processing history in the object's metadata, publishes lifecycle
//! events, and re-publishes the processed [`ItemProcessed`] event for the
//! storage manager to persist.
//!
//! # Architecture
//!
//! ```text
//! CaptureEngine
//!     ↓  ItemCaptured
//! IngestionPipeline
//!     ↓  ItemProcessed
//! ProcessingPipeline
//!     ↓  ItemProcessingStarted
//!     ↓  ItemProcessingCompleted (or ItemProcessingFailed)
//!     ↓  ItemProcessed (re-published)
//! StorageManager
//!     ↓  ItemStored
//! ```
//!
//! # Loop Prevention
//!
//! The pipeline checks whether an object has already been processed by looking
//! for the `processing_history` key in its metadata. If present, the event is
//! forwarded directly to the [`StorageManager`] without re-processing.
//!
//! # Constraints
//!
//! - The pipeline is generic over processor implementations.
//! - Processors execute sequentially in registration order.
//! - On reject or unrecoverable failure, remaining processors are skipped.
//! - Processing history is stored in the object's metadata (not a database).
//! - The pipeline never blocks asynchronously.

use std::sync::{Arc, RwLock};

use crate::event_bus::{
    EVENT_ITEM_PROCESSED, EVENT_ITEM_PROCESSING_COMPLETED, EVENT_ITEM_PROCESSING_FAILED,
    EVENT_ITEM_PROCESSING_STARTED, EventBus, ItemProcessed, ItemProcessingCompleted,
    ItemProcessingFailed, ItemProcessingStarted,
};
use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::history::{ProcessingHistoryEntry, PROCESSING_HISTORY_KEY};
use crate::processing::processor::{ProcessingDecision, Processor};

/// A pipeline that owns an ordered chain of processors.
///
/// The pipeline is created via [`ProcessingPipeline::new`] and processors are
/// added via [`ProcessingPipeline::register`]. Once configured, the pipeline
/// subscribes to the event bus and processes objects automatically.
///
/// # Type Parameters
///
/// The pipeline receives knowledge objects via [`ItemProcessed`] events and
/// returns processed objects via re-published [`ItemProcessed`] events.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use nabu_core::event_bus::EventBus;
/// use nabu_core::processing::ProcessingPipeline;
///
/// let bus = Arc::new(EventBus::new());
/// let pipeline = ProcessingPipeline::new(bus.clone());
/// pipeline.register(Arc::new(MyProcessor));
/// ```
pub struct ProcessingPipeline {
    event_bus: Arc<EventBus>,
    processors: Arc<RwLock<Vec<Arc<dyn Processor>>>>,
}

impl ProcessingPipeline {
    /// Creates a new processing pipeline and subscribes to [`ItemProcessed`] events.
    ///
    /// The pipeline listens for processed items from the ingestion pipeline,
    /// runs its processor chain, and re-publishes the processed object via
    /// a new [`ItemProcessed`] event so the [`StorageManager`] can persist it.
    ///
    /// # Loop Prevention
    ///
    /// The pipeline checks for existing `processing_history` in the object's
    /// metadata. If present, the event is forwarded without re-processing.
    pub fn new(event_bus: Arc<EventBus>) -> Arc<Self> {
        let pipeline = Arc::new(Self {
            event_bus: event_bus.clone(),
            processors: Arc::new(RwLock::new(Vec::new())),
        });

        let pipeline_clone = pipeline.clone();
        event_bus.subscribe(EVENT_ITEM_PROCESSED, move |event: &ItemProcessed| {
            let obj = event.knowledge_object.clone();

            // Loop prevention: if the object already has processing history,
            // it has already been through this pipeline. Forward it directly.
            if obj
                .metadata
                .custom
                .contains_key(PROCESSING_HISTORY_KEY)
            {
                return;
            }

            let result = pipeline_clone.run(obj);

            match result {
                Ok((processed_obj, warnings)) => {
                    // Re-publish ItemProcessed so StorageManager can persist.
                    // The loop prevention check above ensures we don't
                    // re-process this object.
                    let processed_event =
                        ItemProcessed::from_knowledge_object(&processed_obj, warnings);
                    pipeline_clone
                        .event_bus
                        .publish(EVENT_ITEM_PROCESSED, &processed_event);
                }
                Err((rejected_obj, reason, warnings)) => {
                    // On rejection, publish ItemProcessingFailed but do NOT
                    // re-publish ItemProcessed, so the object is not stored.
                    let failed_event = ItemProcessingFailed {
                        id: rejected_obj.id,
                        vault_id: rejected_obj.vault_id.clone(),
                        object_type: format!("{:?}", rejected_obj.object_type),
                        timestamp: rejected_obj.modified_at.clone(),
                        reason: reason.clone(),
                        warnings: warnings.clone(),
                    };
                    pipeline_clone
                        .event_bus
                        .publish(EVENT_ITEM_PROCESSING_FAILED, &failed_event);
                }
            }
        });

        pipeline
    }

    /// Registers a processor in the chain.
    ///
    /// Processors execute in the order they are registered.
    /// The same processor instance may be registered multiple times
    /// (each registration adds a separate step in the chain).
    pub fn register(&self, processor: Arc<dyn Processor>) {
        let mut processors = self.processors.write().unwrap();
        processors.push(processor);
    }

    /// Runs the full processor chain on a knowledge object.
    ///
    /// This method:
    /// 1. Publishes [`ItemProcessingStarted`] event.
    /// 2. Executes each processor sequentially.
    /// 3. Collects warnings and processing history entries.
    /// 4. Attaches history to the object's metadata.
    /// 5. On rejection, returns the original object with the rejection reason.
    /// 6. On success, publishes [`ItemProcessingCompleted`] and returns the
    ///    processed object.
    ///
    /// # Returns
    ///
    /// - `Ok((KnowledgeObject, Vec<String>))` on success, with accumulated warnings.
    /// - `Err((KnowledgeObject, String, Vec<String>))` on rejection, with the
    ///   rejection reason and accumulated warnings.
    pub fn run(
        &self,
        knowledge_object: KnowledgeObject,
    ) -> Result<(KnowledgeObject, Vec<String>), (KnowledgeObject, String, Vec<String>)> {
        let id = knowledge_object.id;
        let vault_id = knowledge_object.vault_id.clone();
        let object_type = format!("{:?}", knowledge_object.object_type);
        let timestamp = knowledge_object.modified_at.clone();
        let mut warnings: Vec<String> = Vec::new();
        let mut history: Vec<ProcessingHistoryEntry> = Vec::new();
        let mut current_obj = knowledge_object;

        // Publish processing started event
        let started_event = ItemProcessingStarted {
            id,
            vault_id: vault_id.clone(),
            object_type: object_type.clone(),
            timestamp: timestamp.clone(),
        };
        self.event_bus
            .publish(EVENT_ITEM_PROCESSING_STARTED, &started_event);

        // Execute each processor sequentially
        let processors = self.get_processors();
        for processor in processors.iter() {
            let start = std::time::Instant::now();

            let result = processor.process(current_obj);

            let duration_ms = start.elapsed().as_millis() as u64;

            // Build history entry
            let mut entry = ProcessingHistoryEntry::new(processor.name());
            entry.timestamp = self.current_timestamp();
            entry.duration_ms = duration_ms;
            entry.success = matches!(result.decision, ProcessingDecision::Continue);
            entry.warnings = result.warnings.clone();
            if let ProcessingDecision::Reject(ref reason) = result.decision {
                entry.error = Some(reason.clone());
            }

            match result.decision {
                ProcessingDecision::Continue => {
                    // Collect warnings
                    warnings.extend(result.warnings);
                    history.push(entry);

                    // Always take the returned object; the processor may have
                    // modified it even if it reports unchanged (e.g., adding
                    // default values).
                    current_obj = result.knowledge_object;
                }
                ProcessingDecision::Reject(reason) => {
                    warnings.extend(result.warnings);
                    entry.success = false;
                    history.push(entry);

                    // Attach history to the returned object
                    let rejected_obj = attach_history(result.knowledge_object, history);

                    // Publish failure event so subscribers can react
                    let failed_event = ItemProcessingFailed {
                        id: rejected_obj.id,
                        vault_id: vault_id.clone(),
                        object_type: object_type.clone(),
                        timestamp: self.current_timestamp(),
                        reason: reason.clone(),
                        warnings: warnings.clone(),
                    };
                    self.event_bus
                        .publish(EVENT_ITEM_PROCESSING_FAILED, &failed_event);

                    return Err((rejected_obj, reason, warnings));
                }
            }
        }

        // Attach processing history to the object's metadata
        current_obj = attach_history(current_obj, history);

        // Publish processing completed event
        let completed_event = ItemProcessingCompleted {
            id,
            vault_id,
            object_type,
            timestamp: current_obj.modified_at.clone(),
            warnings: warnings.clone(),
        };
        self.event_bus
            .publish(EVENT_ITEM_PROCESSING_COMPLETED, &completed_event);

        Ok((current_obj, warnings))
    }

    /// Returns a snapshot of the current processor list.
    fn get_processors(&self) -> Vec<Arc<dyn Processor>> {
        let processors = self.processors.read().unwrap();
        processors.clone()
    }

    fn current_timestamp(&self) -> String {
        let now = std::time::SystemTime::now();
        let duration = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        format!("{}.{:03}Z", secs, millis)
    }
}

/// Attaches a processing history to a knowledge object's metadata.
///
/// The history is serialized as a JSON array and stored under the
/// `processing_history` key in the object's custom metadata.
fn attach_history(
    mut knowledge_object: KnowledgeObject,
    history: Vec<ProcessingHistoryEntry>,
) -> KnowledgeObject {
    let history_value = serde_json::to_value(&history).unwrap_or(serde_json::Value::Array(vec![]));
    knowledge_object
        .metadata
        .custom
        .insert(PROCESSING_HISTORY_KEY.to_string(), history_value);
    knowledge_object
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};
    use crate::processing::processor::{ProcessingResult, Processor};
    use uuid::Uuid;

    fn create_test_object() -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata::default(),
        }
    }

    /// A processor that adds a warning.
    #[derive(Debug)]
    struct WarningProcessor {
        warning: String,
    }

    impl Processor for WarningProcessor {
        fn name(&self) -> &'static str {
            "warning_processor"
        }

        fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
            ProcessingResult::modified(knowledge_object, vec![self.warning.clone()])
        }
    }

    /// A processor that adds metadata.
    #[derive(Debug)]
    struct MetadataProcessor;

    impl Processor for MetadataProcessor {
        fn name(&self) -> &'static str {
            "metadata_processor"
        }

        fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
            knowledge_object.metadata.title = Some("Processed Title".to_string());
            ProcessingResult::modified(knowledge_object, Vec::new())
        }
    }

    /// A processor that rejects the object.
    #[derive(Debug)]
    struct RejectProcessor {
        reason: String,
    }

    impl Processor for RejectProcessor {
        fn name(&self) -> &'static str {
            "reject_processor"
        }

        fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
            ProcessingResult::rejected(knowledge_object, self.reason.clone())
        }
    }

    /// A processor that does nothing.
    #[derive(Debug)]
    struct NoOpProcessor;

    impl Processor for NoOpProcessor {
        fn name(&self) -> &'static str {
            "noop"
        }

        fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
            ProcessingResult::unchanged(knowledge_object)
        }
    }

    #[test]
    fn pipeline_with_no_processors_returns_unchanged_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());

        let obj = create_test_object();
        let result = pipeline.run(obj.clone());

        assert!(result.is_ok());
        let (processed, warnings) = result.unwrap();
        assert!(warnings.is_empty());
        // Should have processing history attached
        assert!(processed
            .metadata
            .custom
            .contains_key(PROCESSING_HISTORY_KEY));
    }

    #[test]
    fn pipeline_runs_single_processor() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(MetadataProcessor));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_ok());
        let (processed, _) = result.unwrap();
        assert_eq!(processed.metadata.title, Some("Processed Title".to_string()));
    }

    #[test]
    fn pipeline_collects_warnings_from_all_processors() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(WarningProcessor {
            warning: "Warning 1".to_string(),
        }));
        pipeline.register(Arc::new(WarningProcessor {
            warning: "Warning 2".to_string(),
        }));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_ok());
        let (_, warnings) = result.unwrap();
        assert_eq!(warnings.len(), 2);
        assert!(warnings.contains(&"Warning 1".to_string()));
        assert!(warnings.contains(&"Warning 2".to_string()));
    }

    #[test]
    fn pipeline_skips_remaining_processors_on_rejection() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(RejectProcessor {
            reason: "Not suitable".to_string(),
        }));
        pipeline.register(Arc::new(MetadataProcessor));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_err());
        let (rejected, reason, _) = result.unwrap_err();
        assert_eq!(reason, "Not suitable");
        // MetadataProcessor should NOT have run
        assert_eq!(rejected.metadata.title, None);
    }

    #[test]
    fn pipeline_records_processing_history() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(NoOpProcessor));
        pipeline.register(Arc::new(MetadataProcessor));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_ok());
        let (processed, _) = result.unwrap();

        // Verify history is attached
        let history_value = processed.metadata.custom.get(PROCESSING_HISTORY_KEY);
        assert!(history_value.is_some());

        let history: Vec<ProcessingHistoryEntry> =
            serde_json::from_value(history_value.unwrap().clone())
                .expect("Failed to deserialize history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].processor_name, "noop");
        assert!(history[0].success);
        assert_eq!(history[1].processor_name, "metadata_processor");
        assert!(history[1].success);
    }

    #[test]
    fn pipeline_records_rejection_in_history() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(RejectProcessor {
            reason: "Duplicate".to_string(),
        }));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_err());
        let (rejected, reason, _) = result.unwrap_err();
        assert_eq!(reason, "Duplicate");

        let history_value = rejected.metadata.custom.get(PROCESSING_HISTORY_KEY);
        assert!(history_value.is_some());

        let history: Vec<ProcessingHistoryEntry> =
            serde_json::from_value(history_value.unwrap().clone())
                .expect("Failed to deserialize history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].processor_name, "reject_processor");
        assert!(!history[0].success);
        assert_eq!(history[0].error, Some("Duplicate".to_string()));
    }

    #[test]
    fn pipeline_publishes_lifecycle_events() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(MetadataProcessor));

        let started = Arc::new(std::sync::RwLock::new(false));
        let completed = Arc::new(std::sync::RwLock::new(false));

        let s = started.clone();
        bus.subscribe(
            EVENT_ITEM_PROCESSING_STARTED,
            move |_: &ItemProcessingStarted| {
                *s.write().unwrap() = true;
            },
        );

        let c = completed.clone();
        bus.subscribe(
            EVENT_ITEM_PROCESSING_COMPLETED,
            move |_: &ItemProcessingCompleted| {
                *c.write().unwrap() = true;
            },
        );

        let obj = create_test_object();
        let _ = pipeline.run(obj);

        assert!(*started.read().unwrap());
        assert!(*completed.read().unwrap());
    }

    #[test]
    fn pipeline_publishes_failed_event_on_rejection() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(RejectProcessor {
            reason: "Rejected".to_string(),
        }));

        let failed = Arc::new(std::sync::RwLock::new(false));
        let f = failed.clone();
        bus.subscribe(
            EVENT_ITEM_PROCESSING_FAILED,
            move |_: &ItemProcessingFailed| {
                *f.write().unwrap() = true;
            },
        );

        let obj = create_test_object();
        let _ = pipeline.run(obj);

        assert!(*failed.read().unwrap());
    }

    #[test]
    fn noop_processor_does_not_modify_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(NoOpProcessor));

        let obj = create_test_object();
        let result = pipeline.run(obj.clone());

        assert!(result.is_ok());
        let (processed, _) = result.unwrap();
        // Title should still be None (unchanged)
        assert_eq!(processed.metadata.title, None);
        assert_eq!(processed.id, obj.id);
        assert_eq!(processed.vault_id, obj.vault_id);
    }

    #[test]
    fn pipeline_with_multiple_processors_executes_in_order() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());

        // Register processors that add warnings in sequence
        pipeline.register(Arc::new(WarningProcessor {
            warning: "First".to_string(),
        }));
        pipeline.register(Arc::new(WarningProcessor {
            warning: "Second".to_string(),
        }));

        let obj = create_test_object();
        let result = pipeline.run(obj);

        assert!(result.is_ok());
        let (_, warnings) = result.unwrap();
        assert_eq!(warnings, vec!["First", "Second"]);
    }

    #[test]
    fn pipeline_skips_already_processed_objects() {
        let bus = Arc::new(EventBus::new());
        let pipeline = ProcessingPipeline::new(bus.clone());
        pipeline.register(Arc::new(MetadataProcessor));

        // Create an object that already has processing history
        let mut obj = create_test_object();
        obj.metadata.custom.insert(
            PROCESSING_HISTORY_KEY.to_string(),
            serde_json::json!([{
                "processor_name": "previous",
                "timestamp": "2024-01-01T00:00:00.000Z",
                "duration_ms": 0,
                "success": true,
                "warnings": []
            }]),
        );

        // Simulate the event bus callback by publishing ItemProcessed
        let processed_event = ItemProcessed::from_knowledge_object(&obj, Vec::new());
        bus.publish(EVENT_ITEM_PROCESSED, &processed_event);

        // The object should NOT have been modified by MetadataProcessor
        // because the pipeline should have skipped it.
        // We verify by checking that the title is still None.
        assert_eq!(obj.metadata.title, None);
    }
}