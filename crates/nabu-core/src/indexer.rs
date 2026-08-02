use crate::event_bus::kinds::INDEX_UPDATED;
use crate::event_bus::{EventBus, IndexOperation, IndexUpdatedEvent, PipelineEvent};
use crate::models::KnowledgeObject;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// The Indexer is the SINGLE search index for the Nabu platform.
///
/// No duplicate indexing systems exist.
/// All search operations go through the Indexer.
///
/// Currently uses in-memory inverted index as a stub.
/// In production, this would use Tantivy for full-text search.
pub struct Indexer {
    index: RwLock<HashMap<String, Vec<String>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
        }
    }

    /// Index a KnowledgeObject for search.
    pub fn index_object(&self, object: &KnowledgeObject) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;

        // Tokenize content and metadata
        let tokens = tokenize_object(object);

        // Add to inverted index
        for token in &tokens {
            index
                .entry(token.clone())
                .or_default()
                .push(object.id.to_string());
        }

        // Publish index updated event
        if let Some(ref bus) = self.event_bus {
            bus.publish(
                INDEX_UPDATED,
                &PipelineEvent::IndexUpdated(IndexUpdatedEvent {
                    object_id: object.id,
                    operation: IndexOperation::Added,
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        Ok(())
    }

    /// Remove an object from the index.
    pub fn remove_object(&self, object_id: Uuid) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;

        index.retain(|_, ids| {
            ids.retain(|id| id != &object_id.to_string());
            !ids.is_empty()
        });

        Ok(())
    }

    /// Search the index for matching tokens.
    pub fn search(&self, query: &str) -> Vec<String> {
        let index = self.index.read().ok();
        let mut results: Vec<String> = Vec::new();

        if let Some(index) = index {
            let query_tokens: Vec<String> = query
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            for token in &query_tokens {
                if let Some(ids) = index.get(token) {
                    results.extend(ids.clone());
                }
            }
        }

        results.sort();
        results.dedup();
        results
    }

    /// Number of unique tokens in the index.
    pub fn token_count(&self) -> usize {
        self.index.read().map(|i| i.len()).unwrap_or(0)
    }

    /// Clear the entire index (for rebuild).
    pub fn clear(&self) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;
        index.clear();
        Ok(())
    }
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

fn tokenize_object(object: &KnowledgeObject) -> Vec<String> {
    let mut tokens = Vec::new();

    // From title
    if let Some(title) = &object.metadata.title {
        tokens.extend(tokenize_str(title));
    }

    // From description
    if let Some(desc) = &object.metadata.description {
        tokens.extend(tokenize_str(desc));
    }

    // From tags
    for tag in &object.tags {
        tokens.push(tag.to_lowercase());
    }

    // From content type
    tokens.push(object.object_type.variant_name().to_string());

    tokens
}

fn tokenize_str(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 2) // Skip very short tokens
        .map(|s| {
            s.trim_matches(|c: char| c.is_ascii_punctuation())
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ObjectContent, ObjectMetadata};

    #[test]
    fn test_index_and_search() {
        let indexer = Indexer::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Hello world".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Test Note".to_string()),
            ..Default::default()
        });

        indexer.index_object(&obj).unwrap();

        let results = indexer.search("test");
        assert!(
            results.contains(&obj.id.to_string()),
            "Should find object by title token"
        );

        let results = indexer.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_from_index() {
        let indexer = Indexer::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::PlainText("Something to index".to_string()),
        );

        indexer.index_object(&obj).unwrap();
        assert!(indexer.token_count() > 0);

        indexer.remove_object(obj.id).unwrap();

        let results = indexer.search("something");
        assert!(
            !results.contains(&obj.id.to_string()),
            "Should not find removed object"
        );
    }
}
