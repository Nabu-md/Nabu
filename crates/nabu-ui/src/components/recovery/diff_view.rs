//! # Side-by-side diff viewer
//!
//! Renders [`DiffRow`]s returned by the backend `versions_diff` command as a
//! two-column comparison. Additions are highlighted green, removals red, and
//! unchanged lines neutral; line numbers are shown for both sides.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// One row of a diff (mirrors the backend `DiffRow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffRow {
    pub kind: DiffKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

/// Side-by-side diff viewer.
#[component]
pub fn DiffView(rows: Vec<DiffRow>, old_label: String, new_label: String) -> impl IntoView {
    view! {
        <div class="diff-view">
            <div class="diff-headers">
                <div class="diff-header diff-header-old">{old_label}</div>
                <div class="diff-header diff-header-new">{new_label}</div>
            </div>
            <div class="diff-body">
                {rows.into_iter().map(|row| {
                    let kind = row.kind;
                    let old_line = row.old_line.map(|l| l.to_string()).unwrap_or_default();
                    let new_line = row.new_line.map(|l| l.to_string()).unwrap_or_default();
                    let display = if row.text.is_empty() {
                        " ".to_string()
                    } else {
                        row.text.clone()
                    };
                    let text_old = display.clone();
                    let text_new = display;
                    let (old_class, new_class, tag) = match kind {
                        DiffKind::Same => ("", "", ""),
                        DiffKind::Added => ("diff-cell dim", "diff-cell diff-added", "+"),
                        DiffKind::Removed => ("diff-cell diff-removed", "diff-cell dim", "−"),
                    };
                    view! {
                        <div class="diff-row">
                            <div class=format!("diff-cell diff-old {old_class}")>
                                <span class="diff-lineno">{old_line}</span>
                                <span class="diff-mark" aria-hidden="true">{if kind == DiffKind::Added { "" } else { tag }}</span>
                                <span class="diff-text">{text_old}</span>
                            </div>
                            <div class=format!("diff-cell diff-new {new_class}")>
                                <span class="diff-lineno">{new_line}</span>
                                <span class="diff-mark" aria-hidden="true">{if kind == DiffKind::Removed { "" } else { tag }}</span>
                                <span class="diff-text">{text_new}</span>
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
