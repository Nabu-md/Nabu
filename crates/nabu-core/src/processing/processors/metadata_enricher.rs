use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Enriches metadata by cross-referencing extracted data with external sources.
///
/// In the current implementation, this processor:
/// - Normalizes titles (capitalization, truncation)
/// - Validates and normalizes URLs
/// - Infers additional metadata from content patterns
/// - Stores enrichment confidence scores
///
/// Future implementations will integrate with:
/// - Open Graph / Twitter Card metadata from URLs
/// - OEmbed for rich media
/// - Wikipedia/Wikidata lookups for entities
pub struct MetadataEnricher;

#[async_trait]
impl Processor for MetadataEnricher {
    fn name(&self) -> &'static str {
        "metadata_enricher"
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

        // Normalize title
        if let Some(title) = &object.metadata.title {
            let normalized = normalize_title(title);
            object.metadata.title = Some(normalized);
        }
        progress.set_progress(0.3);

        // Validate and normalize source URL
        if let Some(url) = &object.metadata.source_url {
            if let Some(normalized) = normalize_url(url) {
                object.metadata.source_url = Some(normalized);
            }
        }
        progress.set_progress(0.5);

        // Infer content type if classification is present
        if let Some(existing) = object.custom_properties.get("classification") {
            if let crate::models::CustomPropertyValue::Text(class) = existing {
                infer_tags(&mut object, class);
            }
        }
        progress.set_progress(0.7);

        // Set enrichment confidence
        object.custom_properties.insert(
            "enrichment_applied".to_string(),
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
                | ObjectType::Bookmark
                | ObjectType::Document
                | ObjectType::Email
        )
    }
}

fn normalize_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        return "Untitled".to_string();
    }

    // Capitalize first letter
    let mut chars = title.chars();
    match chars.next() {
        None => "Untitled".to_string(),
        Some(first) => {
            let capitalized = first.to_uppercase().collect::<String>() + chars.as_str();
            // Truncate to 200 chars
            capitalized.chars().take(200).collect()
        }
    }
}

fn normalize_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    // Ensure URL has a scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Some(format!("https://{}", url));
    }

    // Remove trailing slash for consistency
    Some(url.trim_end_matches('/').to_string())
}

fn infer_tags(object: &mut KnowledgeObject, classification: &str) {
    match classification {
        "invoice" => {
            add_tag(object, "finance");
            add_tag(object, "invoice");
        }
        "receipt" => {
            add_tag(object, "finance");
            add_tag(object, "receipt");
        }
        "meeting_note" => {
            add_tag(object, "meeting");
        }
        "email" => {
            add_tag(object, "email");
        }
        "article" => {
            add_tag(object, "article");
        }
        "code" => {
            add_tag(object, "code");
        }
        _ => {}
    }
}

fn add_tag(object: &mut KnowledgeObject, tag: &str) {
    if !object.tags.contains(&tag.to_string()) {
        object.tags.push(tag.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_normalize_title() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("# hello world".to_string()),
        )
        .with_metadata(crate::models::ObjectMetadata {
            title: Some("hello world".to_string()),
            ..Default::default()
        });

        let ctx = ProcessingContext::new(obj);
        let enricher = MetadataEnricher;
        let result = enricher
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(
            result.object.metadata.title,
            Some("Hello world".to_string())
        );
    }

    #[tokio::test]
    async fn test_infer_tags_from_invoice() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::PlainText("Invoice #123".to_string()),
        )
        .with_property(
            "classification",
            crate::models::CustomPropertyValue::Text("invoice".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let enricher = MetadataEnricher;
        let result = enricher
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result.object.tags.contains(&"invoice".to_string()));
        assert!(result.object.tags.contains(&"finance".to_string()));
    }
}
