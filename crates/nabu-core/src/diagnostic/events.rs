//! # Diagnostic Event System
//!
//! Strongly-typed event types and the `DiagnosticBatch` container that
//! every diagnostic producer in the Nabu Capability Platform publishes
//! through the existing [`EventBus`].
//!
//! ## Purpose
//!
//! Rather than letting each analysis engine (Markdown parser, AI assistant,
//! OCR engine, metadata validator, spell checker, grammar checker, plugins,
//! LSP adapters, automation rules, …) invent its own diagnostic
//! publication mechanism, they all emit a unified [`DiagnosticEvent`].
//! That event is wrapped in a [`PipelineEvent`] and transported by the
//! single, shared [`EventBus`].
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────┐
//!  │ Analysis Engine  │  (spell-checker, AI, plugin, LSP, …)
//!  └────────┬─────────┘
//!           │  builds a DiagnosticBatch
//!           ▼
//!  ┌──────────────────┐
//!  │ DiagnosticBatch  │  — Vec<Diagnostic> + origin + resource + metadata
//!  └────────┬─────────┘
//!           │  wraps in DiagnosticEvent
//!           ▼
//!  ┌──────────────────┐
//!  │ DiagnosticEvent  │  (BatchPublished | BatchCleared | BatchRemoved)
//!  └────────┬─────────┘
//!           │  to_pipeline_event()
//!           ▼
//!  ┌──────────────────┐
//!  │ PipelineEvent    │  ::Diagnostic(DiagnosticEvent)
//!  └────────┬─────────┘
//!           │  EventBus::publish by kind string
//!           ▼
//!  ┌──────────────────┐
//!  │  EventBus         │  (the single event transport)
//!  └────────┬─────────┘
//!           │  subscribers filter by kind
//!           ▼
//!  ┌──────────────────┐
//!  │ Subscribers      │  (indexer, graph, frontend bridge, …)
//!  └──────────────────┘
//! ```
//!
//! ## Event Lifecycle
//!
//! Every diagnostic flows through a small, well-defined lifecycle:
//!
//! 1. **BatchPublished** — An analysis engine completes work and publishes a
//!    [`DiagnosticBatch`] wrapped in `DiagnosticEvent::BatchPublished`.
//!    The batch carries a unique `batch_id` so subscribers can correlate
//!    follow-up events. When `is_incremental` is `false` (the default), the
//!    batch **replaces** all previously published diagnostics from the same
//!    `origin` for the same `resource_id`. When `is_incremental` is `true`,
//!    the diagnostics are **merged** into the existing set.
//!
//! 2. **BatchCleared** — The engine has finished a full re-analysis pass
//!    and wants to wipe its previous output for a `resource_id`. This is
//!    cheaper than publishing an empty batch and conveys clear intent.
//!
//! 3. **BatchRemoved** — A specific, previously-published batch (identified
//!    by `batch_id`) is retracted — for example, because a longer-running
//!    analysis was superseded by a newer result. Subscribers discard the
//!    diagnostics associated with that `batch_id`.
//!
//! ## Batching Strategy
//!
//! Producers **must** prefer batched publication. Publishing one diagnostic
//! at a time via individual events is an anti-pattern. Instead:
//!
//! - Collect all diagnostics for a resource and emit a single `BatchPublished`
//!   event.
//! - For streaming/incremental analysis (e.g. AI refinement), publish an
//!   initial batch with `is_incremental = false` (full replacement), then
//!   follow with incremental deltas marked `is_incremental = true` or use
//!   `replaces` to cancel a stale in-flight batch.
//! - Use `BatchCleared` to signal "no diagnostics remain" rather than
//!   publishing an empty batch.
//!
//! ## Ownership & Publishing Expectations
//!
//! - The EventBus owns nothing — it is a `Clone` handle around an
//!   `Arc<Mutex<…>>`. Producers clone the `EventBus`, call
//!   [`publish_diagnostic_event`], and forget about it.
//! - Events are consumed by value (the EventBus passes `&Events` to each
//!   handler, but handlers clone out what they need). Producers own the
//!   original `DiagnosticEvent` until they publish it.
//! - Multiple producers may publish concurrently to the same `EventBus` —
//!   all event types are `Send + Sync + Clone` and contain no shared mutable
//!   state.
//!
//! ## EventBus Integration
//!
//! Diagnostic events integrate with the existing EventBus **without any new
//! bus, channel, or dispatcher**. The integration path is:
//!
//! 1. `DiagnosticEvent` implements [`DiagnosticEventContract`], which
//!    provides `kind()` (the EventBus subscription string) and
//!    `to_pipeline_event()` (which wraps the event in
//!    `PipelineEvent::Diagnostic(…)`).
//! 2. [`publish_diagnostic_event`] calls `event.kind()` and
//!    `event.to_pipeline_event()`, then delegates to `EventBus::publish`.
//! 3. Subscribers register on the `EventBus<PipelineEvent>` with the
//!    appropriate kind constant from [`crate::event_bus::kinds`] and
//!    pattern-match on `PipelineEvent::Diagnostic(…)`.
//!
//! ## Extension Guidance
//!
//! - **New event variant**: Add it to [`DiagnosticEvent`] with
//!  `#[non_exhaustive]` already in place — downstream matchers already
//!  include a `_` arm. Add the kind constant, the `kind()` arm, and a test.
//! - **New batch field**: Add an `Option<T>` or `bool` with
//!  `#[serde(default)]` so older serialized batches still deserialize.
//! - **New event struct**: Follow the `#[serde(default)]` +
//!  `skip_serializing_if` pattern used throughout.
//!
//! ## Serialization
//!
//! All types in this module derive [`serde::Serialize`] and
//! [`serde::Deserialize`]. Event structs use `#[serde(default)]` and
//! `skip_serializing_if` so the wire format is compact and forward-compatible.

#[allow(unused_imports)]
use crate::diagnostic::{Diagnostic, DiagnosticError};
use crate::event_bus::kinds;
use crate::event_bus::{EventBus, PipelineEvent};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Serde helpers
// ---------------------------------------------------------------------------

/// `skip_serializing_if` predicate: `true` when the bool is `false`.
#[inline]
fn is_false(v: &bool) -> bool {
    !*v
}

// ---------------------------------------------------------------------------
// DiagnosticEventContract trait
// ---------------------------------------------------------------------------

/// The shared contract that every diagnostic event implements.
///
/// This trait abstracts over [`DiagnosticEvent`] and any future custom event
/// types, providing a uniform interface for EventBus publication:
///
/// - **kind**: the EventBus subscription string
/// - **origin**: which subsystem/producer emitted the event
/// - **resource_id**: the document/resource the event pertains to
/// - **timestamp**: when the event was created
/// - **to_pipeline_event**: integration point with the existing
///   [`PipelineEvent`] / [`EventBus`] infrastructure
///
/// All methods are side-effect-free and return values that are safe to use
/// across threads (`Send + Sync + Clone`).
///
/// # Example
///
/// ```ignore
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::diagnostic::events::{
///     DiagnosticBatch, DiagnosticEvent,
///     DiagnosticEventContract, publish_diagnostic_event,
/// };
///
/// let batch = DiagnosticBatch::new(
///     "spell-checker",
///     "vault:notes/example.md",
///     vec![],
/// );
/// let event = DiagnosticEvent::BatchPublished(batch);
/// publish_diagnostic_event(&event_bus, &event);
/// ```
pub trait DiagnosticEventContract:
    Send + Sync + Clone + Serialize + std::fmt::Debug
{
    /// Returns the event kind string used for EventBus subscription.
    fn kind(&self) -> &'static str;

    /// Returns the subsystem/producer that originated this event.
    fn origin(&self) -> &str;

    /// Returns the resource identifier (document path, URI, etc.) this
    /// event pertains to.
    fn resource_id(&self) -> &str;

    /// Returns when the event was created.
    fn timestamp(&self) -> DateTime<Utc>;

    /// Convert this event into a [`PipelineEvent`] for EventBus publication.
    ///
    /// The returned `PipelineEvent::Diagnostic(…)` is published to the
    /// EventBus under the event's [`kind`](Self::kind).
    fn to_pipeline_event(&self) -> PipelineEvent;
}

// ---------------------------------------------------------------------------
// DiagnosticBatch
// ---------------------------------------------------------------------------

/// A collection of diagnostics published together through the EventBus.
///
/// `DiagnosticBatch` is the transport unit for diagnostic publication.
/// Rather than firing one event per diagnostic, producers collect all
/// diagnostics for a resource into a single batch and publish them in one
/// `DiagnosticEvent::BatchPublished`. This keeps the EventBus quiet and
/// lets subscribers process a full analysis result atomically.
///
/// ## Fields
///
/// | Field           | Purpose                                                |
/// |-----------------|--------------------------------------------------------|
/// | `batch_id`      | Unique ID — lets subscribers track, update, or retract.|
/// | `origin`        | Subsystem/producer name (`"spell-checker"`, `"ai"`).  |
/// | `resource_id`   | Document or resource identifier (e.g. vault path).     |
/// | `timestamp`     | When the batch was produced.                           |
/// | `diagnostics`   | The actual diagnostics.                                |
/// | `is_incremental`| `false` = full replacement; `true` = merge/delta.     |
/// | `replaces`      | Batch ID superseded by this one (for streaming/cancel).|
///
/// ## Incremental vs. Full Replacement
///
/// When `is_incremental` is `false` (the default), subscribers should
/// treat the batch as a **complete replacement** of all diagnostics from
/// the same `origin` for the same `resource_id`. When `is_incremental` is
/// `true`, the diagnostics are **merged** into the existing set (new
/// diagnostics are added; existing ones with the same `code` are updated).
///
/// This supports both:
/// - **Full document analysis** — publish once with `is_incremental = false`.
/// - **Incremental analysis** — publish an initial batch, then follow-up
///   batches with `is_incremental = true` to add/modify individual diagnostics.
///
/// ## Future Expansion
///
/// The struct is designed for forward-compatible evolution:
/// - New fields use `Option<T>` with `#[serde(default)]` so older serialized
///   batches still deserialize.
/// - The `replaces` field enables streaming/cancellation semantics for AI
///   analysis without requiring new event types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticBatch {
    /// Unique identifier for this batch. Subscribers may use it to correlate
    /// follow-up `BatchRemoved` events or to implement cancellation of stale
    /// in-flight batches.
    pub batch_id: Uuid,

    /// The subsystem or producer that generated this batch (e.g.
    /// `"spell-checker"`, `"ai-assistant"`, `"lsp-markdown"`,
    /// `"plugin:com.example.linter"`).
    pub origin: String,

    /// The document or resource this batch pertains to (e.g. a vault path
    /// like `"vault:notes/meeting.md"` or a URI).
    pub resource_id: String,

    /// When this batch was produced.
    pub timestamp: DateTime<Utc>,

    /// The diagnostics in this batch.
    pub diagnostics: Vec<Diagnostic>,

    /// When `false` (default), this batch is a **full replacement** of all
    /// diagnostics from the same `origin` for the same `resource_id`.
    /// When `true`, diagnostics are **merged** into the existing set.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_incremental: bool,

    /// If set, this batch supersedes the batch with the given `batch_id`.
    /// Subscribers should discard the old batch's diagnostics and replace
    /// them with this batch's diagnostics.
    ///
    /// This is primarily useful for streaming analysis (e.g. AI refinement)
    /// where an in-progress result is replaced by a newer, more complete one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Uuid>,
}

impl DiagnosticBatch {
    /// Create a new diagnostic batch with the given origin, resource ID,
    /// and diagnostics.
    ///
    /// The `batch_id` is generated fresh (v4 UUID) and `timestamp` is set to
    /// the current UTC time. `is_incremental` defaults to `false` and
    /// `replaces` to `None`.
    ///
    /// This constructor does **not** validate — use [`validate`](Self::validate)
    /// when input comes from untrusted sources.
    #[inline]
    pub fn new(
        origin: impl Into<String>,
        resource_id: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            batch_id: Uuid::new_v4(),
            origin: origin.into(),
            resource_id: resource_id.into(),
            timestamp: Utc::now(),
            diagnostics,
            is_incremental: false,
            replaces: None,
        }
    }

    /// Create a new diagnostic batch, validating all fields.
    ///
    /// Returns [`DiagnosticEventError::EmptyOrigin`] if `origin` is empty,
    /// [`DiagnosticEventError::EmptyResourceId`] if `resource_id` is empty,
    /// [`DiagnosticEventError::EmptyBatch`] if `diagnostics` is empty, or the
    /// first [`DiagnosticError`] from validating any diagnostic in the batch.
    #[inline]
    pub fn try_new(
        origin: impl Into<String>,
        resource_id: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Self, DiagnosticEventError> {
        let origin_val = origin.into();
        let resource_id_val = resource_id.into();
        let batch = Self::new(origin_val, resource_id_val, diagnostics);
        batch.validate()?;
        Ok(batch)
    }

    /// Builder: explicitly set the `batch_id`.
    #[inline]
    pub fn with_batch_id(mut self, batch_id: Uuid) -> Self {
        self.batch_id = batch_id;
        self
    }

    /// Builder: set `is_incremental`.
    #[inline]
    pub fn incremental(mut self, is_incremental: bool) -> Self {
        self.is_incremental = is_incremental;
        self
    }

    /// Builder: set `replaces` — this batch supersedes the given batch ID.
    #[inline]
    pub fn replaces(mut self, replaces: Uuid) -> Self {
        self.replaces = Some(replaces);
        self
    }

    /// Builder: set `timestamp`.
    #[inline]
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Number of diagnostics in this batch.
    #[inline]
    pub fn diagnostic_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// Validate that the batch is well-formed:
    ///
    /// - `origin` is non-empty
    /// - `resource_id` is non-empty
    /// - `diagnostics` is non-empty (an empty batch is meaningless)
    /// - Every diagnostic passes [`Diagnostic::validate`]
    pub fn validate(&self) -> Result<(), DiagnosticEventError> {
        if self.origin.is_empty() {
            return Err(DiagnosticEventError::EmptyOrigin);
        }
        if self.resource_id.is_empty() {
            return Err(DiagnosticEventError::EmptyResourceId);
        }
        if self.diagnostics.is_empty() {
            return Err(DiagnosticEventError::EmptyBatch);
        }
        for (i, diag) in self.diagnostics.iter().enumerate() {
            if let Err(e) = diag.validate() {
                return Err(DiagnosticEventError::InvalidDiagnostic {
                    index: i,
                    detail: e.to_string(),
                });
            }
        }
        Ok(())
    }

    /// True when this batch is a full replacement (not incremental).
    #[inline]
    pub fn is_full_replacement(&self) -> bool {
        !self.is_incremental
    }
}

// ---------------------------------------------------------------------------
// DiagnosticEvent
// ---------------------------------------------------------------------------

/// A diagnostic lifecycle event published through the EventBus.
///
/// Every diagnostic producer creates `DiagnosticEvent` values and publishes
/// them via [`publish_diagnostic_event`], which wraps them in
/// `PipelineEvent::Diagnostic(…)` for the existing [`EventBus`].
///
/// Because this enum is `#[non_exhaustive]`, downstream matchers already
/// include a `_` arm, allowing new variants to be added without breaking
/// changes.
///
/// ## Event Flow
///
/// ```text
/// DiagnosticBatch  ──▶  DiagnosticEvent::BatchPublished  ──▶  EventBus
///                        DiagnosticEvent::BatchCleared    ──▶  EventBus
///                        DiagnosticEvent::BatchRemoved    ──▶  EventBus
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticEvent {
    /// A batch of diagnostics was published (created or updated).
    ///
    /// This is the primary diagnostic publication event. When the batch's
    /// `is_incremental` is `false`, subscribers should replace all existing
    /// diagnostics from the same `origin` for the same `resource_id`.
    /// When `is_incremental` is `true`, diagnostics are merged.
    BatchPublished(DiagnosticBatch),

    /// All diagnostics from an `origin` were cleared for a `resource_id`.
    ///
    /// Emitted when an analysis engine completes a full pass and signals
    /// that its previous output for this resource is fully superseded.
    /// This is cheaper than publishing an empty batch and conveys clear
    /// intent.
    BatchCleared(BatchClearedEvent),

    /// A specific previously-published batch was removed.
    ///
    /// The `batch_id` identifies which batch to retract. Subscribers should
    /// discard the diagnostics associated with that batch ID. This is useful
    /// for streaming/cancellation scenarios.
    BatchRemoved(BatchRemovedEvent),
}

impl DiagnosticEvent {
    /// Returns the event kind string used for EventBus subscription.
    ///
    /// This matches the corresponding constant in [`crate::event_bus::kinds`].
    pub fn kind(&self) -> &'static str {
        match self {
            Self::BatchPublished(_) => kinds::DIAGNOSTIC_BATCH_PUBLISHED,
            Self::BatchCleared(_) => kinds::DIAGNOSTIC_BATCH_CLEARED,
            Self::BatchRemoved(_) => kinds::DIAGNOSTIC_BATCH_REMOVED,
        }
    }

    /// Returns the subsystem/producer that originated this event.
    pub fn origin(&self) -> &str {
        match self {
            Self::BatchPublished(e) => &e.origin,
            Self::BatchCleared(e) => &e.origin,
            Self::BatchRemoved(e) => &e.origin,
        }
    }

    /// Returns the resource identifier this event pertains to.
    pub fn resource_id(&self) -> &str {
        match self {
            Self::BatchPublished(e) => &e.resource_id,
            Self::BatchCleared(e) => &e.resource_id,
            Self::BatchRemoved(e) => &e.resource_id,
        }
    }

    /// Returns the batch ID associated with this event, if applicable.
    ///
    /// `BatchPublished` and `BatchRemoved` carry a batch ID; `BatchCleared`
    /// does not (it clears all batches from an origin for a resource).
    pub fn batch_id(&self) -> Option<Uuid> {
        match self {
            Self::BatchPublished(e) => Some(e.batch_id),
            Self::BatchCleared(_) => None,
            Self::BatchRemoved(e) => Some(e.batch_id),
        }
    }

    /// Returns when this event was created.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::BatchPublished(e) => e.timestamp,
            Self::BatchCleared(e) => e.timestamp,
            Self::BatchRemoved(e) => e.timestamp,
        }
    }

    /// Convert this event into a [`PipelineEvent`] for EventBus publication.
    ///
    /// The returned `PipelineEvent::Diagnostic(…)` is published to the
    /// EventBus under the event's [`kind`](Self::kind).
    pub fn to_pipeline_event(&self) -> PipelineEvent {
        PipelineEvent::Diagnostic(self.clone())
    }

    /// Validate the event and all of its contents.
    ///
    /// This checks structural invariants:
    /// - `origin` and `resource_id` are non-empty
    /// - For `BatchPublished`: the batch validates (includes per-diagnostic
    ///   validation)
    /// - For `BatchCleared`/`BatchRemoved`: origin and resource_id are
    ///   non-empty
    ///
    /// Does **not** perform semantic validation (e.g. whether a `replaces`
    /// batch_id actually exists) — that requires external context.
    pub fn validate(&self) -> Result<(), DiagnosticEventError> {
        match self {
            Self::BatchPublished(e) => e.validate(),
            Self::BatchCleared(e) => e.validate(),
            Self::BatchRemoved(e) => e.validate(),
        }
    }
}

// ---------------------------------------------------------------------------
// DiagnosticEventContract implementation
// ---------------------------------------------------------------------------

impl DiagnosticEventContract for DiagnosticEvent {
    fn kind(&self) -> &'static str {
        self.kind()
    }

    fn origin(&self) -> &str {
        self.origin()
    }

    fn resource_id(&self) -> &str {
        self.resource_id()
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp()
    }

    fn to_pipeline_event(&self) -> PipelineEvent {
        self.to_pipeline_event()
    }
}

// ---------------------------------------------------------------------------
// BatchClearedEvent
// ---------------------------------------------------------------------------

/// Published when all diagnostics from an origin were cleared for a resource.
///
/// This signals that a full re-analysis pass has completed and the engine's
/// previous output for this `resource_id` should be discarded entirely —
/// the engine will publish a fresh `BatchPublished` if any diagnostics remain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchClearedEvent {
    /// Subsystems/producer that cleared its diagnostics.
    pub origin: String,

    /// The resource for which diagnostics were cleared.
    pub resource_id: String,

    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl BatchClearedEvent {
    /// Create a new `BatchClearedEvent` with the current timestamp.
    pub fn new(origin: impl Into<String>, resource_id: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            resource_id: resource_id.into(),
            timestamp: Utc::now(),
        }
    }

    /// Validate that `origin` and `resource_id` are non-empty.
    pub fn validate(&self) -> Result<(), DiagnosticEventError> {
        if self.origin.is_empty() {
            return Err(DiagnosticEventError::EmptyOrigin);
        }
        if self.resource_id.is_empty() {
            return Err(DiagnosticEventError::EmptyResourceId);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BatchRemovedEvent
// ---------------------------------------------------------------------------

/// Published when a specific previously-published diagnostic batch is
/// retracted.
///
/// The `batch_id` identifies which batch to remove. This is primarily used
/// for streaming/cancellation scenarios — e.g. an AI analysis that publishes
/// a preliminary batch and then replaces it with a refined one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRemovedEvent {
    /// Subsystems/producer that published the removed batch.
    pub origin: String,

    /// The resource the removed batch pertained to.
    pub resource_id: String,

    /// The `batch_id` of the batch being removed.
    pub batch_id: Uuid,

    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl BatchRemovedEvent {
    /// Create a new `BatchRemovedEvent` with the current timestamp.
    pub fn new(
        origin: impl Into<String>,
        resource_id: impl Into<String>,
        batch_id: Uuid,
    ) -> Self {
        Self {
            origin: origin.into(),
            resource_id: resource_id.into(),
            batch_id,
            timestamp: Utc::now(),
        }
    }

    /// Validate that `origin` and `resource_id` are non-empty.
    pub fn validate(&self) -> Result<(), DiagnosticEventError> {
        if self.origin.is_empty() {
            return Err(DiagnosticEventError::EmptyOrigin);
        }
        if self.resource_id.is_empty() {
            return Err(DiagnosticEventError::EmptyResourceId);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DiagnosticEventError
// ---------------------------------------------------------------------------

/// Structured errors returned by diagnostic event validation and
/// serialization operations.
///
/// All variants are `Serialize + Deserialize` so they can travel through
/// IPC and plugin boundaries. Errors are returned rather than panicking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticEventError {
    /// The event's `origin` field (subsystem/producer name) is empty.
    EmptyOrigin,

    /// The event's `resource_id` field (document/resource identifier) is
    /// empty.
    EmptyResourceId,

    /// A `DiagnosticBatch` was constructed with no diagnostics — an empty
    /// batch is meaningless. Use [`BatchClearedEvent`] to signal "no
    /// diagnostics remain" instead.
    EmptyBatch,

    /// A diagnostic within a batch failed validation.
    ///
    /// `index` is the position of the offending diagnostic in the batch's
    /// `diagnostics` vector; `detail` is a human-readable description from
    /// [`DiagnosticError`].
    InvalidDiagnostic {
        /// 0-based position of the offending diagnostic.
        index: usize,
        /// Human-readable error detail from `DiagnosticError::to_string()`.
        detail: String,
    },

    /// Serialization or deserialization of a diagnostic event failed.
    SerializationError(String),

    /// An unknown event kind string was encountered during deserialization.
    UnknownEventKind(String),
}

impl DiagnosticEventError {
    /// Convenience constructor for [`InvalidDiagnostic`](Self::InvalidDiagnostic).
    #[inline]
    pub fn invalid_diagnostic(index: usize, detail: impl Into<String>) -> Self {
        Self::InvalidDiagnostic {
            index,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for DiagnosticEventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyOrigin => write!(f, "diagnostic event origin must not be empty"),
            Self::EmptyResourceId => write!(f, "diagnostic event resource_id must not be empty"),
            Self::EmptyBatch => write!(f, "diagnostic batch must contain at least one diagnostic"),
            Self::InvalidDiagnostic { index, detail } => {
                write!(f, "invalid diagnostic at index {}: {}", index, detail)
            }
            Self::SerializationError(detail) => {
                write!(f, "diagnostic event serialization failure: {}", detail)
            }
            Self::UnknownEventKind(kind) => {
                write!(f, "unknown diagnostic event kind: {}", kind)
            }
        }
    }
}

impl std::error::Error for DiagnosticEventError {}

// ---------------------------------------------------------------------------
// EventBus publishing helper
// ---------------------------------------------------------------------------

/// Publish a diagnostic event through the EventBus.
///
/// This is the canonical entry point for diagnostic producers. It:
///
/// 1. Resolves the event's kind string via [`DiagnosticEventContract::kind`].
/// 2. Wraps the event in a [`PipelineEvent`] via
///    [`DiagnosticEventContract::to_pipeline_event`].
/// 3. Publishes it to the [`EventBus`] under the event's kind.
///
/// Producers should **never** call `EventBus::publish` directly with raw
/// `PipelineEvent` values for diagnostics — they should always go through
/// this helper or construct a [`DiagnosticEvent`] and call `to_pipeline_event()`.
///
/// # Example
///
/// ```ignore
/// use nabu_core::diagnostic::events::{
///     DiagnosticBatch, DiagnosticEvent, publish_diagnostic_event,
/// };
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::diagnostic::{Diagnostic, DiagnosticSeverity, TextRange, TextPosition};
///
/// let event_bus = EventBus::<PipelineEvent>::new();
///
/// let batch = DiagnosticBatch::new(
///     "spell-checker",
///     "vault:notes/example.md",
///     vec![Diagnostic::new(
///         DiagnosticSeverity::Warning,
///         TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
///         "possible typo",
///     )],
/// );
///
/// let event = DiagnosticEvent::BatchPublished(batch);
/// publish_diagnostic_event(&event_bus, &event);
/// ```
pub fn publish_diagnostic_event(
    event_bus: &EventBus<PipelineEvent>,
    event: &impl DiagnosticEventContract,
) {
    let kind = event.kind();
    let pipeline_event = event.to_pipeline_event();
    event_bus.publish(kind, &pipeline_event);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{DiagnosticSeverity, TextPosition, TextRange};
    use chrono::TimeZone;
    use std::sync::Arc;

    // Helper: build a simple valid diagnostic.
    fn sample_diagnostic() -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Warning,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 10)),
            "sample diagnostic",
        )
        .with_source("test-producer")
        .with_code("TEST_001")
    }

    fn sample_batch() -> DiagnosticBatch {
        DiagnosticBatch::new(
            "test-producer",
            "vault:notes/sample.md",
            vec![sample_diagnostic()],
        )
    }

    // --- DiagnosticBatch ---

    #[test]
    fn batch_new_sets_fields() {
        let diag = sample_diagnostic();
        let batch = DiagnosticBatch::new("ai-assistant", "vault:doc.md", vec![diag.clone()]);

        assert!(!batch.batch_id.is_nil());
        assert_eq!(batch.origin, "ai-assistant");
        assert_eq!(batch.resource_id, "vault:doc.md");
        assert_eq!(batch.diagnostics.len(), 1);
        assert_eq!(batch.diagnostics[0], diag);
        assert!(!batch.is_incremental);
        assert!(batch.replaces.is_none());
        assert!(batch.batch_id != Uuid::nil());
    }

    #[test]
    fn batch_builder_methods() {
        let batch = sample_batch()
            .incremental(true)
            .replaces(Uuid::new_v4())
            .with_batch_id(Uuid::new_v4())
            .with_timestamp(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap());

        assert!(batch.is_incremental);
        assert!(batch.replaces.is_some());
        assert_eq!(batch.timestamp, Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn batch_diagnostic_count() {
        let batch = DiagnosticBatch::new(
            "test",
            "vault:doc.md",
            vec![sample_diagnostic(), sample_diagnostic(), sample_diagnostic()],
        );
        assert_eq!(batch.diagnostic_count(), 3);
    }

    #[test]
    fn batch_is_full_replacement_default() {
        let batch = sample_batch();
        assert!(batch.is_full_replacement());
    }

    #[test]
    fn batch_incremental_is_not_full_replacement() {
        let batch = sample_batch().incremental(true);
        assert!(!batch.is_full_replacement());
    }

    #[test]
    fn batch_validate_rejects_empty_origin() {
        let batch = DiagnosticBatch::new("", "vault:doc.md", vec![sample_diagnostic()]);
        assert!(matches!(batch.validate(), Err(DiagnosticEventError::EmptyOrigin)));
    }

    #[test]
    fn batch_validate_rejects_empty_resource_id() {
        let batch = DiagnosticBatch::new("producer", "", vec![sample_diagnostic()]);
        assert!(matches!(batch.validate(), Err(DiagnosticEventError::EmptyResourceId)));
    }

    #[test]
    fn batch_validate_rejects_empty_diagnostics() {
        let batch = DiagnosticBatch::new("producer", "vault:doc.md", vec![]);
        assert!(matches!(batch.validate(), Err(DiagnosticEventError::EmptyBatch)));
    }

    #[test]
    fn batch_validate_rejects_invalid_diagnostic() {
        // A diagnostic with an empty message is invalid.
        let bad_diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "", // empty message
        );
        let batch = DiagnosticBatch::new("producer", "vault:doc.md", vec![bad_diag]);
        let err = batch.validate().unwrap_err();
        assert!(matches!(err, DiagnosticEventError::InvalidDiagnostic { .. }));
    }

    #[test]
    fn batch_try_new_accepts_valid() {
        let batch = DiagnosticBatch::try_new(
            "producer",
            "vault:doc.md",
            vec![sample_diagnostic()],
        )
        .unwrap();
        assert_eq!(batch.origin, "producer");
        assert_eq!(batch.diagnostics.len(), 1);
    }

    #[test]
    fn batch_try_new_propagates_validation_error() {
        let result = DiagnosticBatch::try_new("producer", "vault:doc.md", vec![]);
        assert!(matches!(result, Err(DiagnosticEventError::EmptyBatch)));
    }

    #[test]
    fn batch_validation_reports_correct_index() {
        let good = sample_diagnostic();
        let bad = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "",
        );
        let batch = DiagnosticBatch::new("p", "r", vec![good, bad]);
        let err = batch.validate().unwrap_err();
        assert!(matches!(
            err,
            DiagnosticEventError::InvalidDiagnostic {
                index: 1,
                ..
            }
        ));
    }

    // --- BatchClearedEvent ---

    #[test]
    fn cleared_event_new_and_validate() {
        let event = BatchClearedEvent::new("spell-checker", "vault:doc.md");
        assert_eq!(event.origin, "spell-checker");
        assert_eq!(event.resource_id, "vault:doc.md");
        assert!(event.validate().is_ok());
    }

    #[test]
    fn cleared_event_rejects_empty_origin() {
        let event = BatchClearedEvent::new("", "vault:doc.md");
        assert!(matches!(event.validate(), Err(DiagnosticEventError::EmptyOrigin)));
    }

    #[test]
    fn cleared_event_rejects_empty_resource_id() {
        let event = BatchClearedEvent::new("producer", "");
        assert!(matches!(event.validate(), Err(DiagnosticEventError::EmptyResourceId)));
    }

    // --- BatchRemovedEvent ---

    #[test]
    fn removed_event_new_and_validate() {
        let batch_id = Uuid::new_v4();
        let event = BatchRemovedEvent::new("ai-assistant", "vault:doc.md", batch_id);
        assert_eq!(event.origin, "ai-assistant");
        assert_eq!(event.resource_id, "vault:doc.md");
        assert_eq!(event.batch_id, batch_id);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn removed_event_rejects_empty_origin() {
        let event = BatchRemovedEvent::new("", "vault:doc.md", Uuid::new_v4());
        assert!(matches!(event.validate(), Err(DiagnosticEventError::EmptyOrigin)));
    }

    #[test]
    fn removed_event_rejects_empty_resource_id() {
        let event = BatchRemovedEvent::new("producer", "", Uuid::new_v4());
        assert!(matches!(event.validate(), Err(DiagnosticEventError::EmptyResourceId)));
    }

    // --- DiagnosticEvent ---

    #[test]
    fn event_kind_matches_constants() {
        let batch = sample_batch();
        let event = DiagnosticEvent::BatchPublished(batch);
        assert_eq!(event.kind(), kinds::DIAGNOSTIC_BATCH_PUBLISHED);

        let cleared = DiagnosticEvent::BatchCleared(
            BatchClearedEvent::new("p", "r"),
        );
        assert_eq!(cleared.kind(), kinds::DIAGNOSTIC_BATCH_CLEARED);

        let removed = DiagnosticEvent::BatchRemoved(
            BatchRemovedEvent::new("p", "r", Uuid::new_v4()),
        );
        assert_eq!(removed.kind(), kinds::DIAGNOSTIC_BATCH_REMOVED);
    }

    #[test]
    fn event_origin_and_resource_id() {
        let batch = sample_batch();
        let event = DiagnosticEvent::BatchPublished(batch);
        assert_eq!(event.origin(), "test-producer");
        assert_eq!(event.resource_id(), "vault:notes/sample.md");
    }

    #[test]
    fn event_batch_id_present_for_published_and_removed() {
        let batch = sample_batch();
        let batch_id = batch.batch_id;
        let published = DiagnosticEvent::BatchPublished(batch);
        assert_eq!(published.batch_id(), Some(batch_id));

        let removed = DiagnosticEvent::BatchRemoved(
            BatchRemovedEvent::new("p", "r", batch_id),
        );
        assert_eq!(removed.batch_id(), Some(batch_id));

        let cleared = DiagnosticEvent::BatchCleared(
            BatchClearedEvent::new("p", "r"),
        );
        assert_eq!(cleared.batch_id(), None);
    }

    #[test]
    fn event_timestamp() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        assert!(event.timestamp() <= Utc::now());
    }

    #[test]
    fn event_to_pipeline_event_wraps_in_diagnostic_variant() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let pipeline = event.to_pipeline_event();
        match &pipeline {
            PipelineEvent::Diagnostic(inner) => {
                assert_eq!(*inner, event);
            }
            other => panic!("expected PipelineEvent::Diagnostic, got {:?}", other),
        }
    }

    #[test]
    fn event_validate_delegates_to_inner() {
        let batch = DiagnosticBatch::new("", "vault:doc.md", vec![sample_diagnostic()]);
        let event = DiagnosticEvent::BatchPublished(batch);
        assert!(matches!(event.validate(), Err(DiagnosticEventError::EmptyOrigin)));
    }

    // --- DiagnosticEventContract trait ---

    #[test]
    fn contract_methods_work_through_trait_bound() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());

        fn check_contract<E: DiagnosticEventContract>(event: &E) {
            assert!(!event.kind().is_empty());
            assert!(!event.origin().is_empty());
            assert!(!event.resource_id().is_empty());
            let pipeline = event.to_pipeline_event();
            assert!(pipeline.timestamp().is_some());
        }

        check_contract(&event);
    }

    // --- Serialization round-trips ---

    #[test]
    fn batch_serialization_round_trip() {
        let batch = sample_batch();
        let json = serde_json::to_string(&batch).expect("serialize batch");
        let back: DiagnosticBatch = serde_json::from_str(&json).expect("deserialize batch");
        assert_eq!(batch, back);
    }

    #[test]
    fn event_serialization_round_trip() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let json = serde_json::to_string(&event).expect("serialize event");
        let back: DiagnosticEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, back);
    }

    #[test]
    fn cleared_event_serialization_round_trip() {
        let event = DiagnosticEvent::BatchCleared(BatchClearedEvent::new("p", "r"));
        let json = serde_json::to_string(&event).expect("serialize event");
        let back: DiagnosticEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, back);
    }

    #[test]
    fn removed_event_serialization_round_trip() {
        let batch_id = Uuid::new_v4();
        let event = DiagnosticEvent::BatchRemoved(BatchRemovedEvent::new("p", "r", batch_id));
        let json = serde_json::to_string(&event).expect("serialize event");
        let back: DiagnosticEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, back);
    }

    #[test]
    fn event_serializes_to_correct_kind_variant() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"BatchPublished\""));
    }

    // --- Forward compatibility: missing fields default ---

    #[test]
    fn batch_defaults_missing_fields() {
        // A minimal JSON payload with only required fields — is_incremental
        // and replaces should default.
        let batch_id = Uuid::new_v4();
        let minimal = format!(
            r#"{{"batch_id":"{}","origin":"p","resource_id":"r","timestamp":"2024-01-01T00:00:00Z","diagnostics":[]}}"#,
            batch_id
        );
        // Note: diagnostics is empty, so validate() will fail, but
        // deserialization should succeed with defaults.
        let back: DiagnosticBatch = serde_json::from_str(&minimal).unwrap();
        assert_eq!(back.batch_id, batch_id);
        assert!(!back.is_incremental);
        assert!(back.replaces.is_none());
    }

    #[test]
    fn event_ignores_unknown_fields() {
        // Construct a JSON with an extra field inside the BatchPublished variant
        // to verify that serde ignores unknown fields.
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let with_extra = format!(
            r#"{{"BatchPublished":{{"batch_id":"{}","origin":"p","resource_id":"r","timestamp":"{}","diagnostics":[],"future_field":"x"}}}}"#,
            event.batch_id().unwrap(),
            event.timestamp()
        );
        let back: DiagnosticEvent = serde_json::from_str(&with_extra).expect("deserialize with extra field");
        assert!(back.validate().is_err()); // empty diagnostics
    }

    #[test]
    fn cleared_event_defaults_missing_fields() {
        let minimal = r#"{"origin":"p","resource_id":"r","timestamp":"2024-01-01T00:00:00Z"}"#;
        let back: BatchClearedEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.origin, "p");
        assert_eq!(back.resource_id, "r");
    }

    // --- EventBus integration ---

    #[test]
    fn publish_diagnostic_event_delivers_to_subscriber() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let kind = event.kind();

        bus.subscribe(kind, move |pe: &PipelineEvent| {
            if let PipelineEvent::Diagnostic(e) = pe {
                received_clone.lock().unwrap().push(e.clone());
            }
        });

        publish_diagnostic_event(&bus, &event);

        let stored = received.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0], event);
    }

    #[test]
    fn publish_cleared_event_uses_correct_kind() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(0usize));
        let received_clone = received.clone();

        let event = DiagnosticEvent::BatchCleared(
            BatchClearedEvent::new("spell-checker", "vault:doc.md"),
        );
        bus.subscribe(event.kind(), move |pe: &PipelineEvent| {
            if let PipelineEvent::Diagnostic(e) = pe {
                let _ = e;
                *received_clone.lock().unwrap() += 1;
            }
        });

        publish_diagnostic_event(&bus, &event);

        assert_eq!(*received.lock().unwrap(), 1);
    }

    #[test]
    fn publish_removed_event_uses_correct_kind() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(0usize));
        let received_clone = received.clone();

        let event = DiagnosticEvent::BatchRemoved(
            BatchRemovedEvent::new("ai", "vault:doc.md", Uuid::new_v4()),
        );
        bus.subscribe(event.kind(), move |pe: &PipelineEvent| {
            if let PipelineEvent::Diagnostic(_) = pe {
                *received_clone.lock().unwrap() += 1;
            }
        });

        publish_diagnostic_event(&bus, &event);

        assert_eq!(*received.lock().unwrap(), 1);
    }

    #[test]
    fn event_delivered_under_correct_kind_only() {
        let bus = EventBus::<PipelineEvent>::new();
        let cleared_count = Arc::new(std::sync::Mutex::new(0usize));
        let published_count = Arc::new(std::sync::Mutex::new(0usize));
        let cleared_clone = cleared_count.clone();
        let published_clone = published_count.clone();

        bus.subscribe(kinds::DIAGNOSTIC_BATCH_CLEARED, move |_pe: &PipelineEvent| {
            *cleared_clone.lock().unwrap() += 1;
        });
        bus.subscribe(kinds::DIAGNOSTIC_BATCH_PUBLISHED, move |_pe: &PipelineEvent| {
            *published_clone.lock().unwrap() += 1;
        });

        // Publish a BatchPublished event — only the published subscriber
        // should fire.
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        publish_diagnostic_event(&bus, &event);

        assert_eq!(*cleared_count.lock().unwrap(), 0);
        assert_eq!(*published_count.lock().unwrap(), 1);
    }

    #[test]
    fn event_timestamp_available_via_pipeline_event() {
        let event = DiagnosticEvent::BatchPublished(sample_batch());
        let pipeline = event.to_pipeline_event();
        // PipelineEvent::timestamp() should return Some.
        assert!(pipeline.timestamp().is_some());
    }

    // --- Thread safety ---

    #[test]
    fn all_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticBatch>();
        assert_send_sync::<DiagnosticEvent>();
        assert_send_sync::<BatchClearedEvent>();
        assert_send_sync::<BatchRemovedEvent>();
        assert_send_sync::<DiagnosticEventError>();
    }

    // --- Error display ---

    #[test]
    fn error_display_messages() {
        assert!(!DiagnosticEventError::EmptyOrigin.to_string().is_empty());
        assert!(!DiagnosticEventError::EmptyResourceId.to_string().is_empty());
        assert!(!DiagnosticEventError::EmptyBatch.to_string().is_empty());
        assert!(DiagnosticEventError::invalid_diagnostic(2, "bad range").to_string().contains("index 2"));
        assert!(DiagnosticEventError::SerializationError("fail".into()).to_string().contains("fail"));
        assert!(DiagnosticEventError::UnknownEventKind("bad".into()).to_string().contains("bad"));
    }

    #[test]
    fn error_is_serializable() {
        let err = DiagnosticEventError::InvalidDiagnostic {
            index: 3,
            detail: "empty message".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let back: DiagnosticEventError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn error_implements_std_error() {
        let err = DiagnosticEventError::EmptyBatch;
        let _: &dyn std::error::Error = &err;
    }

    // --- Multiple diagnostics in a batch ---

    #[test]
    fn batch_with_multiple_diagnostics() {
        let diags = vec![
            Diagnostic::new(
                DiagnosticSeverity::Error,
                TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
                "error message",
            ),
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                TextRange::new(TextPosition::new(1, 0), TextPosition::new(1, 10)),
                "warning message",
            ),
            Diagnostic::new(
                DiagnosticSeverity::Hint,
                TextRange::empty(TextPosition::new(2, 0)),
                "hint message",
            ),
        ];

        let batch = DiagnosticBatch::new("multi-producer", "vault:doc.md", diags);
        assert_eq!(batch.diagnostic_count(), 3);
        assert!(batch.validate().is_ok());
    }
}
