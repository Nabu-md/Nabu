use wasm_bindgen_futures::spawn_local;

use leptos::prelude::*;

#[component]
pub fn DictationPill() -> impl IntoView {
    let (scratchpad, set_scratchpad) = signal(String::new());

    view! {
        <div class="dictation-pill" data-tauri-drag-region style="background: rgba(0,0,0,0.8); color: white; padding: 10px; border-radius: 20px;">
            <button on:click=move |_| {
                // Trigger dictation start via IPC
                spawn_local(async move {
                    let _ = crate::ipc::tauri_invoke("start_dictation", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
                });
            }>
                "Record"
            </button>
            <textarea 
                prop:value=scratchpad
                on:input=move |ev| set_scratchpad.set(event_target_value(&ev))
                placeholder="Scratchpad..."
                style="background: transparent; color: white; border: none; width: 100%;"
            />
        </div>
    }
}
