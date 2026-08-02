use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Extracts metadata from PDF files (title, author, pages, etc.) via the
/// native PDFKit engine ([`crate::native::pdfkit`]). No simulation.
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

        let pdf_data = match &context.object.content {
            ObjectContent::Binary {
                mime_type, data, ..
            } if mime_type == "application/pdf" => data.clone(),
            _ => return ProcessingResult::unmodified(context.object.clone()),
        };

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        let engine_result =
            tokio::task::spawn_blocking(move || crate::native::pdfkit::extract_metadata(&pdf_data))
                .await;

        let metadata = match engine_result {
            Ok(Ok(meta)) => meta,
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "pdf_metadata_processor",
                    object_id = %object.id,
                    error = %e,
                    "PDFKit metadata extraction unavailable; leaving object unmodified"
                );
                return ProcessingResult::unmodified(object);
            }
            Err(_) => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.6);

        if let Some(title) = metadata.title {
            if object.metadata.title.is_none() {
                object.metadata.title = Some(title);
            }
        }

        if let Some(author) = metadata.author {
            if object.metadata.authors.is_empty() {
                object.metadata.authors = vec![author];
            }
        }

        if let Some(desc) = metadata.subject {
            if object.metadata.description.is_none() {
                object.metadata.description = Some(desc);
            }
        }

        // Real PDF-specific metadata from PDFKit.
        object.custom_properties.insert(
            "pdf_page_count".to_string(),
            crate::models::CustomPropertyValue::Number(metadata.page_count as f64),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    fn fixture_pdf() -> Vec<u8> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/pdf_fixture.pdf"
        );
        std::fs::read(path).expect("pdf fixture should exist")
    }

    #[tokio::test]
    async fn test_pdf_metadata_real() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::Binary {
                mime_type: "application/pdf".to_string(),
                data: fixture_pdf(),
                filename: Some("report.pdf".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfMetadataProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        if cfg!(target_os = "macos") {
            assert!(result
                .object
                .custom_properties
                .contains_key("pdf_metadata_extracted"));
            // Chrome-printed fixture has exactly one page.
            let pages = result
                .object
                .custom_properties
                .get("pdf_page_count")
                .map(|v| match v {
                    crate::models::CustomPropertyValue::Number(n) => *n,
                    _ => -1.0,
                })
                .unwrap_or(-1.0);
            assert_eq!(pages, 1.0);
            // Chrome sets the PDF title from the HTML <title>.
            assert!(result.object.metadata.title.is_some());
        } else {
            assert!(!result.modified);
        }
    }
}
