use crate::components::ui::button::IconButton;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

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
    #[prop(optional)] set_view_mode: Option<
        std::sync::Arc<dyn Fn(crate::components::app::ViewMode) + Send + Sync + 'static>,
    >,
    #[prop(optional)] set_show_sidebar: Option<
        std::sync::Arc<dyn Fn(bool) + Send + Sync + 'static>,
    >,
) -> impl IntoView {
    let (enabled, _set_enabled) = signal(false);

    let open_sidebar = Callback::new(move |_| {
        if let Some(ref f) = set_show_sidebar {
            f(true);
        }
    });

    let open_graph = Callback::new(move |_| {
        if let Some(ref f) = set_view_mode {
            f(crate::components::app::ViewMode::Graph);
        }
    });

    let toggle_dictation = Callback::new(move |_| {
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("toggle_dictation_pill", empty_args).await;
        });
    });

    let open_settings = Callback::new(move |_| {
        spawn_local(async move {
            let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("open_settings", empty_args).await;
        });
    });

    let daily_note = Callback::new(move |_| {
        spawn_local(async move {
            let _ = crate::ipc::tauri_invoke(
                "note_daily",
                serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
            )
            .await;
        });
    });

    view! {
        <div class="w-12 h-screen border-r border-gray-700 bg-gray-900 flex flex-col items-center py-4 space-y-4">
            <IconButton title="Vault Explorer" on_click=open_sidebar>"📁"</IconButton>
            <IconButton title="Global Search">"🔍"</IconButton>
            <IconButton title="Graph View" on_click=open_graph>"🕸️"</IconButton>
            {move || if enabled.get() {
                view! {
                    <IconButton title="Daily Note" on_click=daily_note>"📅"</IconButton>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
            <IconButton title="Dictation" on_click=toggle_dictation>"🎤"</IconButton>
            <IconButton title="Canvas">"🎨"</IconButton>
            <div class="flex-grow"></div>
            <IconButton title="Settings" on_click=open_settings>"⚙️"</IconButton>
        </div>
    }
}
