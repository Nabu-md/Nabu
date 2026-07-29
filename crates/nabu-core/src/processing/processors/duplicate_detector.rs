use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Mutex;

/// Detects duplicate content using SHA-256 hashing.
/// Compares content hashes against previously seen hashes.
/// Does NOT access the storage layer — operates purely on in-memory state.
pub struct DuplicateDetector {
    known_hashes: Mutex<HashSet<String>>,
}

impl DuplicateDetector {
    pub fn new() -> Self {
        Self {
            known_hashes: Mutex::new(HashSet::new()),
        }
    }

    /// Seed the detector with existing hashes (from storage).
    pub fn seed_hashes(&self, hashes: Vec<String>) {
        let mut known = self.known_hashes.lock().unwrap();
        known.extend(hashes);
    }

    fn compute_hash(content: &ObjectContent) -> String {
        match content {
            ObjectContent::Markdown(s) => hash_bytes(s.as_bytes()),
            ObjectContent::RichHtml(s) => hash_bytes(s.as_bytes()),
            ObjectContent::PlainText(s) => hash_bytes(s.as_bytes()),
            ObjectContent::Uri(s) => hash_bytes(s.as_bytes()),
            ObjectContent::Binary { data, .. } => hash_bytes(data),
        }
    }
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[async_trait]
impl Processor for DuplicateDetector {
    fn name(&self) -> &'static str {
        "duplicate_detector"
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

        // Compute content hash
        let hash = Self::compute_hash(&object.content);
        object.content_hash = Some(hash.clone());
        progress.set_progress(0.5);

        // Check against known hashes
        let is_duplicate = {
            let known = self.known_hashes.lock().unwrap();
            known.contains(&hash)
        };

        progress.set_progress(0.8);

        if is_duplicate {
            object.custom_properties.insert(
                "is_duplicate".to_string(),
                crate::models::CustomPropertyValue::Text("true".to_string()),
            );
            object.custom_properties.insert(
                "duplicate_hash".to_string(),
                crate::models::CustomPropertyValue::Text(hash.clone()),
            );
        }

        // Add to known hashes (prevents re-flagging in same batch)
        {
            let mut known = self.known_hashes.lock().unwrap();
            known.insert(hash);
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, _object_type: &ObjectType) -> bool {
        // Run for all object types that have content
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_duplicate() {
        let detector = DuplicateDetector::new();
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hello, world!".to_string()),
        );

        let ctx = ProcessingContext::new(obj.clone());
        let result1 = detector
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        // Second processing of same content should flag duplicate
        let ctx2 = ProcessingContext::new(obj);
        let result2 = detector
            .process(&ctx2, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let is_dup = result2
            .object
            .custom_properties
            .get("is_duplicate")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });
        assert_eq!(is_dup, Some("true".to_string()));
    }
}
