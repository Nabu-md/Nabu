//! Typed event definitions for the knowledge capture pipeline.
//!
//! These events form the backbone of the event-driven architecture, allowing
//! services to communicate without direct coupling.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capture::IngestionRequest;
use crate::models::knowledge_object::KnowledgeObject;

/// Event type name for [`ItemCaptured`].
pub const EVENT_ITEM_CAPTURED: &str = "ItemCaptured";

/// Event type name for [`ItemProcessed`].
pub const EVENT_ITEM_PROCESSED: &str = "ItemProcessed";

/// Event type name for [`ItemStored`].
pub const EVENT_ITEM_STORED: &str = "ItemStored";

/// Event type name for [`ItemProcessingStarted`].
pub const EVENT_ITEM_PROCESSING_STARTED: &str = "ItemProcessingStarted";

/// Event type name for [`ItemProcessingCompleted`].
pub const EVENT_ITEM_PROCESSING_COMPLETED: &str = "ItemProcessingCompleted";

/// Event type name for [`ItemProcessingFailed`].
pub const EVENT_ITEM_PROCESSING_FAILED: &str = "ItemProcessingFailed";

/// Event published when an item is captured by the CaptureEngine.
///
/// This event signals that a capture request has been successfully processed
/// and an ingestion request has been created. It carries the data necessary
/// for the ProcessingPipeline to transform the raw capture into a KnowledgeObject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCaptured {
    /// The unique identifier of the captured item.
    pub id: Uuid,
    /// The source type that triggered the capture (e.g., "file_drop", "dictation").
    pub source: String,
    /// The vault ID where the item will be stored.
    pub vault_id: String,
    /// The timestamp when the capture occurred.
    pub timestamp: String,
    /// Raw bytes of the captured content.
    pub raw_bytes: Vec<u8>,
    /// Detected MIME type of the content.
    pub mime_type: String,
    /// Original source file path, if applicable.
    pub source_file: Option<String>,
}

/// Event published when an item has been processed by the IngestionPipeline.
///
/// This event signals that a raw capture has been transformed into a
/// KnowledgeObject with metadata populated. It carries the full object
/// so the StorageManager can persist it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessed {
    /// The unique identifier of the processed item.
    pub id: Uuid,
    /// The vault ID where the item will be stored.
    pub vault_id: String,
    /// The object type determined during processing.
    pub object_type: String,
    /// The timestamp when processing completed.
    pub timestamp: String,
    /// The fully constructed knowledge object.
    pub knowledge_object: KnowledgeObject,
    /// Non-fatal warnings produced during ingestion.
    pub warnings: Vec<String>,
}

/// Event published when processing of an item has started.
///
/// This event is emitted by the [`ProcessingPipeline`] before executing the
/// processor chain. It signals that a knowledge object is about to be
/// reviewed by the registered processors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingStarted {
    /// The unique identifier of the item being processed.
    pub id: Uuid,
    /// The vault ID where the item will be stored.
    pub vault_id: String,
    /// The object type being processed.
    pub object_type: String,
    /// The timestamp when processing started.
    pub timestamp: String,
}

/// Event published when processing of an item has completed successfully.
///
/// This event is emitted by the [`ProcessingPipeline`] after all processors
/// have executed successfully. It signals that the knowledge object has been
/// reviewed and is ready for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingCompleted {
    /// The unique identifier of the processed item.
    pub id: Uuid,
    /// The vault ID where the item will be stored.
    pub vault_id: String,
    /// The object type of the processed item.
    pub object_type: String,
    /// The timestamp when processing completed.
    pub timestamp: String,
    /// Non-fatal warnings collected during processing.
    pub warnings: Vec<String>,
}

/// Event published when processing of an item has failed or been rejected.
///
/// This event is emitted by the [`ProcessingPipeline`] when a processor
/// rejects the object or an unrecoverable error occurs. The object will NOT
/// be forwarded to the [`StorageManager`] for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingFailed {
    /// The unique identifier of the rejected item.
    pub id: Uuid,
    /// The vault ID of the rejected item.
    pub vault_id: String,
    /// The object type of the rejected item.
    pub object_type: String,
    /// The timestamp when processing failed.
    pub timestamp: String,
    /// The reason for the rejection or failure.
    pub reason: String,
    /// Non-fatal warnings collected before failure.
    pub warnings: Vec<String>,
}

/// Event published when an item has been stored by the StorageManager.
///
/// This event signals that a KnowledgeObject has been persisted to the
/// metadata database. Future subscribers (Search, Graph, AI, etc.) can
/// react to this event to update their indexes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStored {
    /// The unique identifier of the stored item.
    pub id: Uuid,
    /// The vault ID where the item was stored.
    pub vault_id: String,
    /// The object type of the stored item.
    pub object_type: String,
    /// The timestamp when storage completed.
    pub timestamp: String,
}

/// Event map defining all available event types.
///
/// This trait is used to provide compile-time safety for event types.
pub trait KnowledgeEvents: Send + Sync + 'static {}

impl KnowledgeEvents for ItemCaptured {}
impl KnowledgeEvents for ItemProcessed {}
impl KnowledgeEvents for ItemStored {}
impl KnowledgeEvents for ItemProcessingStarted {}
impl KnowledgeEvents for ItemProcessingCompleted {}
impl KnowledgeEvents for ItemProcessingFailed {}

/// Creates an IngestionRequest from an ItemCaptured event.
impl From<&ItemCaptured> for IngestionRequest {
    fn from(event: &ItemCaptured) -> Self {
        Self {
            source: event.source.clone(),
            raw_bytes: event.raw_bytes.clone(),
            mime_type: event.mime_type.clone(),
            vault_id: event.vault_id.clone(),
            source_file: event.source_file.clone(),
            options: crate::capture::IngestionOptions::default(),
        }
    }
}

/// Creates an [`ItemProcessed`] event from a [`KnowledgeObject`].
///
/// # Note
///
/// This conversion does not preserve pipeline warnings. Use [`ItemProcessed::from_knowledge_object`]
/// when warnings are available, or construct [`ItemProcessed`] directly.
impl From<&KnowledgeObject> for ItemProcessed {
    fn from(obj: &KnowledgeObject) -> Self {
        Self::from_knowledge_object(obj, Vec::new())
    }
}

/// Creates an [`ItemStored`] event from a [`KnowledgeObject`].
impl From<&KnowledgeObject> for ItemStored {
    fn from(obj: &KnowledgeObject) -> Self {
        Self {
            id: obj.id,
            vault_id: obj.vault_id.clone(),
            object_type: format!("{:?}", obj.object_type),
            timestamp: obj.modified_at.clone(),
        }
    }
}

impl ItemProcessed {
    /// Creates an [`ItemProcessed`] event from a [`KnowledgeObject`] with warnings.
    ///
    /// This is the preferred constructor when warnings from the ingestion
    /// pipeline are available.
    pub fn from_knowledge_object(obj: &KnowledgeObject, warnings: Vec<String>) -> Self {
        Self {
            id: obj.id,
            vault_id: obj.vault_id.clone(),
            object_type: format!("{:?}", obj.object_type),
            timestamp: obj.modified_at.clone(),
            knowledge_object: obj.clone(),
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};

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

    #[test]
    fn item_processed_can_be_created_from_knowledge_object() {
        let obj = create_test_object();
        let event: ItemProcessed = (&obj).into();
        assert_eq!(event.id, obj.id);
        assert_eq!(event.vault_id, "test-vault");
        assert_eq!(event.knowledge_object.id, obj.id);
    }

    #[test]
    fn item_stored_can_be_created_from_knowledge_object() {
        let obj = create_test_object();
        let event: ItemStored = (&obj).into();
        assert_eq!(event.id, obj.id);
        assert_eq!(event.vault_id, "test-vault");
    }

    #[test]
    fn item_captured_can_be_converted_to_ingestion_request() {
        let event = ItemCaptured {
            id: Uuid::new_v4(),
            source: "file_drop".to_string(),
            vault_id: "vault-1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            raw_bytes: b"Hello".to_vec(),
            mime_type: "text/plain".to_string(),
            source_file: Some("/path/to/file.txt".to_string()),
        };

        let request: IngestionRequest = (&event).into();
        assert_eq!(request.source, "file_drop");
        assert_eq!(request.vault_id, "vault-1");
        assert_eq!(request.raw_bytes, b"Hello");
        assert_eq!(request.mime_type, "text/plain");
        assert_eq!(request.source_file, Some("/path/to/file.txt".to_string()));
    }
}
