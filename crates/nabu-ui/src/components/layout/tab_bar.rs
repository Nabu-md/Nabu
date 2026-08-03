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

use crate::components::ui::menu::{ContextMenu, MenuItem, MenuSeparator};
use crate::components::workspace::{
    activate_tab, close_all, close_others, close_tab, open_tab, pin_tab, refresh_tree, reorder_tab,
    use_workspace,
};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Dispatches a `nabu:reveal-note` window event so the file tree reveals the
/// note (expands its parent folders and selects it).
fn reveal_in_sidebar(path: String) {
    let Some(window) = web_sys::window() else { return };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(&path));
    let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("nabu:reveal-note", &init)
    else {
        return;
    };
    let _ = window.dispatch_event(&event);
}

/// The tab bar.
#[component]
pub fn TabBar() -> impl IntoView {
    let workspace = use_workspace();

    // Drag state for tab reordering: index of the dragged tab.
    let (drag_index, set_drag_index) = signal(None::<usize>);

    let new_note = move |_| {
        let workspace_new = workspace;
        spawn_local(async move {
            // Use a timestamped name so repeatedly clicking "+" never
            // overwrites the same file.
            let name = format!("note-{}.md", js_sys::Date::new_0().get_time() as u64);
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": name.clone() }))
                .unwrap();
            let result = crate::ipc::tauri_invoke("note_create_file", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                open_tab(workspace_new, &name);
                refresh_tree(workspace_new);
            }
        });
    };

    view! {
        <div class="tab-bar flex items-stretch h-9 bg-gray-900 border-b border-gray-700 overflow-x-auto">
            {move || {
                let tabs = workspace.tabs.get();
                let active = workspace.active_path.get();
                tabs.into_iter().enumerate().map(|(index, tab)| {
                    let path = tab.path.clone();
                    let title = tab.title.clone();
                    let pinned = tab.pinned;
                    let is_active = active.as_deref() == Some(path.as_str());
                    // Context-menu items — wrapped in an `Arc` so the value
                    // matches `ChildrenFn` (Arc<dyn Fn() -> AnyView>). Each
                    // callback gets its own clone of the tab path so no
                    // closure ever moves the shared path String.
                    let path_menu = path.clone();
                    let workspace_menu = workspace;
                    let pinned_menu = pinned;
                    let menu_items: ChildrenFn = std::sync::Arc::new(move || {
                        let path_item = path_menu.clone();
                        let ws = workspace_menu;
                        let pinned_item = pinned_menu;

                        let path_close = path_item.clone();
                        let close_cb = Callback::new(move |_| close_tab(ws, &path_close));
                        let path_others = path_item.clone();
                        let others_cb = Callback::new(move |_| close_others(ws, &path_others));
                        let close_all_cb = Callback::new(move |_| close_all(ws));
                        let path_dup = path_item.clone();
                        let dup_cb = Callback::new(move |_| {
                            let ws_dup = ws;
                            let from = path_dup.clone();
                            spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                                    "from": from.clone(),
                                    "dest": from.clone(),
                                }))
                                .unwrap();
                                let result = crate::ipc::tauri_invoke("note_duplicate", args).await;
                                if let Ok(new_path) = serde_wasm_bindgen::from_value::<String>(result) {
                                    open_tab(ws_dup, &new_path);
                                    refresh_tree(ws_dup);
                                }
                            });
                        });
                        let path_pin = path_item.clone();
                        let pin_cb = Callback::new(move |_| pin_tab(ws, &path_pin));
                        let path_reveal = path_item.clone();
                        let reveal_cb = Callback::new(move |_| reveal_in_sidebar(path_reveal.clone()));

                        view! {
                            <MenuItem label="Close".to_string() on_select=close_cb />
                            <MenuItem label="Close Others".to_string() on_select=others_cb />
                            <MenuItem label="Close All".to_string() on_select=close_all_cb />
                            <MenuSeparator />
                            <MenuItem label="Duplicate".to_string() on_select=dup_cb />
                            <MenuItem
                                label=if pinned_item { "Unpin".to_string() } else { "Pin".to_string() }
                                on_select=pin_cb
                            />
                            <MenuSeparator />
                            <MenuItem label="Reveal in Sidebar".to_string() on_select=reveal_cb />
                        }
                        .into_any()
                    });

                    // All handlers are inlined with local `path` clones so the
                    // children closure ContextMenu generates captures nothing
                    // non-Copy (that would make it `FnOnce`).
                    view! {
                        <ContextMenu menu_items=menu_items>
                            <div
                                class=move || format!(
                                    "tab flex items-center gap-1.5 px-3 text-xs whitespace-nowrap cursor-pointer border-r border-gray-800{}",
                                    if is_active { " tab-active" } else { "" }
                                )
                                draggable="true"
                                title=title.clone()
                                on:dragstart={let p = path.clone(); move |ev: web_sys::DragEvent| {
                                    set_drag_index.set(Some(index));
                                    if let Some(dt) = ev.data_transfer() {
                                        let _ = dt.set_data("text/plain", &p);
                                        let _ = dt.set_effect_allowed("move");
                                    }
                                }}
                                on:dragover=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default();
                                    if let Some(from) = drag_index.get() {
                                        if from != index {
                                            reorder_tab(workspace, from, index);
                                            set_drag_index.set(Some(index));
                                        }
                                    }
                                }
                                on:dragend=move |_ev: web_sys::DragEvent| set_drag_index.set(None)
                                on:drop=move |ev: web_sys::DragEvent| {
                                    ev.prevent_default();
                                    set_drag_index.set(None);
                                }
                                on:click={let p = path.clone(); move |_| activate_tab(workspace, &p)}
                                on:auxclick={let p = path.clone(); move |ev: web_sys::MouseEvent| {
                                    // Middle-click closes the tab.
                                    if ev.button() == 1 {
                                        close_tab(workspace, &p);
                                    }
                                }}
                                role="tab"
                                aria-selected=move || is_active
                            >
                                {if pinned {
                                    view! { <span class="text-gray-500" title="Pinned">"📌"</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                                <span class="truncate max-w-40">{title.clone()}</span>
                                <button
                                    class="tab-close text-gray-500 px-0.5 text-xs leading-none"
                                    aria-label=format!("Close {}", path)
                                    on:click={let p = path.clone(); move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        close_tab(workspace, &p);
                                    }}
                                >"✕"</button>
                            </div>
                        </ContextMenu>
                    }
                }).collect_view()
            }}
            <button
                class="tab-new px-3 text-gray-400 text-sm shrink-0"
                title="New note"
                aria-label="New note"
                on:click=new_note
            >"+"</button>
        </div>
    }
}
