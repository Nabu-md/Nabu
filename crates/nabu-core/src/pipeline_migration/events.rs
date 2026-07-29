//! Pipeline lifecycle event definitions.
//!
//! These events are published during the capture → process → store flow
//! and are consumed by:
//! - `StorageManager` (subscribes to `ItemProcessed` to persist results)
//! - `Indexer` (subscribes to `ItemStored` to update search index)
//! - `VaultGraph` (subscribes to `ItemStored` to update graph)
//! - Diagnostics (subscribes to all events for auditing)
//! - Future plugins

use serde::{Deserialize, Serialize};

/// A captured item has been enqueued for processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCaptured {
    /// The capture source type (e.g., "browser", "clipboard").
    pub source: String,
    /// The content type (e.g., "article", "image", "pdf").
    pub content_type: String,
    /// The job ID assigned by the queue.
    pub job_id: String,
}

/// A job has been picked up by a worker and processing has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingStarted {
    pub job_id: String,
    pub content_type: String,
    pub processor_count: u32,
}

/// Processing of the item has completed successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingCompleted {
    pub job_id: String,
    pub content_type: String,
    pub processor_results: u32,
}

/// Processing of the item has failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingFailed {
    pub job_id: String,
    pub content_type: String,
    pub error: String,
    pub retry_count: u32,
}

/// The processed item has been persisted to storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStored {
    pub job_id: String,
    pub content_type: String,
    pub storage_path: Option<String>,
    pub object_id: Option<String>,
}

/// An item has been processed (success or failure).
/// This is the event that StorageManager subscribes to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessed {
    pub job_id: String,
    pub content_type: String,
    pub success: bool,
    pub error: Option<String>,
}
