use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Generates vector embeddings for KnowledgeObject content.
///
/// Embeddings enable semantic search and similarity comparison.
/// Currently a stub — in production this would call:
/// - BGE-micro via Vectra (local-first option)
/// - OpenAI / Anthropic embedding API (cloud option)
/// - Local OSS model via candle or ort (Rust-native option)
///
/// The embedding result:
/// - Stores embedding vector as JSON in custom properties
/// - Sets `embedding_dimensions` for metadata
/// - Sets `embedding_model` for provenance
pub struct EmbeddingGenerator;

#[async_trait]
impl Processor for EmbeddingGenerator {
    fn name(&self) -> &'static str {
        "embedding_generator"
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

        // Get text content for embedding
        let text = match &object.content {
            ObjectContent::Markdown(s) => s.clone(),
            ObjectContent::PlainText(s) => s.clone(),
            ObjectContent::RichHtml(s) => s.clone(),
            _ => return ProcessingResult::unmodified(object),
        };

        // Skip very short content
        if text.len() < 20 {
            return ProcessingResult::unmodified(object);
        }

        progress.set_progress(0.5);

        // Simulate embedding generation
        // In production, this would call the embedding model
        let simulated_embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];

        object.custom_properties.insert(
            "embedding_vector".to_string(),
            crate::models::CustomPropertyValue::Text(
                serde_json::to_string(&simulated_embedding).unwrap_or_default(),
            ),
        );

        object.custom_properties.insert(
            "embedding_dimensions".to_string(),
            crate::models::CustomPropertyValue::Number(simulated_embedding.len() as f64),
        );

        object.custom_properties.insert(
            "embedding_model".to_string(),
            crate::models::CustomPropertyValue::Text("bge-micro-v2".to_string()),
        );

        object.custom_properties.insert(
            "embedding_generated".to_string(),
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(
            object_type,
            ObjectType::Note
                | ObjectType::Article
                | ObjectType::Document
                | ObjectType::Email
                | ObjectType::Bookmark
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    #[tokio::test]
    async fn test_embedding_generation() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("This is a sufficiently long note to generate embeddings for semantic search.".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let processor = EmbeddingGenerator;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("embedding_generated"));
    }

    #[tokio::test]
    async fn test_skips_short_content() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hi".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let processor = EmbeddingGenerator;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(result.modified, false);
    }
}
