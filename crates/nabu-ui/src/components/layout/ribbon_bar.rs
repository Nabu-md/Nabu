use wasm_bindgen_futures::spawn_local;

use leptos::prelude::*;

#[component]
pub fn RibbonBar() -> impl IntoView {
    view! {
        <div class="w-12 h-screen border-r border-gray-700 bg-gray-900 flex flex-col items-center py-4 space-y-4">
            <button title="Vault Explorer">"📁"</button>
            <button title="Global Search">"🔍"</button>
            <button title="Graph View">"🕸️"</button>
            <button title="Daily Note">"📅"</button>
            <button title="Canvas">"🎨"</button>
            <div class="flex-grow"></div>
            <button title="Settings" on:click=move |_| {
                // Settings trigger (Tauri IPC)
                spawn_local(async move {
                    let _ = crate::ipc::tauri_invoke("open_settings", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
                });
            }>"⚙️"</button>
        </div>
    }
}
