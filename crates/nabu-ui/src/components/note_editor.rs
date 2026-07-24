use leptos::prelude::*;
use crate::components::note_view::NoteView;

#[component]
pub fn NoteEditor(initial_content: String) -> impl IntoView {
    let (content, set_content) = signal(initial_content);
    
    view! {
        <div class="note-editor">
            <textarea 
                prop:value=content
                on:input=move |ev| set_content.set(event_target_value(&ev))
                class="editor-textarea"
            />
            <NoteView content=content />
        </div>
    }
}
