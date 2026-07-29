use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::KnowledgeObject;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Context passed to every processor during execution.
/// Contains the current KnowledgeObject being processed and associated metadata.
#[derive(Debug, Clone)]
pub struct ProcessingContext {
    /// The KnowledgeObject being processed
    pub object: KnowledgeObject,

    /// Whether this is a retry
    pub is_retry: bool,

    /// Retry attempt number (0 for first attempt)
    pub retry_attempt: u32,

    /// Arbitrary key-value metadata set by previous processors
    pub metadata: std::collections::HashMap<String, String>,
}

impl ProcessingContext {
    pub fn new(object: KnowledgeObject) -> Self {
        Self {
            object,
            is_retry: false,
            retry_attempt: 0,
            metadata: std::collections::HashMap::new(),
        }
    }
}

/// The result of processing a KnowledgeObject through a processor.
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// The processed KnowledgeObject
    pub object: KnowledgeObject,

    /// Whether the processor made any changes
    pub modified: bool,

    /// Processor-specific metadata to pass to downstream processors
    pub metadata: std::collections::HashMap<String, String>,

    /// Optional error message (present if processing partially failed)
    pub error: Option<String>,
}

impl ProcessingResult {
    pub fn new(object: KnowledgeObject) -> Self {
        Self {
            object,
            modified: true,
            metadata: std::collections::HashMap::new(),
            error: None,
        }
    }

    pub fn unmodified(object: KnowledgeObject) -> Self {
        Self {
            object,
            modified: false,
            metadata: std::collections::HashMap::new(),
            error: None,
        }
    }
}

/// The canonical Processor trait for all processing in Nabu.
///
/// Every processor:
/// - Receives a ProcessingContext
/// - Returns a ProcessingResult
/// - Reports progress via ProgressReporter
/// - Supports cooperative cancellation via CancellationToken
///
/// Processors must remain modular:
/// - No processor instantiates another processor
/// - No processor directly invokes another processor
/// - No processor depends on queue internals
/// - No processor depends on capture internals
#[async_trait]
pub trait Processor: Send + Sync {
    /// The name of this processor (used for job routing and logging).
    fn name(&self) -> &'static str;

    /// Execute this processor on the given context.
    async fn process(
        &self,
        context: &ProcessingContext,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult;

    /// Whether this processor should run for the given object type.
    fn supports(&self, object_type: &crate::models::ObjectType) -> bool {
        // By default, processors run for all types.
        // Override in specific processors to narrow scope.
        true
    }
}
