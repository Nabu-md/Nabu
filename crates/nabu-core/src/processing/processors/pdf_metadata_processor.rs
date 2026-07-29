use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Extracts metadata from PDF files (title, author, pages, etc.).
///
/// In production, this would use:
/// - macOS PDFKit for native PDF metadata extraction
/// - lopdf or pdf-extract for pure Rust fallback
pub struct PdfMetadataProcessor;

#[async_trait]
impl Processor for PdfMetadataProcessor {
    fn name(&self) -> &'static str {
        "pdf_metadata_processor"
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

        let is_pdf = matches!(&context.object.content, ObjectContent::Binary { mime_type, .. } if mime_type == "application/pdf")
            || context.object.object_type == ObjectType::Document;

        if !is_pdf {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        // Simulate PDF metadata extraction
        // In production, this would call PDFKit's metadata API
        let metadata = simulate_pdf_metadata(&object);

        progress.set_progress(0.6);

        if let Some(title) = metadata.title {
            if object.metadata.title.is_none() {
                object.metadata.title = Some(title);
            }
        }

        if !metadata.authors.is_empty() && object.metadata.authors.is_empty() {
            object.metadata.authors = metadata.authors;
        }

        if let Some(desc) = metadata.description {
            if object.metadata.description.is_none() {
                object.metadata.description = Some(desc);
            }
        }

        // PDF-specific metadata
        object.custom_properties.insert(
            "pdf_page_count".to_string(),
            crate::models::CustomPropertyValue::Number(metadata.file_size.unwrap_or(0) as f64),
        );

        object.custom_properties.insert(
            "pdf_metadata_extracted".to_string(),
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(object_type, ObjectType::Document)
    }
}

fn simulate_pdf_metadata(object: &KnowledgeObject) -> ObjectMetadata {
    ObjectMetadata {
        title: object.metadata.title.clone(),
        authors: object.metadata.authors.clone(),
        description: object.metadata.description.clone(),
        file_size: Some(1024 * 50), // simulated 50KB
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pdf_metadata() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::Binary {
                mime_type: "application/pdf".to_string(),
                data: vec![],
                filename: Some("report.pdf".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfMetadataProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("pdf_metadata_extracted"));
        assert!(result
            .object
            .custom_properties
            .contains_key("pdf_page_count"));
    }
}
