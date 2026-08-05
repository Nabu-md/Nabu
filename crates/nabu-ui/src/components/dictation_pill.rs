use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::selection::{Segmented, SegmentedOption};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::DragEvent;

#[component]
pub fn DictationPill() -> impl IntoView {
    let (scratchpad, set_scratchpad) = signal(String::new());
    let mode = RwSignal::new("dictation".to_string());
    let (opacity, set_opacity) = signal(0.8_f32);
    let (clipboard_cache, set_clipboard_cache) = signal(Vec::<String>::new());
    let toasts = crate::components::ui::feedback::use_toast();

    // Load settings for opacity
    spawn_local(async move {
        let args =
            serde_wasm_bindgen::to_value(&serde_json::json!({"key": "floating_pill_opacity"}))
                .unwrap();
        let result = crate::ipc::tauri_invoke("settings_get", args).await;
        if let Ok(op) = serde_wasm_bindgen::from_value::<f32>(result) {
            set_opacity.set(op);
        }
    });

    let (is_dictating, set_is_dictating) = signal(false);
    let (is_dragging, set_is_dragging) = signal(false);

    let mode_options = vec![
        SegmentedOption::new("dictation", "Dictation"),
        SegmentedOption::new("scratchpad", "Scratchpad"),
        SegmentedOption::new("drop", "Drop Zone"),
    ];

    let open_settings = Callback::new(move |_| {
        spawn_local(async move {
            let _ = crate::ipc::tauri_invoke(
                "open_settings",
                serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
            )
            .await;
        });
    });

    let toggle_dictation = Callback::new(move |_| {
        set_is_dictating.set(!is_dictating.get());
        spawn_local(async move {
            let _ = crate::ipc::tauri_invoke(
                "start_dictation",
                serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
            )
            .await;
        });
    });

    let copy_to_clipboard = Callback::new(move |_: web_sys::MouseEvent| {
        let text = scratchpad.get();
        if text.is_empty() {
            toasts.warning("Clipboard", "Nothing to copy — scratchpad is empty");
            return;
        }
        if let Some(window) = web_sys::window() {
            let clipboard = window.navigator().clipboard();
            let _promise = clipboard.write_text(&text);
            toasts.success("Clipboard", "Copied to clipboard");
            let mut cache = clipboard_cache.get();
            cache.push(text.clone());
            if cache.len() > 10 {
                cache.drain(0..cache.len() - 10);
            }
            set_clipboard_cache.set(cache);
        }
    });

    let handle_drop = move |ev: DragEvent| {
        set_is_dragging.set(false);
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            if let Some(file_list) = dt.files() {
                let len = file_list.length();
                for i in 0..len {
                    if let Some(file) = file_list.item(i) {
                        let filename = file.name();
                        let mime_type = file.type_();
                        let toasts_clone = toasts.clone();
                        let file_clone = file.clone();
                        spawn_local(async move {
                            let array_buffer = match file_clone.array_buffer().await {
                                Ok(buf) => buf,
                                Err(_) => {
                                    toasts_clone.error("File Drop", "Could not read the dropped file");
                                    return;
                                }
                            };
                            let data = js_sys::Uint8Array::new(&array_buffer).to_vec();
                            let mime = if mime_type.is_empty() { "application/octet-stream".to_string() } else { mime_type };
                            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                                "filename": filename,
                                "mime_type": mime,
                                "data": data,
                            }))
                            .unwrap();
                            let result = crate::ipc::tauri_invoke("capture_file_drop", args).await;
                            match serde_wasm_bindgen::from_value::<String>(result) {
                                Ok(id) => toasts_clone.success("File Drop", format!("Captured '{}' to inbox", id)),
                                Err(_) => toasts_clone.error("File Drop", "Could not capture the dropped file"),
                            }
                        });
                    }
                }
            }
        }
    };

    let drag_class = move || {
        if is_dragging.get() {
            "scale-105 border-4 border-blue-500"
        } else {
            ""
        }
    };

    view! {
        <div
            class=move || format!("dictation-pill transition-all {}", drag_class())
            style=move || format!("opacity: {}", opacity.get())
            on:mouseenter=move |_| set_opacity.set(1.0)
            on:mouseleave=move |_| set_opacity.set(0.8)
            on:dragenter=move |_| set_is_dragging.set(true)
            on:dragleave=move |_| set_is_dragging.set(false)
            on:drop=handle_drop
            on:dragover=move |ev: DragEvent| {
                ev.prevent_default();
            }
        >
            {move || if is_dictating.get() {
                view! { <div class="flex space-x-1"><div class="h-4 w-1 bg-white animate-pulse"></div><div class="h-6 w-1 bg-white animate-pulse delay-75"></div><div class="h-4 w-1 bg-white animate-pulse delay-150"></div></div> }.into_any()
            } else {
                view! {}.into_any()
            }}

            <Segmented options=mode_options selected=mode class="mode-selector" />

            <div class="flex gap-1">
                <Button variant=ButtonVariant::Ghost aria_label="Copy to clipboard" on_click=copy_to_clipboard>{render_icon_view(Icon::Copy)}</Button>
                <Button variant=ButtonVariant::Ghost aria_label="Open settings" on_click=open_settings>{render_icon_view(Icon::Settings)}</Button>
            </div>

            {move || match mode.get().as_str() {
                "dictation" => view! {
                    <Button
                        variant=ButtonVariant::Primary
                        on_click=toggle_dictation
                    >
                        {move || if is_dictating.get() { "Stop" } else { "Record" }}
                    </Button>
                }.into_any(),
                "scratchpad" => view! {
                    <textarea
                        prop:value=scratchpad
                        on:input=move |ev| set_scratchpad.set(event_target_value(&ev))
                        placeholder="Scratchpad..."
                        style="background: transparent; color: white; border: none; width: 100%;"
                    />
                }.into_any(),
                "drop" => view! {
                    <div class="border-2 border-dashed rounded-lg p-6 text-center transition-colors"
                         style=move || format!("border-color: {};", if is_dragging.get() { "#60a5fa" } else { "#4b5563" })>
                        <div>{render_icon_view(Icon::Upload)}</div>
                        <p class="text-sm text-gray-400 mt-2">"Drop files here or click to browse"</p>
                        <p class="text-xs text-gray-500 mt-1">"Files will be captured to your inbox"</p>
                    </div>
                }.into_any(),
                _ => view! {}.into_any(),
            }}

            {move || if !clipboard_cache.get().is_empty() {
                view! {
                    <div class="mt-2">
                        <div class="text-xs text-gray-500 mb-1">"Recent clipboard:"</div>
                        <div class="max-h-32 overflow-y-auto space-y-1">
                            {move || clipboard_cache.get().iter().rev().map(|entry| {
                                let entry_display = entry.clone();
                                let entry_click = entry.clone();
                                view! {
                                    <div class="text-xs bg-gray-800 rounded px-2 py-1 truncate"
                                         on:click=move |_| {
                                        if let Some(window) = web_sys::window() {
                                            let clipboard = window.navigator().clipboard();
                                            let _ = clipboard.write_text(&entry_click);
                                        }
                                    }>
                                        {entry_display}
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}
