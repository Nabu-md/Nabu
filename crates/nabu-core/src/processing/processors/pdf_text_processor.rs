use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Extracts text content from PDF files.
///
/// For born-digital PDFs, text can be extracted directly.
/// For scanned PDFs, the text is extracted via OCR (handled by OcrProcessor).
///
/// Currently a stub that simulates PDF text extraction.
/// In production, this would call:
/// - macOS PDFKit for native PDF text extraction
/// - pdf-extract or lopdf for pure Rust fallback
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

        // Only process PDF documents
        let is_pdf = matches!(&context.object.content, ObjectContent::Binary { mime_type, .. } if mime_type == "application/pdf")
            || context.object.object_type == ObjectType::Document;

        if !is_pdf {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.4);
        let mut object = context.object.clone();

        // Simulate PDF text extraction
        // In production, this would use PDFKit or pdf-extract
        let extracted_text = simulate_pdf_text_extraction(&object);

        progress.set_progress(0.7);

        if !extracted_text.is_empty() {
            object.custom_properties.insert(
                "pdf_extracted_text".to_string(),
                crate::models::CustomPropertyValue::Text(extracted_text.clone()),
            );

            // Use extracted text as description
            if object.metadata.description.is_none() {
                let desc: String = extracted_text.chars().take(200).collect();
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

fn simulate_pdf_text_extraction(object: &KnowledgeObject) -> String {
    match &object.content {
        ObjectContent::Markdown(s) => s.clone(),
        ObjectContent::PlainText(s) => s.clone(),
        ObjectContent::RichHtml(s) => s.clone(),
        _ => {
            format!(
                "Extracted text from PDF '{}'. This is a placeholder for native PDF text extraction.",
                object.metadata.title.as_deref().unwrap_or("untitled")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pdf_text_extraction() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::Binary {
                mime_type: "application/pdf".to_string(),
                data: vec![0x25, 0x50, 0x44, 0x46], // %PDF header
                filename: Some("document.pdf".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfTextProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("pdf_extracted_text"));
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
