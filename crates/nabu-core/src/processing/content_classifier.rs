//! ContentClassifier processor for deterministic document classification.
//!
//! This processor classifies knowledge objects into categories using
//! deterministic rules based on filename, extracted text, metadata,
//! MIME type, frontmatter, and OCR output. No machine learning is used.
//!
//! # Classification Categories
//!
//! | Category      | Typical Indicators |
//! |---------------|-------------------|
//! | Invoice       | "invoice", "invoice #", "bill to", "total due" |
//! | Receipt       | "receipt", "purchase", "paid", "transaction" |
//! | Meeting Note  | "meeting", "agenda", "minutes", "attendees" |
//! | Letter        | "dear", "sincerely", "regards", "letter" |
//! | Contract      | "contract", "agreement", "party", "clause", "terms" |
//! | Research Paper| "abstract", "methodology", "results", "references" |
//! | Screenshot    | image MIME type, no text content |
//! | Manual        | "chapter", "section", "table of contents", "index" |
//! | Presentation  | "slide", "presentation", "bullet point", "agenda" |
//! | Resume        | "resume", "curriculum vitae", "experience", "education" |
//! | Article       | "article", "blog post", "opinion", "essay" |
//!
//! # Architecture
//!
//! ```text
//! KnowledgeObject
//!     ↓ ContentClassifier
//! Classified KnowledgeObject (object_type updated, confidence in custom metadata)
//! ```

use crate::models::knowledge_object::{KnowledgeObject, ObjectType};
use crate::processing::processor::{ProcessingDecision, ProcessingResult, Processor};
use std::collections::HashMap;

/// Deterministic document classifier.
///
/// Classifies knowledge objects using pattern matching on filename,
/// extracted text, metadata, MIME type, frontmatter, and OCR output.
#[derive(Debug, Default)]
pub struct ContentClassifier {
    /// Minimum confidence threshold for classification (0.0 - 1.0).
    /// Classifications below this threshold are marked as uncertain.
    pub confidence_threshold: f64,
}

/// The result of a classification attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationResult {
    /// The predicted object type.
    pub object_type: ObjectType,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Which signals contributed to the classification.
    pub signals: Vec<String>,
}

impl ContentClassifier {
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.3,
        }
    }

    /// Classify a knowledge object using deterministic rules.
    pub fn classify(&self, obj: &KnowledgeObject) -> ClassificationResult {
        let mut signals: Vec<String> = Vec::new();
        let mut scores: HashMap<ObjectType, f64> = HashMap::new();

        // 1. Check existing object type hints
        self.score_existing_type(obj, &mut scores, &mut signals);

        // 2. Check filename patterns
        self.score_filename(obj, &mut scores, &mut signals);

        // 3. Check MIME type
        self.score_mime_type(obj, &mut scores, &mut signals);

        // 4. Check frontmatter / custom metadata
        self.score_metadata(obj, &mut scores, &mut signals);

        // 5. Check OCR extracted text
        self.score_ocr_text(obj, &mut scores, &mut signals);

        // 6. Check source URL patterns
        self.score_source_url(obj, &mut scores, &mut signals);

        // Determine the best classification
        let (best_type, best_score) = scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((ObjectType::Note, 0.0));

        ClassificationResult {
            object_type: best_type,
            confidence: best_score,
            signals,
        }
    }

    fn score_existing_type(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        // If the object already has a strong type hint, boost it
        match obj.object_type {
            ObjectType::Invoice => {
                *scores.entry(ObjectType::Invoice).or_insert(0.0) += 0.5;
                signals.push("Existing type: Invoice".to_string());
            }
            ObjectType::Receipt => {
                *scores.entry(ObjectType::Receipt).or_insert(0.0) += 0.5;
                signals.push("Existing type: Receipt".to_string());
            }
            ObjectType::Meeting => {
                *scores.entry(ObjectType::Meeting).or_insert(0.0) += 0.5;
                signals.push("Existing type: Meeting".to_string());
            }
            ObjectType::ResearchPaper => {
                *scores.entry(ObjectType::ResearchPaper).or_insert(0.0) += 0.5;
                signals.push("Existing type: ResearchPaper".to_string());
            }
            ObjectType::Scan => {
                *scores.entry(ObjectType::Scan).or_insert(0.0) += 0.3;
                signals.push("Existing type: Scan".to_string());
            }
            ObjectType::Screenshot => {
                *scores.entry(ObjectType::Screenshot).or_insert(0.3);
                signals.push("Existing type: Screenshot".to_string());
            }
            _ => {}
        }
    }

    fn score_filename(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        let filename = obj.metadata.source_file.as_deref().unwrap_or("");
        let filename_lower = filename.to_lowercase();
        let basename = std::path::Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let patterns: &[(&str, ObjectType, f64)] = &[
            // Invoices
            ("invoice", ObjectType::Invoice, 0.4),
            ("inv_", ObjectType::Invoice, 0.3),
            ("bill", ObjectType::Invoice, 0.3),
            ("billing", ObjectType::Invoice, 0.3),
            // Receipts
            ("receipt", ObjectType::Receipt, 0.4),
            ("receipt_", ObjectType::Receipt, 0.3),
            ("purchase", ObjectType::Receipt, 0.3),
            ("transaction", ObjectType::Receipt, 0.3),
            // Meeting notes
            ("meeting", ObjectType::Meeting, 0.4),
            ("meeting_", ObjectType::Meeting, 0.3),
            ("minutes", ObjectType::Meeting, 0.4),
            ("agenda", ObjectType::Meeting, 0.3),
            // Contracts
            ("contract", ObjectType::Document, 0.4),
            ("agreement", ObjectType::Document, 0.4),
            ("clause", ObjectType::Document, 0.3),
            // Research papers
            ("research", ObjectType::ResearchPaper, 0.4),
            ("paper", ObjectType::ResearchPaper, 0.3),
            ("academic", ObjectType::ResearchPaper, 0.3),
            ("thesis", ObjectType::ResearchPaper, 0.4),
            // Manuals
            ("manual", ObjectType::Document, 0.4),
            ("guide", ObjectType::Document, 0.3),
            ("handbook", ObjectType::Document, 0.3),
            ("documentation", ObjectType::Document, 0.3),
            // Presentations
            ("presentation", ObjectType::Document, 0.4),
            ("slide", ObjectType::Document, 0.3),
            ("deck", ObjectType::Document, 0.3),
            // Resumes
            ("resume", ObjectType::Document, 0.4),
            ("cv", ObjectType::Document, 0.4),
            ("curriculum", ObjectType::Document, 0.4),
            ("vitae", ObjectType::Document, 0.4),
            // Letters
            ("letter", ObjectType::Document, 0.4),
            ("correspondence", ObjectType::Document, 0.3),
            // Articles
            ("article", ObjectType::Document, 0.4),
            ("essay", ObjectType::Document, 0.3),
            ("blog", ObjectType::Document, 0.3),
            // Reports
            ("report", ObjectType::Document, 0.3),
            ("analysis", ObjectType::Document, 0.3),
            // Reviews
            ("review", ObjectType::Document, 0.3),
            // Notes
            ("note", ObjectType::Note, 0.2),
            ("notes", ObjectType::Note, 0.2),
        ];

        for (pattern, obj_type, weight) in patterns {
            if basename.contains(pattern) || filename_lower.contains(pattern) {
                *scores.entry(obj_type.clone()).or_insert(0.0) += weight;
                signals.push(format!("Filename match: \"{}\" → {:?}", pattern, obj_type));
            }
        }
    }

    fn score_mime_type(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        if let Some(mime) = &obj.metadata.mime_type {
            let mime_lower = mime.to_lowercase();
            match mime_lower.as_str() {
                "application/pdf" => {
                    *scores.entry(ObjectType::Pdf).or_insert(0.0) += 0.2;
                    signals.push("MIME type: PDF".to_string());
                }
                "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/svg+xml" => {
                    *scores.entry(ObjectType::Image).or_insert(0.0) += 0.2;
                    signals.push("MIME type: Image".to_string());
                }
                "text/markdown" | "text/plain" => {
                    *scores.entry(ObjectType::Note).or_insert(0.0) += 0.1;
                    signals.push("MIME type: Text".to_string());
                }
                "text/html" => {
                    *scores.entry(ObjectType::Website).or_insert(0.0) += 0.1;
                    signals.push("MIME type: HTML".to_string());
                }
                _ => {}
            }
        }
    }

    fn score_metadata(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        // Check frontmatter tags
        if let Some(tags) = obj.metadata.custom.get("tags") {
            if let Some(tags_arr) = tags.as_array() {
                for tag in tags_arr {
                    if let Some(tag_str) = tag.as_str() {
                        let tag_lower = tag_str.to_lowercase();
                        match tag_lower.as_str() {
                            "invoice" | "billing" => {
                                *scores.entry(ObjectType::Invoice).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "receipt" | "purchase" => {
                                *scores.entry(ObjectType::Receipt).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "meeting" | "minutes" | "agenda" => {
                                *scores.entry(ObjectType::Meeting).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "contract" | "agreement" => {
                                *scores.entry(ObjectType::Document).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "research" | "paper" | "academic" => {
                                *scores.entry(ObjectType::ResearchPaper).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "resume" | "cv" => {
                                *scores.entry(ObjectType::Document).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "manual" | "guide" | "handbook" => {
                                *scores.entry(ObjectType::Document).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "presentation" | "slide" => {
                                *scores.entry(ObjectType::Document).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            "article" | "essay" | "blog" => {
                                *scores.entry(ObjectType::Document).or_insert(0.0) += 0.3;
                                signals.push(format!("Tag match: \"{}\"", tag_str));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check custom metadata for document type hints
        if let Some(doc_type) = obj.metadata.custom.get("document_type") {
            if let Some(dt) = doc_type.as_str() {
                let dt_lower = dt.to_lowercase();
                match dt_lower.as_str() {
                    "invoice" => {
                        *scores.entry(ObjectType::Invoice).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "receipt" => {
                        *scores.entry(ObjectType::Receipt).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "meeting" => {
                        *scores.entry(ObjectType::Meeting).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "contract" | "agreement" => {
                        *scores.entry(ObjectType::Document).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "research" | "paper" => {
                        *scores.entry(ObjectType::ResearchPaper).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "resume" | "cv" => {
                        *scores.entry(ObjectType::Document).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "manual" | "guide" => {
                        *scores.entry(ObjectType::Document).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "presentation" | "slide" => {
                        *scores.entry(ObjectType::Document).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    "article" | "essay" => {
                        *scores.entry(ObjectType::Document).or_insert(0.0) += 0.4;
                        signals.push(format!("Metadata document_type: \"{}\"", dt));
                    }
                    _ => {}
                }
            }
        }
    }

    fn score_ocr_text(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        let ocr_text = obj.metadata.custom.get("ocr_info")
            .and_then(|v| v.get("extracted_text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if ocr_text.is_empty() {
            return;
        }

        let text_lower = ocr_text.to_lowercase();
        let text_sample = if ocr_text.len() > 2000 {
            &ocr_text[..2000]
        } else {
            ocr_text
        };
        let text_sample_lower = text_sample.to_lowercase();

        let patterns: &[(&str, ObjectType, f64)] = &[
            ("invoice", ObjectType::Invoice, 0.3),
            ("receipt", ObjectType::Receipt, 0.3),
            ("total due", ObjectType::Invoice, 0.4),
            ("amount due", ObjectType::Invoice, 0.4),
            ("payment", ObjectType::Receipt, 0.3),
            ("meeting", ObjectType::Meeting, 0.3),
            ("agenda", ObjectType::Meeting, 0.3),
            ("minutes", ObjectType::Meeting, 0.3),
            ("attendees", ObjectType::Meeting, 0.3),
            ("contract", ObjectType::Document, 0.3),
            ("agreement", ObjectType::Document, 0.3),
            ("party of the first part", ObjectType::Document, 0.4),
            ("clause", ObjectType::Document, 0.3),
            ("abstract", ObjectType::ResearchPaper, 0.3),
            ("methodology", ObjectType::ResearchPaper, 0.3),
            ("references", ObjectType::ResearchPaper, 0.3),
            ("bibliography", ObjectType::ResearchPaper, 0.3),
            ("chapter", ObjectType::Document, 0.3),
            ("table of contents", ObjectType::Document, 0.4),
            ("slide", ObjectType::Document, 0.3),
            ("experience", ObjectType::Document, 0.3),
            ("education", ObjectType::Document, 0.3),
            ("curriculum vitae", ObjectType::Document, 0.4),
        ];

        for (pattern, obj_type, weight) in patterns {
            if text_sample_lower.contains(pattern) {
                *scores.entry(obj_type.clone()).or_insert(0.0) += weight;
                signals.push(format!("OCR text match: \"{}\" → {:?}", pattern, obj_type));
            }
        }
    }

    fn score_content_text(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        let content_text = match &obj.content {
            crate::models::knowledge_object::ObjectContent::PlainText => "",
            crate::models::knowledge_object::ObjectContent::Markdown => "",
            crate::models::knowledge_object::ObjectContent::Html => "",
            crate::models::knowledge_object::ObjectContent::Structured(json) => json.to_string().as_str(),
            crate::models::knowledge_object::ObjectContent::Binary => return,
        };

        let text_lower = content_text.to_lowercase();
        let text_sample = if content_text.len() > 2000 {
            &content_text[..2000]
        } else {
            content_text
        };
        let text_sample_lower = text_sample.to_lowercase();

        let patterns: &[(&str, ObjectType, f64)] = &[
            ("invoice", ObjectType::Invoice, 0.3),
            ("receipt", ObjectType::Receipt, 0.3),
            ("total due", ObjectType::Invoice, 0.4),
            ("amount due", ObjectType::Invoice, 0.4),
            ("meeting", ObjectType::Meeting, 0.3),
            ("agenda", ObjectType::Meeting, 0.3),
            ("minutes", ObjectType::Meeting, 0.3),
            ("contract", ObjectType::Document, 0.3),
            ("agreement", ObjectType::Document, 0.3),
            ("abstract", ObjectType::ResearchPaper, 0.3),
            ("methodology", ObjectType::ResearchPaper, 0.3),
            ("references", ObjectType::ResearchPaper, 0.3),
            ("chapter", ObjectType::Document, 0.3),
            ("table of contents", ObjectType::Document, 0.4),
            ("resume", ObjectType::Document, 0.3),
            ("curriculum vitae", ObjectType::Document, 0.4),
        ];

        for (pattern, obj_type, weight) in patterns {
            if text_sample_lower.contains(pattern) {
                *scores.entry(obj_type.clone()).or_insert(0.0) += weight;
                signals.push(format!("Content text match: \"{}\" → {:?}", pattern, obj_type));
            }
        }
    }

    fn score_source_url(&self, obj: &KnowledgeObject, scores: &mut HashMap<ObjectType, f64>, signals: &mut Vec<String>) {
        if let Some(url) = &obj.metadata.source_url {
            let url_lower = url.to_lowercase();

            // YouTube videos
            if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
                *scores.entry(ObjectType::Video).or_insert(0.0) += 0.3;
                signals.push("Source URL: YouTube".to_string());
            }

            // GitHub repositories
            if url_lower.contains("github.com") {
                *scores.entry(ObjectType::Repository).or_insert(0.0) += 0.3;
                signals.push("Source URL: GitHub".to_string());
            }

            // Academic papers
            if url_lower.contains("arxiv.org") || url_lower.contains("scholar.google") {
                *scores.entry(ObjectType::ResearchPaper).or_insert(0.0) += 0.3;
                signals.push("Source URL: Academic".to_string());
            }
        }
    }
}

impl Processor for ContentClassifier {
    fn name(&self) -> &'static str {
        "content_classifier"
    }

    fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        let classification = self.classify(&knowledge_object);

        // Only update object type if confidence is above threshold
        // and the current type is a generic type (Note, Document, etc.)
        let current_type = &knowledge_object.object_type;
        let is_generic = matches!(
            current_type,
            ObjectType::Note | ObjectType::Document | ObjectType::Attachment
        );

        if classification.confidence >= self.confidence_threshold && is_generic {
            knowledge_object.object_type = classification.object_type.clone();

            // Store classification info in custom metadata
            knowledge_object.metadata.custom.insert(
                "classification".to_string(),
                serde_json::json!({
                    "object_type": format!("{:?}", classification.object_type),
                    "confidence": classification.confidence,
                    "signals": classification.signals,
                }),
            );

            let warnings = if classification.confidence < 0.6 {
                vec![format!(
                    "Classification confidence low ({:.0}%). Review recommended.",
                    classification.confidence * 100.0
                )]
            } else {
                vec![format!(
                    "Classified as {:?} (confidence {:.0}%)",
                    classification.object_type,
                    classification.confidence * 100.0
                )]
            };

            ProcessingResult::modified(knowledge_object, warnings)
        } else {
            // Still record classification info even if we don't change the type
            knowledge_object.metadata.custom.insert(
                "classification".to_string(),
                serde_json::json!({
                    "object_type": format!("{:?}", classification.object_type),
                    "confidence": classification.confidence,
                    "signals": classification.signals,
                    "skipped": !is_generic,
                }),
            );
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
    fn classifier_identifies_invoice_from_filename() {
        let classifier = ContentClassifier::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.source_file = Some("/invoices/2024-03-invoice.pdf".to_string());

        let result = classifier.classify(&obj);
        assert_eq!(result.object_type, ObjectType::Invoice);
        assert!(result.confidence > 0.0);
        assert!(!result.signals.is_empty());
    }

    #[test]
    fn classifier_identifies_receipt_from_filename() {
        let classifier = ContentClassifier::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.source_file = Some("/receipts/store-receipt-2024.pdf".to_string());

        let result = classifier.classify(&obj);
        assert_eq!(result.object_type, ObjectType::Receipt);
    }

    #[test]
    fn classifier_identifies_meeting_from_filename() {
        let classifier = ContentClassifier::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.source_file = Some("/meetings/team-sync-minutes.md".to_string());

        let result = classifier.classify(&obj);
        assert_eq!(result.object_type, ObjectType::Meeting);
    }

    #[test]
    fn classifier_keeps_existing_strong_type() {
        let classifier = ContentClassifier::new();
        let obj = create_test_object(ObjectType::Invoice);

        let result = classifier.classify(&obj);
        // Invoice type should be boosted by existing type
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn classifier_returns_note_for_generic_content() {
        let classifier = ContentClassifier::new();
        let obj = create_test_object(ObjectType::Note);

        let result = classifier.classify(&obj);
        // With no signals, should default to Note
        assert_eq!(result.object_type, ObjectType::Note);
    }

    #[test]
    fn processor_classifies_invoice_note() {
        let classifier = ContentClassifier::new();
        let mut obj = create_test_object(ObjectType::Note);
        obj.metadata.source_file = Some("/invoices/INV-001.pdf".to_string());

        let result = classifier.process(obj);
        assert!(result.modified);
        assert_eq!(result.knowledge_object.object_type, ObjectType::Invoice);
    }

    #[test]
    fn processor_does_not_override_strong_types() {
        let classifier = ContentClassifier::new();
        let obj = create_test_object(ObjectType::Invoice);

        let result = classifier.process(obj);
        // Invoice is not a "generic" type, so it should not be overridden
        assert_eq!(result.knowledge_object.object_type, ObjectType::Invoice);
    }

    #[test]
    fn classification_result_serializes() {
        let classifier = ContentClassifier::new();
        let obj = create_test_object(ObjectType::Note);

        let result = classifier.classify(&obj);
        let json = serde_json::to_value(&result).unwrap();
        let deserialized: ClassificationResult = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.object_type, result.object_type);
    }
}
