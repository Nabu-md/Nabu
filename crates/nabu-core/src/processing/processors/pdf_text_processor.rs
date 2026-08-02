use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Extracts text content from PDF files via the native PDFKit engine
/// ([`crate::native::pdfkit`]). No simulated extraction exists.
pub struct PdfTextProcessor;

#[async_trait]
impl Processor for PdfTextProcessor {
    fn name(&self) -> &'static str {
        "pdf_text_processor"
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

        // Only process PDF binary content.
        let pdf_data = match &context.object.content {
            ObjectContent::Binary { mime_type, data, .. } if mime_type == "application/pdf" => {
                data.clone()
            }
            _ => return ProcessingResult::unmodified(context.object.clone()),
        };

        progress.set_progress(0.4);
        let mut object = context.object.clone();

        let engine_result = tokio::task::spawn_blocking(move || {
            crate::native::pdfkit::extract_text(&pdf_data)
        })
        .await;

        let extracted = match engine_result {
            Ok(Ok(text)) => text,
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "pdf_text_processor",
                    object_id = %object.id,
                    error = %e,
                    "PDFKit text extraction unavailable; leaving object unmodified"
                );
                return ProcessingResult::unmodified(object);
            }
            Err(_) => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.7);

        if !extracted.text.is_empty() {
            object.custom_properties.insert(
                "pdf_extracted_text".to_string(),
                crate::models::CustomPropertyValue::Text(extracted.text.clone()),
            );

            // Use extracted text as description
            if object.metadata.description.is_none() {
                let desc: String = extracted.text.chars().take(200).collect();
                object.metadata.description = Some(desc);
            }

            object.custom_properties.insert(
                "pdf_text_extracted".to_string(),
                crate::models::CustomPropertyValue::Text("true".to_string()),
            );
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(object_type, ObjectType::Document | ObjectType::Attachment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    fn fixture_pdf() -> Vec<u8> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pdf_fixture.pdf");
        std::fs::read(path).expect("pdf fixture should exist")
    }

    #[tokio::test]
    async fn test_pdf_text_extraction_real() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::Binary {
                mime_type: "application/pdf".to_string(),
                data: fixture_pdf(),
                filename: Some("document.pdf".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfTextProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        if cfg!(target_os = "macos") {
            // Real PDFKit extraction of a Chrome-printed PDF must contain text.
            let extracted = result
                .object
                .custom_properties
                .get("pdf_extracted_text")
                .map(|v| match v {
                    crate::models::CustomPropertyValue::Text(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            assert!(!extracted.is_empty(), "PDFKit should extract text");
            assert!(extracted.contains("QUICK BROWN FOX"));
        } else {
            assert_eq!(result.modified, false);
        }
    }

    #[tokio::test]
    async fn test_skips_non_pdf() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Just a note".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfTextProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(result.modified, false);
    }
}
