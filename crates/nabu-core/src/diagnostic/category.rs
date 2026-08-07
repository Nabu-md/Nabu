//! # Diagnostic Category Classification
//!
//! Strongly-typed classification of *what kind* of issue a diagnostic
//! represents, independent of its
//! [`severity`](super::severity::DiagnosticSeverity).
//!
//! ## Purpose
//!
//! While [`DiagnosticSeverity`](super::severity::DiagnosticSeverity) answers
//! *how bad* something is, [`DiagnosticCategory`] answers *what kind* of thing
//! it is — e.g. a spelling error, a grammar issue, a syntax problem, or a
//! metadata validation failure.
//!
//! Producers should choose the most specific applicable category; consumers
//! use `category` for filtering, routing, and UI grouping (e.g. "show only
//! spell-check diagnostics" or "route OCR diagnostics to the Vision tab").
//!
//! ## Intended Producers
//!
//! | Producer              | Likely category                |
//! |-----------------------|--------------------------------|
//! | Markdown parsers      | `Syntax`, `Formatting`         |
//! | Spell checkers        | `SpellCheck`                   |
//! | Grammar engines       | `Grammar`                      |
//! | AI assistants         | `Ai`                           |
//! | Plugins               | `Plugin` (or their domain)     |
//! | LSP adapters          | `Syntax`, `Semantic`           |
//! | OCR engines           | `Ocr`                          |
//! | Metadata validators   | `Metadata`                     |
//! | Linting rules         | `Linting`                      |
//! | Security scanners     | `Security`                     |
//! | Performance analyzers | `Performance`                  |
//! | Accessibility audits  | `Accessibility`                |
//!
//! ## Extension
//!
//! The enum is `#[non_exhaustive]`: matching downstream code must include a
//! `_` arm. Producers that need a category not covered here use
//! [`DiagnosticCategory::Custom`] with a stable, reverse-DNS-style string
//! (e.g. `"com.example.rules"`).
//!
//! ## Serialization
//!
//! Standard variants serialize to stable kebab-case tokens (`"syntax"`,
//! `"spell-check"`, …). The [`Custom`] variant serializes as an
//! internally-tagged object: `{"custom":"com.example.rules"}`. This is the
//! same pattern used by [`crate::models::RelationType::Custom`].

use serde::{Deserialize, Serialize};

/// Canonical categories classifying the *domain* of a diagnostic.
///
/// Categories describe **what kind** of issue a diagnostic represents
/// (spelling, grammar, syntax, metadata, …), independent of its
/// [`severity`](super::severity::DiagnosticSeverity).
///
/// Choose the most specific applicable category; consumers use `category`
/// for filtering and routing. The enum is `#[non_exhaustive]` so new
/// categories can be added without breaking existing matchers (which must
/// include a `_` arm). For categories not covered here, use
/// [`DiagnosticCategory::Custom`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticCategory {
    /// Syntax or parse errors — the content does not conform to its grammar.
    #[serde(rename = "syntax")]
    Syntax,
    /// Spelling errors detected by a spell checker.
    #[serde(rename = "spell-check")]
    SpellCheck,
    /// Grammar or style issues detected by a grammar engine.
    #[serde(rename = "grammar")]
    Grammar,
    /// Metadata validation failures — missing or invalid frontmatter, tags,
    /// etc.
    #[serde(rename = "metadata")]
    Metadata,
    /// Semantic analysis issues — valid syntax but incorrect meaning
    /// (undefined references, type mismatches, broken links, etc.).
    #[serde(rename = "semantic")]
    Semantic,
    /// Diagnostics produced by an OCR processor (recognition confidence,
    /// layout issues, etc.).
    #[serde(rename = "ocr")]
    Ocr,
    /// Diagnostics produced by an AI assistant.
    #[serde(rename = "ai")]
    Ai,
    /// Diagnostics produced by a plugin.
    #[serde(rename = "plugin")]
    Plugin,
    /// Linting rule violations.
    #[serde(rename = "linting")]
    Linting,
    /// Formatting inconsistencies or style violations.
    #[serde(rename = "formatting")]
    Formatting,
    /// Security-related findings.
    #[serde(rename = "security")]
    Security,
    /// Performance-related findings.
    #[serde(rename = "performance")]
    Performance,
    /// Accessibility-related findings.
    #[serde(rename = "accessibility")]
    Accessibility,
    /// A producer-defined category. The inner string should be a stable,
    /// reverse-DNS-style identifier (e.g. `"com.example.rules"`).
    #[serde(rename = "custom")]
    Custom(String),
}

impl DiagnosticCategory {
    /// Short, kebab-case machine identifier matching the serde token.
    ///
    /// For the [`Custom`] variant this returns `"custom"` — use
    /// [`as_str`](Self::as_str) to obtain the inner string, or
    /// [`Display`](Self) for a human-readable form.
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::SpellCheck => "spell-check",
            Self::Grammar => "grammar",
            Self::Metadata => "metadata",
            Self::Semantic => "semantic",
            Self::Ocr => "ocr",
            Self::Ai => "ai",
            Self::Plugin => "plugin",
            Self::Linting => "linting",
            Self::Formatting => "formatting",
            Self::Security => "security",
            Self::Performance => "performance",
            Self::Accessibility => "accessibility",
            // `Custom`'s inner string is dynamic; return the fixed tag.
            Self::Custom(_) => "custom",
        }
    }

    /// Returns the inner identifier for the [`Custom`] variant, or the
    /// kebab-case name for standard variants.
    #[inline]
    pub fn as_str(&self) -> String {
        match self {
            Self::Syntax => "syntax".to_string(),
            Self::SpellCheck => "spell-check".to_string(),
            Self::Grammar => "grammar".to_string(),
            Self::Metadata => "metadata".to_string(),
            Self::Semantic => "semantic".to_string(),
            Self::Ocr => "ocr".to_string(),
            Self::Ai => "ai".to_string(),
            Self::Plugin => "plugin".to_string(),
            Self::Linting => "linting".to_string(),
            Self::Formatting => "formatting".to_string(),
            Self::Security => "security".to_string(),
            Self::Performance => "performance".to_string(),
            Self::Accessibility => "accessibility".to_string(),
            Self::Custom(s) => s.clone(),
        }
    }

    /// Human-readable label suitable for UI headings.
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Syntax => "Syntax",
            Self::SpellCheck => "Spell Check",
            Self::Grammar => "Grammar",
            Self::Metadata => "Metadata",
            Self::Semantic => "Semantic",
            Self::Ocr => "OCR",
            Self::Ai => "AI",
            Self::Plugin => "Plugin",
            Self::Linting => "Linting",
            Self::Formatting => "Formatting",
            Self::Security => "Security",
            Self::Performance => "Performance",
            Self::Accessibility => "Accessibility",
            Self::Custom(_) => "Custom",
        }
    }
}

impl std::fmt::Display for DiagnosticCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // For Custom, show the inner string; for standard variants, use the
        // human-readable label.
        match self {
            Self::Custom(s) => write!(f, "{}", s),
            other => f.write_str(other.label()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_matches_serde_token() {
        assert_eq!(DiagnosticCategory::Syntax.name(), "syntax");
        assert_eq!(DiagnosticCategory::SpellCheck.name(), "spell-check");
        assert_eq!(DiagnosticCategory::Grammar.name(), "grammar");
        assert_eq!(DiagnosticCategory::Custom("x".into()).name(), "custom");
    }

    #[test]
    fn label_is_human_readable() {
        assert_eq!(DiagnosticCategory::SpellCheck.label(), "Spell Check");
        assert_eq!(DiagnosticCategory::Ai.label(), "AI");
        assert_eq!(DiagnosticCategory::Ocr.label(), "OCR");
    }

    #[test]
    fn custom_category_display_shows_inner() {
        let cat = DiagnosticCategory::Custom("com.example.rule-x".into());
        assert_eq!(cat.name(), "custom");
        assert_eq!(cat.to_string(), "com.example.rule-x");
        assert_eq!(cat.as_str(), "com.example.rule-x");
    }

    #[test]
    fn serialization_round_trips_standard_variants() {
        for variant in [
            DiagnosticCategory::Syntax,
            DiagnosticCategory::SpellCheck,
            DiagnosticCategory::Grammar,
            DiagnosticCategory::Metadata,
            DiagnosticCategory::Semantic,
            DiagnosticCategory::Ocr,
            DiagnosticCategory::Ai,
            DiagnosticCategory::Plugin,
            DiagnosticCategory::Linting,
            DiagnosticCategory::Formatting,
            DiagnosticCategory::Security,
            DiagnosticCategory::Performance,
            DiagnosticCategory::Accessibility,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: DiagnosticCategory =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip failed for {:?}", variant);
        }
    }

    #[test]
    fn custom_variant_serializes_as_tagged_string() {
        let cat = DiagnosticCategory::Custom("com.example.rules".into());
        let json = serde_json::to_string(&cat).unwrap();
        // Internally-tagged: {"custom":"com.example.rules"}
        assert_eq!(json, r#"{"custom":"com.example.rules"}"#);

        let back: DiagnosticCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, back);
    }

    #[test]
    fn category_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DiagnosticCategory>();
    }

    #[test]
    fn is_non_exhaustive_via_underscore_arm() {
        // Ensures downstream matchers can use a `_` arm without breaking.
        let cat = DiagnosticCategory::Syntax;
        let _name = match cat {
            DiagnosticCategory::Syntax => "syntax",
            DiagnosticCategory::SpellCheck => "spell-check",
            DiagnosticCategory::Grammar => "grammar",
            DiagnosticCategory::Metadata => "metadata",
            DiagnosticCategory::Semantic => "semantic",
            DiagnosticCategory::Ocr => "ocr",
            DiagnosticCategory::Ai => "ai",
            DiagnosticCategory::Plugin => "plugin",
            DiagnosticCategory::Linting => "linting",
            DiagnosticCategory::Formatting => "formatting",
            DiagnosticCategory::Security => "security",
            DiagnosticCategory::Performance => "performance",
            DiagnosticCategory::Accessibility => "accessibility",
            DiagnosticCategory::Custom(_) => "custom",
            _ => unreachable!(),
        };
        assert_eq!(_name, "syntax");
    }

    #[test]
    fn as_str_returns_inner_for_custom() {
        let cat = DiagnosticCategory::Custom("my-cat".into());
        assert_eq!(cat.as_str(), "my-cat");
        assert_eq!(DiagnosticCategory::Grammar.as_str(), "grammar");
    }
}
