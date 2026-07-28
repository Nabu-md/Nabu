use crate::models::knowledge_object::KnowledgeObject;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A request to capture knowledge from an external source.
///
/// Every capture operation begins with a `CaptureRequest`. The request carries
/// the raw payload from the source, the target vault, and arbitrary context
/// that handlers may use to enrich the capture.
///
/// # Serialization
///
/// `CaptureRequest` is serializable for IPC between the Tauri command layer
/// and the core capture engine.
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
///
/// Returned by [`CaptureHandler::capture`] after processing a [`CaptureRequest`].
///
/// A successful result carries the [`KnowledgeObject`] created by the handler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureResult {
    /// Whether the capture succeeded.
    pub success: bool,
    /// The created knowledge object, if successful.
    pub knowledge_object: Option<KnowledgeObject>,
    /// The ID of the created knowledge object, if successful.
    pub knowledge_object_id: Option<uuid::Uuid>,
    /// The serialized payload from the handler, if any.
    pub payload: Option<serde_json::Value>,
    /// Error description, if the capture failed.
    pub error: Option<String>,
    /// Human-readable status message.
    pub message: String,
}

impl Default for CaptureResult {
    fn default() -> Self {
        Self {
            success: false,
            knowledge_object: None,
            knowledge_object_id: None,
            payload: None,
            error: None,
            message: String::new(),
        }
    }
}

impl CaptureResult {
    /// Create a successful capture result with a knowledge object.
    pub fn success(knowledge_object: KnowledgeObject) -> Self {
        let id = knowledge_object.id;
        Self {
            success: true,
            knowledge_object: Some(knowledge_object),
            knowledge_object_id: Some(id),
            payload: None,
            error: None,
            message: String::new(),
        }
    }

    /// Create a successful capture result with a knowledge object and payload.
    pub fn success_with_payload(knowledge_object: KnowledgeObject, payload: serde_json::Value) -> Self {
        let id = knowledge_object.id;
        Self {
            success: true,
            knowledge_object: Some(knowledge_object),
            knowledge_object_id: Some(id),
            payload: Some(payload),
            error: None,
            message: String::new(),
        }
    }

    /// Create a failed capture result with an error message.
    pub fn failure(message: String) -> Self {
        Self {
            success: false,
            knowledge_object: None,
            knowledge_object_id: None,
            payload: None,
            error: Some(message.clone()),
            message,
        }
    }

    /// Create a failed capture result with an error message and payload.
    pub fn failure_with_payload(message: String, payload: serde_json::Value) -> Self {
        Self {
            success: false,
            knowledge_object: None,
            knowledge_object_id: None,
            payload: Some(payload),
            error: Some(message.clone()),
            message,
        }
    }
}

/// Typed errors returned by capture handlers and the normaliser.
///
/// All errors in the capture pipeline are represented by this enum. No handler
/// should panic; every failure path must return a typed `CaptureError`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CaptureError {
    /// The provided file path is invalid or inaccessible.
    InvalidFile(String),
    /// MIME type detection failed for the provided file.
    MimeDetectionFailed(String),
    /// Reading the file content failed.
    ReadFailed(String),
    /// Normalization of raw input failed.
    NormalizationFailed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::InvalidFile(msg) => write!(f, "Invalid file: {}", msg),
            CaptureError::MimeDetectionFailed(msg) => write!(f, "MIME detection failed: {}", msg),
            CaptureError::ReadFailed(msg) => write!(f, "Read failed: {}", msg),
            CaptureError::NormalizationFailed(msg) => write!(f, "Normalization failed: {}", msg),
        }
    }
}

impl std::error::Error for CaptureError {}
