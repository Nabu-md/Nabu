use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Processes PDF annotations (highlights, notes, stamps) via the native
/// PDFKit engine ([`crate::native::pdfkit`]). No simulated annotations.
///
/// Extracted annotations are stored as custom properties:
/// - `annotation_count` — number of real annotations found
/// - `annotations_processed` — `"true"` when the engine ran
/// - `pdf_annotations` — JSON array of `{kind, contents, page}` entries
pub struct PdfAnnotationProcessor;

#[async_trait]
impl Processor for PdfAnnotationProcessor {
    fn name(&self) -> &'static str {
        "pdf_annotation_processor"
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
            ObjectContent::Binary { mime_type, data, .. } if mime_type == "application/pdf" => {
                data.clone()
            }
            _ => return ProcessingResult::unmodified(context.object.clone()),
        };

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        let engine_result = tokio::task::spawn_blocking(move || {
            crate::native::pdfkit::extract_annotations(&pdf_data)
        })
        .await;

        let annotations = match engine_result {
            Ok(Ok(anns)) => anns,
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "pdf_annotation_processor",
                    object_id = %object.id,
                    error = %e,
                    "PDFKit annotation extraction unavailable; leaving object unmodified"
                );
                return ProcessingResult::unmodified(object);
            }
            Err(_) => return ProcessingResult::unmodified(object),
        };

        object.custom_properties.insert(
            "annotation_count".to_string(),
            crate::models::CustomPropertyValue::Number(annotations.len() as f64),
        );

        object.custom_properties.insert(
            "annotations_processed".to_string(),
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        if !annotations.is_empty() {
            let json = serde_json::json!({
                "annotations": annotations.iter().map(|a| {
                    serde_json::json!({
                        "kind": a.kind,
                        "contents": a.contents,
                        "page": a.page,
                    })
                }).collect::<Vec<_>>()
            });
            object.custom_properties.insert(
                "pdf_annotations".to_string(),
                crate::models::CustomPropertyValue::Text(json.to_string()),
            );
        }

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
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pdf_fixture.pdf");
        std::fs::read(path).expect("pdf fixture should exist")
    }

    #[tokio::test]
    async fn test_pdf_annotation_processing_real() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::Binary {
                mime_type: "application/pdf".to_string(),
                data: fixture_pdf(),
                filename: Some("annotated.pdf".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = PdfAnnotationProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        if cfg!(target_os = "macos") {
            // Real PDFKit annotation scan on a plain fixture: processed=true,
            // count is whatever the real engine reports (0 for a fresh print).
            assert_eq!(
                result
                    .object
                    .custom_properties
                    .get("annotations_processed")
                    .map(|v| match v {
                        crate::models::CustomPropertyValue::Text(s) => s.as_str(),
                        _ => "",
                    }),
                Some("true")
            );
        } else {
            assert_eq!(result.modified, false);
        }
    }
}
