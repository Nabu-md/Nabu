//! # Harper → Diagnostic Conversion Module
//!
//! Maps [`harper_core::linting::Lint`] objects into Nabu's canonical
//! [`Diagnostic`] data model, producing standardized position ranges,
//! severity, category, and quick-fix suggestions.
//!
//! ## Design
//!
//! - Harper reports spans as `Span<char>` — character indices (not byte offsets)
//!   into the document's `[char]` representation. These are mapped to
//!   `(line, character)` `TextPosition` values by scanning the document text
//!   for newlines.
//! - `LintKind` variants are mapped to the closest
//!   [`DiagnosticCategory`] + [`DiagnosticSeverity`] pair.
//! - Harper `Suggestion` variants (`ReplaceWith`, `InsertAfter`, `Remove`) are
//!   translated into Nabu [`Suggestion`] quick-fix objects.
//! - All conversion is pure — no Harper types leak into `ProcessingResult` or
//!   the EventBus payload. Only `Diagnostic` (which is `Send + Sync`) is stored.
//!
//! ## Thread Safety
//!
//! This module is stateless: `convert_lint` and `char_index_to_position` are
//! pure functions with no shared mutable state. They can be called from any
//! thread context.

use crate::diagnostic::{
    Decoration, Diagnostic, DiagnosticCategory, DiagnosticError, DiagnosticSeverity,
    Suggestion, SuggestionApplicability, SuggestionPriority, TextRange, TextPosition,
};
use crate::diagnostic::style::DecorationCategory;
use harper_core::linting::{Lint, LintKind, Suggestion as HarperSuggestion};

/// Convert a single Harper [`Lint`] into a Nabu [`Diagnostic`].
///
/// Harper reports positions as `Span<char>` — character indices into the
/// document's `[char]` buffer. This function maps those indices to
/// `(line, character)` `TextPosition` values by scanning the document text.
///
/// Returns `DiagnosticError::HarperConversion` if the span cannot be resolved
/// against the document text (e.g. indices are out of bounds).
pub fn convert_lint(lint: &Lint, source_chars: &[char]) -> Result<Diagnostic, DiagnosticError> {
    let start_pos = char_index_to_position(lint.span.start, source_chars)?;
    let end_pos = char_index_to_position(lint.span.end, source_chars)?;

    let severity = lint_kind_to_severity(lint.lint_kind);
    let category = lint_kind_to_category(lint.lint_kind);

    let range = TextRange::try_new(start_pos, end_pos)?;

    let code = lint.lint_kind.to_string_key();

    let suggestions: Vec<Suggestion> = lint
        .suggestions
        .iter()
        .enumerate()
        .filter_map(|(i, suggestion)| {
            convert_suggestion(suggestion, range).ok().map(|mut s| {
                s.title = format!("Apply fix #{}", i + 1);
                s
            })
        })
        .collect();

    let mut diag = Diagnostic::try_new(severity, range, &lint.message)?
        .with_code(code)
        .with_source("harper")
        .with_category(category)
        .with_suggestions(suggestions);

    if lint.priority == 0 {
        diag = diag.with_decoration(Decoration {
            category: DecorationCategory::Background,
            range,
            style: None,
            tooltip: Some(format!("Harper priority: {}", lint.priority)),
        });
    }

    Ok(diag)
}

/// Convert a Harper [`LintKind`] to a Nabu [`DiagnosticSeverity`].
///
/// Priority mapping (Harper `priority` is `u8`, lower = more important):
/// - `Spelling` and `Typo` → `Error` (real problems, not stylistic)
/// - `Grammar`, `Agreement`, `BoundaryError`, `WordOrder`, `Malapropism`,
///   `Eggcorn`, `Nonstandard`, `Usage` → `Error`
/// - `Capitalization`, `Punctuation`, `Readability`, `Style` → `Warning`
/// - `Enhancement`, `Formatting` → `Information`
/// - `Redundancy`, `Repetition`, `Regionalism` → `Warning`
/// - `Miscellaneous`, `WordChoice` → `Warning`
fn lint_kind_to_severity(lint_kind: LintKind) -> DiagnosticSeverity {
    match lint_kind {
        LintKind::Spelling | LintKind::Typo => DiagnosticSeverity::Error,
        LintKind::Grammar
        | LintKind::Agreement
        | LintKind::BoundaryError
        | LintKind::WordOrder
        | LintKind::Malapropism
        | LintKind::Eggcorn
        | LintKind::Nonstandard
        | LintKind::Usage => DiagnosticSeverity::Error,
        LintKind::Capitalization
        | LintKind::Punctuation
        | LintKind::Readability
        | LintKind::Style
        | LintKind::Redundancy
        | LintKind::Repetition
        | LintKind::Regionalism
        | LintKind::Miscellaneous
        | LintKind::WordChoice => DiagnosticSeverity::Warning,
        LintKind::Enhancement | LintKind::Formatting => DiagnosticSeverity::Information,
    }
}

/// Map a Harper [`LintKind`] to the closest Nabu [`DiagnosticCategory`].
///
/// Harper's linter categories are more numerous than Nabu's domain categories;
/// we pick the closest semantic match. Spelling and Typo both map to
/// `SpellCheck`; grammar-family kinds map to `Grammar`; style/formatting
/// kinds map to `Formatting`.
fn lint_kind_to_category(lint_kind: LintKind) -> DiagnosticCategory {
    match lint_kind {
        LintKind::Spelling | LintKind::Typo => DiagnosticCategory::SpellCheck,
        LintKind::Grammar
        | LintKind::Agreement
        | LintKind::BoundaryError
        | LintKind::WordOrder
        | LintKind::Malapropism
        | LintKind::Eggcorn
        | LintKind::Nonstandard
        | LintKind::Usage
        | LintKind::Redundancy
        | LintKind::Repetition
        | LintKind::Readability
        | LintKind::WordChoice => DiagnosticCategory::Grammar,
        LintKind::Capitalization | LintKind::Punctuation => DiagnosticCategory::Formatting,
        LintKind::Formatting | LintKind::Style => DiagnosticCategory::Formatting,
        LintKind::Enhancement => DiagnosticCategory::Linting,
        LintKind::Miscellaneous | LintKind::Regionalism => DiagnosticCategory::Linting,
    }
}

/// Convert a Harper [`Suggestion`] into a Nabu [`Suggestion`] (quick-fix).
///
/// The range is the lint's span range (already resolved to positions).
/// - `ReplaceWith` → suggestion that replaces the span with the new text
/// - `InsertAfter` → suggestion that inserts text at the end of the span
/// - `Remove` → suggestion that replaces the span with empty text
fn convert_suggestion(
    harper_suggestion: &HarperSuggestion,
    range: TextRange,
) -> Result<Suggestion, DiagnosticError> {
    match harper_suggestion {
        HarperSuggestion::ReplaceWith(chars) => {
            let new_text: String = chars.iter().collect();
            Suggestion::try_new(
                "Replace with suggestion".to_string(),
                range,
                new_text,
            )
            .map(|s| {
                s.with_kind("replace")
                    .with_applicability(SuggestionApplicability::Always)
                    .with_priority(SuggestionPriority::High)
            })
        }
        HarperSuggestion::InsertAfter(chars) => {
            let new_text: String = chars.iter().collect();
            Suggestion::try_new(
                "Insert after".to_string(),
                TextRange::empty(range.end),
                new_text,
            )
            .map(|s| {
                s.with_kind("insert")
                    .with_applicability(SuggestionApplicability::Always)
                    .with_priority(SuggestionPriority::Normal)
            })
        }
        HarperSuggestion::Remove => {
            Suggestion::try_new(
                "Remove".to_string(),
                range,
                String::new(),
            )
            .map(|s| {
                s.with_kind("remove")
                    .with_applicability(SuggestionApplicability::Always)
                    .with_priority(SuggestionPriority::High)
            })
        }
    }
}

/// Convert a character index into a `(line, character)` `TextPosition`.
///
/// Character indices count UTF-8 characters (not bytes). The `character`
/// field counts UTF-16 code units per the LSP convention, but for ASCII
/// (the common case for Harper-produced text), character count == UTF-16
/// code-unit count. For non-ASCII text, we compute the exact UTF-16 offset
/// within the target line.
///
/// Returns `DiagnosticError::HarperConversion` if the index is beyond the
/// length of `source_chars`.
pub fn char_index_to_position(
    char_index: usize,
    source_chars: &[char],
) -> Result<TextPosition, DiagnosticError> {
    if char_index > source_chars.len() {
        return Err(DiagnosticError::harper_conversion(
            "char index out of bounds",
            char_index,
            source_chars.len(),
        ));
    }

    let mut line = 0u32;
    let mut line_start_char_index = 0usize;

    for (i, &c) in source_chars.iter().enumerate() {
        if i == char_index {
            let line_chars: String = source_chars[line_start_char_index..i]
                .iter()
                .collect();
            let character = utf8_str_to_utf16_units(&line_chars);
            return Ok(TextPosition::new(line, character as u32));
        }
        if c == '\n' {
            line += 1;
            line_start_char_index = i + 1;
        }
    }

    if char_index == source_chars.len() {
        let line_str: String = source_chars[line_start_char_index..]
            .iter()
            .collect();
        let character = utf8_str_to_utf16_units(&line_str);
        return Ok(TextPosition::new(line, character as u32));
    }

    // Unreachable: the initial guard `char_index > source_chars.len()` already
    // handles all out-of-bounds cases, and the loop + `char_index == len`
    // branch cover all valid indices. The compiler still wants a return
    // value, so we emit an error for the theoretically-impossible case.
    Err(DiagnosticError::harper_conversion(
        "unreachable: char index fell through all branches",
        char_index,
        source_chars.len(),
    ))
}

/// Count the number of UTF-16 code units in a UTF-8 string.
///
/// This is needed because `TextPosition.character` follows the LSP convention
/// of counting UTF-16 code units, while Harper operates on `char` (Unicode
/// scalar) indices. For ASCII text the count is the same, but characters
/// outside the BMP (represented as surrogate pairs in UTF-16) differ.
fn utf8_str_to_utf16_units(s: &str) -> usize {
    s.encode_utf16().count()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lint(
        start: usize,
        end: usize,
        kind: LintKind,
        message: &str,
    ) -> Lint {
        Lint {
            span: harper_core::Span::new(start, end),
            lint_kind: kind,
            suggestions: Vec::new(),
            message: message.to_string(),
            priority: 5,
        }
    }

    #[test]
    fn char_index_to_position_single_line() {
        let source: Vec<char> = "hello world".chars().collect();
        let pos = char_index_to_position(5, &source).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 5);
    }

    #[test]
    fn char_index_to_position_multi_line() {
        let text = "hello\nworld\nfoo";
        let source: Vec<char> = text.chars().collect();

        let pos0 = char_index_to_position(0, &source).unwrap();
        assert_eq!(pos0.line, 0);
        assert_eq!(pos0.character, 0);

        let pos6 = char_index_to_position(6, &source).unwrap();
        assert_eq!(pos6.line, 1);
        assert_eq!(pos6.character, 0);

        let pos11 = char_index_to_position(11, &source).unwrap();
        assert_eq!(pos11.line, 1);
        assert_eq!(pos11.character, 5);

        let pos12 = char_index_to_position(12, &source).unwrap();
        assert_eq!(pos12.line, 2);
        assert_eq!(pos12.character, 0);
    }

    #[test]
    fn char_index_to_position_at_end() {
        let source: Vec<char> = "abc".chars().collect();
        let pos = char_index_to_position(3, &source).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 3);
    }

    #[test]
    fn char_index_to_position_out_of_bounds() {
        let source: Vec<char> = "abc".chars().collect();
        let result = char_index_to_position(10, &source);
        assert!(result.is_err());
    }

    #[test]
    fn convert_spelling_lint() {
        let source: Vec<char> = "ths is a test".chars().collect();
        let lint = make_lint(0, 3, LintKind::Spelling, "Did you mean 'this'?");

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.category, Some(DiagnosticCategory::SpellCheck));
        assert_eq!(diag.source, Some("harper".to_string()));
        assert_eq!(diag.message, "Did you mean 'this'?");
        assert_eq!(diag.range.start.line, 0);
        assert_eq!(diag.range.start.character, 0);
        assert_eq!(diag.range.end.character, 3);
    }

    #[test]
    fn convert_grammar_lint() {
        let source: Vec<char> = "I has a dog".chars().collect();
        let lint = make_lint(2, 5, LintKind::Grammar, "Subject-verb disagreement");

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.category, Some(DiagnosticCategory::Grammar));
    }

    #[test]
    fn convert_enhancement_lint_severity() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(0, 1, LintKind::Enhancement, "Enhancement suggestion");

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.severity, DiagnosticSeverity::Information);
    }

    #[test]
    fn convert_suggestion_replace_with() {
        let source: Vec<char> = "I has a dog".chars().collect();
        let lint = Lint {
            span: harper_core::Span::new(2, 5),
            lint_kind: LintKind::Grammar,
            suggestions: vec![HarperSuggestion::ReplaceWith(vec!['h', 'a', 'v', 'e'])],
            message: "Use 'have' instead of 'has'".to_string(),
            priority: 5,
        };

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].new_text, "have");
        assert_eq!(diag.suggestions[0].range.start.character, 2);
        assert_eq!(diag.suggestions[0].range.end.character, 5);
    }

    #[test]
    fn convert_suggestion_insert_after() {
        let source: Vec<char> = "hello world".chars().collect();
        let lint = Lint {
            span: harper_core::Span::new(5, 5),
            lint_kind: LintKind::Punctuation,
            suggestions: vec![HarperSuggestion::InsertAfter(vec![','])],
            message: "Add a comma here".to_string(),
            priority: 5,
        };

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].new_text, ",");
        assert_eq!(diag.suggestions[0].range.start.character, 5);
        assert!(diag.suggestions[0].range.is_empty());
    }

    #[test]
    fn convert_suggestion_remove() {
        let source: Vec<char> = "hello world".chars().collect();
        let lint = Lint {
            span: harper_core::Span::new(0, 5),
            lint_kind: LintKind::Redundancy,
            suggestions: vec![HarperSuggestion::Remove],
            message: "Remove redundant text".to_string(),
            priority: 5,
        };

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].new_text, "");
    }

    #[test]
    fn convert_empty_suggestions() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(0, 2, LintKind::Typo, "Typo found");

        let diag = convert_lint(&lint, &source).unwrap();
        assert!(diag.suggestions.is_empty());
    }

    #[test]
    fn convert_multi_line_lint_positions() {
        let text = "line one\ntwo\nthree four";
        let source: Vec<char> = text.chars().collect();

        let start = text.find("three").unwrap();
        let end = start + 5;
        let lint = make_lint(start, end, LintKind::Spelling, "Check this word");

        let diag = convert_lint(&lint, &source).unwrap();
        assert_eq!(diag.range.start.line, 2);
        assert_eq!(diag.range.end.line, 2);
    }

    #[test]
    fn convert_lint_sets_code_from_lint_kind() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(0, 2, LintKind::Spelling, "misspelling");

        let diag = convert_lint(&lint, &source).unwrap();
        assert!(diag.code.is_some());
        assert_eq!(diag.code.as_deref(), Some(lint.lint_kind.to_string_key().as_str()));
    }

    #[test]
    fn convert_lint_invalid_range_rejected() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(3, 0, LintKind::Spelling, "inverted span");

        let result = convert_lint(&lint, &source);
        assert!(result.is_err());
    }

    #[test]
    fn convert_lint_empty_message_rejected() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(0, 2, LintKind::Spelling, "");

        let result = convert_lint(&lint, &source);
        assert!(result.is_err());
    }

    #[test]
    fn utf8_str_to_utf16_units_ascii() {
        assert_eq!(utf8_str_to_utf16_units("hello"), 5);
        assert_eq!(utf8_str_to_utf16_units(""), 0);
    }

    #[test]
    fn utf8_str_to_utf16_units_bmp() {
        assert_eq!(utf8_str_to_utf16_units("héllo"), 5);
    }

    #[test]
    fn utf8_str_to_utf16_units_emoji() {
        assert_eq!(utf8_str_to_utf16_units("a😀b"), 4);
    }

    #[test]
    fn all_converted_diagnostics_are_send_sync() {
        let source: Vec<char> = "text".chars().collect();
        let lint = make_lint(0, 2, LintKind::Grammar, "test");
        let diag = convert_lint(&lint, &source).unwrap();
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Diagnostic>();
        // Verify the diagnostic we produced is Send + Sync
        let _: &dyn Send = &diag;
        let _: &dyn Sync = &diag;
    }
}
