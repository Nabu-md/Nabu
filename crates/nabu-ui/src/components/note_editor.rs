use leptos::prelude::*;
use crate::components::editor::slash_menu::SlashMenu;
use crate::components::note_view::NoteView;

#[component]
pub fn NoteEditor(initial_content: String) -> impl IntoView {
    let (content, set_content) = signal(initial_content);
    
    let (show_menu, set_show_menu) = signal(false);
    
    view! {
        <div class="note-editor relative" on:keydown=move |ev| if ev.key() == "/" { set_show_menu.set(true) }>
            <textarea 
                prop:value=content
                on:input=move |ev| set_content.set(event_target_value(&ev))
                class="editor-textarea"
            />
            {move || if show_menu.get() {
                view! { <SlashMenu on_select=Callback::new(move |_item| { set_show_menu.set(false); /* Insert item logic */ }) /> }.into_any()
            } else {
                view! {}.into_any()
            }}
            <NoteView content=content />
        </div>
    }
}
