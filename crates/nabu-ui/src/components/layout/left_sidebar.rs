use wasm_bindgen_futures::spawn_local;

use leptos::prelude::*;

#[component]
pub fn LeftSidebar() -> impl IntoView {
    view! {
        <div class="w-64 border-r border-gray-700 bg-gray-900 h-screen flex flex-col">
                <button class="text-xs p-1 bg-gray-700 rounded" on:click=move |_| {
                    spawn_local(async move {
                        let _ = crate::ipc::tauri_invoke("note_create_file", serde_wasm_bindgen::to_value(&serde_json::json!({"path": "new_note.md"})).unwrap()).await;
                    });
                }>"+ Note"</button>
            <div class="flex-1 p-2 overflow-y-auto">
                <div class="cursor-pointer text-sm text-gray-300">"▼ Notes"</div>
                <div class="ml-4 cursor-pointer text-sm text-primary bg-primary/20 p-1 rounded">"My First Note.md"</div>
                <div class="ml-4 cursor-pointer text-sm text-gray-300 p-1">"Another Note.md"</div>
                <div class="cursor-pointer text-sm text-gray-300 mt-2">"▶ Archive"</div>
            </div>
        </div>
    }
}
