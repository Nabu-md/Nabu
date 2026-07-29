use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Processes PDF annotations (highlights, notes, stamps).
///
/// PDF annotations are stored in `.nabu/pdf-annotations/{hash}.json`.
/// This processor loads and indexes them as part of the pipeline.
///
/// In production, this would:
/// - Load existing annotations from disk
/// - Extract annotation text and type
/// - Store as graph edges or custom properties
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

        let is_pdf = matches!(&context.object.content, ObjectContent::Binary { mime_type, .. } if mime_type == "application/pdf")
            || context.object.object_type == ObjectType::Document;

        if !is_pdf {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        // Simulate annotation processing
        object.custom_properties.insert(
            "annotation_count".to_string(),
            crate::models::CustomPropertyValue::Number(0.0), // simulated: no annotations yet
        );

        object.custom_properties.insert(
            "annotations_processed".to_string(),
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(object_type, ObjectType::Document)
    }
}
