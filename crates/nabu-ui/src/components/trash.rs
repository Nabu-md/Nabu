//! # Trash / Recycle Bin — Frontend
//!
//! A full screen for reviewing and recovering deleted vault items. Deleted
//! notes and folders are never destroyed immediately — they live in the vault
//! trash (`.nabu/trash`) until the user restores them, the retention period
//! elapses, or the user explicitly empties the trash.
//!
//! Features:
//! - list with per-item preview, deletion date and original location
//! - search, sorting and filtering
//! - single and multi-select restore / permanent delete
//! - confirmation dialogs for every irreversible action
//! - "undo" toast after restore so an accidental restore can be reversed
//! - keyboard shortcuts: Delete / Shift+Delete (delete selection, confirmed),
//!   Cmd/Ctrl+Shift+R (restore selection), Cmd/Ctrl+Shift+Backspace (empty)
//!
//! ## Reactivity note
//!
//! Toast and history contexts are `Copy` and are captured at render time, then
//! threaded into async tasks and the window keydown listener as plain values —
//! never via `expect_context` inside a `spawn_local` future or a raw DOM
//! callback, which have no reactive owner.

use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::{use_toast, ToastAction, ToastContext, ToastKind};
use crate::components::ui::selection::{Checkbox, Segmented, SegmentedOption, Select, SelectOption};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use wasm_bindgen::prelude::JsCast;
use wasm_bindgen_futures::spawn_local;

// ── Types ─────────────────────────────────────────────────────────────

/// Backend trash manifest record (mirrors `TrashRecord` in the Tauri layer).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrashRecord {
    pub trash_path: String,
    pub original_path: String,
    #[serde(default)]
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub file_count: usize,
    #[serde(default)]
    pub preview: Option<String>,
}

impl TrashRecord {
    /// Display name — the original basename of the trashed item (the manifest
    /// keeps the original path intact, so this is always the real name, never
    /// the timestamp-prefixed trash storage name).
    fn display_name(&self) -> String {
        Path::new(&self.original_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string())
    }

    fn icon(&self) -> crate::components::ui::icons::Icon {
        if self.is_folder {
            crate::components::ui::icons::Icon::Folder
        } else if self.original_path.ends_with(".md") {
            crate::components::ui::icons::Icon::FileText
        } else {
            crate::components::ui::icons::Icon::File
        }
    }
}

/// Filter bucket for the trash list.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum TrashFilter {
    #[default]
    All,
    Notes,
    Folders,
    Attachments,
}

impl TrashFilter {
    fn matches(self, record: &TrashRecord) -> bool {
        match self {
            TrashFilter::All => true,
            TrashFilter::Notes => !record.is_folder && record.original_path.ends_with(".md"),
            TrashFilter::Folders => record.is_folder,
            TrashFilter::Attachments => {
                !record.is_folder && !record.original_path.ends_with(".md")
            }
        }
    }

    fn from_label(label: &str) -> Self {
        match label {
            "notes" => TrashFilter::Notes,
            "folders" => TrashFilter::Folders,
            "attachments" => TrashFilter::Attachments,
            _ => TrashFilter::All,
        }
    }
}

/// Sort key for the trash list.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum TrashSort {
    #[default]
    Name,
    DeletedAt,
    OriginalPath,
    Size,
}

/// Human-readable relative time ("5m ago", "3d ago").
///
/// Uses the JS clock rather than `chrono::Utc::now()`, which panics on
/// wasm32-unknown-unknown unless chrono is built with the `wasmbind` feature.
/// Chrono is only used here for RFC 3339 parsing, which needs no clock.
fn relative_time(rfc3339: &str) -> String {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(rfc3339) else {
        return "recently".to_string();
    };
    let now_ms = js_sys::Date::now() as i64;
    let then_ms = parsed.timestamp_millis();
    let secs = ((now_ms - then_ms) / 1000).max(0);
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

// ── State ─────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct TrashState {
    items: Vec<TrashRecord>,
    selected: Vec<String>,
    filter: TrashFilter,
    sort: TrashSort,
    sort_ascending: bool,
    query: String,
    preview: Option<String>,
    /// Dialog: permanently delete a single item (trash_path).
    confirm_delete: Option<String>,
    /// Dialog: permanently delete the selected items.
    confirm_delete_selected: bool,
    /// Dialog: empty the whole trash.
    confirm_empty: bool,
}

impl TrashState {
    fn sorted(&self) -> Vec<TrashRecord> {
        let query = self.query.to_lowercase();
        let mut items: Vec<TrashRecord> = self
            .items
            .iter()
            .filter(|r| self.filter.matches(r))
            .filter(|r| {
                if query.is_empty() {
                    return true;
                }
                r.display_name().to_lowercase().contains(&query)
                    || r.original_path.to_lowercase().contains(&query)
                    || r
                        .preview
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            let ord = match self.sort {
                TrashSort::Name => a.display_name().cmp(&b.display_name()),
                TrashSort::DeletedAt => a.deleted_at.cmp(&b.deleted_at),
                TrashSort::OriginalPath => a.original_path.cmp(&b.original_path),
                TrashSort::Size => a.file_count.cmp(&b.file_count),
            };
            if self.sort_ascending {
                ord
            } else {
                ord.reverse()
            }
        });
        items
    }

    fn selected_items(&self) -> Vec<TrashRecord> {
        self.items
            .iter()
            .filter(|r| self.selected.contains(&r.trash_path))
            .cloned()
            .collect()
    }
}

// ── Actions (contexts passed by value — safe from any caller) ─────────

fn fetch_trash(state: RwSignal<TrashState>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("trash_list", empty_args).await;
        if let Ok(records) = serde_wasm_bindgen::from_value::<Vec<TrashRecord>>(result) {
            state.update(|s| {
                s.items = records;
                s.selected.retain(|tp| s.items.iter().any(|r| r.trash_path == *tp));
                if let Some(p) = &s.preview {
                    if !s.items.iter().any(|r| r.trash_path == *p) {
                        s.preview = None;
                    }
                }
            });
        }
    });
}

/// Restores the selected items and shows an undo toast. On undo the backend
/// history manager re-trashes them (reverse of the restore entry).
fn restore_selected(
    state: RwSignal<TrashState>,
    toasts: ToastContext,
    history: crate::history::HistoryContext,
) {
    let selected = state.get().selected.clone();
    if selected.is_empty() {
        return;
    }
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "trash_paths": selected }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("trash_restore_many", args).await;
        match serde_wasm_bindgen::from_value::<Vec<String>>(result) {
            Ok(restored) if !restored.is_empty() => {
                let count = restored.len();
                let toasts_undo = toasts;
                let history_undo = history;
                toasts.push_with_action(
                    ToastKind::Success,
                    format!("Restored {count} item(s)"),
                    "The item(s) are back in their original location.",
                    ToastAction::new(
                        "Undo",
                        Callback::new(move |_| crate::history::undo(history_undo, toasts_undo)),
                    ),
                );
                fetch_trash(state);
            }
            _ => toasts.error("Restore", "Could not restore the selected item(s)"),
        }
    });
}

/// Permanently deletes the selected items (irreversible — the caller has
/// already shown the confirmation dialog).
fn delete_selected(state: RwSignal<TrashState>, toasts: ToastContext) {
    let selected = state.get().selected.clone();
    if selected.is_empty() {
        return;
    }
    delete_paths(state, toasts, selected);
}

/// Permanently deletes the given trashed paths. Shared by the single-item and
/// batch delete flows so the irreversible `trash_delete` IPC call lives in one
/// place.
fn delete_paths(state: RwSignal<TrashState>, toasts: ToastContext, paths: Vec<String>) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "trash_paths": paths }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("trash_delete", args).await;
        match serde_wasm_bindgen::from_value::<usize>(result) {
            Ok(n) => {
                let message = if n == 1 {
                    "Permanently deleted 1 item".to_string()
                } else {
                    format!("Permanently deleted {n} items")
                };
                toasts.warning("Trash", message);
                fetch_trash(state);
            }
            Err(_) => toasts.error("Trash", "Could not delete the selected item(s)"),
        }
    });
}

/// Empties the whole trash (irreversible — caller has confirmed).
fn empty_trash(state: RwSignal<TrashState>, toasts: ToastContext) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("trash_empty", empty_args).await;
        match serde_wasm_bindgen::from_value::<usize>(result) {
            Ok(n) => {
                toasts.warning("Trash", format!("Emptied trash — {n} item(s) permanently deleted"));
                fetch_trash(state);
            }
            Err(_) => toasts.error("Trash", "Could not empty the trash"),
        }
    });
}

/// Returns `true` when keyboard focus is inside an editable element, so Delete
/// and shortcut keys never hijack typing (e.g. in the search box).
fn focus_is_editable() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(active) = document.active_element() else {
        return false;
    };
    let tag = active.tag_name().to_ascii_lowercase();
    tag == "textarea" || tag == "input" || tag == "select"
}

// ── Component ─────────────────────────────────────────────────────────

/// The Trash screen — recoverable deletion, restore, and permanent removal.
#[component]
pub fn Trash() -> impl IntoView {
    let state = RwSignal::new(TrashState::default());
    let toasts = use_toast();
    let history = crate::history::use_history();

    fetch_trash(state);

    // Dialog open signals — synced from state so the ConfirmDialog's internal
    // `open.set(false)` (overlay / Escape / cancel) stays consistent.
    let delete_open = RwSignal::new(false);
    let delete_selected_open = RwSignal::new(false);
    let empty_open = RwSignal::new(false);
    Effect::new(move |_| {
        let s = state.get();
        delete_open.set(s.confirm_delete.is_some());
        delete_selected_open.set(s.confirm_delete_selected);
        empty_open.set(s.confirm_empty);
    });

    // Reactive dialog messages (kept fresh as the selection changes).
    let delete_message = Memo::new(move |_| {
        let s = state.get();
        match &s.confirm_delete {
            Some(tp) => s
                .items
                .iter()
                .find(|r| r.trash_path == *tp)
                .map(|r| {
                    format!(
                        "'{}' will be permanently deleted ({} file(s)). This cannot be undone.",
                        r.display_name(),
                        r.file_count
                    )
                })
                .unwrap_or_default(),
            None => String::new(),
        }
    });
    let delete_selected_message = Memo::new(move |_| {
        let s = state.get();
        let selected = s.selected_items();
        let names: Vec<String> = selected.iter().map(|r| r.display_name()).collect();
        let count = names.len();
        let files: usize = selected.iter().map(|r| r.file_count).sum();
        format!("{count} item(s) ({files} file(s)) will be permanently deleted: {}. This cannot be undone.", names.join(", "))
    });
    let empty_message = Memo::new(move |_| {
        let s = state.get();
        let count = s.items.len();
        let files: usize = s.items.iter().map(|r| r.file_count).sum();
        format!("{count} item(s) ({files} file(s)) in the trash will be permanently deleted. This cannot be undone.")
    });

    // Keyboard shortcuts: Cmd/Ctrl+Shift+R restore, Cmd/Ctrl+Shift+Backspace
    // empty, Delete/Backspace delete selection (confirmed). Uses the Leptos
    // untyped window listener, which is installed per-mount and removed on
    // cleanup — so the captured (Copy) contexts always match the live Trash
    // screen, and no shortcuts fire when the screen is unmounted.
    let shortcut_handle = window_event_listener_untyped("keydown", move |ev: web_sys::Event| {
        let ev = ev.unchecked_ref::<web_sys::KeyboardEvent>();
        if focus_is_editable() {
            return;
        }
        let meta = ev.meta_key() || ev.ctrl_key();
        let shift = ev.shift_key();
        let key = ev.key();
        if meta && shift && key.eq_ignore_ascii_case("r") {
            ev.prevent_default();
            restore_selected(state, toasts, history);
        } else if meta && shift && key == "Backspace" {
            ev.prevent_default();
            if !state.get().items.is_empty() {
                state.update(|s| s.confirm_empty = true);
            }
        } else if !meta && (key == "Delete" || key == "Backspace") {
            ev.prevent_default();
            if !state.get().selected.is_empty() {
                state.update(|s| s.confirm_delete_selected = true);
            }
        }
    });
    on_cleanup(move || shortcut_handle.remove());

    // Undo/redo from anywhere (e.g. the sidebar delete → undo toast) changes
    // what is in the trash. Listen for the window event dispatched by
    // `crate::history::{undo,redo}` and refresh the list so it never goes
    // stale after a reversible operation.
    let history_handle = window_event_listener_untyped("nabu:history-changed", move |_| {
        fetch_trash(state);
    });
    on_cleanup(move || history_handle.remove());

    // The list, footer and dialogs all read `state.get()` directly (not a
    // memo over `sorted()`), so any change — including selection toggles that
    // do not affect the sort order — re-renders the rows immediately.
    let selected_count = move || state.get().selected.len();
    let total_files = move || state.get().items.iter().map(|r| r.file_count).sum::<usize>();

    let toggle_select = move |trash_path: String| {
        state.update(|s| {
            if let Some(pos) = s.selected.iter().position(|t| *t == trash_path) {
                s.selected.remove(pos);
            } else {
                s.selected.push(trash_path);
            }
        });
    };

    let toggle_select_all = move || {
        state.update(|s| {
            if !s.sorted().is_empty() && s.selected.len() == s.sorted().len() {
                s.selected.clear();
            } else {
                s.selected = s.sorted().into_iter().map(|r| r.trash_path).collect();
            }
        });
    };

    let set_preview = move |trash_path: String| {
        state.update(|s| s.preview = Some(trash_path));
    };

    // Filter / sort bound signals so the controls reflect their state.
    let filter_value = RwSignal::new("all".to_string());
    let sort_value = RwSignal::new("name".to_string());
    let on_filter_change = move |filter: String| {
        filter_value.set(filter.clone());
        state.update(|s| s.filter = TrashFilter::from_label(&filter));
    };
    let on_sort_change = move |field: TrashSort| {
        state.update(|s| {
            if s.sort == field {
                s.sort_ascending = !s.sort_ascending;
            } else {
                s.sort = field;
                s.sort_ascending = true;
            }
        });
    };

    let on_query = move |ev| {
        let q = event_target_value(&ev);
        state.update(|s| s.query = q);
    };

    let confirm_delete_single = Callback::new(move |_| {
        if let Some(tp) = state.get().confirm_delete.clone() {
            state.update(|s| s.confirm_delete = None);
            delete_paths(state, toasts, vec![tp]);
        }
    });

    let confirm_delete_selected_cb = Callback::new(move |_| {
        state.update(|s| s.confirm_delete_selected = false);
        delete_selected(state, toasts);
    });
    let confirm_empty_cb = Callback::new(move |_| {
        state.update(|s| s.confirm_empty = false);
        empty_trash(state, toasts);
    });

    view! {
        <div class="trash-screen flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            // ── Left: item list ──
            <div class="flex-none w-96 border-r border-gray-800 flex flex-col min-w-0">
                <div class="px-4 py-3 border-b border-gray-800 flex items-center gap-2">
                    <span class="text-lg" aria-hidden="true">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Trash2)}</span>
                    <div class="flex-1 min-w-0">
                        <h2 class="text-base font-semibold text-gray-50">"Trash"</h2>
                        <p class="text-xs text-gray-500">{move || format!("{} item(s) · {} file(s)", state.get().items.len(), total_files())}</p>
                    </div>
                    {move || {
                        let has_items = !state.get().items.is_empty();
                        view! {
                            <Button
                                variant=ButtonVariant::Destructive
                                size=ButtonSize::Sm
                                disabled=!has_items
                                on_click=Callback::new(move |_| state.update(|s| s.confirm_empty = true))
                            >
                                "Empty Trash"
                            </Button>
                        }.into_any()
                    }}
                </div>

                // Toolbar: search + filters + sort
                <div class="px-3 py-2 border-b border-gray-800 flex flex-col gap-2">
                    <input
                        type="text"
                        placeholder="Search trash…"
                        class="flex-1 bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        on:input=on_query
                    />
                    <div class="flex items-center justify-between gap-2">
                        <Segmented
                            options=vec![
                                SegmentedOption::new("all", "All"),
                                SegmentedOption::new("notes", "Notes"),
                                SegmentedOption::new("folders", "Folders"),
                                SegmentedOption::new("attachments", "Files"),
                            ]
                            selected=filter_value
                            on_change=Callback::new(on_filter_change)
                        />
                        <Select
                            options=vec![
                                SelectOption::new("name", "Sort: Name"),
                                SelectOption::new("deleted_at", "Sort: Deleted"),
                                SelectOption::new("original", "Sort: Location"),
                                SelectOption::new("size", "Sort: Files"),
                            ]
                            value=sort_value
                            on_change=Callback::new(move |v: String| {
                                let field = match v.as_str() {
                                    "deleted_at" => TrashSort::DeletedAt,
                                    "original" => TrashSort::OriginalPath,
                                    "size" => TrashSort::Size,
                                    _ => TrashSort::Name,
                                };
                                sort_value.set(v);
                                on_sort_change(field);
                            })
                        />
                    </div>
                </div>

                // Batch actions bar
                {move || {
                    let count = selected_count();
                    if count > 0 {
                        let names: Vec<String> = state.get().selected_items().into_iter().map(|r| r.display_name()).collect();
                        let preview_text = if names.len() > 3 {
                            format!("{count} selected")
                        } else {
                            names.join(", ")
                        };
                        view! {
                            <div class="flex items-center gap-2 px-3 py-2 bg-blue-900/30 border-b border-blue-800/30">
                                <span class="text-xs text-blue-400">{move || format!("{count} selected")}</span>
                                <div class="flex-1" />
                                <Button size=ButtonSize::Sm on_click=Callback::new(move |_| restore_selected(state, toasts, history))>
                                    {crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Undo)} Restore
                                </Button>
                                <Button
                                    variant=ButtonVariant::Destructive
                                    size=ButtonSize::Sm
                                    on_click=Callback::new(move |_| state.update(|s| s.confirm_delete_selected = true))
                                >
                                    {crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Trash2)} Delete
                                </Button>
                                <span class="text-xs text-gray-500 max-w-40 truncate" title=preview_text.clone()>{preview_text.clone()}</span>
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }
                }}

                // List
                <div class="flex-1 overflow-y-auto">
                    {move || {
                        let items = state.get().sorted();
                        if items.is_empty() {
                            view! {
                                <div class="empty-state">
                                    <div class="empty-state-icon">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Trash2)}</div>
                                    <div class="empty-state-title">"Trash is empty"</div>
                                    <div class="empty-state-desc">"Deleted notes and folders appear here and can be restored before they are permanently removed."</div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800">
                                    {items.iter().map(|record| {
                                        // Clone the (non-Copy) values each handler needs so every
                                        // `move` closure captures its own copy.
                                        let trash_path = record.trash_path.clone();
                                        let path_click = trash_path.clone();
                                        let path_checkbox = trash_path.clone();
                                        let path_restore = trash_path.clone();
                                        let path_delete = trash_path.clone();
                                        let name = record.display_name();
                                        let is_folder = record.is_folder;
                                        let icon = record.icon();
                                        let original = record.original_path.clone();
                                        let original_display = original.clone();
                                        let original_title = original.clone();
                                        let deleted_at = record.deleted_at.clone().unwrap_or_default();
                                        let deleted_at_display = deleted_at.clone();
                                        let relative = record.deleted_at.as_deref().map(relative_time).unwrap_or_default();
                                        let file_count = record.file_count;
                                        let selected = state.get().selected.contains(&trash_path);
                                        let is_preview = state.get().preview.as_deref() == Some(trash_path.as_str());
                                        view! {
                                            <div
                                                class=move || format!(
                                                    "px-3 py-2 cursor-pointer border-l-2 transition-colors {}",
                                                    if selected { "bg-gray-800 border-l-blue-500" } else if is_preview { "bg-gray-900 border-l-gray-500" } else { "border-l-transparent hover:bg-gray-800/60" }
                                                )
                                                on:click=move |_| set_preview(path_click.clone())
                                            >
                                                <div class="flex items-center gap-2">
                                                    <Checkbox
                                                        checked=RwSignal::new(selected)
                                                        label="".to_string()
                                                        on_change=Callback::new(move |_| toggle_select(path_checkbox.clone()))
                                                    />
                                                    <span aria-hidden="true">{crate::components::ui::icons::render_icon_view(icon)}</span>
                                                    <span class="text-sm font-medium truncate flex-1">{name.clone()}</span>
                                                    {if is_folder {
                                                        view! { <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">"Folder"</span> }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }}
                                                    {if file_count > 1 {
                                                        view! { <span class="text-xs text-gray-500">{format!("{file_count} files")}</span> }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }}
                                                </div>
                                                <div class="flex items-center gap-2 mt-1 pl-7">
                                                    <span class="text-xs text-gray-500 truncate max-w-52" title=original_title>{original_display}</span>
                                                    <span class="text-xs text-gray-600">"•"</span>
                                                    <span class="text-xs text-gray-500" title=deleted_at_display.clone()>{relative}</span>
                                                    <div class="flex-1" />
                                                    <button
                                                        class="px-2 py-0.5 text-xs rounded bg-green-700/80 hover:bg-green-600 text-white transition-colors"
                                                        on:click=move |_| {
                                                            restore_one(path_restore.clone(), state, toasts, history);
                                                        }
                                                    >
                                                        "Restore"
                                                    </button>
                                                    <button
                                                        class="px-2 py-0.5 text-xs rounded bg-gray-700 hover:bg-red-700 text-gray-200 hover:text-white transition-colors"
                                                        on:click=move |_| state.update(|s| s.confirm_delete = Some(path_delete.clone()))
                                                    >
                                                        "Delete"
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                <div class="flex items-center justify-between px-3 py-2 border-t border-gray-800 text-xs text-gray-500">
                    <button class="hover:text-gray-300" on:click=move |_| toggle_select_all()>
                        {move || {
                            let total = state.get().sorted().len();
                            if total > 0 && state.get().selected.len() == total { "Deselect all" } else { "Select all" }
                        }}
                    </button>
                    <span>{move || format!("{} selected", selected_count())}</span>
                </div>
            </div>

            // ── Right: preview ──
            <div class="flex-1 overflow-y-auto p-4 min-w-0">
                {move || {
                    let preview_path = state.get().preview.clone();
                    match preview_path {
                        Some(path) => {
                            // Bind the snapshot so the borrow lives long enough.
                            let snapshot = state.get();
                            let record = snapshot.items.iter().find(|r| r.trash_path == path);
                            match record {
                                Some(record) => view! {
                                    <TrashPreview record=record.clone() state=state />
                                }.into_any(),
                                None => view! { <div class="empty-state"><div class="empty-state-desc">"Item not found"</div></div> }.into_any(),
                            }
                        }
                        None => view! {
                            <div class="empty-state">
                                <div class="empty-state-icon">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Eye)}</div>
                                <div class="empty-state-title">"Select an item"</div>
                                <div class="empty-state-desc">"Preview the item, see where it came from, restore it, or remove it permanently."</div>
                            </div>
                        }.into_any(),
                    }
                }}
            </div>

            // ── Confirmation dialogs ──
            <ConfirmDialog
                open=delete_open
                title="Permanently delete this item?".to_string()
                message="".to_string()
                message_signal=delete_message
                confirm_label="Delete Forever"
                cancel_label="Cancel"
                danger=true
                on_confirm=confirm_delete_single
                on_cancel=Callback::new(move |_| state.update(|s| s.confirm_delete = None))
            />

            <ConfirmDialog
                open=delete_selected_open
                title="Delete selected items?".to_string()
                message="".to_string()
                message_signal=delete_selected_message
                confirm_label="Delete Forever"
                cancel_label="Cancel"
                danger=true
                on_confirm=confirm_delete_selected_cb
                on_cancel=Callback::new(move |_| state.update(|s| s.confirm_delete_selected = false))
            />

            <ConfirmDialog
                open=empty_open
                title="Empty Trash?".to_string()
                message="".to_string()
                message_signal=empty_message
                confirm_label="Empty Trash"
                cancel_label="Cancel"
                danger=true
                on_confirm=confirm_empty_cb
                on_cancel=Callback::new(move |_| state.update(|s| s.confirm_empty = false))
            />
        </div>
    }
}

// ── Preview panel ─────────────────────────────────────────────────────

/// Restores a single trashed item via `note_restore` (registers an undoable
/// history entry on the backend). The list is refreshed only after the restore
/// completes, and the success/error toast is shown from the resolved result —
/// never optimistically.
fn restore_one(
    trash_path: String,
    state: RwSignal<TrashState>,
    toasts: ToastContext,
    history: crate::history::HistoryContext,
) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "trash_path": trash_path }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("note_restore", args).await;
        match serde_wasm_bindgen::from_value::<()>(result) {
            Ok(()) => {
                let toasts_undo = toasts;
                let history_undo = history;
                toasts.push_with_action(
                    ToastKind::Success,
                    "Restored".to_string(),
                    "The item is back in its original location.".to_string(),
                    ToastAction::new(
                        "Undo",
                        Callback::new(move |_| crate::history::undo(history_undo, toasts_undo)),
                    ),
                );
                fetch_trash(state);
            }
            Err(_) => toasts.error("Restore", "Could not restore this item"),
        }
    });
}

#[component]
fn TrashPreview(record: TrashRecord, state: RwSignal<TrashState>) -> impl IntoView {
    let toasts = use_toast();
    let history = crate::history::use_history();
    let name = record.display_name();
    let original = record.original_path.clone();
    let is_folder = record.is_folder;
    let file_count = record.file_count;
    let preview = record.preview.clone().unwrap_or_default();
    let restore_path = record.trash_path.clone();
    let delete_path = record.trash_path.clone();
    // Show a friendly relative date (consistent with the list rows), with the
    // full timestamp available on hover.
    let deleted_at_full = record.deleted_at.clone().unwrap_or_else(|| "Unknown".to_string());
    let deleted_at_relative = record
        .deleted_at
        .as_deref()
        .map(relative_time)
        .unwrap_or_else(|| "Unknown".to_string());

    let restore_cb = Callback::new(move |_| {
        restore_one(restore_path.clone(), state, toasts, history);
    });

    let delete_cb = Callback::new(move |_| {
        state.update(|s| s.confirm_delete = Some(delete_path.clone()));
    });

    view! {
        <div class="trash-preview space-y-4">
            <div class="flex items-start gap-3">
                <span class="text-3xl" aria-hidden="true">{crate::components::ui::icons::render_icon_view(record.icon())}</span>
                <div class="flex-1 min-w-0">
                    <h3 class="text-lg font-semibold text-gray-50 truncate">{name.clone()}</h3>
                    <p class="text-xs text-gray-500 truncate" title=original.clone()>{original.clone()}</p>
                </div>
            </div>

            <div class="grid grid-cols-2 gap-3 text-sm">
                <div>
                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Kind"</label>
                    <p class="text-gray-300">
                        {if is_folder { "Folder" } else { "File" }}
                    </p>
                </div>
                <div>
                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Files"</label>
                    <p class="text-gray-300">{file_count}</p>
                </div>
                <div>
                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Deleted"</label>
                    <p class="text-gray-300" title=deleted_at_full>{deleted_at_relative}</p>
                </div>
                <div>
                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Original location"</label>
                    <p class="text-gray-300 text-xs truncate" title=original.clone()>{original.clone()}</p>
                </div>
            </div>

            {if !preview.is_empty() {
                view! {
                    <div>
                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Preview"</label>
                        <pre class="mt-1 text-xs text-gray-300 bg-gray-900 p-3 rounded-lg overflow-auto max-h-64 whitespace-pre-wrap border border-gray-800">{preview}</pre>
                    </div>
                }.into_any()
            } else if is_folder {
                view! {
                    <div class="callout callout-info">
                        "This folder and its contents were moved to trash together."
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            <div class="flex items-center gap-2 pt-3 border-t border-gray-800">
                <Button on_click=restore_cb>
                    {crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Undo)} Restore
                </Button>
                <Button variant=ButtonVariant::Destructive on_click=delete_cb>
                    {crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Trash2)} Delete Forever
                </Button>
            </div>
        </div>
    }
}
