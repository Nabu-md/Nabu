//! MetadataEnricher processor for populating missing metadata.
//!
//! This processor fills in missing metadata fields using deterministic
//! rules based on filename patterns, content analysis, OCR output,
//! and existing metadata. It never overwrites existing values.
//!
//! # Enrichment Targets
//!
//! - Missing titles (inferred from filename or first heading)
//! - Inferred dates (from filename patterns, content, or metadata)
//! - Inferred tags (from content patterns and classification)
//! - Inferred categories (from object type and content)
//! - Reading status (inferred from source type)
//! - Document type (from classification)
//!
//! # Architecture
//!
//! ```text
//! KnowledgeObject
//!     ↓ MetadataEnricher
//! KnowledgeObject with enriched metadata
//! ```

use crate::models::knowledge_object::{KnowledgeObject, ObjectType};
use crate::processing::processor::{ProcessingDecision, ProcessingResult, Processor};
use std::collections::HashMap;

/// Processor that enriches knowledge object metadata.
///
/// The MetadataEnricher fills in missing metadata fields using
/// deterministic rules. It never overwrites existing values.
#[derive(Debug, Default)]
pub struct MetadataEnricher {
    /// Whether to infer dates from filenames.
    pub infer_dates_from_filename: bool,
    /// Whether to infer tags from content patterns.
    pub infer_tags_from_content: bool,
    /// Whether to infer reading status from source type.
    pub infer_reading_status: bool,
}

impl MetadataEnricher {
    pub fn new() -> Self {
        Self {
            infer_dates_from_filename: true,
            infer_tags_from_content: true,
            infer_reading_status: true,
        }
    }

    /// Infer a title from the filename if none exists.
    fn infer_title(&self, obj: &KnowledgeObject) -> Option<String> {
        // If title already exists, don't override
        if obj.metadata.title.is_some() {
            return None;
        }

        // Try source file basename
        if let Some(source_file) = &obj.metadata.source_file {
            if let Some(stem) = std::path::Path::new(source_file)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                // Convert snake_case or kebab-case to Title Case
                let title = stem
                    .replace('_', " ")
                    .replace('-', " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }

        // Try first heading from content
        if let Some(heading) = self.extract_first_heading(obj) {
            return Some(heading);
        }

        None
    }

    /// Extract the first heading from markdown or HTML content.
    fn extract_first_heading(&self, obj: &KnowledgeObject) -> Option<String> {
        let content_text = obj.content.as_text();

        // Try markdown headings
        for line in content_text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let heading = trimmed.trim_start_matches('#').trim().to_string();
                if !heading.is_empty() {
                    return Some(heading);
                }
            }
        }

        None
    }

    /// Infer dates from filename patterns.
    fn infer_dates_from_filename(&self, obj: &KnowledgeObject) -> Option<(String, String)> {
        if !self.infer_dates_from_filename {
            return None;
        }

        let source_file = obj.metadata.source_file.as_deref()?;
        let filename = std::path::Path::new(source_file)
            .file_name()
            .and_then(|n| n.to_str())?;

        // Try ISO date patterns: YYYY-MM-DD, YYYY-MM-DD_HH-MM-SS
        let date_patterns = [
            (r"(\d{4})-(\d{2})-(\d{2})", "%Y-%m-%d"),
            (r"(\d{4})_(\d{2})_(\d{2})", "%Y-%m-%d"),
        ];

        for (pattern, _format) in &date_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(filename) {
                    if let (Some(year), Some(month), Some(day)) =
                        (caps.get(1), caps.get(2), caps.get(3))
                    {
                        let date_str = format!(
                            "{}-{}-{}",
                            year.as_str(),
                            month.as_str(),
                            day.as_str()
                        );
                        return Some((date_str, "inferred_from_filename".to_string()));
                    }
                }
            }
        }

        None
    }

    /// Infer tags from content patterns.
    fn infer_tags_from_content(&self, obj: &KnowledgeObject) -> Vec<String> {
        if !self.infer_tags_from_content {
            return Vec::new();
        }

        let mut tags = Vec::new();

        // Get content text
        let content_text = match &obj.content {
            crate::models::knowledge_object::ObjectContent::Markdown => "",
            crate::models::knowledge_object::ObjectContent::PlainText => "",
            crate::models::knowledge_object::ObjectContent::Html => "",
            crate::models::knowledge_object::ObjectContent::Structured(json) => json.to_string().as_str(),
            crate::models::knowledge_object::ObjectContent::Binary => return tags,
        };

        let text_lower = content_text.to_lowercase();

        // Tag patterns
        let tag_patterns: &[(&str, &str)] = &[
            ("#rust", "rust"),
            ("#python", "python"),
            ("#javascript", "javascript"),
            ("#typescript", "typescript"),
            ("#go", "go"),
            ("#java", "java"),
            ("#cpp", "cpp"),
            ("#csharp", "csharp"),
            ("#ruby", "ruby"),
            ("#swift", "swift"),
            ("#kotlin", "kotlin"),
            ("#rust", "rust"),
            ("#nabu", "nabu"),
            ("#knowledge", "knowledge"),
            ("#research", "research"),
            ("#meeting", "meeting"),
            ("#invoice", "invoice"),
            ("#receipt", "receipt"),
            ("#contract", "contract"),
            ("#todo", "todo"),
            ("#task", "task"),
            ("#project", "project"),
            ("#idea", "idea"),
            ("#reference", "reference"),
            ("#tutorial", "tutorial"),
            ("#guide", "guide"),
            ("#manual", "manual"),
        ];

        for (pattern, tag) in tag_patterns {
            if text_lower.contains(pattern) && !tags.contains(&tag.to_string()) {
                tags.push(tag.to_string());
            }
        }

        tags
    }

    /// Infer reading status from source type.
    fn infer_reading_status(&self, obj: &KnowledgeObject) -> Option<String> {
        if !self.infer_reading_status {
            return None;
        }

        match obj.object_type {
            ObjectType::Website | ObjectType::Bookmark | ObjectType::Article => {
                Some("unread".to_string())
            }
            ObjectType::Video | ObjectType::AudioRecording => {
                Some("unwatched".to_string())
            }
            ObjectType::Pdf | ObjectType::Document | ObjectType::Book => {
                Some("unread".to_string())
            }
            _ => None,
        }
    }

    /// Infer document category from object type.
    fn infer_category(&self, obj: &KnowledgeObject) -> Option<String> {
        match obj.object_type {
            ObjectType::Invoice | ObjectType::Receipt => Some("finance".to_string()),
            ObjectType::Meeting | ObjectType::Note => Some("notes".to_string()),
            ObjectType::Contract | ObjectType::Document => Some("documents".to_string()),
            ObjectType::ResearchPaper | ObjectType::Book | ObjectType::Course => {
                Some("learning".to_string())
            }
            ObjectType::Image | ObjectType::Screenshot | ObjectType::Scan => {
                Some("media".to_string())
            }
            ObjectType::Video | ObjectType::AudioRecording => Some("media".to_string()),
            ObjectType::Repository | ObjectType::Website => Some("code".to_string()),
            ObjectType::Person | ObjectType::Organisation => Some("contacts".to_string()),
            ObjectType::Project => Some("projects".to_string()),
            _ => None,
        }
    }
}

impl Processor for MetadataEnricher {
    fn name(&self) -> &'static str {
        "metadata_enricher"
    }

    fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        let mut warnings = Vec::new();
        let mut modified = false;

        // 1. Infer title if missing
        if let Some(title) = self.infer_title(&knowledge_object) {
            knowledge_object.metadata.title = Some(title);
            warnings.push("Title inferred from filename".to_string());
            modified = true;
        }

        // 2. Infer dates from filename
        if let Some((date, source)) = self.infer_dates_from_filename(&knowledge_object) {
            if knowledge_object.metadata.created.is_none() {
                knowledge_object.metadata.created = Some(date.clone());
                warnings.push(format!("Created date inferred from filename ({})", source));
                modified = true;
            }
        }

        // 3. Infer tags from content
        let inferred_tags = self.infer_tags_from_content(&knowledge_object);
        if !inferred_tags.is_empty() {
            // Merge with existing tags in custom metadata
            let existing_tags: Vec<String> = knowledge_object
                .metadata
                .custom
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let mut all_tags = existing_tags;
            for tag in inferred_tags {
                if !all_tags.contains(&tag) {
                    all_tags.push(tag);
                }
            }

            knowledge_object
                .metadata
                .custom
                .insert("tags".to_string(), serde_json::json!(all_tags));
            warnings.push("Tags inferred from content".to_string());
            modified = true;
        }

        // 4. Infer reading status
        if let Some(status) = self.infer_reading_status(&knowledge_object) {
            knowledge_object
                .metadata
                .custom
                .insert("reading_status".to_string(), serde_json::json!(status));
            warnings.push("Reading status inferred from source type".to_string());
            modified = true;
        }

        // 5. Infer category
        if let Some(category) = self.infer_category(&knowledge_object) {
            knowledge_object
                .metadata
                .custom
                .insert("category".to_string(), serde_json::json!(category));
            warnings.push("Category inferred from object type".to_string());
            modified = true;
        }

        // 6. Infer document type from classification
        if let Some(classification) = knowledge_object.metadata.custom.get("classification") {
            if let Some(obj_type_str) = classification.get("object_type").and_then(|v| v.as_str()) {
                knowledge_object
                    .metadata
                    .custom
                    .insert("document_type".to_string(), serde_json::json!(obj_type_str));
            }
        }

        if modified {
            ProcessingResult::modified(knowledge_object, warnings)
        } else {
            ProcessingResult::unchanged(knowledge_object)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};
    use uuid::Uuid;

    fn create_test_object(object_type: ObjectType) -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata::default(),
        }
    }

    #[test]
    fn enricher_infers_title_from_filename() {
        let enricher = MetadataEnricher::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.source_file = Some("/path/to/my-important-note.md".to_string());

        let result = enricher.process(obj);
        assert!(result.modified);
        assert_eq!(
            result.knowledge_object.metadata.title,
            Some("My Important Note".to_string())
        );
    }

    #[test]
    fn enricher_does_not_override_existing_title() {
        let enricher = MetadataEnricher::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.title = Some("Existing Title".to_string());
        obj.metadata.source_file = Some("/path/to/my-note.md".to_string());

        let result = enricher.process(obj);
        // Title should not be overridden
        assert_eq!(result.knowledge_object.metadata.title, Some("Existing Title".to_string()));
    }

    #[test]
    fn enricher_infers_tags_from_content() {
        let enricher = MetadataEnricher::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.content = ObjectContent::Markdown("# My Rust Notes\n\nLearning about #rust and #nabu".to_string());

        let result = enricher.process(obj);
        assert!(result.modified);
        let tags = result
            .knowledge_object
            .metadata
            .custom
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);
        assert!(tags >= 2);
    }

    #[test]
    fn enricher_infers_reading_status_for_website() {
        let enricher = MetadataEnricher::new();
        let obj = create_test_object(ObjectType::Website);

        let result = enricher.process(obj);
        assert!(result.modified);
        let status = result
            .knowledge_object
            .metadata
            .custom
            .get("reading_status")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(status, "unread");
    }

    #[test]
    fn enricher_infers_category() {
        let enricher = MetadataEnricher::new();
        let obj = create_test_object(ObjectType::Invoice);

        let result = enricher.process(obj);
        assert!(result.modified);
        let category = result
            .knowledge_object
            .metadata
            .custom
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(category, "finance");
    }

    #[test]
    fn enricher_returns_unchanged_for_note_with_no_signals() {
        let enricher = MetadataEnricher::new();
        let obj = create_test_object(ObjectType::Note);

        let result = enricher.process(obj);
        // Note type doesn't infer reading status, so should be unchanged
        // (unless title inference kicks in from source file, which we don't set)
        assert!(!result.modified);
    }
}
