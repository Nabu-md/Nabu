use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn SettingsPanel() -> impl IntoView {
    let (settings, set_settings) = signal(serde_json::json!({}));
    let (active_tab, set_active_tab) = signal("General".to_string());
    
    spawn_local(async move {
        let res = crate::ipc::tauri_invoke("get_settings", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
        if let Ok(s) = serde_wasm_bindgen::from_value::<serde_json::Value>(res) {
            set_settings.set(s);
        }
    });

    let save_settings = move |new_settings: serde_json::Value| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&new_settings).unwrap();
            let _ = crate::ipc::tauri_invoke("settings_set_all", args).await;
        });
    };

    view! {
        <div class="flex h-full bg-gray-900 text-white">
            <div class="w-48 border-r border-gray-700 p-4 space-y-2">
                {["General", "Editor", "Whispr", "Appearance", "Files", "Graph"].iter().map(|&tab| {
                    let tab = tab.to_string();
                    view! {
                        <button 
                            class=move || format!("w-full p-2 text-left rounded {}", if active_tab.get() == tab { "bg-gray-800" } else { "" })
                            on:click=move |_| set_active_tab.set(tab.clone())
                        >
                            {tab}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="flex-1 p-6">
                <h1 class="text-xl font-bold mb-4">{move || active_tab.get()}</h1>
                <div class="space-y-4">
                    <label class="flex items-center">
                        <input type="checkbox" checked=move || settings.get()["enable_daily_notes"].as_bool().unwrap_or(false)
                            on:change=move |ev| {
                                let mut s = settings.get();
                                s["enable_daily_notes"] = serde_json::json!(event_target_checked(&ev));
                                set_settings.set(s.clone());
                                save_settings(s);
                            }
                        />
                        <span class="ml-2">"Enable Daily Notes"</span>
                    </label>
                </div>
            </div>
        </div>
    }
}
