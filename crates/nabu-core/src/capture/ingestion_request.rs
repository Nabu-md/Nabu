use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A canonical ingestion request produced by the normaliser.
///
/// This is the standard intermediate representation for all captured input
/// before it is transformed into a [`crate::models::knowledge_object::KnowledgeObject`].
///
/// Every handler produces an `IngestionRequest` as its normalized output.
/// The engine passes this directly to the [`IngestionPipeline`] without
/// further transformation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestionRequest {
    /// The capture source that produced this request (e.g., "file_drop").
    pub source: String,
    /// Raw bytes of the captured content.
    pub raw_bytes: Vec<u8>,
    /// Detected MIME type of the content.
    pub mime_type: String,
    /// Target vault identifier.
    pub vault_id: String,
    /// Original source file path, if applicable.
    pub source_file: Option<String>,
    /// Options controlling how the request should be processed.
    pub options: IngestionOptions,
}

/// Options that control ingestion behaviour.
///
/// Passed by the handler to the normaliser and carried through to the
/// [`IngestionPipeline`]. Handlers may populate `custom` with
/// source-specific options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IngestionOptions {
    /// Whether to create a KnowledgeObject from this request.
    pub create_knowledge_object: bool,
    /// Whether to extract metadata from the content.
    pub extract_metadata: bool,
    /// Arbitrary options supplied by the caller or upstream system.
    pub custom: HashMap<String, serde_json::Value>,
}
