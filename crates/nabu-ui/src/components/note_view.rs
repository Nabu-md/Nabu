use leptos::prelude::*;
use nabu_core::parser::parse_markdown_to_html;

#[component]
pub fn NoteView(content: ReadSignal<String>) -> impl IntoView {
    let html = move || parse_markdown_to_html(&content.get());
    view! {
        <div class="note-view" inner_html=html()></div>
    }
}
