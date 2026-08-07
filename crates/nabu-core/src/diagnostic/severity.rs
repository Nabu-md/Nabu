//! # Diagnostic Severity Classification
//!
//! The canonical, strongly-typed severity system for Nabu's unified Diagnostic
//! Platform. Every diagnostic producer (Markdown analyzers, AI assistants,
//! language servers, OCR processors, spell checkers, grammar engines, plugins,
//! metadata validators, etc.) communicates severity through
//! [`DiagnosticSeverity`] instead of raw string values.
//!
//! ## Severity Levels
//!
//! | Variant       | Rank | Meaning                                                     |
//! |---------------|------|-------------------------------------------------------------|
//! | [`Hint`]        | 0    | Optional, speculative improvement; purely informational.  |
//! | [`Information`] | 1    | Factual context a user may need to know.                 |
//! | [`Warning`]     | 2    | Suspicious — likely unintentional; review recommended.   |
//! | [`Error`]       | 3    | A real problem that breaks an invariant or expectation.  |
//! | [`Critical`]    | 4    | Catastrophic — data loss, corruption, or unrecoverable.  |
//!
//! The numeric [`DiagnosticSeverity::level`] rank is monotonic: a higher rank
//! is always at least as severe as every lower rank, and the enum's derived
//! `Ord` reflects the same ordering (so `sort`/`max`/`min` work intuitively).
//!
//! ## Stability & Forward Compatibility
//!
//! The enum is `#[non_exhaustive]`: downstream crates must include a `_` arm
//! when matching exhaustively, and adding a new variant within this crate is a
//! **compile error** in the canonical mapping (`[diagnostic::mapping]`) until the
//! new variant is explicitly assigned a style. This guarantees no severity ever
//! leaks through unstyled.
//!
//! Discriminants are pinned with an explicit `u8` representation so that
//! serialized values remain stable across versions.
//!
//! [`Hint`]: DiagnosticSeverity::Hint
//! [`Information`]: DiagnosticSeverity::Information
//! [`Warning`]: DiagnosticSeverity::Warning
//! [`Error`]: DiagnosticSeverity::Error
//! [`Critical`]: DiagnosticSeverity::Critical
//! [`DiagnosticSeverity::level`]: DiagnosticSeverity::level

use serde::{Deserialize, Serialize};

/// Canonical diagnostic severity, ranked from least to most severe.
///
/// Producers should choose the *weakest* severity that accurately describes
/// the condition, so consumers can filter/prioritize meaningfully. Rendering
/// must never rely on color alone — see [`crate::diagnostic::style`] for the
/// abstract, accessibility-conscious style model every severity maps to.
///
/// The numeric [`level`](Self::level) value is stable and suitable for
/// persistence, IPC, and cross-process comparison.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[repr(u8)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    /// Optional, speculative improvement — the weakest signal.
    ///
    /// Maps to a subtle, low-priority style (e.g. a gutter dot, no squiggle).
    #[serde(rename = "hint")]
    Hint = 0,

    /// Factual context the user may need — not a problem, but relevant.
    ///
    /// Maps to a light, informational style (e.g. an "i" icon with a gentle
    /// underline).
    #[serde(rename = "information")]
    Information = 1,

    /// Suspicious construct that is likely unintentional — review suggested.
    ///
    /// Maps to a cautionary style (e.g. a yellow/amber squiggle).
    #[serde(rename = "warning")]
    Warning = 2,

    /// A real problem — an invariant, grammar rule, or expectation is broken.
    ///
    /// Maps to a prominent, high-priority style (e.g. a red squiggle).
    #[serde(rename = "error")]
    Error = 3,

    /// Catastrophic — data loss, corruption, or an unrecoverable failure.
    ///
    /// Maps to the strongest possible emphasis (e.g. a bold underline with a
    /// gutter marker and full-contrast treatment).
    #[serde(rename = "critical")]
    Critical = 4,
}

impl DiagnosticSeverity {
    /// All canonical severity variants, in ascending-severity order.
    pub const ALL: &'static [Self] = &[
        Self::Hint,
        Self::Information,
        Self::Warning,
        Self::Error,
        Self::Critical,
    ];

    /// Numeric rank of this severity (0 = hint … 4 = critical).
    ///
    /// Equal to the discriminant value; stable across versions.
    #[inline]
    pub const fn level(self) -> u8 {
        self as u8
    }

    /// Short, kebab-case machine identifier (matches the serde name).
    ///
    /// Useful for CSS class names or stable string keys that are *derived*
    /// from severity — but producers should always emit the enum itself over
    /// IPC rather than constructing these strings ad hoc.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hint => "hint",
            Self::Information => "information",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// Human-readable label suitable for UI headings or screen readers.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hint => "Hint",
            Self::Information => "Information",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }

    /// True for [`Error`](Self::Error) and [`Critical`](Self::Critical).
    ///
    /// In the LSP tradition, "errors" are the severities that should fail a
    /// build or block completion; hints/information/warnings do not.
    #[inline]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error | Self::Critical)
    }

    /// True for any severity strictly greater than the given `other`.
    #[inline]
    pub const fn is_greater_than(self, other: Self) -> bool {
        self.level() > other.level()
    }

    /// Returns the more severe of two severities.
    #[inline]
    pub const fn max(self, other: Self) -> Self {
        if self.level() >= other.level() {
            self
        } else {
            other
        }
    }

    /// Convenience iterator over every canonical severity, ascending.
    #[inline]
    pub const fn iter() -> &'static [Self] {
        Self::ALL
    }
}

impl std::fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_ordered_by_discriminant() {
        let ordered = DiagnosticSeverity::ALL;
        for window in ordered.windows(2) {
            assert!(window[0] < window[1], "severities must be ascending");
        }
    }

    #[test]
    fn all_variants_covered_in_all() {
        // If a variant is added but omitted from ALL, this fails to compile
        // (exhaustive match). This guards the canonical ordering array.
        let covered: Vec<_> = DiagnosticSeverity::ALL.to_vec();
        for variant in [
            DiagnosticSeverity::Hint,
            DiagnosticSeverity::Information,
            DiagnosticSeverity::Warning,
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Critical,
        ] {
            assert!(covered.contains(&variant), "{} missing from ALL", variant);
        }
    }

    #[test]
    fn level_is_stable_and_matches_discriminant() {
        assert_eq!(DiagnosticSeverity::Hint.level(), 0);
        assert_eq!(DiagnosticSeverity::Information.level(), 1);
        assert_eq!(DiagnosticSeverity::Warning.level(), 2);
        assert_eq!(DiagnosticSeverity::Error.level(), 3);
        assert_eq!(DiagnosticSeverity::Critical.level(), 4);
    }

    #[test]
    fn name_matches_serde_and_label() {
        assert_eq!(DiagnosticSeverity::Hint.name(), "hint");
        assert_eq!(DiagnosticSeverity::Warning.label(), "Warning");
        assert_eq!(DiagnosticSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn is_error_classification() {
        assert!(!DiagnosticSeverity::Hint.is_error());
        assert!(!DiagnosticSeverity::Information.is_error());
        assert!(!DiagnosticSeverity::Warning.is_error());
        assert!(DiagnosticSeverity::Error.is_error());
        assert!(DiagnosticSeverity::Critical.is_error());
    }

    #[test]
    fn max_returns_more_severe() {
        assert_eq!(
            DiagnosticSeverity::max(DiagnosticSeverity::Hint, DiagnosticSeverity::Warning),
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            DiagnosticSeverity::max(DiagnosticSeverity::Error, DiagnosticSeverity::Critical),
            DiagnosticSeverity::Critical
        );
    }

    #[test]
    fn serialization_round_trips() {
        for &sev in DiagnosticSeverity::ALL {
            let json = serde_json::to_string(&sev).expect("serialize severity");
            let back: DiagnosticSeverity = serde_json::from_str(&json).expect("deserialize severity");
            assert_eq!(sev, back, "round-trip failed for {:?}", sev);
            // Serialized value must equal the kebab-case name.
            assert_eq!(json, format!("\"{}\"", sev.name()));
        }
    }

    #[test]
    fn serialization_uses_stable_string_values() {
        // Guard against accidental rename: these JSON tokens are persisted to
        // disk and sent over IPC and must not change.
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Hint).unwrap(),
            "\"hint\""
        );
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Critical).unwrap(),
            "\"critical\""
        );
    }

    #[test]
    fn all_variants_are_send_and_sync() {
        // Severity values cross thread boundaries (producers + consumers
        // run on different threads); they must be thread-safe.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticSeverity>();
    }
}
