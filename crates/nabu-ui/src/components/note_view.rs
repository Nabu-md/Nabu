use leptos::*;
use nabu_core::parser::parse_markdown_to_html;

#[component]
pub fn NoteView(content: String) -> impl IntoView {
    let html = parse_markdown_to_html(&content);
    view! {
        <div class="note-view" inner_html=html></div>
    }
}
