//! # Note View — rendered markdown presentation (Dioxus)
//!
//! Read-only view of a note's content. Renders the raw markdown source text
//! with `whitespace-pre-wrap` so formatting is visible without a markdown-to-HTML
//! renderer. A full markdown rendering pipeline is a future enhancement; this
//! preserves the existing LePtOS behaviour exactly.

use dioxus::prelude::*;

/// Read-only view of a note's content.
///
/// Renders the raw markdown source. A markdown renderer was originally
/// referenced here (`nabu_core::parser::parse_markdown_to_html`), but no such
/// module exists in nabu-core, so the view renders the source text directly
/// until a renderer is introduced.
#[component]
pub fn NoteView(content: Signal<String>) -> Element {
    rsx! {
        div {
            class: "note-view whitespace-pre-wrap text-gray-300 text-sm leading-relaxed",
            "{content.read()}"
        }
    }
}
