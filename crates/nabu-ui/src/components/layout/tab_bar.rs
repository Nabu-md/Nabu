//! # Tab bar
//!
//! Renders the workspace's open tabs from [`WorkspaceContext`]:
//!
//! - click to activate, middle-click (or the ✕) to close
//! - right-click context menu: Close, Close Others, Close All, Duplicate,
//!   Pin / Unpin, Reveal in Sidebar
//! - drag-and-drop reordering between tabs
//! - the trailing "+" creates a new note and opens it
//!
//! Tabs are driven by the shared workspace signal so the file tree (open
//! note), editor (active note) and session restore all stay in sync.

use crate::components::contexts::{
    activate_tab, close_all, close_others, close_tab, open_tab, pin_tab, reorder_tab,
    refresh_tree, use_workspace, WorkspaceContext,
};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::menu::{ContextMenu, MenuItem, MenuSeparator};
use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Dispatches a `nabu:reveal-note` window event so the file tree reveals the
/// note (expands its parent folders and selects it).
fn reveal_in_sidebar(path: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(&path));
    let Ok(event) =
        web_sys::CustomEvent::new_with_event_init_dict("nabu:reveal-note", &init)
    else {
        return;
    };
    let _ = window.dispatch_event(&event);
}

/// Pre-computed render data for one open tab (avoids `let` inside `rsx!` loops).
#[derive(Clone)]
struct TabRenderData {
    path: String,
    title: String,
    pinned: bool,
    active: bool,
}

/// The tab bar.
#[component]
pub fn TabBar() -> Element {
    let workspace: WorkspaceContext = use_workspace();
    let drag_index = use_signal(|| None::<usize>);

    // Pre-compute tab render data.
    let active_path = workspace.active_path.read().clone();
    let tabs: Vec<TabRenderData> = workspace
        .tabs
        .read()
        .iter()
        .map(|tab| TabRenderData {
            path: tab.path.clone(),
            title: tab.title.clone(),
            pinned: tab.pinned,
            active: active_path.as_deref() == Some(&tab.path),
        })
        .collect();

    // Build tab elements outside rsx! (avoids `let` inside for loops).
    let tab_elements: Vec<VNode> = tabs
        .iter()
        .map(|tab| {
            let tab = tab.clone();
            let ws = workspace;
            let ws_close = workspace;
            let ws_others = workspace;
            let ws_dup = workspace;
            let ws_pin = workspace;
            let ws_activate = workspace;
            let ws_reorder = workspace;
            let ws_ctx = workspace;
            let di = drag_index;
            let di2 = drag_index;

            rsx! {
                ContextMenu {
                    menu_items: rsx! {
                        MenuItem {
                            label: "Close".to_string(),
                            on_select: move |_| close_tab(ws_close, &tab.path),
                        }
                        MenuItem {
                            label: "Close Others".to_string(),
                            on_select: move |_| close_others(ws_others, &tab.path),
                        }
                        MenuItem {
                            label: "Close All".to_string(),
                            on_select: move |_| close_all(ws),
                        }
                        MenuSeparator {}
                        MenuItem {
                            label: "Duplicate".to_string(),
                            on_select: move |_| {
                                let ws_local = ws_dup;
                                let from = tab.path.clone();
                                spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                                        "from": from.clone(),
                                        "dest": from.clone(),
                                    }))
                                    .unwrap();
                                    let result = crate::ipc::tauri_invoke("note_duplicate", args).await;
                                    if let Ok(new_path) = serde_wasm_bindgen::from_value::<String>(result) {
                                        open_tab(ws_local, &new_path);
                                        refresh_tree(ws_local);
                                    }
                                });
                            },
                        }
                        MenuItem {
                            label: if tab.pinned { "Unpin".to_string() } else { "Pin".to_string() },
                            on_select: move |_| pin_tab(ws_pin, &tab.path),
                        }
                        MenuSeparator {}
                        MenuItem {
                            label: "Reveal in Sidebar".to_string(),
                            on_select: move |_| reveal_in_sidebar(tab.path.clone()),
                        }
                    },
                    div {
                        tabindex: "0",
                        class: if tab.active { "tab flex items-center gap-1.5 px-3 text-xs whitespace-nowrap cursor-pointer border-r border-gray-800 tab-active" } else { "tab flex items-center gap-1.5 px-3 text-xs whitespace-nowrap cursor-pointer border-r border-gray-800" },
                        title: "{tab.title}",
                        draggable: "true",
                        role: "tab",
                        "aria-selected": "{tab.active}",
                        onclick: move |_| activate_tab(ws_activate, &tab.path),
                        ondragstart: move |ev: web_sys::DragEvent| {
                            if let Some(dt) = ev.data_transfer() {
                                let _ = dt.set_data("text/plain", &tab.path);
                                let _ = dt.set_effect_allowed("move");
                            }
                            let idx = ws.tabs.read().iter().position(|t| t.path == tab.path);
                            if let Some(idx) = idx {
                                di.set(Some(idx));
                            }
                        },
                        ondragover: move |ev: web_sys::DragEvent| {
                            ev.prevent_default();
                            if let Some(from) = *di.read() {
                                if let Some(to) = ws.tabs.read().iter().position(|t| t.path == tab.path) {
                                    if from != to {
                                        reorder_tab(ws_reorder, from, to);
                                        di.set(Some(to));
                                    }
                                }
                            }
                        },
                        ondragend: move |_: web_sys::DragEvent| {
                            di2.set(None);
                        },
                        // Close button (X).
                        button {
                            class: "tab-close text-gray-500 px-0.5 text-xs leading-none",
                            "aria-label": "Close {tab.path}",
                            onclick: move |ev: MouseEvent| {
                                ev.stop_propagation();
                                close_tab(ws_ctx, &tab.path);
                            },
                            {render_icon_view(Icon::X)}
                        }
                        if tab.pinned {
                            span { class: "text-gray-500", title: "Pinned", "aria-hidden": "true", {render_icon_view(Icon::MapPin)} }
                        }
                        span { class: "truncate max-w-40", "{tab.title}" }
                    }
                }
            }
        })
        .collect();

    // New-note button handler.
    let new_note = move |_: MouseEvent| {
        let ws = workspace;
        spawn_local(async move {
            let name =
                format!("note-{}.md", js_sys::Date::new_0().get_time() as u64);
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": name.clone() }))
                .unwrap();
            let result = crate::ipc::tauri_invoke("note_create_file", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                open_tab(ws, &name);
                refresh_tree(ws);
            }
        });
    };

    rsx! {
        div {
            class: "tab-bar flex items-stretch h-9 bg-gray-900 border-b border-gray-700 overflow-x-auto",
            "role": "tablist",
            "aria-label": "Open notes",
        }
        {tab_elements}
        // New-note button (trailing +).
        button {
            class: "tab-new px-3 text-gray-400 text-sm shrink-0",
            title: "New note",
            "aria-label": "New note",
            onclick: new_note,
            {render_icon_view(Icon::Plus)}
        }
    }
}
