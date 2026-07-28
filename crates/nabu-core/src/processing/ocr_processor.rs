//! OCR processor using Apple's Vision framework.
//!
//! This processor performs optical character recognition on images, screenshots,
//! and scanned PDFs using `VNRecognizeTextRequest`. It enriches [`KnowledgeObject`]
//! metadata with extracted text, confidence scores, and recognition details.
//!
//! # Constraints
//!
//! - macOS/iOS only (requires Vision framework).
//! - Never rejects storage on OCR failure.
//! - Never blocks worker threads.
//! - Never overwrites existing document content.
//! - Never invents text.

use std::collections::HashMap;
use std::sync::Arc;

use crate::processing::processor::{ProcessingDecision, ProcessingResult, Processor};
use crate::models::knowledge_object::KnowledgeObject;
use serde::{Deserialize, Serialize};

#[cfg(all(target_os = "macos", target_os = "ios"))]
use objc2::rc::Retained;
#[cfg(all(target_os = "macos", target_os = "ios"))]
use objc2_vision::*;

/// Structured OCR information attached to a knowledge object.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OcrInfo {
    /// Extracted text from the document.
    pub extracted_text: Option<String>,
    /// Average confidence of text recognition (0.0 - 1.0).
    pub confidence: Option<f64>,
    /// Recognition language(s) used by Vision (e.g., "en-US").
    pub recognition_language: Option<String>,
    /// Number of pages processed (for multi-page documents).
    pub page_count: Option<u32>,
    /// Processing duration in milliseconds.
    pub processing_duration_ms: Option<u64>,
    /// Whether this was a scanned document requiring OCR.
    pub is_scanned: Option<bool>,
    /// Warning message if OCR failed or was skipped.
    pub warning: Option<String>,
}

/// Processor that performs OCR using Apple's Vision framework.
///
/// This processor targets:
/// - Images (ObjectType::Image, Screenshot, Scan)
/// - Scanned PDFs (not born-digital PDFs with selectable text)
///
/// OCR results are stored in metadata.custom.ocr_info.
///
/// # Platform Support
///
/// Requires macOS or iOS. On unsupported platforms, the processor records
/// a warning and continues without modifying the object.
#[derive(Debug)]
pub struct OcrProcessor {
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    #[allow(dead_code)]
    recognition_level: VNRequestTextRecognitionLevel,
}

#[cfg(all(target_os = "macos", target_os = "ios"))]
impl Default for OcrProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrProcessor {
    /// Creates a new OCR processor with high-accuracy settings.
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    pub fn new() -> Self {
        Self {
            recognition_level: VNRequestTextRecognitionLevel::Accurate,
        }
    }

    /// Creates a new OCR processor with the specified recognition level.
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    pub fn with_recognition_level(level: VNRequestTextRecognitionLevel) -> Self {
        Self {
            recognition_level: level,
        }
    }

    /// Creates an OCR processor for unsupported platforms (no-op).
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub fn new() -> Self {
        Self {}
    }

    /// Determines whether an object is a candidate for OCR.
    fn is_ocr_candidate(object: &KnowledgeObject) -> bool {
        matches!(
            object.object_type,
            crate::models::knowledge_object::ObjectType::Image
                | crate::models::knowledge_object::ObjectType::Screenshot
                | crate::models::knowledge_object::ObjectType::Scan
                | crate::models::knowledge_object::ObjectType::Pdf
        )
    }

    /// Checks if a PDF is likely scanned (vs born-digital).
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    fn is_scanned_pdf(&self, _object: &KnowledgeObject) -> bool {
        // Placeholder: In a production implementation, this would inspect PDF metadata
        // or attempt to extract text using PDFKit. For now, we treat all PDFs as candidates.
        true
    }

    /// Performs OCR on an image using Vision framework.
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    fn ocr_image(&self, _image_path: &str) -> OcrInfo {
        let start = std::time::Instant::now();
        let mut info = OcrInfo::default();

        // Placeholder for actual Vision framework integration.
        // Full implementation would:
        // 1. Load image via CIImage or CGImage
        // 2. Create VNImageRequestHandler
        // 3. Create VNRecognizeTextRequest with .accurate level
        // 4. Set recognition languages (en + system locale)
        // 5. Request handler.perform([request])
        // 6. Extract observations, text, confidence
        // 7. Aggregate results

        info.warning = Some("OCR not yet fully integrated".to_string());
        info.processing_duration_ms = Some(start.elapsed().as_millis() as u64);

        info
    }

    /// Performs OCR on a PDF document using Vision framework.
    #[cfg(all(target_os = "macos", target_os = "ios"))]
    fn ocr_pdf(&self, _pdf_path: &str) -> OcrInfo {
        let start = std::time::Instant::now();
        let mut info = OcrInfo::default();

        // Placeholder for PDF OCR.
        // Would use PDFPage + CGImage extraction per page, then OCR each page.

        info.warning = Some("PDF OCR not yet fully integrated".to_string());
        info.processing_duration_ms = Some(start.elapsed().as_millis() as u64);

        info
    }
}

impl Processor for OcrProcessor {
    fn name(&self) -> &'static str {
        "ocr_processor"
    }

    fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        if !Self::is_ocr_candidate(&knowledge_object) {
            return ProcessingResult::unchanged(knowledge_object);
        }

        #[cfg(all(target_os = "macos", target_os = "ios"))]
        {
            self.process_macos(knowledge_object)
        }

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let info = OcrInfo {
                warning: Some("OCR not supported on this platform".to_string()),
                ..Default::default()
            };
            knowledge_object.metadata.custom.insert(
                "ocr_info".to_string(),
                serde_json::to_value(&info).unwrap_or_default(),
            );
            let warnings = vec!["OCR not supported on this platform".to_string()];
            ProcessingResult::modified(knowledge_object, warnings)
        }
    }

    #[cfg(all(target_os = "macos", target_os = "ios"))]
    fn process_macos(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        let source_file = match &knowledge_object.metadata.source_file {
            Some(path) => path,
            None => {
                let warnings = vec!["OCR skipped: no source file".to_string()];
                let info = OcrInfo {
                    warning: Some("No source file".to_string()),
                    ..Default::default()
                };
                knowledge_object.metadata.custom.insert(
                    "ocr_info".to_string(),
                    serde_json::to_value(&info).unwrap_or_default(),
                );
                return ProcessingResult::modified(knowledge_object, warnings);
            }
        };

        let is_pdf = matches!(
            knowledge_object.object_type,
            crate::models::knowledge_object::ObjectType::Pdf
        );

        let is_scanned = if is_pdf {
            self.is_scanned_pdf(&knowledge_object)
        } else {
            true
        };

        if !is_scanned {
            let info = OcrInfo {
                is_scanned: Some(false),
                warning: Some("Born-digital PDF; OCR skipped".to_string()),
                ..Default::default()
            };
            knowledge_object.metadata.custom.insert(
                "ocr_info".to_string(),
                serde_json::to_value(&info).unwrap_or_default(),
            );
            let warnings = vec!["Born-digital PDF; OCR skipped".to_string()];
            return ProcessingResult::modified(knowledge_object, warnings);
        }

        let ocr_info = if is_pdf {
            self.ocr_pdf(source_file)
        } else {
            self.ocr_image(source_file)
        };

        knowledge_object.metadata.custom.insert(
            "ocr_info".to_string(),
            serde_json::to_value(&ocr_info).unwrap_or_default(),
        );

        let warnings = if let Some(ref warn) = ocr_info.warning {
            vec![warn.clone()]
        } else if ocr_info.extracted_text.is_some() {
            vec![format!(
                "OCR completed: confidence={:.2}",
                ocr_info.confidence.unwrap_or(0.0)
            )]
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

    fn create_image_object() -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Image,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::Binary,
            metadata: ObjectMetadata {
                title: Some("Screenshot".to_string()),
                source_file: Some("/path/to/screenshot.png".to_string()),
                mime_type: Some("image/png".to_string()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn processor_skips_non_image_objects() {
        let processor = OcrProcessor::new();
        let mut obj = KnowledgeObject {
            object_type: ObjectType::Note,
            ..create_image_object()
        };
        let result = processor.process(obj.clone());
        assert!(!result.modified);
    }

    #[test]
    fn processor_marks_image_as_modified() {
        let processor = OcrProcessor::new();
        let obj = create_image_object();
        let result = processor.process(obj);
        assert!(result.modified);
        assert!(result.knowledge_object.metadata.custom.contains_key("ocr_info"));
    }

    #[test]
    fn processor_handles_missing_source_file() {
        let processor = OcrProcessor::new();
        let mut obj = create_image_object();
        obj.metadata.source_file = None;
        let result = processor.process(obj);
        assert!(result.modified);
        let info: OcrInfo = serde_json::from_value(
            result.knowledge_object.metadata.custom.get("ocr_info").unwrap().clone()
        ).unwrap();
        assert!(info.warning.is_some());
    }

    #[test]
    fn ocr_info_serializes_correctly() {
        let info = OcrInfo {
            extracted_text: Some("Hello world".to_string()),
            confidence: Some(0.95),
            recognition_language: Some("en-US".to_string()),
            page_count: Some(1),
            processing_duration_ms: Some(150),
            is_scanned: Some(true),
            warning: None,
        };

        let json = serde_json::to_value(&info).unwrap();
        let deserialized: OcrInfo = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.extracted_text, Some("Hello world".to_string()));
        assert_eq!(deserialized.confidence, Some(0.95));
    }
}

#[cfg(all(target_os = "macos", target_os = "ios"))]
#[cfg(test)]
mod vision_tests {
    use super::*;
    use objc2_vision::*;

    #[test]
    fn vision_framework_loads() {
        // Verify Vision framework constants/types are accessible
        let level = VNRequestTextRecognitionLevel::Accurate;
        assert!(matches!(level, VNRequestTextRecognitionLevel::Accurate));
    }
}