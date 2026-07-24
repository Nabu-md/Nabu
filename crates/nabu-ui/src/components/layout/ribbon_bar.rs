use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;

#[component]
pub fn RibbonBar() -> impl IntoView {
    let (enabled, set_enabled) = signal(false);
    
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({"key": "enable_daily_notes"})).unwrap();
                    let result = crate::ipc::tauri_invoke("get_settings", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
        if let Ok(val) = serde_wasm_bindgen::from_value::<bool>(result) {
            set_enabled.set(val);
        }
    });

    view! {
        <div class="w-12 h-screen border-r border-gray-700 bg-gray-900 flex flex-col items-center py-4 space-y-4">
            <button title="Vault Explorer" on:click=move |_| println!("Toggle Vault Explorer")>"📁"</button>
            <button title="Global Search" on:click=move |_| println!("Trigger Search")>"🔍"</button>
            <button title="Graph View" on:click=move |_| println!("Switch to Graph View")>"🕸️"</button>
            {move || if enabled.get() {
                view! {
                    <button title="Daily Note" on:click=move |_| {
                        spawn_local(async move {
                            let _ = crate::ipc::tauri_invoke("note_daily", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
                        });
                    }>"📅"</button>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
            <button title="Canvas" on:click=move |_| println!("Open Canvas")>"🎨"</button>
            <div class="flex-grow"></div>
            <button title="Settings" on:click=move |_| {
                spawn_local(async move {
                    let _ = crate::ipc::tauri_invoke("open_settings", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
                });
            }>"⚙️"</button>
        </div>
    }
}
