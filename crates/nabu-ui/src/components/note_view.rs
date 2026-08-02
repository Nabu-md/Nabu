use leptos::prelude::*;

/// Read-only view of a note's content.
///
/// Renders the raw markdown source. A markdown renderer was originally
/// referenced here (`nabu_core::parser::parse_markdown_to_html`), but no such
/// module exists in nabu-core, so the view renders the source text directly
/// until a renderer is introduced.
#[component]
pub fn NoteView(content: ReadSignal<String>) -> impl IntoView {
    view! {
        <div class="note-view whitespace-pre-wrap text-gray-300 text-sm leading-relaxed">
            {content.get()}
        </div>
    }
}
