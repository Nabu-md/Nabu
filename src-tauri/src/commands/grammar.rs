//! Harper grammar checking (Apache-2.0, offline, in-process).

use harper_core::linting::{Lint, LintGroup, Linter, Suggestion};
use harper_core::FstDictionary;
use serde::Serialize;

/// A single grammar finding surfaced to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarIssue {
    pub message: String,
    pub suggestions: Vec<String>,
    pub start: usize,
    pub end: usize,
    pub severity: &'static str,
}

fn to_issue(lint: &Lint, chars: &[char]) -> GrammarIssue {
    let span = lint.span;
    let suggestions: Vec<String> = lint
        .suggestions
        .iter()
        .filter_map(|s| match s {
            Suggestion::ReplaceWith(cs) => Some(cs.to_string()),
            _ => None,
        })
        .collect();
    let severity = if suggestions.is_empty() {
        "info"
    } else {
        "warning"
    };
    let end = span.end.min(chars.len());
    let start = span.start.min(end);
    let message: String = chars[start..end].iter().collect();
    GrammarIssue {
        message,
        suggestions,
        start,
        end,
        severity,
    }
}

/// Check text with Harper and return findings. Offsets are Unicode char indices
/// so the frontend can map them directly onto JS string indices.
#[tauri::command]
pub fn grammar_check(text: String, extra_words: Vec<String>) -> Result<Vec<GrammarIssue>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let dict = FstDictionary::curated();
    if !extra_words.is_empty() {
        let owned = dict.clone();
        for word in &extra_words {
            let _ = owned.extend_lexicon(&[word.to_lowercase().as_str().into()]);
        }
        let linter = LintGroup::new(owned);
        return run_lints(&linter, &text);
    }
    let linter = LintGroup::new(dict);
    run_lints(&linter, &text)
}

fn run_lints(linter: &LintGroup<FstDictionary>, text: &str) -> Result<Vec<GrammarIssue>, String> {
    let doc = harper_core::Document::new_markdown_default(text);
    let mut lints = linter.lint(&doc);
    lints.sort_by_key(|l| l.span.start);
    let chars: Vec<char> = text.chars().collect();
    Ok(lints.iter().map(|l| to_issue(l, &chars)).collect())
}

/// Return just the issue count for quick UI badge updates.
#[tauri::command]
pub fn grammar_issue_count(text: String, extra_words: Vec<String>) -> Result<usize, String> {
    Ok(grammar_check(text, extra_words)?.len())
}
