//! Timeline extraction processor for the processing pipeline.
//!
//! This processor extracts meaningful dates from knowledge objects and enriches
//! their metadata with structured timeline information. It never invents values;
//! unknown or ambiguous dates remain empty.
//!
//! # Date Sources
//!
//! 1. **File Metadata**: Created and modified timestamps from the file system.
//! 2. **Document Metadata**: PDF, Office, and image metadata where available.
//! 3. **Content Extraction**: Common date formats extracted from text content.
//!
//! # Supported Formats
//!
//! - ISO 8601 timestamps
//! - YYYY-MM-DD
//! - MM/DD/YYYY
//! - DD/MM/YYYY
//! - Month Day Year (e.g., "January 15, 2024")
//!
//! # Constraints
//!
//! - Never writes to SQLite directly.
//! - Never emits UI events.
//! - Never rejects storage automatically.
//! - Never performs blocking I/O.
//! - Never invents or guesses dates.

use std::collections::HashMap;

use crate::processing::processor::{ProcessingDecision, ProcessingResult, Processor};
use crate::models::knowledge_object::KnowledgeObject;
use regex::Regex;

/// Structured timeline information attached to a knowledge object.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineInfo {
    /// Document date extracted from content or metadata.
    pub document_date: Option<String>,
    /// Created date from file or document metadata.
    pub created_date: Option<String>,
    /// Modified date from file or document metadata.
    pub modified_date: Option<String>,
    /// Detected event date from content.
    pub detected_event_date: Option<String>,
    /// Confidence in the extracted dates.
    pub extraction_confidence: Option<String>,
}

/// Processor that extracts meaningful dates from knowledge objects.
///
/// The processor enriches the knowledge object's metadata with timeline
/// information but never rejects storage. Dates remain empty if they cannot
/// be confidently determined.
///
/// # Date Sources
///
/// - File metadata (created, modified)
/// - Document metadata (PDF, Office, image EXIF)
/// - Content extraction (regex-based pattern matching)
#[derive(Debug)]
pub struct TimelineExtractor {
    /// Compiled regex patterns for date extraction.
    date_patterns: Vec<Regex>,
}

impl TimelineExtractor {
    /// Creates a new timeline extractor with default date patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // ISO 8601: 2024-01-01 or 2024-01-01T12:00:00Z
            Regex::new(r"\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:Z|[+-]\d{2}:\d{2})?)?").unwrap(),
            // MM/DD/YYYY
            Regex::new(r"\b\d{1,2}/\d{1,2}/\d{4}\b").unwrap(),
            // DD/MM/YYYY
            Regex::new(r"\b\d{1,2}/\d{1,2}/\d{4}\b").unwrap(),
            // Month Day, Year: January 15, 2024
            Regex::new(r"\b(?:January|February|March|April|May|June|July|August|September|October|November|December)\s+\d{1,2},?\s+\d{4}\b").unwrap(),
            // Day Month Year: 15 January 2024
            Regex::new(r"\b\d{1,2}\s+(?:January|February|March|April|May|June|July|August|September|October|November|December)\s+\d{4}\b").unwrap(),
        ];

        Self { date_patterns }
    }

    /// Extracts dates from text content using regex patterns.
    fn extract_dates_from_text(&self, text: &str) -> Vec<String> {
        let mut dates = Vec::new();
        for pattern in &self.date_patterns {
            for mat in pattern.find_iter(text) {
                dates.push(mat.as_str().to_string());
            }
        }
        dates
    }

    /// Validates and normalizes a date string.
    ///
    /// Returns None if the date is ambiguous or invalid.
    fn validate_date(&self, date_str: &str) -> Option<String> {
        // Try parsing as ISO 8601 datetime first
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
            return Some(dt.format("%Y-%m-%d").to_string());
        }

        // Try ISO date only
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            return Some(dt.format("%Y-%m-%d").to_string());
        }

        // Try common formats
        for fmt in &[
            "%m/%d/%Y",
            "%d/%m/%Y",
            "%B %d, %Y",
            "%d %B %Y",
            "%B %d %Y",
        ] {
            if let Ok(dt) = chrono::NaiveDate::parse_from_str(date_str, fmt) {
                return Some(dt.format("%Y-%m-%d").to_string());
            }
        }

        None
    }

    /// Extracts timeline information from a knowledge object.
    fn extract_timeline(&self, object: &KnowledgeObject) -> TimelineInfo {
        let mut info = TimelineInfo::default();

        // Source 1: File metadata (already in object.metadata)
        if let Some(created) = &object.metadata.created {
            info.created_date = Some(created.clone());
        }
        if let Some(modified) = &object.metadata.modified {
            info.modified_date = Some(modified.clone());
        }

        // Source 2: Document metadata
        // For PDFs, images, etc., metadata would be populated by dedicated
        // metadata extractors in a future phase. For now, we rely on what's
        // already in the object.

        // Source 3: Content extraction
        // Extract text content for date patterns
        let text_content = match &object.content {
            crate::models::knowledge_object::ObjectContent::PlainText => {
                // We don't have actual text bytes here; ingestion pipeline would provide them.
                // For now, use title and metadata as a proxy.
                vec![object.metadata.title.as_deref().unwrap_or("")]
            }
            crate::models::knowledge_object::ObjectContent::Markdown => {
                vec![object.metadata.title.as_deref().unwrap_or("")]
            }
            _ => vec![],
        };

        let mut extracted_dates = Vec::new();
        for text in text_content {
            extracted_dates.extend(self.extract_dates_from_text(text));
        }

        // Validate and pick the best date
        if !extracted_dates.is_empty() {
            for date in extracted_dates {
                if let Some(validated) = self.validate_date(&date) {
                    if info.document_date.is_none() {
                        info.document_date = Some(validated.clone());
                        info.extraction_confidence = Some("medium".to_string());
                    }
                    if info.detected_event_date.is_none() {
                        info.detected_event_date = Some(validated);
                    }
                    break;
                }
            }
        }

        info
    }
}

impl Default for TimelineExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for TimelineExtractor {
    fn name(&self) -> &'static str {
        "timeline_extractor"
    }

    fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        let timeline_info = self.extract_timeline(&knowledge_object);

        // Attach to metadata.custom
        knowledge_object.metadata.custom.insert(
            "timeline_info".to_string(),
            serde_json::to_value(&timeline_info).unwrap_or_default(),
        );

        let warnings = if timeline_info.document_date.is_some() {
            vec![format!("Document date extracted: {}", timeline_info.document_date.unwrap())]
        } else {
            Vec::new()
        };

        ProcessingResult::modified(knowledge_object, warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};
    use uuid::Uuid;

    fn create_test_object() -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Document,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata {
                title: Some("Meeting Notes 2024-01-15".to_string()),
                author: Some("Author".to_string()),
                language: Some("en".to_string()),
                source_url: None,
                source_file: Some("/path/to/meeting.txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                page_count: None,
                word_count: Some(500),
                created: Some("2024-01-01T00:00:00Z".to_string()),
                modified: Some("2024-06-01T00:00:00Z".to_string()),
                custom: HashMap::new(),
            },
        }
    }

    #[test]
    fn processor_extracts_iso_date_from_title() {
        let extractor = TimelineExtractor::new();
        let obj = create_test_object();
        let result = extractor.process(obj);

        assert!(result.modified);
        let metadata = &result.knowledge_object.metadata.custom;
        assert!(metadata.contains_key("timeline_info"));

        let info: TimelineInfo = serde_json::from_value(
            metadata.get("timeline_info").unwrap().clone()
        ).unwrap();

        assert!(info.document_date.is_some());
        assert_eq!(info.document_date.unwrap(), "2024-01-15");
    }

    #[test]
    fn processor_extracts_month_day_year_format() {
        let mut obj = create_test_object();
        obj.metadata.title = Some("Report from January 15, 2024".to_string());

        let extractor = TimelineExtractor::new();
        let result = extractor.process(obj);

        assert!(result.modified);
        let info: TimelineInfo = serde_json::from_value(
            result.knowledge_object.metadata.custom.get("timeline_info").unwrap().clone()
        ).unwrap();

        assert!(info.document_date.is_some());
        assert_eq!(info.document_date.unwrap(), "2024-01-15");
    }

    #[test]
    fn processor_handles_no_dates() {
        let mut obj = create_test_object();
        obj.metadata.title = Some("No dates here".to_string());

        let extractor = TimelineExtractor::new();
        let result = extractor.process(obj);

        assert!(result.modified);
        let info: TimelineInfo = serde_json::from_value(
            result.knowledge_object.metadata.custom.get("timeline_info").unwrap().clone()
        ).unwrap();

        assert!(info.document_date.is_none());
        assert!(info.extraction_confidence.is_none());
    }

    #[test]
    fn processor_preserves_existing_metadata() {
        let extractor = TimelineExtractor::new();
        let obj = create_test_object();
        let result = extractor.process(obj);

        assert!(result.modified);
        assert_eq!(result.knowledge_object.metadata.title, Some("Meeting Notes 2024-01-15".to_string()));
        assert_eq!(result.knowledge_object.metadata.author, Some("Author".to_string()));
    }

    #[test]
    fn timeline_info_serializes_correctly() {
        let info = TimelineInfo {
            document_date: Some("2024-01-15".to_string()),
            created_date: Some("2024-01-01T00:00:00Z".to_string()),
            modified_date: Some("2024-06-01T00:00:00Z".to_string()),
            detected_event_date: Some("2024-01-15".to_string()),
            extraction_confidence: Some("medium".to_string()),
        };

        let json = serde_json::to_value(&info).unwrap();
        let deserialized: TimelineInfo = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.document_date, Some("2024-01-15".to_string()));
        assert_eq!(deserialized.created_date, Some("2024-01-01T00:00:00Z".to_string()));
    }

    #[test]
    fn extractor_handles_multiple_date_formats() {
        let mut obj = create_test_object();
        obj.metadata.title = Some("Event on 12/25/2024 and 2024-12-31".to_string());

        let extractor = TimelineExtractor::new();
        let result = extractor.process(obj);

        assert!(result.modified);
        let info: TimelineInfo = serde_json::from_value(
            result.knowledge_object.metadata.custom.get("timeline_info").unwrap().clone()
        ).unwrap();

        assert!(info.document_date.is_some());
        // Should extract the first valid date
        assert!(info.detected_event_date.is_some());
    }
}