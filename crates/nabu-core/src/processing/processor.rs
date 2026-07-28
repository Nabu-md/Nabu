//! Processor trait and result types for the processing pipeline.
//!
//! This module defines the [`Processor`] trait, which represents a single
//! processing step in the processing pipeline. Each processor inspects and
//! optionally modifies a [`KnowledgeObject`], returning a structured
//! [`ProcessingResult`].
//!
//! # Constraints
//!
//! - Processors must NOT directly write to SQLite.
//! - Processors must NOT emit UI events.
//! - Processors must be composable and stateless where possible.
//! - Processors must never block asynchronously.

use std::fmt::Debug;

use crate::models::knowledge_object::KnowledgeObject;

/// The result of executing a single processor on a knowledge object.
///
/// A processor returns this result to indicate whether processing should
/// continue, whether the object was modified, and to attach warnings or
/// errors.
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingDecision {
    /// Processing completed successfully; continue to the next processor.
    Continue,
    /// Processing completed successfully but the object should not be stored.
    /// The pipeline will record the rejection in history and stop.
    Reject(String),
}

/// Structured result returned by a processor after execution.
///
/// The processing pipeline collects these results to build a complete
/// processing history and determine whether to proceed to storage.
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// The (possibly modified) knowledge object.
    pub knowledge_object: KnowledgeObject,
    /// Whether to continue or reject.
    pub decision: ProcessingDecision,
    /// Non-fatal warnings produced during processing.
    pub warnings: Vec<String>,
    /// Whether the processor modified the object's metadata or content.
    pub modified: bool,
}

impl ProcessingResult {
    /// Creates a successful processing result with no modifications.
    pub fn unchanged(knowledge_object: KnowledgeObject) -> Self {
        Self {
            knowledge_object,
            decision: ProcessingDecision::Continue,
            warnings: Vec::new(),
            modified: false,
        }
    }

    /// Creates a successful processing result with metadata modifications.
    pub fn modified(knowledge_object: KnowledgeObject, warnings: Vec<String>) -> Self {
        Self {
            knowledge_object,
            decision: ProcessingDecision::Continue,
            warnings,
            modified: true,
        }
    }

    /// Creates a result that rejects the object from further processing.
    pub fn rejected(knowledge_object: KnowledgeObject, reason: String) -> Self {
        Self {
            knowledge_object,
            decision: ProcessingDecision::Reject(reason),
            warnings: Vec::new(),
            modified: false,
        }
    }
}

/// A single processing step in the processing pipeline.
///
/// Implementors inspect a [`KnowledgeObject`], optionally modify its metadata,
/// optionally attach warnings, and optionally reject processing.
///
/// # Contract
///
/// - Implementors must NOT write to SQLite or any persistent storage.
/// - Implementors must NOT emit UI events.
/// - Implementors should be stateless or thread-safe.
/// - Implementors must NOT perform async blocking operations.
///
/// # Type Parameters
///
/// `E` is the error type used by the pipeline. It must implement
/// [`std::error::Error`] and be `Send + Sync + 'static`.
pub trait Processor: Debug + Send + Sync {
    /// Returns the human-readable name of this processor.
    ///
    /// Used for logging and storing processing history entries.
    fn name(&self) -> &'static str;

    /// Processes a knowledge object and returns a [`ProcessingResult`].
    ///
    /// # Arguments
    ///
    /// * `knowledge_object` - The object to process. The processor may
    ///   inspect or clone it, but should return the (possibly modified)
    ///   object in the result.
    ///
    /// # Returns
    ///
    /// A [`ProcessingResult`] indicating whether processing succeeded,
    /// whether the object was modified, and any warnings or rejection
    /// reason.
    fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{ObjectContent, ObjectMetadata, ObjectType};
    use uuid::Uuid;

    fn create_test_object() -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata::default(),
        }
    }

    #[test]
    fn unchanged_result_does_not_mark_modified() {
        let obj = create_test_object();
        let result = ProcessingResult::unchanged(obj.clone());
        assert!(!result.modified);
        assert_eq!(result.decision, ProcessingDecision::Continue);
        assert!(result.warnings.is_empty());
        assert_eq!(result.knowledge_object, obj);
    }

    #[test]
    fn modified_result_marks_modified() {
        let obj = create_test_object();
        let result = ProcessingResult::modified(
            obj.clone(),
            vec!["Low quality scan".to_string()],
        );
        assert!(result.modified);
        assert_eq!(result.decision, ProcessingDecision::Continue);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn rejected_result_provides_reason() {
        let obj = create_test_object();
        let result = ProcessingResult::rejected(obj.clone(), "Duplicate content".to_string());
        assert!(!result.modified);
        assert_eq!(
            result.decision,
            ProcessingDecision::Reject("Duplicate content".to_string())
        );
    }

    #[test]
    fn default_processor_trait_is_object_safe() {
        // This test verifies that Processor is object-safe by using
        // Box<dyn Processor>.
        let _processor: Box<dyn Processor> = Box::new(NoOpProcessor);
    }

    /// A processor that does nothing — used to verify trait object safety.
    #[derive(Debug)]
    struct NoOpProcessor;

    impl Processor for NoOpProcessor {
        fn name(&self) -> &'static str {
            "noop"
        }

        fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
            ProcessingResult::unchanged(knowledge_object)
        }
    }

    #[test]
    fn noop_processor_returns_unchanged() {
        let processor = NoOpProcessor;
        let obj = create_test_object();
        let result = processor.process(obj);
        assert!(!result.modified);
        assert_eq!(result.decision, ProcessingDecision::Continue);
    }
}