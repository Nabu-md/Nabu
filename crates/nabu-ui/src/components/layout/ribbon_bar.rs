use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RibbonAction {
    ToggleSidebar,
    OpenSearch,
    OpenGraph,
    OpenCanvas,
    OpenSettings,
}

#[component]
pub fn RibbonBar(
    #[prop(optional)] set_view_mode: Option<std::sync::Arc<dyn Fn(crate::components::app::ViewMode) + Send + Sync + 'static>>,
    #[prop(optional)] set_show_sidebar: Option<std::sync::Arc<dyn Fn(bool) + Send + Sync + 'static>>,
) -> impl IntoView {
    let (enabled, _set_enabled) = signal(false);

    let handle_open_graph = move |_| {
        if let Some(ref f) = set_view_mode {
            f(crate::components::app::ViewMode::Graph);
        }
    };

    let handle_toggle_dictation = move |_| {
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("toggle_dictation_pill", empty_args).await;
        });
    };

    let handle_open_settings = move |_| {
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("open_settings", empty_args).await;
        });
    };

    view! {
        <div class="w-12 h-screen border-r border-gray-700 bg-gray-900 flex flex-col items-center py-4 space-y-4">
            <button title="Vault Explorer" on:click=move |_| {
                if let Some(ref f) = set_show_sidebar {
                    f(true);
                }
            }>"📁"</button>
            <button title="Global Search" on:click=move |_| {
                spawn_local(async move {
                    // TODO: wire search command when implemented
                });
            }>"🔍"</button>
            <button title="Graph View" on:click=handle_open_graph>"🕸️"</button>
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
            <button title="Dictation" on:click=handle_toggle_dictation>"🎤"</button>
            <button title="Canvas" on:click=move |_| println!("Open Canvas")>"🎨"</button>
            <div class="flex-grow"></div>
            <button title="Settings" on:click=handle_open_settings>"⚙️"</button>
        </div>
    }
}