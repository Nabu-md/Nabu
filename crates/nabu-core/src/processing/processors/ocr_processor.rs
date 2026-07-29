use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Performs OCR on image, scan, and screenshot content.
///
/// Currently a stub that simulates OCR processing.
/// In production, this would call the native OCR engine
/// (macOS Vision Framework `VNRecognizeTextRequest`).
///
/// The OCR result populates:
/// - `extracted_text` custom property with recognized text
/// - `ocr_confidence` metadata field
/// - Extracted text is added as metadata description
pub struct OcrProcessor;

#[async_trait]
impl Processor for OcrProcessor {
    fn name(&self) -> &'static str {
        "ocr_processor"
    }

    async fn process(
        &self,
        context: &ProcessingContext,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult {
        if cancellation.is_cancelled() {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.1);
        let mut object = context.object.clone();

        // Only process binary/image type objects
        match &object.content {
            ObjectContent::Binary { mime_type, .. } => {
                if !mime_type.starts_with("image/") && mime_type != "application/pdf" {
                    return ProcessingResult::unmodified(object);
                }
            }
            _ => {
                // Screenshots and Scans may only have metadata, check custom properties
                if !matches!(object.object_type, ObjectType::Screenshot | ObjectType::Scan | ObjectType::Image) {
                    return ProcessingResult::unmodified(object);
                }
            }
        }

        progress.set_progress(0.4);

        // Simulate OCR processing
        // In production, this would call the native OCR engine
        let simulated_text = simulate_ocr(&object);

        if !simulated_text.is_empty() {
            progress.set_progress(0.7);

            // Add extracted text as description if no description exists
            if object.metadata.description.is_none() {
                let desc: String = simulated_text.chars().take(200).collect();
                object.metadata.description = Some(desc);
            }

            object.custom_properties.insert(
                "extracted_text".to_string(),
                crate::models::CustomPropertyValue::Text(simulated_text),
            );

            object.metadata.ocr_confidence = Some(0.85);
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(
            object_type,
            ObjectType::Image | ObjectType::Screenshot | ObjectType::Scan | ObjectType::Document
        )
    }
}

/// Simulate OCR by returning a placeholder text.
/// In production, this would be replaced with the native OCR call.
fn simulate_ocr(object: &KnowledgeObject) -> String {
    // If there's existing text content, return it
    match &object.content {
        ObjectContent::Markdown(s) => s.clone(),
        ObjectContent::PlainText(s) => s.clone(),
        ObjectContent::RichHtml(s) => s.clone(),
        _ => {
            // Generate a placeholder based on object type
            match object.object_type {
                ObjectType::Screenshot => {
                    format!(
                        "Screenshot OCR result for '{}'. Text detected: Sample extracted text from screenshot capture.",
                        object.metadata.title.as_deref().unwrap_or("untitled")
                    )
                }
                ObjectType::Image => {
                    format!(
                        "Image OCR result for '{}'. Text detected: Sample extracted text from image.",
                        object.metadata.title.as_deref().unwrap_or("untitled")
                    )
                }
                _ => "No text detected in this scan.".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ocr_screenshot() {
        let obj = KnowledgeObject::new(
            ObjectType::Screenshot,
            ObjectContent::Binary {
                mime_type: "image/png".to_string(),
                data: vec![0, 1, 2, 3],
                filename: Some("screenshot.png".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let ocr = OcrProcessor;
        let result = ocr
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("extracted_text"));
        assert!(result.object.metadata.ocr_confidence.is_some());
    }

    #[tokio::test]
    async fn test_ocr_skips_non_image() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hello world".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let ocr = OcrProcessor;
        let result = ocr
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        // Non-image types should not be modified
        assert_eq!(result.modified, false);
    }
}
