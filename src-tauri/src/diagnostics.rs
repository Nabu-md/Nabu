//! # Editor Diagnostic Bridge
//!
//! Connects the Tauri backend's [`DiagnosticBatch`] delivery to the editor
//! frontend via a structured, editor-agnostic representation.
//!
//! ## Request/Response Flow
//!
//! ```text
//!  Editor
//!      │  1. diagnostic_requested(text, resource_id, origin?)
//!      ▼
//!  IPC Handler → DiagnosticPlatform::retrieve(text, resource_id, origin)
//!      │  2. Provider (e.g. Harper) runs analysis → Vec<Diagnostic>
//!      │  3. DiagnosticPlatform validates + publishes
//!      │     DiagnosticEvent::BatchPublished via EventBus
//!      │  4. DiagnosticBatch + DiagnosticStyleMap returned
//!      ▼
//!  DiagnosticResponse { batch, style_map }
//!      │
//!      ▼
//!  EditorDiagnosticBridge::resolve_response(&response)
//!      │  5. Each Diagnostic paired with its resolved DiagnosticStyle
//!      ▼
//!  ResolvedBatch { resource_id, origin, batch_id, diagnostics: [EditorDiagnostic] }
//!      │
//!      ▼
//!  Editor renders: squiggles, gutters, tooltips, quick-fixes
//! ```
//!
//! ## EventBus Compatibility
//!
//! The platform publishes `DiagnosticEvent::BatchPublished` events through the
//! EventBus on every retrieval. These events are forwarded to the frontend via
//! the `nabu-event` channel (see `event_bridge.rs`), enabling async subscribers
//! (background panels, lint lists) to receive diagnostic updates without an
//! explicit IPC request.
//!
//! - **IPC** is used for explicit, on-demand requests (e.g. editor opens a
//!   document).
//! - **EventBus** is used for asynchronous, pub/sub delivery (e.g. live
//!   background analysis, plugin diagnostics).
//!
//! These channels do not duplicate each other — the IPC response is the
//! authoritative result for the requesting editor; the EventBus event is a
//! side-channel notification for subscribers that don't participate in the IPC
//! round-trip.
//!
//! ## Provider Independence
//!
//! The bridge is completely independent of any analysis engine. The
//! `DiagnosticPlatform` resolves providers by `origin` string (e.g. `"harper"`,
//! `"ai-assistant"`, `"lsp-markdown"`). Future engines register a new
//! [`DiagnosticProvider`](nabu_core::diagnostic::DiagnosticProvider) — no
//! backend or editor changes required.
//!
//! ## Architecture
//!
//! ```text
//!   Editor IPC
//!       │  (invoke)
//!       ▼
//!   diagnostic_requested  ──▶  DiagnosticBatch + DiagnosticStyleMap
//!       │  (Response)
//!       ▼
//!   EditorDiagnosticBridge
//!       │  (resolve each Diagnostic → EditorDiagnostic)
//!       ▼
//!   Vec<EditorDiagnostic>
//!       │  (render via resolved DiagnosticStyle)
//!       ▼
//!   Editor decorations / gutters / panels
//! ```
//!
//! The bridge is **editor-agnostic** — it resolves abstract presentation intent
//! (`DiagnosticStyle`) into a concrete `EditorDiagnostic` value that any editor
//! can translate into its native decoration/gutter/squiggle APIs. It holds no
//! editor-specific state and is `Send + Sync`.
//!
//! ## Responsibilities
//!
//! - Wrap the IPC response into a typed bridge structure.
//! - Resolve each `Diagnostic`'s `DiagnosticStyle` via the provided
//!   `DiagnosticStyleMap` (falling back to the canonical default mapping).
//! - Provide an editor-agnostic `EditorDiagnostic` representation with all
//!   fields needed for rendering: severity, style, range, message, suggestions,
//!   and decorations.
//! - Keep the bridge pure and side-effect-free — no I/O, no EventBus
//!   interaction, no Tauri handle access.
//!
//! ## Integration Workflow
//!
//! Future editor implementations (Dioxus, Monaco, CodeMirror, mobile) should:
//!
//! 1. Call the `diagnostic_requested` Tauri command with a
//!    `DiagnosticRequest` containing `text`, `resource_id`, and optional
//!    `origin`.
//! 2. Receive the `DiagnosticResponse` containing a `DiagnosticBatch` and
//!    `DiagnosticStyleMap`.
//! 3. Pass the response to `EditorDiagnosticBridge::resolve_response` to
//!    obtain a `ResolvedBatch` with each diagnostic paired to its resolved style.
//! 4. Translate each `EditorDiagnostic` into the editor's native rendering
//!    API (squiggles, gutters, tooltips, quick-fixes).

use nabu_core::diagnostic::{
    Diagnostic, DiagnosticBatch, DiagnosticStyleMap, DiagnosticStyle,
};
use serde::{Deserialize, Serialize};

/// An editor-agnostic diagnostic ready for rendering.
///
/// This is the bridge's primary output: a fully resolved diagnostic that carries
/// both the canonical diagnostic data and its resolved presentation style, so
/// the editor needs no second lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDiagnostic {
    /// The underlying diagnostic data (range, message, severity, suggestions,
    /// etc.).
    #[serde(flatten)]
    pub diagnostic: Diagnostic,

    /// The resolved presentation style for this diagnostic's severity.
    /// Computed by looking up the diagnostic's severity in the provided
    /// `DiagnosticStyleMap` (or the canonical default).
    pub style: DiagnosticStyle,
}

/// A resolved diagnostic batch: every `Diagnostic` in the batch is paired with
/// its resolved `DiagnosticStyle`.
///
/// Constructed via [`EditorDiagnosticBridge::resolve_batch`], which pairs each
/// diagnostic with its style from the batch's style map or a provided
/// `DiagnosticStyleMap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedBatch {
    /// The resource identifier the diagnostics pertain to.
    pub resource_id: String,
    /// The origin (producer) of the diagnostics.
    pub origin: String,
    /// The unique batch ID for correlation.
    pub batch_id: uuid::Uuid,
    /// Fully-resolved diagnostics, each with its style attached.
    pub diagnostics: Vec<EditorDiagnostic>,
}

/// The editor diagnostic bridge.
///
/// Wraps the diagnostic IPC response into editor-agnostic, style-resolved
/// structures. This type is stateless — it only transforms data — so a single
/// instance (or even a unit struct) can be shared across editor instances.
///
/// ## Thread Safety
///
/// `EditorDiagnosticBridge` contains no interior mutability and no references.
/// It is `Send + Sync` and can be freely shared across threads.
#[derive(Debug, Default, Clone, Copy)]
pub struct EditorDiagnosticBridge;

impl EditorDiagnosticBridge {
    /// Create a new bridge instance.
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    /// Resolve a single [`Diagnostic`] into an [`EditorDiagnostic`], attaching
    /// the canonical (or provided) style for the diagnostic's severity.
    ///
    /// If `style_map` is `None`, the canonical default `DiagnosticStyleMap` is
    /// used (via [`DiagnosticStyleMap::default`]).
    pub fn resolve_diagnostic(
        &self,
        diagnostic: Diagnostic,
        style_map: Option<&DiagnosticStyleMap>,
    ) -> EditorDiagnostic {
        let style = Self::resolve_style(&diagnostic, style_map);
        EditorDiagnostic {
            diagnostic,
            style,
        }
    }

    /// Resolve a full [`DiagnosticBatch`] into a [`ResolvedBatch`], pairing
    /// every diagnostic with its style.
    ///
    /// If `style_map` is `None`, the canonical default `DiagnosticStyleMap` is
    /// used.
    pub fn resolve_batch(
        &self,
        batch: &DiagnosticBatch,
        style_map: Option<&DiagnosticStyleMap>,
    ) -> ResolvedBatch {
        let diagnostics = batch
            .diagnostics
            .iter()
            .cloned()
            .map(|d| self.resolve_diagnostic(d, style_map))
            .collect();

        ResolvedBatch {
            resource_id: batch.resource_id.clone(),
            origin: batch.origin.clone(),
            batch_id: batch.batch_id,
            diagnostics,
        }
    }

    /// Resolve a `DiagnosticResponse` (as returned by the `diagnostic_requested`
    /// IPC command) into a [`ResolvedBatch`].
    ///
    /// This is the primary entry point for editors: after calling the IPC
    /// command and receiving a `DiagnosticResponse`, editors pass it here to
    /// get a fully style-resolved batch ready for rendering.
    ///
    /// The response's embedded `style_map` is used for style resolution,
    /// falling back to the canonical default map if the response's style map
    /// is missing a severity.
    pub fn resolve_response(
        &self,
        response: &crate::commands::DiagnosticResponse,
    ) -> ResolvedBatch {
        self.resolve_batch(&response.batch, Some(&response.style_map))
    }

    /// Resolve the style for a diagnostic using the provided style map, or the
    /// canonical default if none is provided.
    fn resolve_style(
        diagnostic: &Diagnostic,
        style_map: Option<&DiagnosticStyleMap>,
    ) -> DiagnosticStyle {
        match style_map {
            Some(map) => map.style(diagnostic.severity),
            None => diagnostic.style(),
        }
    }

    /// Filter resolved diagnostics by severity level (inclusive range).
    ///
    /// Returns only diagnostics whose severity level falls within
    /// `[min_level, max_level]`. Severity levels are: Hint=0, Information=1,
    /// Warning=2, Error=3, Critical=4.
    pub fn filter_by_severity<'a>(
        &self,
        resolved: &'a ResolvedBatch,
        min_level: u8,
        max_level: u8,
    ) -> Vec<&'a EditorDiagnostic> {
        resolved
            .diagnostics
            .iter()
            .filter(|d| {
                let level = d.diagnostic.severity.level();
                level >= min_level && level <= max_level
            })
            .collect()
    }

    /// Summarize a resolved batch: count diagnostics per severity level.
    ///
    /// Returns a fixed-size array indexed by severity level (0=hint through
    /// 4=critical), matching `DiagnosticSeverity::ALL` ordering.
    pub fn summarize(&self, resolved: &ResolvedBatch) -> [usize; 5] {
        let mut counts = [0usize; 5];
        for diag in &resolved.diagnostics {
            let level = diag.diagnostic.severity.level() as usize;
            if level < counts.len() {
                counts[level] += 1;
            }
        }
        counts
    }
}

// ---------------------------------------------------------------------------
// Convenience conversions
// ---------------------------------------------------------------------------

/// Convert a `ResolvedBatch` into a flat list of `(Diagnostic, &DiagnosticStyle)`
/// pairs — useful for editors that render diagnostics and their styles in a
/// single pass.
impl From<ResolvedBatch> for Vec<(Diagnostic, DiagnosticStyle)> {
    fn from(resolved: ResolvedBatch) -> Self {
        resolved
            .diagnostics
            .into_iter()
            .map(|ed| (ed.diagnostic, ed.style))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nabu_core::diagnostic::{
        DiagnosticCategory, DiagnosticSeverity, TextPosition, TextRange,
    };

    fn sample_batch() -> DiagnosticBatch {
        let diag = Diagnostic::try_new(
            DiagnosticSeverity::Warning,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            "typo detected",
        )
        .unwrap()
        .with_code("TYP001")
        .with_source("harper")
        .with_category(DiagnosticCategory::SpellCheck);

        DiagnosticBatch::new("harper", "vault:notes/sample.md", vec![diag])
    }

    #[test]
    fn bridge_resolves_single_diagnostic() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();

        let resolved = bridge.resolve_batch(&batch, None);
        assert_eq!(resolved.diagnostics.len(), 1);

        let ed = &resolved.diagnostics[0];
        assert_eq!(ed.diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(ed.diagnostic.message, "typo detected");
        // Style should be resolved from the canonical default map.
        assert_eq!(
            ed.style,
            nabu_core::diagnostic::mapping::diagnostic_style(DiagnosticSeverity::Warning)
        );
    }

    #[test]
    fn bridge_resolves_with_custom_style_map() {
        let batch = sample_batch();
        let mut custom_map = DiagnosticStyleMap::default();
        let custom_style = nabu_core::diagnostic::mapping::diagnostic_style(
            DiagnosticSeverity::Warning,
        );
        custom_map.insert(DiagnosticSeverity::Warning, custom_style.clone());

        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, Some(&custom_map));
        assert_eq!(resolved.diagnostics[0].style, custom_style);
    }

    #[test]
    fn bridge_summarize_counts_by_severity() {
        let mut diags = Vec::new();
        for (sev, msg) in [
            (DiagnosticSeverity::Hint, "hint"),
            (DiagnosticSeverity::Information, "info"),
            (DiagnosticSeverity::Warning, "warn"),
            (DiagnosticSeverity::Error, "err"),
            (DiagnosticSeverity::Critical, "crit"),
        ] {
            diags.push(
                Diagnostic::try_new(
                    sev,
                    TextRange::empty(TextPosition::new(0, 0)),
                    msg,
                )
                .unwrap(),
            );
        }
        let batch = DiagnosticBatch::new("test", "vault:doc.md", diags);
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);
        let summary = bridge.summarize(&resolved);

        assert_eq!(summary, [1, 1, 1, 1, 1]);
    }

    #[test]
    fn bridge_filter_by_severity() {
        let mut diags = Vec::new();
        for (sev, msg) in [
            (DiagnosticSeverity::Hint, "h"),
            (DiagnosticSeverity::Warning, "w"),
            (DiagnosticSeverity::Error, "e"),
            (DiagnosticSeverity::Critical, "c"),
        ] {
            diags.push(
                Diagnostic::try_new(
                    sev,
                    TextRange::empty(TextPosition::new(0, 0)),
                    msg,
                )
                .unwrap(),
            );
        }
        let batch = DiagnosticBatch::new("test", "vault:doc.md", diags);
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);

        // Filter: only Warning (level 2) and Error (level 3).
        let filtered = bridge.filter_by_severity(&resolved, 2, 3);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(filtered[1].diagnostic.severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn resolved_batch_serializes() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);

        let json = serde_json::to_string(&resolved).expect("serialize ResolvedBatch");
        let back: ResolvedBatch = serde_json::from_str(&json).expect("deserialize ResolvedBatch");
        assert_eq!(resolved, back);
    }

    #[test]
    fn editor_diagnostic_serializes_with_flattened_diagnostic() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);

        let json = serde_json::to_string(&resolved.diagnostics[0]).unwrap();
        // The flattened Diagnostic fields should be at the top level.
        assert!(json.contains("\"severity\""));
        assert!(json.contains("\"warning\""));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"typo detected\""));
        // The style should be nested under "style".
        assert!(json.contains("\"style\""));
    }

    #[test]
    fn bridge_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EditorDiagnosticBridge>();
        assert_send_sync::<EditorDiagnostic>();
        assert_send_sync::<ResolvedBatch>();
    }

    #[test]
    fn resolved_batch_preserves_batch_metadata() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);

        assert_eq!(resolved.resource_id, "vault:notes/sample.md");
        assert_eq!(resolved.origin, "harper");
        assert_eq!(resolved.batch_id, batch.batch_id);
    }

    #[test]
    fn resolved_batch_to_vec_conversion() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);

        let vec: Vec<(Diagnostic, DiagnosticStyle)> = resolved.into();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0].0.severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn empty_batch_resolves_to_empty() {
        let batch = DiagnosticBatch::new("test", "vault:doc.md", vec![]);
        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_batch(&batch, None);
        assert!(resolved.diagnostics.is_empty());
        assert_eq!(bridge.summarize(&resolved), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn bridge_resolves_response_with_style_map() {
        let batch = sample_batch();
        let response = crate::commands::DiagnosticResponse {
            batch,
            style_map: DiagnosticStyleMap::default(),
        };

        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_response(&response);

        assert_eq!(resolved.diagnostics.len(), 1);
        assert_eq!(resolved.origin, "harper");
        assert_eq!(resolved.resource_id, "vault:notes/sample.md");

        let ed = &resolved.diagnostics[0];
        assert_eq!(ed.diagnostic.severity, DiagnosticSeverity::Warning);
        // Style resolved from the response's style map.
        assert_eq!(
            ed.style,
            nabu_core::diagnostic::mapping::diagnostic_style(DiagnosticSeverity::Warning)
        );
    }

    #[test]
    fn bridge_resolves_response_with_empty_diagnostics() {
        let batch = DiagnosticBatch::new("harper", "vault:empty.md", vec![]);
        let response = crate::commands::DiagnosticResponse {
            batch,
            style_map: DiagnosticStyleMap::default(),
        };

        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_response(&response);

        assert!(resolved.diagnostics.is_empty());
        assert_eq!(resolved.origin, "harper");
        assert_eq!(resolved.resource_id, "vault:empty.md");
    }

    #[test]
    fn bridge_resolves_response_preserves_batch_id() {
        let batch = sample_batch();
        let batch_id = batch.batch_id;
        let response = crate::commands::DiagnosticResponse {
            batch,
            style_map: DiagnosticStyleMap::default(),
        };

        let bridge = EditorDiagnosticBridge::new();
        let resolved = bridge.resolve_response(&response);

        assert_eq!(resolved.batch_id, batch_id);
    }

    #[test]
    fn bridge_full_workflow_from_batch_to_render_ready() {
        let batch = sample_batch();
        let bridge = EditorDiagnosticBridge::new();

        // Step 1: Resolve with default style map
        let resolved = bridge.resolve_batch(&batch, None);
        assert_eq!(resolved.diagnostics.len(), 1);

        // Step 2: Filter by severity (only errors and above: level 3-4)
        let filtered = bridge.filter_by_severity(&resolved, 3, 4);
        assert_eq!(filtered.len(), 0); // Our sample is a Warning (level 2)

        // Step 3: Summarize
        let summary = bridge.summarize(&resolved);
        assert_eq!(summary, [0, 0, 1, 0, 0]); // One warning

        // Step 4: Get render-ready pairs
        let pairs: Vec<(Diagnostic, DiagnosticStyle)> = resolved.into();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0.severity, DiagnosticSeverity::Warning);
        assert_eq!(pairs[0].0.message, "typo detected");
    }
}
