//! # Smart Folders — virtual collections powered by queries
//!
//! Phase 13.2 lets users define reusable queries (tags, folders, dates, file
//! types and full-text terms) and surface them as folders that always stay up
//! to date as the vault evolves. The filesystem is never touched — these are
//! pure overlays on the vault index.
//!
//! The query language is the backend `smart_folder_evaluate` mini-language:
//! whitespace-separated, ANDed tokens:
//! - `tag:name`       — frontmatter tag contains `name`
//! - `folder:path`    — note lives in `path` or a subfolder
//! - `date:YYYY-MM-DD`/`before:…`/`after:…` — dated notes
//! - anything else    — case-insensitive full-text search (title + content)
//!
//! ## Reactivity note
//!
//! Toast and navigation contexts are `Copy` and captured at render time, then
//! threaded into async tasks as plain values — never `expect_context` inside a
//! `spawn_local` future (no reactive owner on the failure path).

use crate::components::navigation::state::{
    remove_smart_folder, save_smart_folder, use_nav,
};
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::card::{Card, CardBody, CardHeader};
use crate::components::ui::feedback::{use_toast, ToastContext};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use crate::components::ui::input::{TextInput, Textarea};
use crate::components::workspace::{open_tab, use_workspace};
use crate::models::organisation::SmartFolder;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// One evaluated result (mirrors the backend `NoteIndexEntry`).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct SmartFolderResult {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub modified_at: String,
    #[serde(default)]
    pub pinned: bool,
}

/// The Smart Folders workspace.
#[component]
pub fn SmartFoldersPage() -> impl IntoView {
    let nav = use_nav();
    let ws = use_workspace();
    let toasts = use_toast();

    // Create / edit form state. `TextInput`/`Textarea` bind an `RwSignal`.
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let name = RwSignal::new(String::new());
    let icon = RwSignal::new(String::from("📁"));
    let query = RwSignal::new(String::new());
    let (show_form, set_show_form) = signal(false);

    // Results for the currently selected smart folder.
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (results, set_results) = signal(Vec::<SmartFolderResult>::new());
    let (evaluating, set_evaluating) = signal(false);

    /// Runs a smart folder query and fills the results list.
    fn run_query(query: String, set_results: WriteSignal<Vec<SmartFolderResult>>, set_evaluating: WriteSignal<bool>, toasts: ToastContext) {
        set_evaluating.set(true);
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "query": query })).unwrap();
            let result = crate::ipc::tauri_invoke("smart_folder_evaluate", args).await;
            set_evaluating.set(false);
            match serde_wasm_bindgen::from_value::<Vec<SmartFolderResult>>(result) {
                Ok(hits) => set_results.set(hits),
                Err(_) => {
                    set_results.set(Vec::new());
                    toasts.error("Smart folder", "Could not run that query.");
                }
            }
        });
    }

    // Select a smart folder → run it.
    let on_select = Callback::new(move |f: SmartFolder| {
        set_selected_id.set(Some(f.id.clone()));
        name.set(f.name.clone());
        icon.set(if f.icon.is_empty() { "📁".to_string() } else { f.icon.clone() });
        query.set(f.query.clone());
        run_query(f.query.clone(), set_results, set_evaluating, toasts);
    });

    // Begin editing a new smart folder.
    let on_new = Callback::new(move |_| {
        set_editing_id.set(None);
        name.set(String::new());
        icon.set(String::from("📁"));
        query.set(String::new());
        set_show_form.set(true);
    });

    // Begin editing an existing smart folder.
    let on_edit = Callback::new(move |f: SmartFolder| {
        set_editing_id.set(Some(f.id.clone()));
        name.set(f.name.clone());
        icon.set(if f.icon.is_empty() { "📁".to_string() } else { f.icon.clone() });
        query.set(f.query.clone());
        set_show_form.set(true);
    });

    // Save (create or update) the current form.
    let save_click = Callback::new(move |_| {
        let trimmed = name.get_untracked().trim().to_string();
        if trimmed.is_empty() || query.get_untracked().trim().is_empty() {
            toasts.warning("Smart folder", "Give it a name and a query first.");
            return;
        }
        let id = editing_id.get_untracked().unwrap_or_else(|| {
            format!("sf-{}", js_sys::Date::new_0().get_time() as u64)
        });
        let folder = SmartFolder {
            id: id.clone(),
            name: trimmed,
            icon: icon.get_untracked(),
            query: query.get_untracked().trim().to_string(),
            pinned: false,
        };
        save_smart_folder(nav, folder.clone());
        set_editing_id.set(Some(id.clone()));
        set_selected_id.set(Some(id));
        set_show_form.set(false);
        toasts.success("Smart folder", format!("Saved \"{}\"", folder.name));
        run_query(folder.query, set_results, set_evaluating, toasts);
    });

    // Delete a smart folder.
    let delete_click = Callback::new(move |_| {
        if let Some(id) = selected_id.get_untracked() {
            remove_smart_folder(nav, &id);
            if editing_id.get_untracked().as_deref() == Some(id.as_str()) {
                set_show_form.set(false);
                set_editing_id.set(None);
            }
            set_selected_id.set(None);
            set_results.set(Vec::new());
            toasts.info("Smart folder", "Deleted.");
        }
    });

    // Toggle pinned on a smart folder.
    let toggle_pin = Callback::new(move |f: SmartFolder| {
        let mut updated = f.clone();
        updated.pinned = !f.pinned;
        save_smart_folder(nav, updated);
    });

    view! {
        <div class="space-y-6">
            <header>
                <h1 class="text-xl font-semibold text-gray-100">"Smart Folders"</h1>
                <p class="text-sm text-gray-400 mt-1">
                    "Virtual collections powered by queries — always up to date as your vault evolves."
                </p>
            </header>

            <div class="flex gap-4">
                // Left column: the list of saved smart folders.
                <Card class="w-80 flex-none">
                    <CardHeader title="Saved folders".to_string()>
                        <Button
                            size=ButtonSize::Sm
                            variant=ButtonVariant::Primary
                            on_click=Callback::new(move |_| on_new.run(()))
                        >
                            "＋ New"
                        </Button>
                    </CardHeader>
                    <CardBody>
                        <div class="space-y-1">
                            {move || {
                                let folders = nav.smart_folders.get();
                                if folders.is_empty() {
                                    view! {
                                        <EmptyState
                                            icon=Icon::FolderTree
                                            title="No smart folders yet".to_string()
                                            description="Create one to surface a query as a folder.".to_string()
                                        />
                                    }.into_any()
                                } else {
                                    folders.into_iter().map(|f| {
                                        let is_active = selected_id.get().as_deref() == Some(f.id.as_str());
                                        let icon = f.icon.clone();
                                        let name = f.name.clone();
                                        let pinned = f.pinned;
                                        // Each `move` handler owns its own copy of the
                                        // folder so no closure steals it from the others.
                                        let folder_select = f.clone();
                                        let folder_keydown = f.clone();
                                        let folder_pin = f.clone();
                                        let folder_edit = f.clone();
                                        view! {
                                            <div
                                                class=format!(
                                                    "w-full text-left px-3 py-2 rounded-md text-sm flex items-center gap-2 cursor-pointer select-none {}",
                                                    if is_active { "bg-gray-700 text-gray-100" } else { "text-gray-300 hover:bg-gray-800" }
                                                )
                                                role="button"
                                                tabindex="0"
                                                aria-pressed=is_active
                                                on:click=move |_| on_select.run(folder_select.clone())
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    if ev.key() == "Enter" || ev.key() == " " {
                                                        ev.prevent_default();
                                                        on_select.run(folder_keydown.clone());
                                                    }
                                                }
                                            >
                                                <span aria-hidden="true">{icon.clone()}</span>
                                                <span class="flex-1 truncate">{name.clone()}</span>
                                                <button
                                                    type="button"
                                                    class="text-xs hover:text-gray-100"
                                                    title="Toggle pinned"
                                                    aria-label="Toggle pinned"
                                                    on:click=move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        toggle_pin.run(folder_pin.clone());
                                                    }
                                                >
                                                        {if pinned { render_icon_view(Icon::MapPin) } else { render_icon_view(Icon::Circle) }}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="text-xs text-gray-500 hover:text-gray-200"
                                                    title="Edit"
                                                    aria-label="Edit"
                                                    on:click=move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        on_edit.run(folder_edit.clone());
                                                    }
                                                >
                                                    {render_icon_view(Icon::PenLine)}
                                                </button>
                                            </div>
                                        }
                                    }).collect_view().into_any()
                                }
                            }}
                        </div>
                    </CardBody>
                </Card>

                // Right column: form + results.
                <div class="flex-1 min-w-0 space-y-4">
                    {move || if show_form.get() {
                        view! {
                            <Card>
                                <CardHeader title=format!("{}", if editing_id.get().is_some() { "Edit smart folder" } else { "New smart folder" })>
                                    <span></span>
                                </CardHeader>
                                <CardBody>
                                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                        <TextInput
                                            value=name
                                            label="Name"
                                            placeholder="Project notes"
                                        />
                                        <TextInput
                                            value=icon
                                            label="Icon (Unicode)"
                                            placeholder="📁"
                                        />
                                    </div>
                                    <div class="mt-4">
                                        <Textarea
                                            value=query
                                            label="Query"
                                            placeholder="tag:project after:2026-01-01 roadmap"
                                            hint="Tokens are ANDed. Try tag:, folder:, date:, before:, after:, or plain text."
                                            rows=3
                                        />
                                    </div>
                                    <div class="flex gap-2 mt-4">
                                        <Button
                                            variant=ButtonVariant::Primary
                                            on_click=Callback::new(move |_| save_click.run(()))
                                        >
                                            "Save"
                                        </Button>
                                        <Button
                                            on_click=Callback::new(move |_| set_show_form.set(false))
                                        >
                                            "Cancel"
                                        </Button>
                                    </div>
                                </CardBody>
                            </Card>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}

                    // Results for the selected folder.
                    {move || if selected_id.get().is_some() {
                        view! {
                            <Card>
                                <CardHeader title="Results".to_string()>
                                    {if evaluating.get() {
                                        view! { <span class="text-xs text-gray-400">"Evaluating…"</span> }.into_any()
                                    } else {
                                        view! { <span class="text-xs text-gray-400">{results.get().len()} " note(s)"</span> }.into_any()
                                    }}
                                </CardHeader>
                                <CardBody>
                                    {move || {
                                        let hits = results.get();
                                        if hits.is_empty() && !evaluating.get() {
                                            view! {
                                                <EmptyState
                                                    icon=Icon::Search
                                                    title="No matches".to_string()
                                                    description="Try widening the query.".to_string()
                                                />
                                            }.into_any()
                                        } else {
                                            hits.into_iter().map(|hit| {
                                                let path = hit.path.clone();
                                                let title = hit.title.clone();
                                                let folder = hit.folder.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="w-full text-left px-3 py-2 rounded-md hover:bg-gray-800 flex items-center gap-2"
                                                        on:click=move |_| open_tab(ws, &path)
                                                    >
                                                        <span aria-hidden="true">{render_icon_view(Icon::FileText)}</span>
                                                        <span class="flex-1 min-w-0">
                                                            <span class="block text-sm text-gray-200 truncate">{title}</span>
                                                            <span class="block text-xs text-gray-500 truncate">{folder}</span>
                                                        </span>
                                                    </button>
                                                }
                                            }).collect_view().into_any()
                                        }
                                    }}
                                </CardBody>
                                <div class="px-4 pb-4">
                                    <Button
                                        size=ButtonSize::Sm
                                        variant=ButtonVariant::Destructive
                                        on_click=Callback::new(move |_| delete_click.run(()))
                                    >
                                        "Delete folder"
                                    </Button>
                                </div>
                            </Card>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </div>
            </div>
        </div>
    }
}
