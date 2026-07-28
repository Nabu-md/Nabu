use serde::{Deserialize, Serialize};

use crate::models::knowledge_object::KnowledgeObject;

/// The status of an ingestion operation.
///
/// Returned as part of [`IngestionResult`] to indicate whether the pipeline
/// successfully produced a [`KnowledgeObject`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IngestionStatus {
    /// The ingestion completed successfully.
    Success,
    /// The ingestion failed with a description.
    Failed(String),
}

/// The result of running an [`IngestionPipeline`].
///
/// Contains the produced [`KnowledgeObject`] (if successful), the source,
/// timestamp, status, and any warnings generated during ingestion.
///
/// This is the final output of the capture engine's `ingest` method and the
/// standard return type for all capture operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestionResult {
    /// The created knowledge object, if ingestion succeeded.
    pub knowledge_object: Option<KnowledgeObject>,
    /// Identifier of the created knowledge object, if successful.
    pub knowledge_object_id: Option<uuid::Uuid>,
    /// The capture source that initiated ingestion.
    pub source: String,
    /// ISO 8601 timestamp of when ingestion completed.
    pub timestamp: String,
    /// The outcome of the ingestion operation.
    pub status: IngestionStatus,
    /// Non-fatal warnings produced during ingestion.
    pub warnings: Vec<String>,
}
