//! Reading Queue model and service.
//!
//! Provides an organizational layer over KnowledgeObjects, allowing users
//! to manage their reading list (Unread, Reading, Read, Archived).
//! State is stored in KnowledgeObject metadata.

use crate::models::knowledge_object::KnowledgeObject;
use serde::{Deserialize, Serialize};

/// Reading status of a knowledge object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReadingStatus {
    #[default]
    Unread,
    Reading,
    Read,
    Archived,
}

/// Priority level for queued items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReadingPriority {
    Low,
    #[default]
    Normal,
    High,
}

/// Reading-specific metadata stored in KnowledgeObject's custom metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadingMetadata {
    pub status: ReadingStatus,
    pub priority: ReadingPriority,
    pub progress: f32, // 0.0 to 1.0
}

pub const READING_METADATA_KEY: &str = "reading_queue";

impl ReadingMetadata {
    pub fn from_object(obj: &KnowledgeObject) -> Self {
        obj.metadata
            .custom
            .get(READING_METADATA_KEY)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }
}
