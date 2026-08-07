use crate::models::{CaptureSource, ObjectType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All pipeline lifecycle events emitted through the EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// A new item has been captured and enqueued
    ItemCaptured(ItemCapturedEvent),
    /// Processing of an item has started
    ItemProcessingStarted(ItemProcessingStartedEvent),
    /// Processing progress update (0.0–1.0)
    ItemProcessingProgress(ItemProcessingProgressEvent),
    /// Processing completed successfully
    ItemProcessingCompleted(ItemProcessingCompletedEvent),
    /// Processing failed
    ItemProcessingFailed(ItemProcessingFailedEvent),
    /// Item has been stored permanently
    ItemStored(ItemStoredEvent),
    /// Index has been updated
    IndexUpdated(IndexUpdatedEvent),
    /// Graph has been updated
    GraphUpdated(GraphUpdatedEvent),
    /// Item has been cancelled
    ItemCancelled(ItemCancelledEvent),
    /// Item has been retried
    ItemRetried(ItemRetriedEvent),
    /// A capability was enabled or disabled
    CapabilityStateChanged(CapabilityStateEvent),
}

/// Event kind string constants for EventBus subscriptions
pub mod kinds {
    pub const ITEM_CAPTURED: &str = "item.captured";
    pub const ITEM_PROCESSING_STARTED: &str = "item.processing.started";
    pub const ITEM_PROCESSING_PROGRESS: &str = "item.processing.progress";
    pub const ITEM_PROCESSING_COMPLETED: &str = "item.processing.completed";
    pub const ITEM_PROCESSING_FAILED: &str = "item.processing.failed";
    pub const ITEM_STORED: &str = "item.stored";
    pub const INDEX_UPDATED: &str = "index.updated";
    pub const GRAPH_UPDATED: &str = "graph.updated";
    pub const ITEM_CANCELLED: &str = "item.cancelled";
    pub const ITEM_RETRIED: &str = "item.retried";
    /// A capability was enabled or disabled.
    pub const CAPABILITY_STATE_CHANGED: &str = "capability.state.changed";
}

impl PipelineEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            PipelineEvent::ItemCaptured(_) => kinds::ITEM_CAPTURED,
            PipelineEvent::ItemProcessingStarted(_) => kinds::ITEM_PROCESSING_STARTED,
            PipelineEvent::ItemProcessingProgress(_) => kinds::ITEM_PROCESSING_PROGRESS,
            PipelineEvent::ItemProcessingCompleted(_) => kinds::ITEM_PROCESSING_COMPLETED,
            PipelineEvent::ItemProcessingFailed(_) => kinds::ITEM_PROCESSING_FAILED,
            PipelineEvent::ItemStored(_) => kinds::ITEM_STORED,
            PipelineEvent::IndexUpdated(_) => kinds::INDEX_UPDATED,
            PipelineEvent::GraphUpdated(_) => kinds::GRAPH_UPDATED,
            PipelineEvent::ItemCancelled(_) => kinds::ITEM_CANCELLED,
            PipelineEvent::ItemRetried(_) => kinds::ITEM_RETRIED,
            PipelineEvent::CapabilityStateChanged(_) => kinds::CAPABILITY_STATE_CHANGED,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCapturedEvent {
    pub object_id: Uuid,
    pub object_type: ObjectType,
    pub capture_source: CaptureSource,
    pub title: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingStartedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingProgressEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub progress: f64,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingCompletedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingFailedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub error: String,
    pub retry_count: u32,
    pub will_retry: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStoredEvent {
    pub object_id: Uuid,
    pub vault_path: String,
    pub object_type: ObjectType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUpdatedEvent {
    pub object_id: Uuid,
    pub operation: IndexOperation,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexOperation {
    Added,
    Updated,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphUpdatedEvent {
    pub object_id: Uuid,
    pub operation: GraphOperation,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphOperation {
    NodeAdded,
    NodeUpdated,
    NodeRemoved,
    EdgeAdded,
    EdgeRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCancelledEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRetriedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub retry_count: u32,
    pub max_retries: u32,
    pub timestamp: DateTime<Utc>,
}

/// A capability was enabled or disabled at runtime.
///
/// Published on the `capability.state.changed` kind whenever the
/// [`crate::plugin::capability::CapabilityRegistry`] enables or disables a
/// capability. The EventBus bridge forwards these to the UI as
/// `nabu-event` payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityStateEvent {
    /// Full capability identifier (`namespace:name`).
    pub capability_id: String,
    /// Whether the capability is now enabled.
    pub enabled: bool,
    pub timestamp: DateTime<Utc>,
}

impl ItemCapturedEvent {
    pub fn new(
        object_id: Uuid,
        object_type: ObjectType,
        capture_source: CaptureSource,
        title: Option<String>,
        job_id: Option<Uuid>,
    ) -> Self {
        Self {
            object_id,
            object_type,
            capture_source,
            title,
            timestamp: Utc::now(),
            job_id,
        }
    }
}

impl ItemStoredEvent {
    pub fn new(object_id: Uuid, vault_path: String, object_type: ObjectType) -> Self {
        Self {
            object_id,
            vault_path,
            object_type,
            timestamp: Utc::now(),
        }
    }
}
