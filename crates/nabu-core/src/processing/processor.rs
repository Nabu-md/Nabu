use crate::diagnostic::Diagnostic;
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

/// Processing statistics produced during a processing run.
///
/// Each processor may populate this with its own metrics (e.g. number of
/// lints found, duration, bytes processed). The pipeline aggregates
/// these into the final [`ProcessingResult`].
///
/// All fields are optional with `#[serde(default)]` for forward compatibility —
/// future processors can add new keys without breaking deserialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessingStats {
    /// Number of diagnostics produced by this processor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics_count: Option<usize>,

    /// Number of lints/issues found by Harper (or other analyzer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lints_found: Option<usize>,

    /// Duration of processing in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Arbitrary processor-specific key-value metrics.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra: std::collections::HashMap<String, String>,
}

impl ProcessingStats {
    /// Create a new empty `ProcessingStats`.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set the diagnostics count.
    #[inline]
    pub fn with_diagnostics_count(mut self, count: usize) -> Self {
        self.diagnostics_count = Some(count);
        self
    }

    /// Builder: set the lints found count.
    #[inline]
    pub fn with_lints_found(mut self, count: usize) -> Self {
        self.lints_found = Some(count);
        self
    }

    /// Builder: set the processing duration in milliseconds.
    #[inline]
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Builder: insert an arbitrary key-value metric.
    #[inline]
    pub fn with_metric(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// Returns `true` when this stats object is in its default (all-`None`/empty) state.
    #[inline]
    pub fn is_default(&self) -> bool {
        self.diagnostics_count.is_none()
            && self.lints_found.is_none()
            && self.duration_ms.is_none()
            && self.extra.is_empty()
    }
}

/// Execution status of a processor or pipeline run.
///
/// This is a high-level status indicator. Detailed errors are captured in
/// [`ProcessingResult::error`]. Diagnostics from partial failures are
/// surfaced in [`ProcessingResult::diagnostics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ExecutionStatus {
    /// Processing completed successfully (may still have diagnostics).
    Success,
    /// Processing completed with one or more diagnostics but no fatal error.
    CompletedWithDiagnostics,
    /// Processing partially failed — some diagnostics may be present.
    PartialFailure,
    /// Processing was cancelled by the caller.
    Cancelled,
    /// Processing failed entirely.
    Failed,
}

impl Default for ExecutionStatus {
    #[inline]
    fn default() -> Self {
        Self::Success
    }
}

/// Serde predicate: `true` when `ExecutionStatus` is the default (`Success`).
#[inline]
fn is_default_status(status: &ExecutionStatus) -> bool {
    *status == ExecutionStatus::Success
}

/// The result of processing a KnowledgeObject through a processor.
///
/// ## Diagnostics
///
/// ProcessingResult may carry a collection of standardized
/// [`Diagnostic`] objects produced during processing. Every diagnostic
/// producer (currently Harper, soon spell checker, grammar engine, AI
/// assistants, OCR, metadata validators, plugins, LSP adapters) populates
/// this field rather than inventing its own report type.
///
/// The pipeline reads [`diagnostics`](Self::diagnostics) from each
/// `ProcessingResult` and, when an [`EventBus`](crate::event_bus::EventBus)
/// is attached, publishes them as a single
/// [`DiagnosticBatch`] via
/// [`publish_diagnostic_event`](crate::diagnostic::events::publish_diagnostic_event).
/// This keeps the EventBus quiet (one event per processor per resource) and
/// lets subscribers process a full analysis result atomically.
///
/// ## Thread Safety
///
/// `diagnostics` is a plain `Vec<Diagnostic>` — `Diagnostic` is `Send + Sync`,
/// so `ProcessingResult` remains safe to move across thread boundaries.
/// No shared mutable diagnostic state exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// The processed KnowledgeObject
    pub object: KnowledgeObject,

    /// Whether the processor made any changes
    pub modified: bool,

    /// Processor-specific metadata to pass to downstream processors
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,

    /// Optional error message (present if processing partially failed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Standardized diagnostics produced by this processor.
    ///
    /// Populated by analysis engines (Harper, spell checker, AI, OCR, etc.)
    /// using the shared [`Diagnostic`] model — **not** a producer-specific
    /// type. The pipeline publishes these through the EventBus as a
    /// [`DiagnosticBatch`] when an EventBus is available.
    ///
    /// Empty by default — processors that produce no diagnostics leave this
    /// as `Vec::new()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,

    /// Lightweight processing statistics (lint count, duration, etc.).
    ///
    /// Populated by processors that choose to report metrics. Forward-compatible:
    /// new fields use `#[serde(default)]` so older serialized payloads still
    /// deserialize.
    #[serde(default, skip_serializing_if = "ProcessingStats::is_default")]
    pub stats: ProcessingStats,

    /// High-level execution status of the processing run.
    ///
    /// Defaults to [`ExecutionStatus::Success`]. Processors that encounter
    /// non-fatal issues set `CompletedWithDiagnostics` or `PartialFailure`.
    #[serde(default, skip_serializing_if = "is_default_status")]
    pub status: ExecutionStatus,
}

impl ProcessingResult {
    pub fn new(object: KnowledgeObject) -> Self {
        Self {
            object,
            modified: true,
            metadata: std::collections::HashMap::new(),
            error: None,
            diagnostics: Vec::new(),
            stats: ProcessingStats::new(),
            status: ExecutionStatus::Success,
        }
    }

    pub fn unmodified(object: KnowledgeObject) -> Self {
        Self {
            object,
            modified: false,
            metadata: std::collections::HashMap::new(),
            error: None,
            diagnostics: Vec::new(),
            stats: ProcessingStats::new(),
            status: ExecutionStatus::Success,
        }
    }

    /// Attach standardized diagnostics to this result.
    ///
    /// This is the canonical way for a processor to surface analysis findings
    /// (spelling errors, grammar issues, metadata violations, OCR confidence
    /// problems, AI suggestions, etc.). Each entry must be a shared
    /// [`Diagnostic`] — no producer-specific diagnostic types should be
    /// introduced.
    ///
    /// Diagnostics are published through the EventBus as a single
    /// [`DiagnosticBatch`] by the pipeline, so callers should collect all
    /// diagnostics for a resource into one call rather than calling this
    /// method per-finding.
    #[inline]
    pub fn with_diagnostics(mut self, diagnostics: Vec<Diagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Append a single diagnostic to the result's diagnostic collection.
    ///
    /// Prefer `with_diagnostics` when you have all diagnostics at once.
    /// Use this method when diagnostics arrive incrementally.
    #[inline]
    pub fn add_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Returns `true` if this result carries any diagnostics.
    #[inline]
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Builder: set processing statistics.
    #[inline]
    pub fn with_stats(mut self, stats: ProcessingStats) -> Self {
        self.stats = stats;
        self
    }

    /// Builder: set execution status.
    #[inline]
    pub fn with_status(mut self, status: ExecutionStatus) -> Self {
        self.status = status;
        self
    }

    /// Builder: set the optional error message.
    #[inline]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Builder: set the optional error message only if `Some`.
    #[inline]
    pub fn with_error_opt(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
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
    fn supports(&self, _object_type: &crate::models::ObjectType) -> bool {
        // By default, processors run for all types.
        // Override in specific processors to narrow scope.
        true
    }
}
