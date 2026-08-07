//! # Severity → Style Mapping
//!
//! The centralized, canonical mapping from [`DiagnosticSeverity`] to
//! [`DiagnosticStyle`]. This is the **single source of truth** for how each
//! built-in severity is presented; every future editor, panel, and inline
//! annotation derives its appearance from [`diagnostic_style`] (or a
//! theme/plugin-customized [`DiagnosticStyleMap`] built on the same schema).
//!
//! ## Architecture
//!
//! ```text
//!  DiagnosticSeverity  ──[diagnostic_style()]──▶  DiagnosticStyle
//!        (enum)              (canonical fn)          (struct of intent)
//!
//!  DiagnosticStyleMap  ──[Default]──▶  {severity → style}  (all 5 built-ins)
//!        ▲  themes/plugins clone + override individual severities
//!        │  (same schema, forward-compatible via #[serde(default)])
//!        └── renderers resolve a severity through `style(severity)`
//! ```
//!
//! ## Why a function, not a `const`?
//!
//! `DiagnosticStyle` owns short `String` labels for accessibility (so it can
//! derive `Serialize`/`Deserialize`), which means it cannot be a `const`.
//! `diagnostic_style` is a pure, allocation-light function returning an owned
//! value; renderers cache the result. No rendering or UI frameworks are
//! touched here — only abstract presentation intent.
//!
//! ## Extension Strategy
//!
//! - **New severity variant**: adding a variant to `DiagnosticSeverity`
//!   (which is `#[non_exhaustive]`) is a compile error in this module until a
//!   style is assigned — guaranteeing nothing ships unstyled.
//! - **Custom theme/plugin styles**: clone the [`Default`]
//!   [`DiagnosticStyleMap`], call [`insert`](super::style::DiagnosticStyleMap::insert) for any
//!   severities you wish to restyle, and hand the map to your renderer.
//! - **User-defined severity mappings**: out of scope here (no extra severity
//!   values exist), but the `DiagnosticStyleMap` abstraction is ready for
//!   it — future user severities just become extra keys in the map.
//!
//! [`DiagnosticStyle`]: super::style::DiagnosticStyle
//! [`DiagnosticStyleMap`]: super::style::DiagnosticStyleMap

use super::severity::DiagnosticSeverity;
use super::style::{
    AccessibilityMeta, DecorationCategory, DiagnosticIcon, GutterIndicator, HighlightStyle,
    Priority, VisualEmphasis, VisualIndicator,
};

/// Returns the canonical [`DiagnosticStyle`](super::style::DiagnosticStyle) for a given severity.
///
/// This is the central lookup every renderer should start from. To restyle a
/// severity, clone [`DiagnosticStyleMap::default`](super::style::DiagnosticStyleMap::default) and [`insert`](super::style::DiagnosticStyleMap::insert)
/// an override rather than branching on severity in rendering code.
///
/// # Guarantees
///
/// - Defined for every variant in `DiagnosticSeverity::ALL`.
/// - Never returns a style whose `accessibility.label` is empty — severity is
///   always perceivable via a non-color indicator.
/// - Pure / allocation-light: constructs a small struct of enums and two short
///   owned strings per call. Renderers cache the result.
pub fn diagnostic_style(severity: DiagnosticSeverity) -> super::style::DiagnosticStyle {
    // Each arm wires a severity to its full presentation intent. Keep this
    // match exhaustive: `DiagnosticSeverity` is `#[non_exhaustive]`, so adding
    // a variant is a compile error here until a style is assigned.
    let (
        visual_emphasis,
        icon,
        highlight_style,
        decoration_category,
        gutter_indicator,
        priority,
        accessibility,
    ) = match severity {
        DiagnosticSeverity::Hint => (
            VisualEmphasis::Subtle,
            DiagnosticIcon::Dot,
            HighlightStyle::Underline,
            DecorationCategory::Underline,
            GutterIndicator::Dot,
            Priority::Low,
            AccessibilityMeta {
                label: "Hint".to_string(),
                description: Some("an optional improvement or speculative suggestion".to_string()),
                visual_indicator: VisualIndicator::Dot,
                color_safe: false,
            },
        ),

        DiagnosticSeverity::Information => (
            VisualEmphasis::Moderate,
            DiagnosticIcon::Info,
            HighlightStyle::Underline,
            DecorationCategory::Underline,
            GutterIndicator::Dot,
            Priority::Normal,
            AccessibilityMeta {
                label: "Information".to_string(),
                description: Some("factual context the user may need to know".to_string()),
                visual_indicator: VisualIndicator::Glyph,
                color_safe: false,
            },
        ),

        DiagnosticSeverity::Warning => (
            VisualEmphasis::Strong,
            DiagnosticIcon::Warning,
            HighlightStyle::Squiggly,
            DecorationCategory::Underline,
            GutterIndicator::Bar,
            Priority::High,
            AccessibilityMeta {
                label: "Warning".to_string(),
                description: Some("suspicious — likely unintentional; review recommended".to_string()),
                visual_indicator: VisualIndicator::Shape,
                color_safe: true,
            },
        ),

        DiagnosticSeverity::Error => (
            VisualEmphasis::Strong,
            DiagnosticIcon::Error,
            HighlightStyle::Squiggly,
            DecorationCategory::Underline,
            GutterIndicator::Bar,
            Priority::High,
            AccessibilityMeta {
                label: "Error".to_string(),
                description: Some("a real problem that breaks an invariant or expectation".to_string()),
                visual_indicator: VisualIndicator::Shape,
                color_safe: true,
            },
        ),

        DiagnosticSeverity::Critical => (
            VisualEmphasis::Prominent,
            DiagnosticIcon::Critical,
            HighlightStyle::UnderlineAndBackground,
            DecorationCategory::Block,
            GutterIndicator::Bar,
            Priority::Critical,
            AccessibilityMeta {
                label: "Critical".to_string(),
                description: Some("a catastrophic condition — data loss, corruption, or an unrecoverable failure".to_string()),
                visual_indicator: VisualIndicator::Pattern,
                color_safe: true,
            },
        ),
    };

    super::style::DiagnosticStyle {
        visual_emphasis,
        icon,
        highlight_style,
        decoration_category,
        gutter_indicator,
        priority,
        accessibility,
    }
}

/// Returns every canonical severity→style pair, in ascending severity order.
///
/// Convenience for renderers that want to enumerate the full default palette
/// (e.g. to build a legend or a settings preview).
pub fn default_severity_styles() -> Vec<(DiagnosticSeverity, super::style::DiagnosticStyle)> {
    DiagnosticSeverity::ALL.iter().copied().map(|s| (s, diagnostic_style(s))).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::style::DiagnosticStyleMap;

    #[test]
    fn mapping_covers_every_severity() {
        for &sev in DiagnosticSeverity::ALL {
            let style = diagnostic_style(sev);
            // Every style must carry a non-empty accessibility label so that
            // severity is always perceivable via a screen reader.
            assert!(!style.accessibility.label.is_empty(), "{:?} has empty label", sev);
            // `VisualIndicator` has no `None` variant, so every style always
            // carries a concrete non-color cue — severity is never color-only.
            let _ = style.accessibility.visual_indicator;
        }
    }

    #[test]
    fn mapping_is_monotonic_in_priority() {
        let severities = DiagnosticSeverity::ALL;
        let priorities: Vec<_> = severities.iter().copied().map(|s| diagnostic_style(s).priority).collect();
        for window in priorities.windows(2) {
            assert!(window[0] <= window[1], "priority must be non-decreasing with severity");
        }
    }

    #[test]
    fn mapping_is_monotonic_in_visual_emphasis() {
        let severities = DiagnosticSeverity::ALL;
        let emphases: Vec<_> = severities.iter().copied().map(|s| diagnostic_style(s).visual_emphasis).collect();
        for window in emphases.windows(2) {
            assert!(window[0] <= window[1], "emphasis must be non-decreasing with severity");
        }
    }

    #[test]
    fn warning_and_error_share_squiggle_but_differ_in_icon() {
        let warn = diagnostic_style(DiagnosticSeverity::Warning);
        let err = diagnostic_style(DiagnosticSeverity::Error);
        assert_eq!(warn.highlight_style, HighlightStyle::Squiggly);
        assert_eq!(err.highlight_style, HighlightStyle::Squiggly);
        assert_ne!(warn.icon, err.icon);
    }

    #[test]
    fn critical_uses_block_decoration_and_pattern_indicator() {
        let crit = diagnostic_style(DiagnosticSeverity::Critical);
        assert_eq!(crit.decoration_category, DecorationCategory::Block);
        assert_eq!(crit.highlight_style, HighlightStyle::UnderlineAndBackground);
        assert_eq!(crit.accessibility.visual_indicator, VisualIndicator::Pattern);
        assert!(crit.accessibility.color_safe);
    }

    #[test]
    fn hint_is_subtle_with_dot_indicator() {
        let hint = diagnostic_style(DiagnosticSeverity::Hint);
        assert_eq!(hint.visual_emphasis, VisualEmphasis::Subtle);
        assert_eq!(hint.icon, DiagnosticIcon::Dot);
        assert_eq!(hint.gutter_indicator, GutterIndicator::Dot);
        assert_eq!(hint.priority, Priority::Low);
    }

    #[test]
    fn default_severity_styles_returns_all_pairs_ordered() {
        let pairs = default_severity_styles();
        assert_eq!(pairs.len(), DiagnosticSeverity::ALL.len());
        for (i, (sev, _)) in pairs.iter().enumerate() {
            assert_eq!(*sev, DiagnosticSeverity::ALL[i]);
        }
    }

    #[test]
    fn default_map_resolves_every_severity_without_fallback() {
        let map = DiagnosticStyleMap::default();
        for &sev in DiagnosticSeverity::ALL {
            assert!(map.get(sev).is_some());
            let resolved = map.style(sev);
            // The default map should return the canonical style exactly.
            assert_eq!(resolved, diagnostic_style(sev));
        }
    }
}
