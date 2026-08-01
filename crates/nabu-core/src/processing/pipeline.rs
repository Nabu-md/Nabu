use crate::event_bus::EventBus;
use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::KnowledgeObject;
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use std::sync::Arc;

/// The ProcessingPipeline is the ordered chain of all registered processors.
///
/// Every captured KnowledgeObject flows through this pipeline.
/// Processors execute in registration order.
/// Each processor receives the output of the previous processor.
///
/// No duplicate processing systems exist — this is THE single pipeline.
pub struct ProcessingPipeline {
    processors: Vec<Arc<dyn Processor>>,
    event_bus: Option<EventBus<crate::event_bus::PipelineEvent>>,
}

impl ProcessingPipeline {
    /// Create a new empty processing pipeline.
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            event_bus: None,
        }
    }

    /// Create a pipeline with an event bus for publishing events.
    pub fn with_event_bus(event_bus: EventBus<crate::event_bus::PipelineEvent>) -> Self {
        Self {
            processors: Vec::new(),
            event_bus: Some(event_bus),
        }
    }

    /// Register a processor in the pipeline.
    /// Processors execute in the order they are registered.
    pub fn register(&mut self, processor: Arc<dyn Processor>) {
        self.processors.push(processor);
    }

    /// Register a processor at a specific position.
    pub fn register_at(&mut self, index: usize, processor: Arc<dyn Processor>) {
        self.processors.insert(index, processor);
    }

    /// Run the full processing pipeline on a KnowledgeObject.
    ///
    /// Each processor runs in sequence, receiving the output of the previous.
    /// Progress is reported per-processor.
    /// Cancellation is checked between processors.
    pub async fn run(
        &self,
        mut object: KnowledgeObject,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult {
        let span = tracing::debug_span!(
            "nabu",
            subsystem = "processing",
            component = "pipeline",
            operation = "run",
            object_id = %object.id,
            object_type = %object.object_type.variant_name(),
            processor_count = self.processors.len(),
        );
        let _guard = span.enter();

        tracing::info!(
            subsystem = "processing",
            component = "pipeline",
            operation = "run",
            object_id = %object.id,
            "Pipeline execution started"
        );

        let total_processors = self.processors.len() as f64;

        for (i, processor) in self.processors.iter().enumerate() {
            // Check cancellation
            if cancellation.is_cancelled() {
                tracing::warn!(
                    subsystem = "processing",
                    component = "pipeline",
                    operation = "run",
                    object_id = %object.id,
                    processor_index = i,
                    "Pipeline cancelled"
                );
                return ProcessingResult {
                    object,
                    modified: true,
                    metadata: std::collections::HashMap::new(),
                    error: Some("Pipeline cancelled".to_string()),
                };
            }

            // Skip if processor doesn't support this object type
            if !processor.supports(&object.object_type) {
                tracing::trace!(
                    subsystem = "processing",
                    component = "pipeline",
                    operation = "skip",
                    processor = processor.name(),
                    object_type = %object.object_type.variant_name(),
                    "Processor skipped (unsupported type)"
                );
                continue;
            }

            // Calculate per-processor progress segment
            let base_progress = i as f64 / total_processors;
            let segment_size = 1.0 / total_processors;

            progress.set_progress(base_progress);

            // Create context
            let context = ProcessingContext::new(object);

            // Run processor with a child span
            let processor_span = tracing::debug_span!(
                "nabu",
                subsystem = "processing",
                component = "processor",
                operation = "process",
                processor = processor.name(),
                object_id = %context.object.id,
            );
            let _processor_guard = processor_span.enter();

            let start = std::time::Instant::now();
            let result = processor
                .process(&context, progress.clone(), cancellation.clone())
                .await;
            let duration = start.elapsed();

            drop(_processor_guard);

            tracing::debug!(
                subsystem = "processing",
                component = "pipeline",
                operation = "processor_result",
                processor = processor.name(),
                object_id = %context.object.id,
                duration_ms = duration.as_secs_f64() * 1000.0,
                has_error = result.error.is_some(),
                "Processor completed"
            );

            object = result.object;

            // Report progress after this processor
            progress.set_progress(base_progress + segment_size * 0.8);

            // Publish processor-level event
            if let Some(ref bus) = self.event_bus {
                if let Some(ref err) = result.error {
                    bus.publish(
                        crate::event_bus::kinds::ITEM_PROCESSING_FAILED,
                        &crate::event_bus::PipelineEvent::ItemProcessingFailed(
                            crate::event_bus::ItemProcessingFailedEvent {
                                object_id: object.id,
                                job_id: uuid::Uuid::nil(),
                                processor_name: processor.name().to_string(),
                                error: err.clone(),
                                retry_count: context.retry_attempt,
                                will_retry: false,
                                timestamp: chrono::Utc::now(),
                            },
                        ),
                    );
                } else {
                    bus.publish(
                        crate::event_bus::kinds::ITEM_PROCESSING_COMPLETED,
                        &crate::event_bus::PipelineEvent::ItemProcessingCompleted(
                            crate::event_bus::ItemProcessingCompletedEvent {
                                object_id: object.id,
                                job_id: uuid::Uuid::nil(),
                                processor_name: processor.name().to_string(),
                                timestamp: chrono::Utc::now(),
                            },
                        ),
                    );
                }
            }
        }

        progress.set_progress(1.0);

        tracing::info!(
            subsystem = "processing",
            component = "pipeline",
            operation = "run",
            object_id = %object.id,
            "Pipeline execution completed"
        );

        ProcessingResult {
            object,
            modified: true,
            metadata: std::collections::HashMap::new(),
            error: None,
        }
    }

    /// Number of registered processors.
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// List all registered processor names in order.
    pub fn processor_names(&self) -> Vec<&'static str> {
        self.processors.iter().map(|p| p.name()).collect()
    }

    /// Check if a processor with the given name is registered.
    pub fn has_processor(&self, name: &str) -> bool {
        self.processors.iter().any(|p| p.name() == name)
    }

    /// Remove a processor by name.
    pub fn remove_processor(&mut self, name: &str) {
        self.processors.retain(|p| p.name() != name);
    }
}

impl Default for ProcessingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the standard processing pipeline with all default processors.
pub fn build_standard_pipeline(event_bus: Option<EventBus<crate::event_bus::PipelineEvent>>) -> ProcessingPipeline {
    let mut pipeline = match event_bus {
        Some(bus) => ProcessingPipeline::with_event_bus(bus),
        None => ProcessingPipeline::new(),
    };

    // Register processors in the correct dependency order.
    // Later processors may depend on the output of earlier ones.
    let processors: Vec<Arc<dyn Processor>> = vec![
        // Phase 1: Content understanding
        Arc::new(super::processors::ContentClassifier),
        Arc::new(super::processors::DuplicateDetector::new()),
        Arc::new(super::processors::TimelineExtractor),
        // Phase 2: Metadata extraction & enrichment
        Arc::new(super::processors::MetadataExtractor),
        Arc::new(super::processors::MetadataEnricher),
        // Phase 3: Document processing
        Arc::new(super::processors::OcrProcessor),
        Arc::new(super::processors::PdfTextProcessor),
        Arc::new(super::processors::PdfMetadataProcessor),
        Arc::new(super::processors::PdfAnnotationProcessor),
        // Phase 4: AI-powered processing
        Arc::new(super::processors::WhisperProcessor),
        Arc::new(super::processors::EmbeddingGenerator),
        Arc::new(super::processors::SemanticEnricher),
        Arc::new(super::processors::AiSummariser),
        // Phase 5: Organization
        Arc::new(super::processors::AutoFiler::new()),
    ];

    for processor in processors {
        pipeline.register(processor);
    }

    pipeline
}

/// Pipeline execution ordering constants.
pub mod ordering {
    pub const CONTENT_CLASSIFIER: usize = 0;
    pub const DUPLICATE_DETECTOR: usize = 1;
    pub const TIMELINE_EXTRACTOR: usize = 2;
    pub const METADATA_EXTRACTOR: usize = 3;
    pub const METADATA_ENRICHER: usize = 4;
    pub const OCR_PROCESSOR: usize = 5;
    pub const PDF_TEXT_PROCESSOR: usize = 6;
    pub const PDF_METADATA_PROCESSOR: usize = 7;
    pub const PDF_ANNOTATION_PROCESSOR: usize = 8;
    pub const WHISPER_PROCESSOR: usize = 9;
    pub const EMBEDDING_GENERATOR: usize = 10;
    pub const SEMANTIC_ENRICHER: usize = 11;
    pub const AI_SUMMARISER: usize = 12;
    pub const AUTO_FILER: usize = 13;
}
