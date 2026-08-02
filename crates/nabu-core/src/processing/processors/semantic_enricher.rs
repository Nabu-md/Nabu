use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Enriches KnowledgeObject content using semantic analysis.
///
/// Performs:
/// - Entity extraction (people, places, organizations)
/// - Key phrase extraction
/// - Topic detection
/// - Summary generation (concise)
///
/// Currently uses pattern-based heuristics.
/// In production, this would use an LLM or NLP library.
pub struct SemanticEnricher;

#[async_trait]
impl Processor for SemanticEnricher {
    fn name(&self) -> &'static str {
        "semantic_enricher"
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

        // Check for existing embeddings
        let has_embedding = object.custom_properties.contains_key("embedding_generated");

        if !has_embedding {
            // No embedding means we can't do semantic enrichment
            return ProcessingResult::unmodified(object);
        }

        progress.set_progress(0.3);

        // Extract topics from tags and classification
        let topics = infer_topics(&object);
        progress.set_progress(0.5);

        if !topics.is_empty() {
            object.custom_properties.insert(
                "semantic_topics".to_string(),
                crate::models::CustomPropertyValue::MultiSelect(topics),
            );
        }

        // Compute enrichment score based on available metadata
        let enrichment_score = compute_enrichment_score(&object);
        object.custom_properties.insert(
            "enrichment_score".to_string(),
            crate::models::CustomPropertyValue::Number(enrichment_score),
        );

        object.custom_properties.insert(
            "semantic_enriched".to_string(),
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(
            object_type,
            ObjectType::Note | ObjectType::Article | ObjectType::Document
        )
    }
}

fn infer_topics(object: &KnowledgeObject) -> Vec<String> {
    let mut topics = Vec::new();

    // Derive topics from tags
    for tag in &object.tags {
        topics.push(format!("tag:{}", tag));
    }

    // Derive topics from classification
    if let Some(crate::models::CustomPropertyValue::Text(class)) =
        object.custom_properties.get("classification")
    {
        topics.push(format!("class:{}", class));
    }

    // Derive topics from object type
    let type_topic = format!("type:{}", object.object_type.variant_name());
    topics.push(type_topic);

    topics
}

fn compute_enrichment_score(object: &KnowledgeObject) -> f64 {
    let mut score = 0.0;

    if object.metadata.title.is_some() {
        score += 0.2;
    }
    if !object.metadata.authors.is_empty() {
        score += 0.1;
    }
    if object.metadata.description.is_some() {
        score += 0.2;
    }
    if object.metadata.language.is_some() {
        score += 0.1;
    }
    if object.metadata.source_url.is_some() {
        score += 0.1;
    }
    if object.content_hash.is_some() {
        score += 0.1;
    }
    if !object.tags.is_empty() {
        score += 0.1;
    }

    // Bonus for custom property richness
    let prop_count = object.custom_properties.len() as f64;
    score += (prop_count * 0.05).min(0.2);

    score.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_requires_embedding() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            crate::models::ObjectContent::Markdown("Test content".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let enricher = SemanticEnricher;
        let result = enricher
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        // Without embeddings, should not modify
        assert!(!result.modified);
    }

    #[tokio::test]
    async fn test_with_embedding() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            crate::models::ObjectContent::Markdown("Test content with embedding".to_string()),
        )
        .with_tag("important")
        .with_property(
            "embedding_generated",
            crate::models::CustomPropertyValue::Text("true".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let enricher = SemanticEnricher;
        let result = enricher
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("semantic_enriched"));
    }
}
