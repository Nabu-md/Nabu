//! # Dictation Pill — Dioxus migration
//!
//! Floating scratchpad / dictation / file-drop pill. Preserves all behaviour
//! from the LePtOS version: opacity loading from settings, clipboard cache
//! panel, drop-zone IPC integration, and three-mode switch (dictation,
//! scratchpad, drop zone).

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::selection::{Segmented, SegmentedOption};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen_futures::spawn_local;

/// The floating dictation / scratchpad / drop-zone pill.
#[component]
pub fn DictationPill() -> Element {
    let mut scratchpad = use_signal(String::new);
    let mut mode = use_signal(|| "dictation".to_string());
    let mut opacity = use_signal(|| 0.8_f32);
    let mut clipboard_cache = use_signal(Vec::<String>::new);
    let toasts = crate::components::ui::feedback::use_toast();

    // Load opacity from backend settings on mount.
    spawn_local({
        let mut opacity_load = opacity;
        async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({"key": "floating_pill_opacity"}))
                    .unwrap();
            let result = crate::ipc::tauri_invoke("settings_get", args).await;
            if let Ok(op) = serde_wasm_bindgen::from_value::<f32>(result) {
                opacity_load.set(op);
            }
        }
    });

    let mut is_dictating = use_signal(|| false);
    let mut is_dragging = use_signal(|| false);

    let mode_options = vec![
        SegmentedOption::new("dictation", "Dictation"),
        SegmentedOption::new("scratchpad", "Scratchpad"),
        SegmentedOption::new("drop", "Drop Zone"),
    ];

    rsx! {
        div {
            class: "dictation-pill flex flex-col items-center gap-2 p-3 rounded-lg shadow-lg bg-gray-800 text-gray-100",
            style: format!("opacity: {};", opacity.read()),
            onmouseenter: move |_: MouseEvent| opacity.set(1.0),
            onmouseleave: move |_: MouseEvent| opacity.set(0.8),
            ondragenter: move |_: DragEvent| is_dragging.set(true),
            ondragleave: move |_: DragEvent| is_dragging.set(false),
            ondragover: move |ev: DragEvent| ev.prevent_default(),
            ondrop: move |ev: DragEvent| {
                ev.prevent_default();
                is_dragging.set(false);
                let web = ev.data().as_web_event();
                if let Some(dt) = web.data_transfer() {
                    if let Some(file_list) = dt.files() {
                        let len = file_list.length();
                        for i in 0..len {
                            if let Some(file) = file_list.item(i) {
                                let filename = file.name();
                                let mime_type = file.type_();
                                let toasts_clone = toasts;
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
                                    let mime = if mime_type.is_empty() {
                                        "application/octet-stream".to_string()
                                    } else {
                                        mime_type
                                    };
                                    let args = serde_wasm_bindgen::to_value(
                                        &serde_json::json!({
                                            "filename": filename,
                                            "mime_type": mime,
                                            "data": data,
                                        }),
                                    ).unwrap();
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
            },
        }

        // Recording pulse indicator
        {if *is_dictating.read() {
            rsx! {
                div { class: "flex space-x-1" }
                div { class: "h-4 w-1 bg-white animate-pulse" }
                div { class: "h-6 w-1 bg-white animate-pulse delay-75" }
                div { class: "h-4 w-1 bg-white animate-pulse delay-150" }
            }
        } else { rsx! {} }}

        // Mode selector
        Segmented {
            options: mode_options,
            selected: mode,
            class: "mode-selector",
        }

        // Action buttons
        div { class: "flex gap-1" }
        Button {
            variant: ButtonVariant::Ghost,
            aria_label: "Copy to clipboard",
            on_click: move |_: MouseEvent| {
                let text = scratchpad.read().clone();
                if text.is_empty() {
                    toasts.warning("Clipboard", "Nothing to copy — scratchpad is empty");
                    return;
                }
                if let Some(window) = web_sys::window() {
                    let clipboard = window.navigator().clipboard();
                    let _promise = clipboard.write_text(&text);
                    toasts.success("Clipboard", "Copied to clipboard");
                    clipboard_cache.with_mut(|cache| {
                        cache.push(text.clone());
                        if cache.len() > 10 {
                            cache.drain(0..cache.len() - 10);
                        }
                    });
                }
            },
            {render_icon_view(Icon::Copy)}
        }
        Button {
            variant: ButtonVariant::Ghost,
            aria_label: "Open settings",
            on_click: move |_: MouseEvent| {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
                spawn_local(async move {
                    let _ = crate::ipc::tauri_invoke("open_settings", args).await;
                });
            },
            {render_icon_view(Icon::Settings)}
        }

        // Mode-specific content
        {match mode.read().as_str() {
            "dictation" => rsx! {
                Button {
                    variant: ButtonVariant::Primary,
                    on_click: move |_: MouseEvent| {
                        is_dictating.set(!*is_dictating.read());
                        let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
                        spawn_local(async move {
                            let _ = crate::ipc::tauri_invoke("start_dictation", args).await;
                        });
                    },
                    {if *is_dictating.read() { "Stop" } else { "Record" }}
                }
            },
            "scratchpad" => rsx! {
                textarea {
                    placeholder: "Scratchpad...",
                    style: "background: transparent; color: white; border: none; width: 100%;",
                    value: "{scratchpad.read().as_str()}",
                    oninput: move |ev: FormEvent| scratchpad.set(ev.value()),
                }
            },
            "drop" => rsx! {
                div {
                    class: "border-2 border-dashed rounded-lg p-6 text-center transition-colors",
                    style: format!("border-color: {};", if *is_dragging.read() { "#60a5fa" } else { "#4b5563" }),
                }
                div { {render_icon_view(Icon::Upload)} }
                p { class: "text-sm text-gray-400 mt-2", "Drop files here or click to browse" }
                p { class: "text-xs text-gray-500 mt-1", "Files will be captured to your inbox" }
            },
            _ => rsx! {},
        }}

        // Clipboard cache panel
        {if !clipboard_cache.read().is_empty() {
            rsx! {
                div { class: "mt-2" }
                div { class: "text-xs text-gray-500 mb-1", "Recent clipboard:" }
                div { class: "max-h-32 overflow-y-auto space-y-1" }
                for entry in clipboard_cache.read().iter().rev() {
                    {
                        let entry_click = entry.clone();
                        rsx! {
                            div {
                                class: "text-xs bg-gray-800 rounded px-2 py-1 truncate",
                                onclick: move |_: MouseEvent| {
                                    if let Some(window) = web_sys::window() {
                                        let clipboard = window.navigator().clipboard();
                                        let _ = clipboard.write_text(&entry_click);
                                    }
                                },
                                "{entry}"
                            }
                        }
                    }
                }
            }
        } else { rsx! {} }}
    }
}
