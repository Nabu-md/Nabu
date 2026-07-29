use std::fmt::Debug;

/// The context passed to each processor during pipeline execution.
///
/// Contains the current state of the item being processed, including
/// the inbound content, extracted metadata, and processing history.
/// Each processor can read and modify this context.
#[derive(Debug, Clone)]
pub struct ProcessingContext {
    /// The type of content being processed (e.g., "article", "image", "pdf").
    pub content_type: String,

    /// Raw content bytes from the capture source.
    pub content: Vec<u8>,

    /// Content metadata extracted by the capture handler.
    pub metadata: std::collections::HashMap<String, String>,

    /// Results from processor execution, keyed by processor name.
    pub processor_results: std::collections::HashMap<String, serde_json::Value>,

    /// Validation and warning messages accumulated during processing.
    pub messages: Vec<ProcessingMessage>,

    /// Whether processing should continue. A processor can set this to `true`
    /// to abort remaining processors (e.g., if a duplicate is detected).
    pub abort: bool,
}

impl ProcessingContext {
    /// Creates a new processing context with the given content.
    pub fn new(content_type: impl Into<String>, content: Vec<u8>) -> Self {
        ProcessingContext {
            content_type: content_type.into(),
            content,
            metadata: std::collections::HashMap::new(),
            processor_results: std::collections::HashMap::new(),
            messages: Vec::new(),
            abort: false,
        }
    }

    /// Adds a metadata key-value pair.
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Stores a processor result.
    pub fn add_result(&mut self, processor: impl Into<String>, value: serde_json::Value) {
        self.processor_results.insert(processor.into(), value);
    }

    /// Adds a processing message.
    pub fn add_message(&mut self, level: MessageLevel, message: impl Into<String>) {
        self.messages.push(ProcessingMessage {
            level,
            text: message.into(),
        });
    }
}

/// A message produced during processing (info, warning, or error).
#[derive(Debug, Clone)]
pub struct ProcessingMessage {
    pub level: MessageLevel,
    pub text: String,
}

/// The severity level of a processing message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// The result of processing a single item through the pipeline.
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// The final processing context after all processors have run.
    pub context: ProcessingContext,

    /// Whether the processing completed successfully.
    pub success: bool,

    /// Error message if processing failed.
    pub error: Option<String>,
}

impl ProcessingResult {
    /// Creates a successful processing result.
    pub fn success(context: ProcessingContext) -> Self {
        ProcessingResult {
            context,
            success: true,
            error: None,
        }
    }

    /// Creates a failed processing result.
    pub fn failure(context: ProcessingContext, error: impl Into<String>) -> Self {
        ProcessingResult {
            context,
            success: false,
            error: Some(error.into()),
        }
    }
}

/// The `Processor` trait — implemented by all pipeline processors.
///
/// Each processor performs a single transformation on the processing context.
/// Processors are chained together in the `ProcessingPipeline` and run in order.
///
/// ## Existing Processor Types (documented in architecture docs)
///
/// | Processor | Responsibility |
/// |-----------|---------------|
/// | `ContentClassifier` | Detects content type (invoice, receipt, meeting, etc.) |
/// | `DuplicateDetector` | SHA-256 content hash + filename similarity check |
/// | `TimelineExtractor` | Extracts dates from content/metadata |
/// | `MetadataExtractor` | Extracts title, author, publication date, etc. |
/// | `MetadataEnricher` | Enriches metadata with derived information |
/// | `AutoFiler` | Suggests destination folder based on content classification |
/// | `OcrProcessor` | Optical character recognition for images/scanned PDFs |
/// | `PdfTextProcessor` | Text extraction from born-digital PDFs |
/// | `PdfMetadataProcessor` | PDF metadata extraction (title, author, pages) |
/// | `PdfAnnotationProcessor` | PDF annotation extraction as graph edges |
pub trait Processor: Debug + Send + Sync {
    /// Returns the name of this processor (used for logging and result lookups).
    fn name(&self) -> &str;

    /// Processes the given context and returns a (potentially modified) context.
    ///
    /// If the processor detects an issue that should abort remaining processing,
    /// it should set `ctx.abort = true` before returning.
    fn process(&self, ctx: &mut ProcessingContext);
}

/// A no-op processor used as a placeholder during testing.
#[derive(Debug)]
pub struct NoopProcessor;

impl Processor for NoopProcessor {
    fn name(&self) -> &str {
        "noop"
    }

    fn process(&self, _ctx: &mut ProcessingContext) {
        // Does nothing
    }
}

/// A test processor that counts how many times it was invoked.
#[derive(Debug)]
pub struct CountingProcessor {
    pub name: String,
    pub count: std::sync::atomic::AtomicUsize,
}

impl CountingProcessor {
    pub fn new(name: impl Into<String>) -> Self {
        CountingProcessor {
            name: name.into(),
            count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl Processor for CountingProcessor {
    fn name(&self) -> &str {
        &self.name
    }

    fn process(&self, ctx: &mut ProcessingContext) {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ctx.add_message(MessageLevel::Info, format!("{} ran", self.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_context_creation() {
        let ctx = ProcessingContext::new("article", b"hello world".to_vec());
        assert_eq!(ctx.content_type, "article");
        assert_eq!(ctx.content, b"hello world");
        assert!(!ctx.abort);
    }

    #[test]
    fn test_processor_context_metadata() {
        let mut ctx = ProcessingContext::new("test", Vec::new());
        ctx.add_metadata("title", "Test Title");
        assert_eq!(
            ctx.metadata.get("title"),
            Some(&"Test Title".to_string())
        );
    }

    #[test]
    fn test_counting_processor() {
        let processor = CountingProcessor::new("counter");
        let mut ctx = ProcessingContext::new("test", Vec::new());

        processor.process(&mut ctx);
        assert_eq!(processor.count.load(std::sync::atomic::Ordering::SeqCst), 1);

        processor.process(&mut ctx);
        assert_eq!(processor.count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn test_noop_processor() {
        let processor = NoopProcessor;
        assert_eq!(processor.name(), "noop");

        let mut ctx = ProcessingContext::new("test", Vec::new());
        processor.process(&mut ctx);
        // No panic = pass
    }
}
