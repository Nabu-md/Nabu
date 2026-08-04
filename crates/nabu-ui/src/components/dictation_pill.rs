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
    let (_opacity, set_opacity) = signal(0.8_f32);

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

    let drag_class = move || {
        if is_dragging.get() {
            "scale-105 border-4 border-blue-500"
        } else {
            ""
        }
    };
    view! {
        <div class=move || format!("dictation-pill transition-all {}", drag_class())
             on:mouseenter=move |_| set_opacity.set(1.0)
             on:mouseleave=move |_| set_opacity.set(0.8)
             on:dragenter=move |_| set_is_dragging.set(true)
             on:dragleave=move |_| set_is_dragging.set(false)
             on:drop=move |ev: DragEvent| {
                set_is_dragging.set(false);
                ev.prevent_default();
             }
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

            <Button variant=ButtonVariant::Ghost aria_label="Copy to clipboard">{render_icon_view(Icon::Copy)}</Button>
            <Button variant=ButtonVariant::Ghost aria_label="Open settings" on_click=open_settings>{render_icon_view(Icon::Settings)}</Button>
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
                    <div style="border: 2px dashed white; padding: 20px;">"Drop Files Here"</div>
                }.into_any(),
                _ => view! {}.into_any(),
            }}
        </div>
    }
}
