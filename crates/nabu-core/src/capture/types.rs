use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A request to capture knowledge from an external source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequest {
    /// The type of capture source (e.g., "browser", "watch_folder", "clipboard").
    pub source_type: String,
    /// Raw payload provided by the capture source.
    pub payload: serde_json::Value,
    /// Target vault identifier.
    pub vault_id: String,
    /// Arbitrary context supplied by the caller or upstream system.
    pub context: HashMap<String, serde_json::Value>,
}

/// The result of a capture operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureResult {
    /// Whether the capture succeeded.
    pub success: bool,
    /// Identifier of the created knowledge object, if successful.
    pub knowledge_object_id: Option<Uuid>,
    /// Error description, if the capture failed.
    pub error: Option<String>,
    /// Human-readable status message.
    pub message: String,
}
