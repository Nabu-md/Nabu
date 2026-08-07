//! # Diagnostic Style Definitions
//!
//! Reusable, platform-agnostic descriptions of how a diagnostic should be
//! presented. These types describe **presentation intent** — never concrete
//! rendering values. Every editor, panel, and inline annotation derives its
//! appearance from a [`DiagnosticStyle`] (or a theme override built on the same
//! schema) rather than inventing its own severity mappings.
//!
//! ## What This Is
//!
//! A [`DiagnosticStyle`] bundles seven orthogonal, abstract properties:
//!
//! | Property            | Type                     | Drives                 |
//! |---------------------|--------------------------|------------------------|
//! | Visual emphasis     | [`VisualEmphasis`]        | overall strength       |
//! | Icon identifier     | [`DiagnosticIcon`]        | gutter/inline icons    |
//! | Highlight style     | [`HighlightStyle`]        | text squiggles/underline |
//! | Decoration category | [`DecorationCategory`]   | _where_ it renders     |
//! | Gutter indicator    | [`GutterIndicator`]       | the editor gutter      |
//! | Priority            | [`Priority`]              | z-order / preemption   |
//! | Accessibility       | [`AccessibilityMeta`]     | screen readers, labels |
//!
//! ## What This Is NOT
//!
//! - No CSS, no HTML, no color values, no pixel sizes.
//! - No Dioxus/Monaco/CodeMirror/Tailwind dependencies.
//! - No rendering logic — only metadata that a renderer *consumes*.
//!
//! ## Rendering Contract
//!
//! A concrete renderer (editor or UI) is expected to translate these abstract
//! values into platform primitives:
//!
//! - [`DiagnosticIcon`] → a glyph/SVG from the active icon/theme set. The
//!   [`DiagnosticIcon::Custom`] variant lets plugins name a theme-registered
//!   icon without the core knowing its concrete appearance.
//! - [`HighlightStyle`] / [`DecorationCategory`] → the renderer's underline,
//!   background, and bracket APIs.
//! - [`AccessibilityMeta`] → `aria-label`, `aria-description`, and a non-color
//!   indicator so severity is never conveyed by color alone.
//!
//! Future custom themes and plugin-defined styles build values of this same
//! struct, so they interoperate with the canonical severity mapping in
//! [`crate::diagnostic::mapping`] without breaking changes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::severity::DiagnosticSeverity;

// ---------------------------------------------------------------------------
// Style sub-enums
// ---------------------------------------------------------------------------

/// How strongly a diagnostic should be visually emphasized.
///
/// Renderers translate this into concrete treatments (e.g. subtle = gutter
/// dot only; prominent = full-line block highlight). It is deliberately
/// independent of color so that emphasis survives theme/color changes and
/// high-contrast modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum VisualEmphasis {
    /// Minimal — a marker only (e.g. gutter dot); no text decoration.
    Subtle,
    /// Moderate — text decoration (underline/squiggle) without a heavy
    /// background.
    Moderate,
    /// Strong — background tint or a prominent underline.
    Strong,
    /// Prominent — block-level emphasis (full-line background, banner).
    Prominent,
}

/// Abstract icon identifier for a diagnostic style.
///
/// The core enum covers the canonical diagnostic icons. Plugins and themes that
/// need bespoke icons use [`DiagnosticIcon::Custom`], passing a stable string
/// name that the active renderer resolves against its icon/theme registry.
///
/// This is an *identifier*, not a glyph — keeping the core platform
/// UI-framework-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticIcon {
    /// No icon — rely on other indicators (gutter shape, underline).
    None,
    /// A plain dot/bullet marker.
    Dot,
    /// An information "i" in a circle.
    Info,
    /// An exclamation mark.
    Warning,
    /// A cross / X mark.
    Error,
    /// A hazard/bomb symbol for catastrophic conditions.
    Critical,
    /// A lightbulb or wrench (quick-fix / suggestion).
    Suggestion,
    /// A plugin/theme-provided icon, identified by a stable registered name.
    /// The renderer resolves this string against its active icon set; if the
    /// icon is unknown it falls back to [`DiagnosticIcon::Dot`].
    Custom(String),
}

/// The kind of text highlight a renderer should apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HighlightStyle {
    /// No text highlight.
    None,
    /// Solid underline.
    Underline,
    /// Squiggly (wavy) underline — the classic "spelling error" look.
    Squiggly,
    /// Solid wavy underline.
    Wave,
    /// Background color tint behind the affected text.
    Background,
    /// Both an underline and a background tint.
    UnderlineAndBackground,
}

/// Where, structurally, a decoration is applied relative to the text.
///
/// This describes *placement intent* — a renderer maps each category to the
/// relevant editor API (e.g. `Underline` → text-decorations, `Gutter` → the
/// gutter zone, `Block` → a full-width banner line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DecorationCategory {
    /// No additional structural decoration — rely on highlight/underline only.
    None,
    /// A background tint behind the affected text range.
    Background,
    /// An underline (possibly squiggly) on the affected text range.
    Underline,
    /// A marker on the line's leading bracket/brace.
    Bracket,
    /// An indicator in the editor gutter (left margin).
    Gutter,
    /// An inline decoration replacing/injecting text.
    Inline,
    /// A block-level banner or full-width line decoration.
    Block,
}

/// How a severity is represented in the editor's gutter (left margin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GutterIndicator {
    /// No gutter indicator.
    None,
    /// A small dot.
    Dot,
    /// A left border along the line.
    Border,
    /// A colored bar spanning the line height.
    Bar,
    /// An icon (resolved from [`DiagnosticIcon`]).
    Icon,
}

/// Rendering precedence when multiple diagnostics overlap the same location.
///
/// `Critical` preempts `High`, etc. This is a *rendering* priority (which
/// style wins the pixel), distinct from [`DiagnosticSeverity`] (which a
/// producer reports). The canonical mapping assigns priority from severity,
/// but themes may diverge (e.g. always highlight errors first regardless of
/// severity) by supplying a custom [`DiagnosticStyleMap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Priority {
    /// Lowest — yield to all other priorities.
    Low,
    /// Default precedence for moderate/weak signals.
    Normal,
    /// High — preempts Low/Normal overlaps.
    High,
    /// Highest — preempts everything (reserved for Critical-equivalent styles).
    Critical,
}

/// The kind of non-color visual cue a diagnostic uses, so severity remains
/// perceivable without relying on color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum VisualIndicator {
    /// A filled dot or bullet shape.
    Dot,
    /// A typographic glyph (letter/symbol).
    Glyph,
    /// A geometric shape (triangle, square, diamond).
    Shape,
    /// A texture/stipple pattern — critical for print-friendly diagnostics.
    Pattern,
    /// Inline text (a prefix/suffix token).
    Text,
    /// A dedicated icon.
    Icon,
}

// ---------------------------------------------------------------------------
// Accessibility metadata
// ---------------------------------------------------------------------------

/// Accessibility metadata for a diagnostic style.
///
/// Every diagnostic must be perceivable without color, so each style carries
/// an explicit screen-reader label, an optional longer description, and a
/// non-color [`VisualIndicator`]. Renderers surface these via `aria-label`,
/// `aria-description`, and a visible non-color cue (glyph, shape, pattern).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilityMeta {
    /// Short, screen-reader-friendly label (e.g. `"Error"`, `"Warning"`).
    /// Non-empty by contract — the canonical mapping always provides one.
    pub label: String,

    /// Optional extended description for assistive technology, e.g.
    /// `"A real problem that breaks an invariant"`. When `None`, the
    /// renderer falls back to the severity's [`label`](DiagnosticSeverity::label).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The primary non-color visual indicator. Ensures the diagnostic's
    /// importance is perceivable in grayscale, print, or to color-blind users.
    pub visual_indicator: VisualIndicator,

    /// True when this style is designed to remain distinguishable without
    /// color (i.e. it always supplies a [`visual_indicator`](Self::visual_indicator)
    /// that differs from the `None` treatment). Consumers may gate
    /// high-contrast enforcement on this flag.
    pub color_safe: bool,
}

// ---------------------------------------------------------------------------
// The canonical style
// ---------------------------------------------------------------------------

/// A reusable, platform-agnostic description of how a diagnostic should be
/// presented.
///
/// A `DiagnosticStyle` is *presentation intent*: it says *what* the
/// renderer should convey, not *how*. Concrete renderers (Monaco, CodeMirror,
/// Dioxus, native, print) translate each field into platform primitives.
///
/// Styles are immutable value types and therefore safe to share across
/// threads. The canonical severity→style mapping lives in
/// [`crate::diagnostic::mapping`]; themes and plugins produce their own
/// `DiagnosticStyle` values (optionally collected into a
/// [`DiagnosticStyleMap`]) using the same schema, so they interoperate without breaking changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticStyle {
    /// Overall strength of the visual treatment.
    pub visual_emphasis: VisualEmphasis,

    /// Abstract icon to render (gutter or inline).
    pub icon: DiagnosticIcon,

    /// How the affected text is highlighted.
    pub highlight_style: HighlightStyle,

    /// Where, structurally, the decoration is placed.
    pub decoration_category: DecorationCategory,

    /// What to show in the editor gutter.
    pub gutter_indicator: GutterIndicator,

    /// Rendering precedence when styles overlap.
    pub priority: Priority,

    /// Accessibility metadata (label, description, non-color indicator).
    pub accessibility: AccessibilityMeta,
}

/// A mapping from each [`DiagnosticSeverity`] to its [`DiagnosticStyle`].
///
/// This type is the backbone of future theming and plugin overrides:
///
/// - [`Default`] returns the centralized, canonical mapping (the single source
///   of truth for the five built-in severities).
/// - A theme or plugin can clone it, override individual severities via
///   [`insert`](Self::insert), and supply the customized map to renderers.
///
/// Because `DiagnosticStyle` is a stable schema, overrides are forward-
/// and backward-compatible: a renderer written against today's schema keeps
/// working if future styles add fields (those default to their `Default`
/// impls via `#[serde(default)]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticStyleMap(HashMap<DiagnosticSeverity, DiagnosticStyle>);

impl Default for DiagnosticStyleMap {
    /// The canonical severity→style mapping — the single source of truth for
    /// how the five built-in severities are presented.
    ///
    /// Every future editor or frontend should derive its appearance from this
    /// mapping (or a clone with targeted overrides).
    fn default() -> Self {
        Self::from_severity_fn(super::mapping::diagnostic_style)
    }
}

impl DiagnosticStyleMap {
    /// Build a map by applying `f` to every canonical severity.
    pub fn from_severity_fn(f: impl Fn(DiagnosticSeverity) -> DiagnosticStyle) -> Self {
        Self(DiagnosticSeverity::ALL.iter().copied().map(|s| (s, f(s))).collect())
    }

    /// Look up the style for a severity, or `None` if explicitly removed.
    pub fn get(&self, severity: DiagnosticSeverity) -> Option<&DiagnosticStyle> {
        self.0.get(&severity)
    }

    /// Resolve a style for a severity, falling back to the canonical default
    /// when the severity has no explicit entry (e.g. an unknown severity from
    /// a newer producer on an older core).
    pub fn style(&self, severity: DiagnosticSeverity) -> DiagnosticStyle {
        self.0
            .get(&severity)
            .cloned()
            .unwrap_or_else(|| super::mapping::diagnostic_style(severity))
    }

    /// Insert or replace the style for a severity.
    pub fn insert(&mut self, severity: DiagnosticSeverity, style: DiagnosticStyle) {
        self.0.insert(severity, style);
    }

    /// Number of severity→style entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over all (severity, style) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&DiagnosticSeverity, &DiagnosticStyle)> {
        self.0.iter()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_sub_enums_are_stable_and_ordered() {
        // Guard the canonical ordering of enum variants used by renderers.
        assert!(VisualEmphasis::Subtle < VisualEmphasis::Prominent);
        assert!(Priority::Low < Priority::Critical);
    }

    #[test]
    fn default_style_map_covers_every_severity() {
        let map = DiagnosticStyleMap::default();
        assert_eq!(map.len(), DiagnosticSeverity::ALL.len());
        for &sev in DiagnosticSeverity::ALL {
            assert!(map.get(sev).is_some(), "{:?} missing from default map", sev);
        }
    }

    #[test]
    fn style_map_style_falls_back_for_unknown_severity() {
        let map = DiagnosticStyleMap::default();
        // Even an unknown severity resolves via the canonical fallback.
        let style = map.style(DiagnosticSeverity::Warning);
        assert_eq!(style.priority, Priority::High);
    }

    #[test]
    fn style_map_insert_overrides() {
        let mut map = DiagnosticStyleMap::default();
        let custom = DiagnosticStyle {
            visual_emphasis: VisualEmphasis::Prominent,
            icon: DiagnosticIcon::Custom("my-bolt".to_string()),
            highlight_style: HighlightStyle::Background,
            decoration_category: DecorationCategory::Block,
            gutter_indicator: GutterIndicator::Bar,
            priority: Priority::Critical,
            accessibility: AccessibilityMeta {
                label: "custom".to_string(),
                description: None,
                visual_indicator: VisualIndicator::Shape,
                color_safe: true,
            },
        };
        map.insert(DiagnosticSeverity::Hint, custom.clone());
        assert_eq!(map.get(DiagnosticSeverity::Hint), Some(&custom));
    }

    #[test]
    fn style_map_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticStyleMap>();
        assert_send_sync::<DiagnosticStyle>();
        assert_send_sync::<AccessibilityMeta>();
    }

    #[test]
    fn custom_icon_serializes_as_string() {
        let icon = DiagnosticIcon::Custom("bolt".to_string());
        let json = serde_json::to_string(&icon).unwrap();
        let back: DiagnosticIcon = serde_json::from_str(&json).unwrap();
        assert_eq!(icon, back);
    }

    #[test]
    fn accessibility_description_is_skipped_when_none() {
        let meta = AccessibilityMeta {
            label: "Error".to_string(),
            description: None,
            visual_indicator: VisualIndicator::Glyph,
            color_safe: true,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("description"));
    }

    #[test]
    fn style_map_round_trips_through_serde_json() {
        let map = DiagnosticStyleMap::default();
        let json = serde_json::to_string(&map).unwrap();
        let back: DiagnosticStyleMap = serde_json::from_str(&json).unwrap();
        assert_eq!(map, back);
    }
}
