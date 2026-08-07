//! # Diagnostic Domain Models
//!
//! The canonical domain types for text-based diagnostics produced by Markdown
//! analyzers, AI assistants, language servers, spell checkers, grammar
//! engines, OCR processors, plugins, and metadata validators.
//!
//! ## Types
//!
//! | Type            | Role                                                     |
//! |-----------------|----------------------------------------------------------|
//! | [`TextPosition`] | A single 0-based (line, character) location.            |
//! | [`TextRange`]    | An inclusive start-to-end range with optional byte offsets. |
//! | [`Suggestion`]   | A quick-fix / code-action tied to a range.               |
//! | [`Decoration`]   | An additional abstract decoration on a range.            |
//! | [`Diagnostic`]   | The central type: severity + range + message + extras. |
//!
//! ## Severity
//!
//! Every [`Diagnostic`] carries exactly one [`DiagnosticSeverity`] — the
//! canonical classification system defined in [`crate::diagnostic::severity`].
//! Producers never use raw strings for severity: they emit the enum, which
//! serializes to a stable kebab-case token (e.g. `"error"`, `"critical"`).
//!
//! To derive a diagnostic's *appearance*, look it up through the canonical
//! mapping in [`crate::diagnostic::mapping`] (or a themed
//! [`DiagnosticStyleMap`](crate::diagnostic::style::DiagnosticStyleMap)).
//! These models describe *data*, not rendering.
//!
//! ## Positions & Offsets
//!
//! Line and character offsets are 0-based; the character offset counts
//! UTF-16 code units (matching the Language Server Protocol and most editor
//! conventions). Producers that work in byte offsets convert at the document
//! boundary before constructing these types — but [`TextRange`] *also*
//! carries optional UTF-8 byte offsets (`start_offset`, `end_offset`) so that
//! producers that operate directly in byte space can communicate them without
//! ambiguity. Byte offsets are optional: producers that never need them simply
//! leave them `None`.
//!
//! ## Validation
//!
//! Every model provides a `validate()` method returning
//! `Result<(), [`DiagnosticError`]>` and a `try_new()` constructor that
//! validates eagerly. Unchecked constructors (`new`) are also available for
//! cases where the caller has already established validity — they never
//! panic. All validation is **local** — it checks structural invariants
//! (positions in order, non-empty messages) without needing the source
//! document.
//!
//! ## Serialization
//!
//! All models derive [`serde::Serialize`] and [`serde::Deserialize`].
//! Optional fields use `skip_serializing_if` to keep payloads compact, and
//! new fields use `#[serde(default)]` so that older serialized payloads keep
//! deserializing as the schema evolves.

use serde::{Deserialize, Serialize};

use super::category::DiagnosticCategory;
use super::error::DiagnosticError;
use super::severity::DiagnosticSeverity;
use super::style::{DecorationCategory, DiagnosticStyle};

// ---------------------------------------------------------------------------
// Text positions and ranges
// ---------------------------------------------------------------------------

/// A single position within a text document.
///
/// Both fields are 0-based. `character` counts UTF-16 code units (the LSP
/// convention) so frontends can share a unified position model; producers
/// that work in byte offsets convert at the document boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextPosition {
    /// 0-based line number.
    pub line: u32,
    /// 0-based UTF-16 code-unit offset within the line.
    pub character: u32,
}

impl TextPosition {
    /// Create a position from line/character.
    #[inline]
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

impl std::fmt::Display for TextPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.character)
    }
}

/// An inclusive range of text between a `start` and `end` position.
///
/// When `start == end` the range is *empty*: it still identifies a point
/// (useful for zero-width markers such as a cursor-position hint or a
/// decoration that anchors insertion).
///
/// In addition to the position pair, the range optionally carries UTF-8 byte
/// offsets (`start_offset`, `end_offset`). These are `None` when the producer
/// operates purely in line/character space; when present they must be
/// monotonically ordered (`start_offset <= end_offset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRange {
    /// Inclusive start of the range.
    pub start: TextPosition,
    /// Inclusive end of the range.
    pub end: TextPosition,
    /// Optional 0-based UTF-8 byte offset of the range start within the
    /// document. `None` when the producer works in position space only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Optional 0-based UTF-8 byte offset of the range end within the
    /// document. `None` when the producer works in position space only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
}

impl TextRange {
    /// Create a range spanning from `start` to `end` (both inclusive),
    /// without byte offsets.
    ///
    /// This constructor does **not** validate that `start <= end`. Use
    /// [`try_new`](Self::try_new) when the ordering is not known to hold.
    #[inline]
    pub const fn new(start: TextPosition, end: TextPosition) -> Self {
        Self {
            start,
            end,
            start_offset: None,
            end_offset: None,
        }
    }

    /// An empty range anchored at `position` (start == end).
    #[inline]
    pub const fn empty(position: TextPosition) -> Self {
        Self {
            start: position,
            end: position,
            start_offset: None,
            end_offset: None,
        }
    }

    /// Create a range, validating that `start <= end`.
    ///
    /// Returns [`DiagnosticError::InvalidRange`] when the start position is
    /// lexicographically after the end position.
    #[inline]
    pub fn try_new(start: TextPosition, end: TextPosition) -> Result<Self, DiagnosticError> {
        if cmp_position(start, end) == std::cmp::Ordering::Greater {
            Err(DiagnosticError::InvalidRange { start, end })
        } else {
            Ok(Self::new(start, end))
        }
    }

    /// Create a range with both position and byte-offset bounds, validating
    /// all invariants:
    ///
    /// - `start <= end` (lexicographically by `(line, character)`)
    /// - `start_offset <= end_offset`
    #[inline]
    pub fn try_with_offsets(
        start: TextPosition,
        end: TextPosition,
        start_offset: usize,
        end_offset: usize,
    ) -> Result<Self, DiagnosticError> {
        if cmp_position(start, end) == std::cmp::Ordering::Greater {
            return Err(DiagnosticError::InvalidRange { start, end });
        }
        if start_offset > end_offset {
            return Err(DiagnosticError::InvalidOffset {
                start: start_offset,
                end: end_offset,
            });
        }
        Ok(Self {
            start,
            end,
            start_offset: Some(start_offset),
            end_offset: Some(end_offset),
        })
    }

    /// Fluent builder: attach byte offsets to an existing range without
    /// validation. Use [`validate`](Self::validate) to check invariants.
    #[inline]
    pub fn with_byte_offsets(mut self, start: usize, end: usize) -> Self {
        self.start_offset = Some(start);
        self.end_offset = Some(end);
        self
    }

    /// `true` when the range encloses no characters (start == end).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// `true` when `position` falls within this range (inclusive of both
    /// endpoints). Containment across lines is determined lexicographically
    /// by `(line, character)`.
    #[inline]
    pub fn contains(&self, position: TextPosition) -> bool {
        cmp_position(self.start, position) <= std::cmp::Ordering::Equal
            && cmp_position(position, self.end) <= std::cmp::Ordering::Equal
    }

    /// The start byte offset, if present.
    #[inline]
    pub fn start_byte_offset(&self) -> Option<usize> {
        self.start_offset
    }

    /// The end byte offset, if present.
    #[inline]
    pub fn end_byte_offset(&self) -> Option<usize> {
        self.end_offset
    }

    /// Validate structural invariants of this range.
    ///
    /// Checks:
    /// - `start <= end` (lexicographic ordering of positions)
    /// - If byte offsets are present, `start_offset <= end_offset`
    ///
    /// This is a purely local check — it does not require access to the
    /// source document and therefore cannot verify that offsets actually
    /// correspond to the correct positions.
    #[inline]
    pub fn validate(&self) -> Result<(), DiagnosticError> {
        if cmp_position(self.start, self.end) == std::cmp::Ordering::Greater {
            return Err(DiagnosticError::InvalidRange {
                start: self.start,
                end: self.end,
            });
        }
        if let (Some(s), Some(e)) = (self.start_offset, self.end_offset) {
            if s > e {
                return Err(DiagnosticError::InvalidOffset {
                    start: s,
                    end: e,
                });
            }
        }
        Ok(())
    }
}

/// Lexicographic comparison of two positions by `(line, character)`.
#[inline]
fn cmp_position(a: TextPosition, b: TextPosition) -> std::cmp::Ordering {
    match a.line.cmp(&b.line) {
        std::cmp::Ordering::Equal => a.character.cmp(&b.character),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Suggestion applicability & priority
// ---------------------------------------------------------------------------

/// Applicability of a [`Suggestion`] — whether it can be safely and
/// automatically applied, or whether it requires user intervention.
///
/// This is the suggestion-side analog of "confidence": it tells a renderer
/// whether to offer the suggestion as an auto-fix, a user-requested quick-fix,
/// or whether it should be withheld entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum SuggestionApplicability {
    /// The suggestion is always safe and applicable.
    /// Renderers may offer it as an auto-fix without user confirmation.
    #[serde(rename = "always")]
    Always,
    /// The suggestion is applicable, but only when explicitly requested
    /// (e.g. via a "quick fix" / lightbulb action). Not auto-applied.
    #[serde(rename = "on-request")]
    #[default]
    OnRequest,
    /// The suggestion is applicable but may change semantics in ways the
    /// producer cannot fully guarantee. Requires manual review/confirmation.
    #[serde(rename = "manual")]
    Manual,
    /// The suggestion is not currently applicable in this context.
    /// Renderers should not offer it.
    #[serde(rename = "not-applicable")]
    NotApplicable,
}

impl SuggestionApplicability {
    /// Short, kebab-case machine identifier matching the serde token.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::OnRequest => "on-request",
            Self::Manual => "manual",
            Self::NotApplicable => "not-applicable",
        }
    }

    /// Human-readable label suitable for UI.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Always => "Always applicable",
            Self::OnRequest => "On request",
            Self::Manual => "Requires review",
            Self::NotApplicable => "Not applicable",
        }
    }
}

impl std::fmt::Display for SuggestionApplicability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Priority ranking for a [`Suggestion`], influencing display ordering
/// and prominence in quick-fix UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum SuggestionPriority {
    /// Low priority — shown after other suggestions.
    #[serde(rename = "low")]
    Low,
    /// Normal priority — the default and most common.
    #[serde(rename = "normal")]
    #[default]
    Normal,
    /// High priority — surfaced before other suggestions.
    #[serde(rename = "high")]
    High,
}

impl SuggestionPriority {
    /// Short, kebab-case machine identifier matching the serde token.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

impl std::fmt::Display for SuggestionPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// Suggestion
// ---------------------------------------------------------------------------

/// A suggestion (quick-fix / code-action) associated with a [`Diagnostic`].
///
/// A producer may attach zero or more suggestions to a diagnostic; each one
/// describes a concrete, reversible edit (title + range + replacement text).
/// Renderers surface these to users via "lightbulb" / quick-fix UI.
///
/// Suggestions describe *possible actions* — they never auto-apply. The
/// [`applicability`](SuggestionApplicability) field tells the renderer whether
/// auto-application is safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// Human-readable title shown to the user (e.g. "Replace with 'because'").
    pub title: String,
    /// The text range the suggestion applies to.
    pub range: TextRange,
    /// The replacement text to write over `range`.
    pub new_text: String,
    /// Optional machine-readable kind (e.g. `"quickfix"`, `"refactor"`,
    /// `"spell"`). When `None`, the renderer treats this as a generic fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Whether the suggestion can be safely and automatically applied.
    /// Defaults to [`SuggestionApplicability::OnRequest`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applicability: Option<SuggestionApplicability>,

    /// Priority ranking influencing display order. Defaults to
    /// [`SuggestionPriority::Normal`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<SuggestionPriority>,
}

impl Suggestion {
    /// Create a suggestion with no explicit `kind`, defaulting
    /// `applicability` and `priority` to their defaults.
    #[inline]
    pub fn simple(
        title: impl Into<String>,
        range: TextRange,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            range,
            new_text: new_text.into(),
            kind: None,
            applicability: None,
            priority: None,
        }
    }

    /// Create a suggestion, validating that the title is non-empty and the
    /// range is well-formed.
    #[inline]
    pub fn try_new(
        title: impl Into<String>,
        range: TextRange,
        new_text: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let title_val = title.into();
        if title_val.is_empty() {
            return Err(DiagnosticError::EmptySuggestionTitle);
        }
        range.validate()?;
        Ok(Self {
            title: title_val,
            range,
            new_text: new_text.into(),
            kind: None,
            applicability: None,
            priority: None,
        })
    }

    /// Builder: attach a machine-readable kind.
    #[inline]
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// Builder: set the applicability.
    #[inline]
    pub fn with_applicability(mut self, applicability: SuggestionApplicability) -> Self {
        self.applicability = Some(applicability);
        self
    }

    /// Builder: set the priority.
    #[inline]
    pub fn with_priority(mut self, priority: SuggestionPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Validate structural invariants of this suggestion.
    #[inline]
    pub fn validate(&self) -> Result<(), DiagnosticError> {
        if self.title.is_empty() {
            return Err(DiagnosticError::EmptySuggestionTitle);
        }
        self.range.validate()
    }

    /// Returns the applicability, or `OnRequest` if not explicitly set.
    #[inline]
    pub fn effective_applicability(&self) -> SuggestionApplicability {
        self.applicability.unwrap_or(SuggestionApplicability::OnRequest)
    }

    /// Returns the priority, or `Normal` if not explicitly set.
    #[inline]
    pub fn effective_priority(&self) -> SuggestionPriority {
        self.priority.unwrap_or(SuggestionPriority::Normal)
    }
}

// ---------------------------------------------------------------------------
// Decoration
// ---------------------------------------------------------------------------

/// An additional, abstract decoration tied to a [`Diagnostic`].
///
/// `Decoration` describes *extra* presentation beyond what the severity's
/// [`DiagnosticStyle`] already conveys (e.g. a bracket marker or a background
/// tint on a secondary range). It is intentionally separate from the severity
/// style so the canonical severity mapping stays untouched.
///
/// A `Decoration` with `style: None` inherits the style resolved for its
/// diagnostic's severity via [`crate::diagnostic::mapping::diagnostic_style`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decoration {
    /// Where, structurally, the decoration renders.
    pub category: DecorationCategory,
    /// The text range to decorate.
    pub range: TextRange,
    /// Optional explicit style override. `None` means "use the severity style".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<DiagnosticStyle>,
    /// Optional tooltip text shown when the user hovers over the decorated
    /// range. When `None`, the renderer may fall back to the diagnostic's
    /// message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

impl Decoration {
    /// Builder: attach a tooltip.
    #[inline]
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Builder: set an explicit style override.
    #[inline]
    pub fn with_style(mut self, style: DiagnosticStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Validate structural invariants (range ordering).
    #[inline]
    pub fn validate(&self) -> Result<(), DiagnosticError> {
        self.range.validate()
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single diagnostic produced by an analyzer (spell checker, grammar
/// engine, AI assistant, language server, OCR processor, plugin, etc.).
///
/// This is the **canonical** diagnostic type: no duplicate domain models
/// exist. Every producer emits a `Diagnostic`; every consumer (editor,
/// panel, IPC, persistence) reads one.
///
/// ## Severity
///
/// `severity` is a [`DiagnosticSeverity`] enum, never a raw string. The enum
/// is the single source of truth for classification; appearance is derived
/// via [`crate::diagnostic::mapping::diagnostic_style`].
///
/// ## Category
///
/// `category` is an optional [`DiagnosticCategory`] that classifies *what
/// kind* of diagnostic this is (spelling, grammar, metadata, etc.), enabling
/// filtering and routing by domain.
///
/// ## Validation
///
/// Use [`new`](Self::new) when you know the inputs are valid (zero-cost
/// construction). Use [`try_new`](Self::try_new) or [`validate`](Self::validate)
/// when input comes from an untrusted source (plugins, IPC, deserialization).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// The severity of this diagnostic, as a strongly-typed enum.
    pub severity: DiagnosticSeverity,

    /// The text range this diagnostic applies to.
    pub range: TextRange,

    /// Human-readable message describing the problem or suggestion.
    pub message: String,

    /// Optional stable machine-readable code (e.g. `"E0123"`, `"SPELL"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Optional source identifier (e.g. `"harper"`, `"spelling"`,
    /// `"ai-assistant"`). Lets consumers attribute and filter diagnostics by
    /// producer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Optional category classifying the *domain* of this diagnostic
    /// (spelling, grammar, metadata, etc.). Lets consumers filter and route
    /// by kind independently of severity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<DiagnosticCategory>,

    /// Optional quick-fix suggestions (code actions) attached to this
    /// diagnostic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<Suggestion>,

    /// Optional additional decorations beyond the severity style.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decorations: Vec<Decoration>,
}

impl Diagnostic {
    /// Create a diagnostic with the given severity, range, and message.
    ///
    /// This constructor does **not** validate — use [`try_new`](Self::try_new)
    /// when inputs come from untrusted sources. The `category` field is
    /// left `None`; use [`with_category`](Self::with_category) to attach one.
    #[inline]
    pub fn new(
        severity: DiagnosticSeverity,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            range,
            message: message.into(),
            code: None,
            source: None,
            category: None,
            suggestions: Vec::new(),
            decorations: Vec::new(),
        }
    }

    /// Create a diagnostic, validating that the message is non-empty and the
    /// range is well-formed.
    ///
    /// Returns [`DiagnosticError::EmptyMessage`] if the message is empty, or
    /// the error from [`TextRange::validate`] if the range is invalid.
    #[inline]
    pub fn try_new(
        severity: DiagnosticSeverity,
        range: TextRange,
        message: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let message_val = message.into();
        if message_val.is_empty() {
            return Err(DiagnosticError::EmptyMessage);
        }
        range.validate()?;
        Ok(Self::new(severity, range, message_val))
    }

    /// Builder: attach a machine-readable code.
    #[inline]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Builder: attach a machine-readable code only if `Some`.
    #[inline]
    pub fn with_code_opt(mut self, code: Option<impl Into<String>>) -> Self {
        self.code = code.map(|c| c.into());
        self
    }

    /// Builder: attribute this diagnostic to a source/producer.
    #[inline]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Builder: classify this diagnostic by category.
    #[inline]
    pub fn with_category(mut self, category: DiagnosticCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Builder: attach a suggestion (quick-fix).
    #[inline]
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Builder: attach multiple suggestions at once.
    #[inline]
    pub fn with_suggestions(mut self, suggestions: impl IntoIterator<Item = Suggestion>) -> Self {
        self.suggestions.extend(suggestions);
        self
    }

    /// Builder: attach an additional decoration.
    #[inline]
    pub fn with_decoration(mut self, decoration: Decoration) -> Self {
        self.decorations.push(decoration);
        self
    }

    /// Builder: attach multiple decorations at once.
    #[inline]
    pub fn with_decorations(
        mut self,
        decorations: impl IntoIterator<Item = Decoration>,
    ) -> Self {
        self.decorations.extend(decorations);
        self
    }

    /// Validate structural invariants of this diagnostic.
    ///
    /// Checks:
    /// - `message` is non-empty
    /// - `range` is well-formed (see [`TextRange::validate`])
    /// - Each attached suggestion validates its own range and title
    /// - Each attached decoration validates its own range
    ///
    /// Does **not** verify that offsets correspond to actual document text —
    /// that requires the source document, which this method does not have.
    pub fn validate(&self) -> Result<(), DiagnosticError> {
        if self.message.is_empty() {
            return Err(DiagnosticError::EmptyMessage);
        }
        self.range.validate()?;
        for suggestion in &self.suggestions {
            suggestion.validate()?;
        }
        for decoration in &self.decorations {
            decoration.validate()?;
        }
        if let Some(ref code) = self.code {
            if code.is_empty() {
                return Err(DiagnosticError::EmptyCode);
            }
        }
        Ok(())
    }

    /// The canonical style for this diagnostic, resolved from its severity.
    ///
    /// Renderers call this to obtain presentation intent from the diagnostic's
    /// classification rather than branching on severity themselves.
    #[inline]
    pub fn style(&self) -> DiagnosticStyle {
        crate::diagnostic::mapping::diagnostic_style(self.severity)
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source {
            Some(source) => write!(f, "{}: {} {}", source, self.severity, self.message),
            None => write!(f, "{} {}", self.severity, self.message),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::style::{DiagnosticIcon, Priority};

    #[test]
    fn text_position_basic() {
        let pos = TextPosition::new(3, 12);
        assert_eq!(pos.line, 3);
        assert_eq!(pos.character, 12);
        assert_eq!(pos.to_string(), "3:12");
    }

    #[test]
    fn text_range_empty_and_contains() {
        let pos = TextPosition::new(1, 5);
        let empty = TextRange::empty(pos);
        assert!(empty.is_empty());
        assert!(empty.contains(pos));

        let span = TextRange::new(TextPosition::new(1, 0), TextPosition::new(1, 10));
        assert!(!span.is_empty());
        assert!(span.contains(TextPosition::new(1, 5)));
        assert!(!span.contains(TextPosition::new(2, 0)));
    }

    #[test]
    fn range_cross_line_containment() {
        let span = TextRange::new(TextPosition::new(1, 20), TextPosition::new(3, 5));
        assert!(span.contains(TextPosition::new(2, 0)));
        assert!(!span.contains(TextPosition::new(0, 99)));
        assert!(!span.contains(TextPosition::new(4, 0)));
    }

    #[test]
    fn range_byte_offsets() {
        let range = TextRange::try_with_offsets(
            TextPosition::new(0, 0),
            TextPosition::new(0, 5),
            0,
            5,
        )
        .unwrap();
        assert_eq!(range.start_byte_offset(), Some(0));
        assert_eq!(range.end_byte_offset(), Some(5));
    }

    #[test]
    fn range_validate_rejects_inverted_positions() {
        let result = TextRange::try_new(TextPosition::new(2, 0), TextPosition::new(1, 5));
        assert!(matches!(
            result,
            Err(DiagnosticError::InvalidRange { .. })
        ));
    }

    #[test]
    fn range_try_new_accepts_valid() {
        let range = TextRange::try_new(
            TextPosition::new(0, 0),
            TextPosition::new(0, 5),
        )
        .unwrap();
        assert!(!range.is_empty());
    }

    #[test]
    fn range_try_new_accepts_equal_positions() {
        let result = TextRange::try_new(TextPosition::new(1, 2), TextPosition::new(1, 2));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn range_with_offsets_rejects_inverted_byte_offsets() {
        let result = TextRange::try_with_offsets(
            TextPosition::new(0, 0),
            TextPosition::new(0, 5),
            10,
            5,
        );
        assert!(matches!(
            result,
            Err(DiagnosticError::InvalidOffset { start: 10, end: 5 })
        ));
    }

    #[test]
    fn range_with_offsets_rejects_inverted_positions() {
        let result = TextRange::try_with_offsets(
            TextPosition::new(2, 0),
            TextPosition::new(1, 5),
            0,
            5,
        );
        assert!(matches!(
            result,
            Err(DiagnosticError::InvalidRange { .. })
        ));
    }

    #[test]
    fn range_validate_rejects_inverted_offsets() {
        let range = TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5))
            .with_byte_offsets(100, 50);
        assert!(matches!(
            range.validate(),
            Err(DiagnosticError::InvalidOffset { .. })
        ));
    }

    #[test]
    fn range_validate_accepts_valid_offsets() {
        let range = TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5))
            .with_byte_offsets(0, 5);
        assert!(range.validate().is_ok());
    }

    #[test]
    fn range_validate_accepts_no_offsets() {
        let range = TextRange::empty(TextPosition::new(0, 0));
        assert!(range.validate().is_ok());
    }

    // --- Suggestion ---

    #[test]
    fn suggestion_simple_constructor() {
        let sug = Suggestion::simple(
            "Fix typo",
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 3)),
            "because",
        );
        assert_eq!(sug.title, "Fix typo");
        assert_eq!(sug.new_text, "because");
        assert!(sug.kind.is_none());
        assert_eq!(sug.effective_applicability(), SuggestionApplicability::OnRequest);
        assert_eq!(sug.effective_priority(), SuggestionPriority::Normal);
    }

    #[test]
    fn suggestion_with_applicability_and_priority() {
        let sug = Suggestion::simple(
            "Apply fix",
            TextRange::empty(TextPosition::new(0, 0)),
            "replacement",
        )
        .with_applicability(SuggestionApplicability::Always)
        .with_priority(SuggestionPriority::High)
        .with_kind("quickfix");

        assert_eq!(sug.applicability, Some(SuggestionApplicability::Always));
        assert_eq!(sug.priority, Some(SuggestionPriority::High));
        assert_eq!(sug.kind.as_deref(), Some("quickfix"));
        assert_eq!(sug.effective_applicability(), SuggestionApplicability::Always);
        assert_eq!(sug.effective_priority(), SuggestionPriority::High);
    }

    #[test]
    fn suggestion_try_new_rejects_empty_title() {
        let result = Suggestion::try_new(
            "",
            TextRange::empty(TextPosition::new(0, 0)),
            "text",
        );
        assert!(matches!(
            result,
            Err(DiagnosticError::EmptySuggestionTitle)
        ));
    }

    #[test]
    fn suggestion_try_new_rejects_invalid_range() {
        let result = Suggestion::try_new(
            "Fix",
            TextRange::new(TextPosition::new(2, 0), TextPosition::new(1, 0)),
            "text",
        );
        assert!(matches!(
            result,
            Err(DiagnosticError::InvalidRange { .. })
        ));
    }

    #[test]
    fn suggestion_validate_rejects_empty_title() {
        let sug = Suggestion::simple("", TextRange::empty(TextPosition::new(0, 0)), "x");
        assert!(matches!(
            sug.validate(),
            Err(DiagnosticError::EmptySuggestionTitle)
        ));
    }

    #[test]
    fn suggestion_applicability_name_and_display() {
        assert_eq!(SuggestionApplicability::Always.name(), "always");
        assert_eq!(SuggestionApplicability::OnRequest.to_string(), "on-request");
        assert_eq!(SuggestionApplicability::Manual.label(), "Requires review");
        assert_eq!(SuggestionApplicability::NotApplicable.name(), "not-applicable");
    }

    #[test]
    fn suggestion_priority_name_and_display() {
        assert_eq!(SuggestionPriority::High.name(), "high");
        assert_eq!(SuggestionPriority::Normal.to_string(), "normal");
        assert_eq!(SuggestionPriority::Low.name(), "low");
    }

    #[test]
    fn suggestion_applicability_default_is_on_request() {
        assert_eq!(SuggestionApplicability::default(), SuggestionApplicability::OnRequest);
    }

    #[test]
    fn suggestion_priority_default_is_normal() {
        assert_eq!(SuggestionPriority::default(), SuggestionPriority::Normal);
    }

    #[test]
    fn suggestion_serialization_round_trip() {
        let sug = Suggestion::simple(
            "Fix",
            TextRange::try_with_offsets(
                TextPosition::new(0, 0),
                TextPosition::new(0, 3),
                0,
                3,
            )
            .unwrap(),
            "because",
        )
        .with_applicability(SuggestionApplicability::Always)
        .with_priority(SuggestionPriority::High);

        let json = serde_json::to_string(&sug).expect("serialize suggestion");
        let back: Suggestion = serde_json::from_str(&json).expect("deserialize suggestion");
        assert_eq!(sug, back);
        assert!(json.contains("\"always\""));
        assert!(json.contains("\"high\""));
    }

    // --- Decoration ---

    #[test]
    fn decoration_with_tooltip() {
        let dec = Decoration {
            category: DecorationCategory::Background,
            range: TextRange::empty(TextPosition::new(0, 0)),
            style: None,
            tooltip: None,
        }
        .with_tooltip("Suspicious")
        .with_style(DiagnosticStyle {
            visual_emphasis: crate::diagnostic::style::VisualEmphasis::Strong,
            icon: DiagnosticIcon::Error,
            highlight_style: crate::diagnostic::style::HighlightStyle::Squiggly,
            decoration_category: DecorationCategory::Underline,
            gutter_indicator: crate::diagnostic::style::GutterIndicator::Bar,
            priority: Priority::High,
            accessibility: crate::diagnostic::style::AccessibilityMeta {
                label: "Error".to_string(),
                description: None,
                visual_indicator: crate::diagnostic::style::VisualIndicator::Shape,
                color_safe: true,
            },
        });

        assert_eq!(dec.tooltip.as_deref(), Some("Suspicious"));
        assert!(dec.style.is_some());
    }

    #[test]
    fn decoration_serialization_round_trip() {
        let dec = Decoration {
            category: DecorationCategory::Underline,
            range: TextRange::empty(TextPosition::new(1, 2)),
            style: None,
            tooltip: Some("hover text".to_string()),
        };
        let json = serde_json::to_string(&dec).expect("serialize decoration");
        let back: Decoration = serde_json::from_str(&json).expect("deserialize decoration");
        assert_eq!(dec, back);
        // DecorationCategory uses PascalCase serde tokens (no rename_all).
        assert!(json.contains("\"Underline\""));
    }

    // --- Diagnostic ---

    #[test]
    fn diagnostic_new_and_builders() {
        let range = TextRange::new(TextPosition::new(1, 0), TextPosition::new(1, 10));
        let mut diag = Diagnostic::new(DiagnosticSeverity::Warning, range, "typo")
            .with_code("TYP001")
            .with_source("harper")
            .with_category(DiagnosticCategory::SpellCheck)
            .with_suggestion(Suggestion::simple("Fix", range, "because"));

        assert_eq!(diag.severity, DiagnosticSeverity::Warning);
        assert_eq!(diag.code.as_deref(), Some("TYP001"));
        assert_eq!(diag.source.as_deref(), Some("harper"));
        assert_eq!(diag.category, Some(DiagnosticCategory::SpellCheck));
        assert_eq!(diag.suggestions.len(), 1);
        assert!(diag.decorations.is_empty());

        diag = diag.with_decoration(Decoration {
            category: DecorationCategory::Background,
            range,
            style: None,
            tooltip: None,
        });
        assert_eq!(diag.decorations.len(), 1);
        assert_eq!(diag.decorations[0].category, DecorationCategory::Background);
    }

    #[test]
    fn diagnostic_with_suggestions_batch() {
        let range = TextRange::empty(TextPosition::new(0, 0));
        let diag = Diagnostic::new(DiagnosticSeverity::Warning, range, "multi")
            .with_suggestions(vec![
                Suggestion::simple("Fix 1", range, "a"),
                Suggestion::simple("Fix 2", range, "b"),
            ]);
        assert_eq!(diag.suggestions.len(), 2);
    }

    #[test]
    fn diagnostic_with_decorations_batch() {
        let range = TextRange::empty(TextPosition::new(0, 0));
        let diag = Diagnostic::new(DiagnosticSeverity::Error, range, "deco")
            .with_decorations(vec![
                Decoration {
                    category: DecorationCategory::Underline,
                    range,
                    style: None,
                    tooltip: None,
                },
                Decoration {
                    category: DecorationCategory::Block,
                    range,
                    style: None,
                    tooltip: None,
                },
            ]);
        assert_eq!(diag.decorations.len(), 2);
    }

    #[test]
    fn diagnostic_try_new_rejects_empty_message() {
        let result = Diagnostic::try_new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "",
        );
        assert!(matches!(result, Err(DiagnosticError::EmptyMessage)));
    }

    #[test]
    fn diagnostic_try_new_rejects_invalid_range() {
        let result = Diagnostic::try_new(
            DiagnosticSeverity::Error,
            TextRange::new(TextPosition::new(2, 0), TextPosition::new(1, 0)),
            "bad range",
        );
        assert!(matches!(
            result,
            Err(DiagnosticError::InvalidRange { .. })
        ));
    }

    #[test]
    fn diagnostic_try_new_accepts_valid() {
        let diag = Diagnostic::try_new(
            DiagnosticSeverity::Warning,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            "ok",
        )
        .unwrap();
        assert_eq!(diag.message, "ok");
    }

    #[test]
    fn diagnostic_validate_rejects_nested_invalid_range() {
        let bad_range = TextRange::new(TextPosition::new(2, 0), TextPosition::new(1, 0));
        let diag = Diagnostic::new(DiagnosticSeverity::Warning, bad_range, "msg")
            .with_suggestion(Suggestion::simple("Fix", bad_range, "x"));
        assert!(matches!(
            diag.validate(),
            Err(DiagnosticError::InvalidRange { .. })
        ));
    }

    #[test]
    fn diagnostic_validate_rejects_empty_code() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Warning,
            TextRange::empty(TextPosition::new(0, 0)),
            "msg",
        )
        .with_code("");
        assert!(matches!(diag.validate(), Err(DiagnosticError::EmptyCode)));
    }

    #[test]
    fn diagnostic_validate_accepts_well_formed() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            "something broke",
        )
        .with_code("E123")
        .with_source("lsp")
        .with_category(DiagnosticCategory::Semantic)
        .with_suggestion(
            Suggestion::try_new(
                "Fix it",
                TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
                "fixed",
            )
            .unwrap(),
        )
        .with_decoration(Decoration {
            category: DecorationCategory::Background,
            range: TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
            style: None,
            tooltip: Some("Broke here".to_string()),
        });
        assert!(diag.validate().is_ok());
    }

    #[test]
    fn diagnostic_style_resolves_from_severity() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "broken",
        );
        let style = diag.style();
        assert_eq!(style.priority, Priority::High);
        assert_eq!(style.icon, DiagnosticIcon::Error);
    }

    #[test]
    fn diagnostic_display_with_source() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Error,
            TextRange::empty(TextPosition::new(0, 0)),
            "boom",
        )
        .with_source("ocr");
        assert_eq!(diag.to_string(), "ocr: error boom");
    }

    #[test]
    fn diagnostic_display_without_source() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Hint,
            TextRange::empty(TextPosition::new(0, 0)),
            "tip",
        );
        assert_eq!(diag.to_string(), "hint tip");
    }

    #[test]
    fn diagnostic_serialization_round_trip() {
        let range = TextRange::try_with_offsets(
            TextPosition::new(2, 4),
            TextPosition::new(2, 9),
            100,
            105,
        )
        .unwrap();
        let diag = Diagnostic::new(DiagnosticSeverity::Critical, range, "corruption detected")
            .with_code("DISK99")
            .with_source("storage")
            .with_category(DiagnosticCategory::Metadata)
            .with_suggestion(Suggestion {
                title: "Revert snapshot".into(),
                range,
                new_text: "old-content".into(),
                kind: Some("quickfix".into()),
                applicability: Some(SuggestionApplicability::Always),
                priority: Some(SuggestionPriority::High),
            });
        let json = serde_json::to_string(&diag).expect("serialize diagnostic");
        let back: Diagnostic = serde_json::from_str(&json).expect("deserialize diagnostic");
        assert_eq!(diag, back);
        // Severity survives as a stable string token.
        assert!(json.contains("\"critical\""));
    }

    #[test]
    fn diagnostic_empty_fields_are_skipped_in_json() {
        let diag = Diagnostic::new(
            DiagnosticSeverity::Warning,
            TextRange::empty(TextPosition::new(0, 0)),
            "minor",
        );
        let json = serde_json::to_string(&diag).unwrap();
        assert!(!json.contains("code"));
        assert!(!json.contains("source"));
        assert!(!json.contains("category"));
        // Severity token still present.
        assert!(json.contains("\"warning\""));
    }

    #[test]
    fn diagnostic_empty_fields_deserialize_with_defaults() {
        // A minimal JSON payload (just severity, range, message) should
        // deserialize successfully, with all Option/Vec fields defaulting.
        let json = r#"{"severity":"warning","range":{"start":{"line":0,"character":0},"end":{"line":0,"character":5}},"message":"hi"}"#;
        let diag: Diagnostic = serde_json::from_str(json).expect("deserialize minimal");
        assert_eq!(diag.code, None);
        assert_eq!(diag.source, None);
        assert_eq!(diag.category, None);
        assert!(diag.suggestions.is_empty());
        assert!(diag.decorations.is_empty());
    }

    #[test]
    fn diagnostic_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Diagnostic>();
        assert_send_sync::<TextRange>();
        assert_send_sync::<TextPosition>();
        assert_send_sync::<Suggestion>();
        assert_send_sync::<Decoration>();
    }
}
