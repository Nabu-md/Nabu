//! # Unified Diagnostic Platform — Nabu Core
//!
//! Strongly-typed severity classification and a platform-agnostic styling
//! foundation for every diagnostic Nabu produces or consumes (Markdown
//! analyzers, AI assistants, plugins, language servers, metadata validators,
//! OCR processors, spell checkers, grammar engines).
//!
//! ## Architecture
//!
//! ```text
//!  [Producer]                                     [Renderer]
//!  Spell checker / AI / plugin / LSP ...           Editor / Panel / IPC
//!        │                                                │
//!        │  emits: Diagnostic { severity, range, .. }   │
//!        ▼                                                │
//!  diagnostic::*  ──►  DiagnosticSeverity (enum)  ────────►  resolves:
//!                 │                                        │
//!                 │  diagnostic::mapping::diagnostic_style │
//!                 │                                        ▼
//!                 └──►  DiagnosticStyle  (abstract intent)  DiagnosticStyle
//!                          (emphasis, icon, highlight,              │
//!                           category, gutter, priority,            │
//!                           accessibility)                         │
//!                          │                                        │
//!                          └────────────── shared schema ─────────────┘
//!                                        ▼
//!                          Theme/plugin override:
//!                          DiagnosticStyleMap { severity → style }
//! ```
//!
//! ## Core Types
//!
//! | Type                  | Module        | Purpose                              |
//! |-----------------------|---------------|--------------------------------------|
//! | [`DiagnosticSeverity`] | [`severity`]  | Canonical severity enum (5 levels).  |
//! | [`DiagnosticStyle`]    | [`style`]     | Abstract, reusable presentation intent. |
//! | [`DiagnosticStyleMap`] | [`style`]     | Serializable severity→style map; `Default` = canonical. |
//! | `diagnostic_style()`   | [`mapping`]   | Centralized severity→style lookup (single source of truth). |
//! | [`Diagnostic`]         | [`model`]     | Canonical diagnostic data model.     |
//! | [`Decoration`]         | [`model`]     | Extra abstract decoration on a range.|
//! | [`Suggestion`]         | [`model`]     | Quick-fix / code-action.             |
//! | [`TextPosition`]       | [`model`]     | A single 0-based (line, character).  |
//! | [`TextRange`]          | [`model`]     | Inclusive range with optional offsets.|
//! | [`SuggestionApplicability`] | [`model`] | Whether a suggestion can auto-apply.|
//! | [`SuggestionPriority`]      | [`model`] | Display ordering of suggestions.     |
//! | [`DiagnosticCategory`] | [`category`]  | Domain of the diagnostic (spelling, etc.). |
//! | [`DiagnosticError`]    | [`error`]     | Structured construction/validation errors. |
//!
//! ## Severities
//!
//! See [`severity`] for the full definition. Briefly: `Hint` < `Information`
//! < `Warning` < `Error` < `Critical`, with stable `u8` discriminants and
//! kebab-case serde tokens (`"hint"`, `"critical"`, …).
//!
//! ## Styling & Rendering
//!
//! Styles are **abstract intent**, never concrete CSS/HTML/pixel values. The
//! [`mapping`] module is the single source of truth for the canonical mapping;
//! themes and plugins clone [`DiagnosticStyleMap::default`] and override
//! individual entries, then hand the customized map to their renderer.
//!
//! This module introduces **no** UI-framework dependencies (no Dioxus, CSS,
//! Monaco, CodeMirror, Tailwind, HTML, or native rendering APIs). It only
//! *describes* presentation so that renderers can implement it.
//!
//! ## Validation & Error Handling
//!
//! Every model provides a `validate()` method returning `Result<(),
//! [`DiagnosticError`]>` and a `try_new()` constructor that validates eagerly.
//! Unchecked `new()` constructors are also available for cases where the
//! caller has already established validity. All validation is **local** — it
//! checks structural invariants (positions in order, non-empty messages)
//! without needing the source document. Errors are `Serialize + Deserialize`
//! so they can travel through IPC and plugin boundaries.
//!
//! ## Serialization
//!
//! All public types derive [`serde::Serialize`] and [`serde::Deserialize`].
//! Severities use stable kebab-case tokens. New model fields use
//! `#[serde(default)]` + `skip_serializing_if` so existing serialized
//! payloads keep deserializing as the schema evolves.
//!
//! ## Accessibility
//!
//! Every style carries an [`AccessibilityMeta`]
//! with a non-empty screen-reader label, an optional description, and a
//! concrete [`VisualIndicator`] so severity is never
//! conveyed by color alone.
//!
//! ## Thread Safety
//!
//! All types are immutable value types composed of owned/Copy data
//! (enums, short `String`s, `Vec`, `HashMap`). They are `Send + Sync` and
//! contain no shared mutable styling state.
//!
//! ## Stability & Extension
//!
//! - [`DiagnosticSeverity`] is `#[non_exhaustive]`: adding a severity is a
//!   compile error in [`mapping`] until a style is assigned.
//! - [`DiagnosticStyle`] uses a stable schema; new fields must be
//!   `Option` + `#[serde(default)]` for forward compatibility.
//! - `DiagnosticStyleMap` is the extension point for themes, plugins, and
//!   user-defined severity→style mappings — without touching the canonical
//!   [`mapping`] module.

pub mod category;
pub mod error;
pub mod events;
pub mod mapping;
pub mod model;
pub mod severity;
pub mod style;

pub use category::DiagnosticCategory;
pub use error::DiagnosticError;
pub use events::{
    BatchClearedEvent, BatchRemovedEvent, DiagnosticBatch, DiagnosticEvent,
    DiagnosticEventContract, DiagnosticEventError, publish_diagnostic_event,
};
pub use mapping::{default_severity_styles, diagnostic_style};
pub use model::{
    Decoration, Diagnostic, Suggestion, SuggestionApplicability, SuggestionPriority, TextPosition,
    TextRange,
};
pub use severity::DiagnosticSeverity;
pub use style::{
    AccessibilityMeta, DecorationCategory, DiagnosticIcon, DiagnosticStyle, DiagnosticStyleMap,
    GutterIndicator, HighlightStyle, Priority, VisualEmphasis, VisualIndicator,
};
