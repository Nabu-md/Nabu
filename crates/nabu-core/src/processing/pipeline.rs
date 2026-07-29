use crate::event_bus::EventBus;

use super::processor::{MessageLevel, ProcessingContext, ProcessingResult, Processor};

/// An ordered chain of processors that transforms captured content.
///
/// The `ProcessingPipeline` maintains an ordered list of processors and runs
/// them in sequence. Each processor receives the output of the previous one,
/// allowing incremental enrichment of the processing context.
///
/// ## Event Integration
///
/// The pipeline can optionally be connected to an `EventBus` to publish
/// lifecycle events (started, completed, failed). When running inside a
/// `PipelineExecutor`, these events are wired automatically.
///
/// ## Thread Safety
///
/// The pipeline is `Send + Sync` and can be shared across workers via `Arc`.
#[derive(Debug)]
pub struct ProcessingPipeline {
    /// Ordered list of processors to run.
    processors: Vec<Box<dyn Processor>>,

    /// Optional event bus for publishing lifecycle events.
    event_bus: Option<EventBus<PipelineEvent>>,
}

/// Events emitted by the pipeline during processing.
#[derive(Debug, Clone)]
pub enum PipelineEvent {
    /// A processor in the pipeline has started.
    ProcessorStarted {
        processor_name: String,
        content_type: String,
    },
    /// A processor in the pipeline has completed.
    ProcessorCompleted {
        processor_name: String,
        content_type: String,
        has_messages: bool,
    },
    /// The full pipeline has completed.
    PipelineCompleted {
        content_type: String,
        processor_count: usize,
        success: bool,
    },
}

impl ProcessingPipeline {
    /// Creates a new empty pipeline.
    pub fn new() -> Self {
        ProcessingPipeline {
            processors: Vec::new(),
            event_bus: None,
        }
    }

    /// Creates a new pipeline with an event bus for lifecycle events.
    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        ProcessingPipeline {
            processors: Vec::new(),
            event_bus: Some(event_bus),
        }
    }

    /// Adds a processor to the end of the pipeline chain.
    pub fn add_processor<P: Processor + 'static>(&mut self, processor: P) {
        self.processors.push(Box::new(processor));
    }

    /// Returns the number of registered processors.
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// Returns the names of all registered processors, in order.
    pub fn processor_names(&self) -> Vec<&str> {
        self.processors.iter().map(|p| p.name()).collect()
    }

    /// Runs all processors on the given context.
    ///
    /// Processors are executed in registration order. If a processor sets
    /// `ctx.abort = true`, the remaining processors are skipped.
    ///
    /// Returns a `ProcessingResult` indicating success or failure.
    pub fn run(&self, ctx: &mut ProcessingContext) -> ProcessingResult {
        for processor in &self.processors {
            // Publish processor started event
            if let Some(ref bus) = self.event_bus {
                bus.publish(&PipelineEvent::ProcessorStarted {
                    processor_name: processor.name().to_string(),
                    content_type: ctx.content_type.clone(),
                });
            }

            // Run the processor
            processor.process(ctx);

            // Publish processor completed event
            if let Some(ref bus) = self.event_bus {
                bus.publish(&PipelineEvent::ProcessorCompleted {
                    processor_name: processor.name().to_string(),
                    content_type: ctx.content_type.clone(),
                    has_messages: !ctx.messages.is_empty(),
                });
            }

            // Check for abort or error
            if ctx.abort {
                // Find the error message if any
                let error = ctx
                    .messages
                    .iter()
                    .find(|m| m.level == MessageLevel::Error)
                    .map(|m| m.text.clone());
                return ProcessingResult::failure(
                    ctx.clone(),
                    error.unwrap_or_else(|| format!("aborted by processor '{}'", processor.name())),
                );
            }
        }

        // Publish pipeline completed event
        if let Some(ref bus) = self.event_bus {
            bus.publish(&PipelineEvent::PipelineCompleted {
                content_type: ctx.content_type.clone(),
                processor_count: self.processors.len(),
                success: true,
            });
        }

        ProcessingResult::success(ctx.clone())
    }
}

impl Default for ProcessingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::processor::CountingProcessor;

    #[test]
    fn test_empty_pipeline() {
        let pipeline = ProcessingPipeline::new();
        let mut ctx = ProcessingContext::new("test", b"hello".to_vec());
        let result = pipeline.run(&mut ctx);

        assert!(result.success);
        assert_eq!(pipeline.processor_count(), 0);
    }

    #[test]
    fn test_single_processor() {
        let mut pipeline = ProcessingPipeline::new();
        pipeline.add_processor(CountingProcessor::new("counter"));

        assert_eq!(pipeline.processor_count(), 1);
        assert_eq!(pipeline.processor_names(), vec!["counter"]);

        let mut ctx = ProcessingContext::new("test", Vec::new());
        let result = pipeline.run(&mut ctx);

        assert!(result.success);
        assert_eq!(result.context.messages.len(), 1);
        assert_eq!(result.context.messages[0].text, "counter ran");
    }

    #[test]
    fn test_multiple_processors_run_in_order() {
        let mut pipeline = ProcessingPipeline::new();
        pipeline.add_processor(CountingProcessor::new("first"));
        pipeline.add_processor(CountingProcessor::new("second"));
        pipeline.add_processor(CountingProcessor::new("third"));

        let mut ctx = ProcessingContext::new("test", Vec::new());
        let result = pipeline.run(&mut ctx);

        assert!(result.success);
        assert_eq!(result.context.messages.len(), 3);
        assert_eq!(result.context.messages[0].text, "first ran");
        assert_eq!(result.context.messages[1].text, "second ran");
        assert_eq!(result.context.messages[2].text, "third ran");
    }

    #[test]
    fn test_processor_aborts_pipeline() {
        use super::super::processor::MessageLevel;

        struct AbortProcessor;
        impl Processor for AbortProcessor {
            fn name(&self) -> &str {
                "abort"
            }
            fn process(&self, ctx: &mut ProcessingContext) {
                ctx.abort = true;
                ctx.add_message(MessageLevel::Error, "fatal error");
            }
        }

        let mut pipeline = ProcessingPipeline::new();
        pipeline.add_processor(CountingProcessor::new("before"));
        pipeline.add_processor(AbortProcessor);
        pipeline.add_processor(CountingProcessor::new("after"));

        let mut ctx = ProcessingContext::new("test", Vec::new());
        let result = pipeline.run(&mut ctx);

        assert!(!result.success);
        assert_eq!(result.context.messages.len(), 2); // before + abort (after skipped)
    }

    #[test]
    fn test_event_bus_integration() {
        let bus = EventBus::<PipelineEvent>::new();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let ev = events.clone();
        bus.subscribe(move |event: &PipelineEvent| {
            ev.lock().unwrap().push(event.clone());
        });

        let mut pipeline = ProcessingPipeline::with_event_bus(bus);
        pipeline.add_processor(CountingProcessor::new("p1"));
        pipeline.add_processor(CountingProcessor::new("p2"));

        let mut ctx = ProcessingContext::new("test", Vec::new());
        let result = pipeline.run(&mut ctx);

        assert!(result.success);

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 5); // 2 processors × (started + completed) + pipeline completed
    }
}
