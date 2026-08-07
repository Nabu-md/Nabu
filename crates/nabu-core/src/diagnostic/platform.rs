//! # Diagnostic Platform — Unified Diagnostic Retrieval
//!
//! The canonical platform for retrieving diagnostics from any analysis engine.
//! Editors (Dioxus, Monaco, CodeMirror, mobile, plugins) request diagnostics
//! through this platform rather than calling individual analyzers (Harper, AI,
//! etc.) directly.
//!
//! ## Architecture
//!
//! ```text
//!  Editor (IPC)
//!      │  diagnostic_requested(request)
//!      ▼
//!  DiagnosticPlatform  ──►  Vec<DiagnosticProvider>
//!      │  each provider.run_analysis(&text) → Vec<Diagnostic>
//!      │  (providers are isolated; the platform never exposes them)
//!      ▼
//!  DiagnosticBatch  +  DiagnosticStyleMap
//!      │
//!      ▼
//!  DiagnosticEvent::BatchPublished  ──▶  EventBus (async subscribers)
//!      │
//!      ▼
//!  DiagnosticResponse (IPC)
//! ```
//!
//! ## Provider Independence
//!
//! The platform is completely independent of any specific analysis engine.
//! Providers register via the [`DiagnosticProvider`] trait and are identified
//! by their `origin` string (e.g. `"harper"`, `"ai-assistant"`). The platform
//! routes requests to providers by name without exposing the provider type
//! itself — editors never see Harper, LSP, or any other engine type.
//!
//! ## Thread Safety
//!
//! `DiagnosticPlatform` uses a `RwLock<HashMap>` for provider storage, allowing
//! concurrent read access (multiple editors requesting diagnostics simultaneously)
//! while serializing only provider registration. All registered providers must
//! be `Send + Sync`. The platform itself is `Send + Sync`.
//!
//! ## EventBus Integration
//!
//! Every retrieval publishes a `DiagnosticEvent::BatchPublished` through the
//! EventBus (when one is attached), enabling asynchronous event-based
//! subscribers to stay in sync. This is **in addition to** — not a replacement
//! for — the IPC response. IPC is used for explicit, on-demand requests; the
//! EventBus carries asynchronous updates for streaming, live, or background
//! analysis.
//!
//! ## Future Compatibility
//!
//! - Live diagnostics: providers can register a `live: true` flag and the
//!   platform will poll or stream their results.
//! - Incremental diagnostics: batches carry `is_incremental` and `replaces`
//!   fields for streaming/cancellation.
//! - LSP integration: an LSP provider simply implements `DiagnosticProvider`.
//! - Plugin diagnostics: a plugin-provided provider is registered through the
//!   Capability Platform.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::diagnostic::{
    Diagnostic, DiagnosticBatch, DiagnosticError, DiagnosticEvent,
};
use crate::diagnostic::events::publish_diagnostic_event;
use crate::event_bus::{EventBus, PipelineEvent};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Structured errors for the diagnostic platform
// ---------------------------------------------------------------------------

/// Structured errors returned by the `DiagnosticPlatform` and the
/// `diagnostic_requested` IPC command.
///
/// All variants are `Serialize + Deserialize` so they can travel through IPC
/// to frontend consumers. Errors are returned as `Result::Err` — never via
/// panicking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticPlatformError {
    /// No provider is registered under the requested origin name.
    ProviderNotFound {
        /// The origin/producer name that was not found (e.g. `"harper"`).
        origin: String,
    },

    /// The request was rejected: text or resource_id was empty.
    InvalidRequest {
        /// Human-readable detail about why the request was rejected.
        detail: String,
    },

    /// A provider's analysis failed. The `origin` identifies which provider
    /// failed; `detail` contains the underlying error message.
    ProviderError {
        /// The origin/producer name that encountered the error.
        origin: String,
        /// Human-readable error detail from the provider.
        detail: String,
    },

    /// A diagnostic produced by a provider failed validation.
    ///
    /// `origin` identifies the provider; `detail` describes the validation
    /// failure.
    InvalidDiagnostic {
        /// The origin/producer name.
        origin: String,
        /// Human-readable detail about the validation failure.
        detail: String,
    },
}

impl std::fmt::Display for DiagnosticPlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProviderNotFound { origin } => {
                write!(f, "no diagnostic provider registered for origin '{}'", origin)
            }
            Self::InvalidRequest { detail } => {
                write!(f, "invalid diagnostic request: {}", detail)
            }
            Self::ProviderError { origin, detail } => {
                write!(f, "diagnostic provider '{}' failed: {}", origin, detail)
            }
            Self::InvalidDiagnostic { origin, detail } => {
                write!(
                    f,
                    "provider '{}' produced an invalid diagnostic: {}",
                    origin, detail
                )
            }
        }
    }
}

impl std::error::Error for DiagnosticPlatformError {}

// ---------------------------------------------------------------------------
// DiagnosticProvider trait
// ---------------------------------------------------------------------------

/// The unified interface every diagnostic analysis engine implements.
///
/// Providers are registered with the [`DiagnosticPlatform`] under their `origin`
/// string (e.g. `"harper"`). The platform invokes [`analyze`](Self::analyze)
/// when an editor requests diagnostics, without exposing the provider type to
/// the editor.
///
/// ## Thread Safety
///
/// All methods are `&self` (no `&mut self`), so providers are inherently
/// reentrant. Providers may internally use `spawn_blocking` or other isolation
/// to satisfy `Send` requirements of underlying engines (e.g. Harper's `!Send`
/// types). The trait itself is `Send + Sync` because providers may be shared
/// across threads.
///
/// ## Isolation
///
/// The platform never exposes provider types to editors or IPC. Only
/// [`Diagnostic`] values (which are `Send + Sync`) cross the boundary.
pub trait DiagnosticProvider: Send + Sync {
    /// The unique origin/producer name for this provider (e.g. `"harper"`).
    ///
    /// This string is embedded in every `DiagnosticBatch.origin` field and
    /// used by editors to filter or route diagnostics by source.
    fn origin(&self) -> &str;

    /// Run analysis on the given text and return standardized diagnostics.
    ///
    /// The text is the full document content. The provider should return
    /// all diagnostics it produces for this text, converted into Nabu's
    /// canonical [`Diagnostic`] model. An empty `Vec` is valid (no issues found).
    ///
    /// Providers must not expose their internal types in the return value.
    /// Only owned `Diagnostic` values — which are `Send + Sync` — cross
    /// this boundary.
    ///
    /// Returns `DiagnosticError` if the conversion or analysis fails in a
    /// way that should be surfaced to the editor.
    fn analyze(&self, text: &str) -> Result<Vec<Diagnostic>, DiagnosticError>;
}

// ---------------------------------------------------------------------------
// DiagnosticPlatform
// ---------------------------------------------------------------------------

/// The unified diagnostic retrieval platform.
///
/// Aggregates one or more [`DiagnosticProvider`] implementations and exposes a
/// single `retrieve` method that editors (and the `diagnostic_requested` IPC
/// command) use to obtain diagnostics.
///
/// ## Provider Registration
///
/// Providers are registered by their `origin` name. When `retrieve` is called
/// with a specific origin, the platform resolves the provider and delegates
/// the analysis. This means future editors never need backend changes to add
/// a new analysis engine — they just use a different `origin` string.
///
/// ## EventBus Integration
///
/// If an `EventBus` is attached (via [`with_event_bus`](Self::with_event_bus)),
/// every `retrieve` call publishes a `DiagnosticEvent::BatchPublished` event
/// through it. This enables asynchronous subscribers (e.g. background panels,
/// lint lists) to receive diagnostic updates without an explicit IPC request.
///
/// The IPC response is independent of the EventBus — editors get their results
/// directly from the IPC return value, not by subscribing to the bus.
///
/// ## Thread Safety
///
/// Provider storage uses `RwLock<HashMap>`, allowing concurrent reads (multiple
/// editors requesting diagnostics simultaneously) while serializing provider
/// registration. The `EventBus` handle is `Clone` and `Send + Sync`.
pub struct DiagnosticPlatform {
    /// Registered diagnostic providers, keyed by `origin` string.
    providers: RwLock<HashMap<String, Arc<dyn DiagnosticProvider>>>,

    /// Optional EventBus for publishing diagnostic events.
    /// `None` when no event bus is attached (e.g. in standalone tests).
    event_bus: Option<EventBus<PipelineEvent>>,
}

impl DiagnosticPlatform {
    /// Create a new diagnostic platform with no providers and no EventBus.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            event_bus: None,
        }
    }

    /// Create a new platform with an EventBus attached.
    ///
    /// When an EventBus is attached, every `retrieve` call publishes a
    /// `DiagnosticEvent::BatchPublished` event, enabling asynchronous
    /// subscribers to receive diagnostic updates.
    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
        }
    }

    /// Register a diagnostic provider under its `origin` name.
    ///
    /// If a provider with the same origin is already registered, it is replaced.
    /// This is typically called once during application startup for each
    /// analysis engine (e.g. Harper).
    pub fn register_provider<P: DiagnosticProvider + 'static>(&self, provider: Arc<P>) {
        let origin = provider.origin().to_string();
        let mut providers = self.providers.write().expect("providers lock poisoned");
        providers.insert(origin, provider);
    }

    /// Check if a provider is registered for the given origin.
    pub fn has_provider(&self, origin: &str) -> bool {
        let providers = self.providers.read().expect("providers lock poisoned");
        providers.contains_key(origin)
    }

    /// List all registered provider origins.
    pub fn provider_origins(&self) -> Vec<String> {
        let providers = self.providers.read().expect("providers lock poisoned");
        providers.keys().cloned().collect()
    }

    /// Check if the platform has an EventBus attached.
    pub fn has_event_bus(&self) -> bool {
        self.event_bus.is_some()
    }

    /// Attach an EventBus for diagnostic event publication.
    pub fn set_event_bus(&mut self, event_bus: EventBus<PipelineEvent>) {
        self.event_bus = Some(event_bus);
    }

    /// Retrieve diagnostics for a document from the provider matching `origin`.
    ///
    /// This is the canonical entry point for editor diagnostic requests. It:
    ///
    /// 1. Validates the request (non-empty text and resource_id).
    /// 2. Resolves the provider by origin name.
    /// 3. Runs the provider's `analyze` method.
    /// 4. Validates each produced diagnostic.
    /// 5. Publishes a `DiagnosticEvent::BatchPublished` through the EventBus
    ///    (if attached).
    /// 6. Returns the validated `DiagnosticBatch`.
    ///
    /// If `origin` is `None`, the default provider `"harper"` is used.
    ///
    /// # Errors
    ///
    /// Returns `DiagnosticPlatformError` for:
    /// - Invalid input (empty text or resource_id)
    /// - Provider not found for the origin
    /// - Provider analysis failure
    /// - Invalid diagnostic produced by a provider
    pub fn retrieve(
        &self,
        text: &str,
        resource_id: &str,
        origin: Option<&str>,
    ) -> Result<DiagnosticBatch, DiagnosticPlatformError> {
        // 1. Validate input
        if text.is_empty() {
            return Err(DiagnosticPlatformError::InvalidRequest {
                detail: "document text must not be empty".to_string(),
            });
        }
        if resource_id.is_empty() {
            return Err(DiagnosticPlatformError::InvalidRequest {
                detail: "resource_id must not be empty".to_string(),
            });
        }

        // 2. Resolve provider (default to "harper" if origin is None)
        let origin = origin.unwrap_or("harper");
        let providers = self.providers.read().expect("providers lock poisoned");
        let provider = providers.get(origin).ok_or_else(|| {
            DiagnosticPlatformError::ProviderNotFound {
                origin: origin.to_string(),
            }
        })?;

        // 3. Clone the provider Arc to avoid holding the read lock during analysis
        let provider_clone = Arc::clone(provider);
        drop(providers);

        // 4. Run analysis (providers handle their own thread safety internally)
        let diagnostics = provider_clone
            .analyze(text)
            .map_err(|e| DiagnosticPlatformError::ProviderError {
                origin: origin.to_string(),
                detail: e.to_string(),
            })?;

        // 5. Validate each diagnostic
        for (i, diag) in diagnostics.iter().enumerate() {
            if let Err(e) = diag.validate() {
                return Err(DiagnosticPlatformError::InvalidDiagnostic {
                    origin: origin.to_string(),
                    detail: format!("diagnostic at index {}: {}", i, e),
                });
            }
        }

        // 6. Construct the batch
        let batch = DiagnosticBatch::new(origin, resource_id.to_string(), diagnostics);

        // 7. Publish through EventBus (if attached)
        if let Some(bus) = &self.event_bus {
            let event = DiagnosticEvent::BatchPublished(batch.clone());
            publish_diagnostic_event(bus, &event);
        }

        Ok(batch)
    }
}

impl Default for DiagnosticPlatform {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{DiagnosticCategory, DiagnosticSeverity, TextPosition, TextRange};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A test provider that returns configurable diagnostics.
    struct TestProvider {
        origin: String,
        diagnostics: Vec<Diagnostic>,
        call_count: AtomicUsize,
    }

    impl DiagnosticProvider for TestProvider {
        fn origin(&self) -> &str {
            &self.origin
        }

        fn analyze(&self, _text: &str) -> Result<Vec<Diagnostic>, DiagnosticError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.diagnostics.clone())
        }
    }

    impl TestProvider {
        fn new(origin: &str, diagnostics: Vec<Diagnostic>) -> Self {
            Self {
                origin: origin.to_string(),
                diagnostics,
                call_count: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    /// A provider that always fails analysis.
    struct FailingProvider {
        origin: String,
    }

    impl DiagnosticProvider for FailingProvider {
        fn origin(&self) -> &str {
            &self.origin
        }

        fn analyze(&self, _text: &str) -> Result<Vec<Diagnostic>, DiagnosticError> {
            Err(DiagnosticError::EmptyMessage)
        }
    }

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic::try_new(
            DiagnosticSeverity::Warning,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            "sample diagnostic",
        )
        .unwrap()
        .with_code("TEST_001")
        .with_source("test-producer")
        .with_category(DiagnosticCategory::SpellCheck)
    }

    #[test]
    fn platform_new_has_no_providers() {
        let platform = DiagnosticPlatform::new();
        assert!(platform.provider_origins().is_empty());
        assert!(!platform.has_event_bus());
    }

    #[test]
    fn platform_registers_and_finds_provider() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        assert!(platform.has_provider("harper"));
        let origins = platform.provider_origins();
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0], "harper");
    }

    #[test]
    fn platform_retrieve_returns_diagnostics() {
        let platform = DiagnosticPlatform::new();
        let diag = sample_diagnostic();
        let provider = Arc::new(TestProvider::new("harper", vec![diag.clone()]));
        platform.register_provider(provider);

        let batch = platform.retrieve("hello world", "vault:doc.md", None).unwrap();
        assert_eq!(batch.origin, "harper");
        assert_eq!(batch.resource_id, "vault:doc.md");
        assert_eq!(batch.diagnostics.len(), 1);
        assert_eq!(batch.diagnostics[0], diag);
        assert_eq!(batch.diagnostic_count(), 1);
    }

    #[test]
    fn platform_retrieve_with_explicit_origin() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("custom-checker", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let batch = platform.retrieve("text", "vault:doc.md", Some("custom-checker")).unwrap();
        assert_eq!(batch.origin, "custom-checker");
    }

    #[test]
    fn platform_retrieve_defaults_to_harper() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let batch = platform.retrieve("text", "vault:doc.md", None).unwrap();
        assert_eq!(batch.origin, "harper");
    }

    #[test]
    fn platform_retrieve_rejects_empty_text() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let err = platform.retrieve("", "vault:doc.md", None).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::InvalidRequest { .. }));
        assert!(err.to_string().contains("text must not be empty"));
    }

    #[test]
    fn platform_retrieve_rejects_empty_resource_id() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let err = platform.retrieve("text", "", None).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::InvalidRequest { .. }));
        assert!(err.to_string().contains("resource_id must not be empty"));
    }

    #[test]
    fn platform_retrieve_provider_not_found() {
        let platform = DiagnosticPlatform::new();
        let result = platform.retrieve("text", "vault:doc.md", Some("nonexistent"));
        assert!(matches!(result, Err(DiagnosticPlatformError::ProviderNotFound { .. })));
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn platform_retrieve_default_origin_not_found() {
        let platform = DiagnosticPlatform::new();
        let result = platform.retrieve("text", "vault:doc.md", None);
        assert!(matches!(result, Err(DiagnosticPlatformError::ProviderNotFound { .. })));
        assert!(result.unwrap_err().to_string().contains("harper"));
    }

    #[test]
    fn platform_retrieve_propagates_provider_error() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(FailingProvider { origin: "failing".to_string() });
        platform.register_provider(provider);

        let err = platform.retrieve("text", "vault:doc.md", Some("failing")).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::ProviderError { .. }));
        assert!(err.to_string().contains("failing"));
    }

    #[test]
    fn platform_retrieve_rejects_invalid_diagnostic() {
        let platform = DiagnosticPlatform::new();
        // A diagnostic with an empty message is invalid.
        let bad_diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "", // empty message — invalid
        );
        let provider = Arc::new(TestProvider::new("bad", vec![bad_diag]));
        platform.register_provider(provider);

        let err = platform.retrieve("text", "vault:doc.md", Some("bad")).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::InvalidDiagnostic { .. }));
        assert!(err.to_string().contains("bad"));
    }

    #[test]
    fn platform_retrieve_rejects_invalid_range() {
        let platform = DiagnosticPlatform::new();
        let bad_diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::new(TextPosition::new(2, 0), TextPosition::new(1, 0)),
            "bad range",
        );
        let provider = Arc::new(TestProvider::new("bad-range", vec![bad_diag]));
        platform.register_provider(provider);

        let err = platform.retrieve("text", "vault:doc.md", Some("bad-range")).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::InvalidDiagnostic { .. }));
    }

    #[test]
    fn platform_retrieve_validates_diagnostics_in_order() {
        let platform = DiagnosticPlatform::new();
        let good = sample_diagnostic();
        let bad = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "", // empty message — invalid
        );
        let provider = Arc::new(TestProvider::new("mixed", vec![good, bad]));
        platform.register_provider(provider);

        let err = platform.retrieve("text", "vault:doc.md", Some("mixed")).unwrap_err();
        assert!(matches!(err, DiagnosticPlatformError::InvalidDiagnostic { .. }));
        if let DiagnosticPlatformError::InvalidDiagnostic { detail, .. } = err {
            assert!(detail.contains("index 1"));
        }
    }

    #[test]
    fn platform_retrieve_returns_empty_batch_for_empty_results() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![]));
        platform.register_provider(provider);

        let batch = platform.retrieve("text", "vault:doc.md", None).unwrap();
        assert_eq!(batch.diagnostics.len(), 0);
        assert_eq!(batch.diagnostic_count(), 0);
    }

    #[test]
    fn platform_retrieve_publishes_event_on_event_bus() {
        let event_bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        event_bus.subscribe(crate::event_bus::kinds::DIAGNOSTIC_BATCH_PUBLISHED, move |pe: &PipelineEvent| {
            if let PipelineEvent::Diagnostic(e) = pe {
                received_clone.lock().unwrap().push(e.clone());
            }
        });

        let platform = DiagnosticPlatform::with_event_bus(event_bus);
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let batch = platform.retrieve("text", "vault:doc.md", None).unwrap();
        assert_eq!(batch.diagnostics.len(), 1);

        let stored = received.lock().unwrap();
        assert_eq!(stored.len(), 1);
        match &stored[0] {
            crate::diagnostic::DiagnosticEvent::BatchPublished(b) => {
                assert_eq!(b.origin, "harper");
                assert_eq!(b.resource_id, "vault:doc.md");
                assert_eq!(b.diagnostics.len(), 1);
            }
            other => panic!("expected BatchPublished, got {:?}", other),
        }
    }

    #[test]
    fn platform_without_event_bus_does_not_publish() {
        let platform = DiagnosticPlatform::new();
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let batch = platform.retrieve("text", "vault:doc.md", None).unwrap();
        assert_eq!(batch.diagnostics.len(), 1);
    }

    #[test]
    fn platform_overwrite_provider_same_origin() {
        let platform = DiagnosticPlatform::new();
        let provider1 = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider1);

        let provider2 = Arc::new(TestProvider::new("harper", vec![]));
        platform.register_provider(provider2);

        assert_eq!(platform.provider_origins().len(), 1);
    }

    #[test]
    fn platform_concurrent_retrieve_is_safe() {
        let platform = Arc::new(DiagnosticPlatform::new());
        let provider = Arc::new(TestProvider::new("harper", vec![sample_diagnostic()]));
        platform.register_provider(provider);

        let mut handles = Vec::new();
        for i in 0..10 {
            let platform = Arc::clone(&platform);
            handles.push(std::thread::spawn(move || {
                let resource_id = format!("vault:doc_{}.md", i);
                platform.retrieve("text", &resource_id, None).unwrap()
            }));
        }

        for handle in handles {
            let batch = handle.join().unwrap();
            assert_eq!(batch.diagnostics.len(), 1);
        }
    }

    #[test]
    fn platform_error_is_serializable() {
        let err = DiagnosticPlatformError::ProviderNotFound {
            origin: "missing".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let back: DiagnosticPlatformError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn platform_error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticPlatformError>();
        assert_send_sync::<DiagnosticPlatform>();
    }

    #[test]
    fn platform_default_is_empty() {
        let platform: DiagnosticPlatform = Default::default();
        assert!(platform.provider_origins().is_empty());
    }

    #[test]
    fn platform_with_event_bus_attaches_bus() {
        let bus = EventBus::<PipelineEvent>::new();
        let platform = DiagnosticPlatform::with_event_bus(bus);
        assert!(platform.has_event_bus());
    }

    #[test]
    fn platform_set_event_bus() {
        let mut platform = DiagnosticPlatform::new();
        assert!(!platform.has_event_bus());

        let bus = EventBus::<PipelineEvent>::new();
        platform.set_event_bus(bus);
        assert!(platform.has_event_bus());
    }
}
