use serde::{Deserialize, Serialize};

use crate::models::knowledge_object::KnowledgeObject;

/// The status of an ingestion operation.
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestionResult {
    /// The created knowledge object, if ingestion succeeded.
    pub knowledge_object: Option<KnowledgeObject>,
    /// The capture source that initiated ingestion.
    pub source: String,
    /// ISO 8601 timestamp of when ingestion completed.
    pub timestamp: String,
    /// The outcome of the ingestion operation.
    pub status: IngestionStatus,
    /// Non-fatal warnings produced during ingestion.
    pub warnings: Vec<String>,
}
