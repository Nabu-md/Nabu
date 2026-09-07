//! Harper grammar checking (Apache-2.0, offline, in-process).

use std::sync::Arc;

use harper_core::linting::{LintGroup, Linter, Suggestion};
use harper_core::spell::FstDictionary;
use harper_core::{DictWordMetadata, Dialect, Document};
use serde::Serialize;

/// A single grammar finding surfaced to the frontend. Offsets are Unicode char
/// indices so they map directly onto JS string indices.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarIssue {
    pub message: String,
    pub suggestions: Vec<String>,
    pub start: usize,
    pub end: usize,
    pub severity: &'static str,
}

fn to_issue(lint: &harper_core::linting::Lint, chars_len: usize) -> GrammarIssue {
    let span = lint.span;
    let suggestions: Vec<String> = lint
        .suggestions
        .iter()
        .filter_map(|s| match s {
            Suggestion::ReplaceWith(chars) => Some(chars.iter().collect::<String>()),
            Suggestion::Remove => Some(String::new()),
            _ => None,
        })
        .collect();
    let severity = if suggestions.is_empty() {
        "info"
    } else {
        "warning"
    };
    let start = span.start.min(chars_len);
    let end = span.end.min(chars_len).max(start);
    GrammarIssue {
        message: lint.message.clone(),
        suggestions,
        start,
        end,
        severity,
    }
}

/// Check text with Harper and return findings.
#[tauri::command]
pub fn grammar_check(text: String, extra_words: Vec<String>) -> Result<Vec<GrammarIssue>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let curated = FstDictionary::curated();
    let dictionary: Arc<FstDictionary> = if extra_words.is_empty() {
        Arc::clone(&curated)
    } else {
        let mut owned = harper_core::spell::MutableDictionary::new();
        for word in &extra_words {
            owned.append_word_str(&word.to_lowercase(), DictWordMetadata::default());
        }
        Arc::new(FstDictionary::from(owned))
    };
    let mut linter = LintGroup::new_curated(Arc::clone(&dictionary), Dialect::American);
    let doc = Document::new_markdown_default(&text, dictionary.as_ref());
    let mut lints = linter.lint(&doc);
    lints.sort_by_key(|l| l.span.start);
    Ok(lints.iter().map(|l| to_issue(l, text.chars().count())).collect())
}

/// Return just the issue count for quick UI badge updates.
#[tauri::command]
pub fn grammar_issue_count(text: String, extra_words: Vec<String>) -> Result<usize, String> {
    Ok(grammar_check(text, extra_words)?.len())
}
