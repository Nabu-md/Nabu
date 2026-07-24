use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;
use web_sys::DragEvent;

#[component]
pub fn DictationPill() -> impl IntoView {
    let (scratchpad, set_scratchpad) = signal(String::new());
    let (mode, set_mode) = signal("dictation".to_string());
    let (opacity, set_opacity) = signal(0.8_f32);

    // Load settings for opacity
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({"key": "floating_pill_opacity"})).unwrap();
        let result = crate::ipc::tauri_invoke("settings_get", args).await;
        if let Ok(op) = serde_wasm_bindgen::from_value::<f32>(result) {
            set_opacity.set(op);
        }
    });

    view! {
        <div class="dictation-pill" 
             data-tauri-drag-region 
             on:mouseenter=move |_| set_opacity.set(1.0)
             on:mouseleave=move |_| {
                 set_opacity.set(0.8);
             }
             on:drop=move |ev: DragEvent| {
                ev.prevent_default();
                if let Some(files) = ev.data_transfer().unwrap().files() {
                    let mut file_paths = Vec::new();
                    for i in 0..files.length() {
                        if let Some(file) = files.item(i) {
                            file_paths.push(file.name());
                        }
                    }
                    spawn_local(async move {
                        let args = serde_wasm_bindgen::to_value(&serde_json::json!({"paths": file_paths})).unwrap();
                        let _ = crate::ipc::tauri_invoke("stage_files", args).await;
                    });
                }
             }
             style=move || format!("background: rgba(0,0,0,{}); color: white; padding: 10px; border-radius: 20px; opacity: {}; transition: opacity 0.2s;", opacity.get(), opacity.get())
             on:drop=move |ev: DragEvent| {
                ev.prevent_default();
                let files = ev.data_transfer().unwrap().files().unwrap();
                // Process files...
             }
             on:dragover=move |ev: DragEvent| {
                ev.prevent_default();
             }
        >
            <div class="mode-selector">
                <button on:click=move |_| set_mode.set("dictation".to_string())>"Dictation"</button>
                <button on:click=move |_| set_mode.set("scratchpad".to_string())>"Scratchpad"</button>
                <button on:click=move |_| set_mode.set("drop".to_string())>"Drop Zone"</button>
            </div>
            
            {move || match mode.get().as_str() {
                "dictation" => view! {
                    <button on:click=move |_| {
                        spawn_local(async move {
                            let _ = crate::ipc::tauri_invoke("start_dictation", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
                        });
                    }>
                        "Record"
                    </button>
                }.into_view(),
                "scratchpad" => view! {
                    <textarea 
                        prop:value=scratchpad
                        on:input=move |ev| set_scratchpad.set(event_target_value(&ev))
                        placeholder="Scratchpad..."
                        style="background: transparent; color: white; border: none; width: 100%;"
                    />
                }.into_view(),
                "drop" => view! {
                    <div style="border: 2px dashed white; padding: 20px;">"Drop Files Here"</div>
                }.into_view(),
                _ => view! {}.into_view(),
            }}
        </div>
    }
}
