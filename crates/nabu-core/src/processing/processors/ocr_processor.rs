use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Performs OCR on image, scan, and screenshot content.
///
/// Uses the real macOS Vision framework (`VNRecognizeTextRequest`) through
/// [`crate::native::vision`]. No simulated OCR exists; when the native engine
/// is unavailable or detects no text, the object is returned unmodified.
///
/// The OCR result populates:
/// - `extracted_text` custom property with recognized text
/// - `ocr_confidence` metadata field (average confidence of detected lines)
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

        // Only process binary image content through Vision.
        let image_data = match &object.content {
            ObjectContent::Binary { mime_type, data, .. }
                if mime_type.starts_with("image/") =>
            {
                data.clone()
            }
            _ => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.4);

        // Vision OCR is a blocking native call; run it off the async executor.
        let engine_result = tokio::task::spawn_blocking(move || {
            crate::native::vision::recognize_text(&image_data)
        })
        .await;

        let recognized = match engine_result {
            Ok(Ok(lines)) if !lines.is_empty() => lines,
            Ok(Ok(_)) => return ProcessingResult::unmodified(object),
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "ocr_processor",
                    object_id = %object.id,
                    error = %e,
                    "Vision OCR unavailable; leaving object unmodified"
                );
                return ProcessingResult::unmodified(object);
            }
            Err(_) => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.7);

        let text = recognized
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let confidence = recognized.iter().map(|l| l.confidence).sum::<f64>()
            / recognized.len() as f64;

        // Add extracted text as description if no description exists
        if object.metadata.description.is_none() {
            let desc: String = text.chars().take(200).collect();
            object.metadata.description = Some(desc);
        }

        object.custom_properties.insert(
            "extracted_text".to_string(),
            crate::models::CustomPropertyValue::Text(text),
        );

        object.metadata.ocr_confidence = Some(confidence);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    fn fixture_png() -> Vec<u8> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ocr_fixture.png");
        std::fs::read(path).expect("ocr fixture should exist")
    }

    #[tokio::test]
    async fn test_ocr_real_image() {
        let obj = KnowledgeObject::new(
            ObjectType::Screenshot,
            ObjectContent::Binary {
                mime_type: "image/png".to_string(),
                data: fixture_png(),
                filename: Some("screenshot.png".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let ocr = OcrProcessor;
        let result = ocr
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        if cfg!(target_os = "macos") {
            // Real Vision OCR on a fixture with large text must detect it.
            let extracted = result
                .object
                .custom_properties
                .get("extracted_text")
                .map(|v| match v {
                    crate::models::CustomPropertyValue::Text(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            assert!(
                !extracted.is_empty(),
                "Vision OCR should extract text from the fixture"
            );
            assert!(result.object.metadata.ocr_confidence.is_some());
        } else {
            // Graceful no-op on non-macOS.
            assert_eq!(result.modified, false);
        }
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
