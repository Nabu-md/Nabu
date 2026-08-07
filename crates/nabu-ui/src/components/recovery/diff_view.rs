//! # Side-by-side diff viewer (Dioxus migration)
//!
//! Renders [`DiffRow`]s returned by the backend `versions_diff` command as a
//! two-column comparison.
//!
//! Changes: `leptos::prelude::*` → `dioxus::prelude::*`, `view!` → `rsx!`,
//! `impl IntoView` → `Element`, `.into_any()` removed, `collect_view()` → `for`.

use dioxus::prelude::*;
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
pub fn DiffView(rows: Vec<DiffRow>, old_label: String, new_label: String) -> Element {
    rsx! {
        div { class: "diff-view" }
        div { class: "diff-headers" }
        div { class: "diff-header diff-header-old", "{old_label}" }
        div { class: "diff-header diff-header-new", "{new_label}" }

        div { class: "diff-body" }
        for row in rows {
            {
                let kind = row.kind;
                let old_line = row.old_line.map(|l| l.to_string()).unwrap_or_default();
                let new_line = row.new_line.map(|l| l.to_string()).unwrap_or_default();
                let display = if row.text.is_empty() { " " } else { &row.text };
                let text_old = display;
                let text_new = display;
                let (old_class, new_class, tag) = match kind {
                    DiffKind::Same => ("", "", ""),
                    DiffKind::Added => ("diff-cell dim", "diff-cell diff-added", "+"),
                    DiffKind::Removed => ("diff-cell diff-removed", "diff-cell dim", "−"),
                };
                let old_class = old_class;
                let new_class = new_class;
                let mark_old = if kind == DiffKind::Added { "" } else { tag };
                let mark_new = if kind == DiffKind::Removed { "" } else { tag };
                rsx! {
                    div { class: "diff-row" }
                    div { class: {format!("diff-cell diff-old {}", old_class)} }
                    span { class: "diff-lineno", "{old_line}" }
                    span { class: "diff-mark", "aria-hidden": "true", "{mark_old}" }
                    span { class: "diff-text", "{text_old}" }

                    div { class: {format!("diff-cell diff-new {}", new_class)} }
                    span { class: "diff-lineno", "{new_line}" }
                    span { class: "diff-mark", "aria-hidden": "true", "{mark_new}" }
                    span { class: "diff-text", "{text_new}" }
                }
            }
        }
    }
}
