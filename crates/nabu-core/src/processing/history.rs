//! Processing history model for tracking processor execution on knowledge objects.
//!
//! Processing history is stored within [`ObjectMetadata::custom`] under the key
//! `"processing_history"` to avoid requiring schema changes. This ensures
//! history travels with the object and is automatically serialized/deserialized
//! alongside all other metadata.

use serde::{Deserialize, Serialize};

/// A single entry in a knowledge object's processing history.
///
/// Each entry records the execution of one processor in the pipeline.
/// The history is stored as part of the object's metadata, not in a
/// separate database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct ProcessingHistoryEntry {
    /// Name of the processor that executed.
    pub processor_name: String,
    /// ISO 8601 timestamp when processing started.
    pub timestamp: String,
    /// Duration of the processing step in milliseconds.
    pub duration_ms: u64,
    /// Whether the processor completed successfully.
    pub success: bool,
    /// Non-fatal warnings produced by this processor.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Optional error message if the processor failed.
    #[serde(default)]
    pub error: Option<String>,
}

impl ProcessingHistoryEntry {
    /// Creates a new processing history entry.
    pub fn new(processor_name: impl Into<String>) -> Self {
        Self {
            processor_name: processor_name.into(),
            timestamp: String::new(),
            duration_ms: 0,
            success: false,
            warnings: Vec::new(),
            error: None,
        }
    }
}

/// JSON key used to store processing history in [`ObjectMetadata::custom`].
pub const PROCESSING_HISTORY_KEY: &str = "processing_history";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_entry_can_be_created() {
        let entry = ProcessingHistoryEntry::new("test_processor");
        assert_eq!(entry.processor_name, "test_processor");
        assert!(entry.timestamp.is_empty());
        assert_eq!(entry.duration_ms, 0);
        assert!(!entry.success);
        assert!(entry.warnings.is_empty());
        assert!(entry.error.is_none());
    }

    #[test]
    fn history_entry_serializes_and_deserializes() {
        let entry = ProcessingHistoryEntry {
            processor_name: "ocr_processor".to_string(),
            timestamp: "2024-06-01T12:00:00.000Z".to_string(),
            duration_ms: 1234,
            success: true,
            warnings: vec!["Low confidence on page 3".to_string()],
            error: None,
        };

        let serialized = serde_json::to_string(&entry).expect("Failed to serialize");
        let deserialized: ProcessingHistoryEntry =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(entry, deserialized);
    }

    #[test]
    fn history_entry_with_error_serializes() {
        let entry = ProcessingHistoryEntry {
            processor_name: "classifier".to_string(),
            timestamp: "2024-06-01T12:00:00.000Z".to_string(),
            duration_ms: 500,
            success: false,
            warnings: Vec::new(),
            error: Some("Unknown object type".to_string()),
        };

        let serialized = serde_json::to_string(&entry).expect("Failed to serialize");
        let deserialized: ProcessingHistoryEntry =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(entry, deserialized);
        assert!(deserialized.error.is_some());
        assert_eq!(deserialized.error.unwrap(), "Unknown object type");
    }

    #[test]
    fn history_entry_default_warnings_when_missing() {
        let json = r#"{
            "processor_name": "test",
            "timestamp": "2024-06-01T12:00:00.000Z",
            "duration_ms": 100,
            "success": true
        }"#;
        let deserialized: ProcessingHistoryEntry =
            serde_json::from_str(json).expect("Failed to deserialize with missing fields");
        assert!(deserialized.warnings.is_empty());
        assert!(deserialized.error.is_none());
    }
}