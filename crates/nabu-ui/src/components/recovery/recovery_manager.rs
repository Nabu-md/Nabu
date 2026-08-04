//! # Recovery Manager / Snapshot Browser
//!
//! A dedicated screen for inspecting every recoverable snapshot across the
//! vault. Lists all notes that have version history, shows retention
//! summaries, and provides a per-note timeline with restore / duplicate /
//! diff actions (reusing the version timeline renderer from
//! [`crate::components::recovery::version_history`]).

use crate::components::recovery::version_history::{NoteSummary, VersionMeta, absolute_time};
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::use_toast;
use crate::components::ui::info::EmptyState;
use leptos::prelude::*;
use std::path::Path;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Default)]
struct ManagerState {
    notes: Vec<NoteSummary>,
    search: String,
    /// Expanded note path → its version timeline.
    expanded: Option<String>,
    versions: Vec<VersionMeta>,
    /// Dialog: restore confirmation.
    confirm_restore: bool,
    selected_version: Option<String>,
}

fn fetch_notes(state: RwSignal<ManagerState>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("versions_all", empty_args).await;
        if let Ok(notes) = serde_wasm_bindgen::from_value::<Vec<NoteSummary>>(result) {
            state.update(|s| s.notes = notes);
        }
    });
}

fn expand_note(path: String, state: RwSignal<ManagerState>) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).unwrap();
        let result = crate::ipc::tauri_invoke("versions_list", args).await;
        match serde_wasm_bindgen::from_value::<Vec<VersionMeta>>(result) {
            Ok(versions) => {
                let newest = versions.last().map(|v| v.id.clone());
                state.update(|s| {
                    s.expanded = Some(path);
                    s.versions = versions;
                    s.selected_version = newest;
                });
            }
            Err(_) => state.update(|s| {
                s.expanded = None;
                s.versions.clear();
            }),
        }
    });
}

/// The Recovery Manager screen (ViewMode::Recovery).
#[component]
pub fn RecoveryManager() -> impl IntoView {
    let state = RwSignal::new(ManagerState::default());
    let toasts = use_toast();

    // Dialog open signal — synced from state so the ConfirmDialog's internal
    // `open.set(false)` (overlay / Escape / cancel) stays consistent.
    let restore_open = RwSignal::new(false);
    Effect::new(move |_| {
        let s = state.get();
        restore_open.set(s.confirm_restore);
    });

    fetch_notes(state);

    let on_search = move |ev| {
        let q = event_target_value(&ev);
        state.update(|s| s.search = q);
    };

    let filtered = move || {
        let s = state.get();
        let q = s.search.to_lowercase();
        let mut notes = s.notes.clone();
        if !q.is_empty() {
            notes.retain(|n| n.path.to_lowercase().contains(&q));
        }
        notes
    };

    let confirm_restore = Callback::new(move |_| {
        state.update(|s| s.confirm_restore = false);
        let Some(path) = state.get().expanded.clone() else { return; };
        let Some(id) = state.get().selected_version.clone() else { return; };
        let toasts_c = toasts;
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path, "id": id }))
                    .unwrap();
            let result = crate::ipc::tauri_invoke("versions_restore", args).await;
            match serde_wasm_bindgen::from_value::<()>(result) {
                Ok(()) => {
                    toasts_c.success("Restored", "The note was restored to this version.");
                }
                Err(e) => toasts_c.error("Restore", e.to_string()),
            }
        });
    });

    view! {
        <div class="recovery-manager flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            <div class="flex-1 overflow-y-auto p-4">
                <div class="max-w-4xl mx-auto space-y-4">
                    <div>
                        <h2 class="text-lg font-semibold text-gray-50">"Recovery Manager"</h2>
                        <p class="text-xs text-gray-500">
                            "Every note with version history, snapshotted automatically on save. Restore or duplicate any version — nothing is lost."
                        </p>
                    </div>

                    <input
                        type="text"
                        placeholder="Search notes with snapshots…"
                        class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        on:input=on_search
                    />

                    {move || {
                        let notes = filtered();
                        if notes.is_empty() {
                            view! {
                                <EmptyState
                                    icon=crate::components::ui::icons::Icon::LifeBuoy
                                    title="Nothing to recover yet".to_string()
                                    description="Notes appear here once they have been saved at least once.".to_string()
                                ></EmptyState>
                            }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800 rounded-lg border border-gray-800">
                                    {notes.into_iter().map(|note| {
                                        let path = note.path.clone();
                                        // Clone for the closure so `path` stays usable below.
                                        let path_check = path.clone();
                                        let is_expanded = move || state.get().expanded.as_deref() == Some(path_check.as_str());
                                        let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                                        view! {
                                            <div>
                                                <div class="flex items-center gap-2 px-3 py-2 hover:bg-gray-800/40 transition-colors">
                                                    <button
                                                        class="flex-1 text-left"
                                                        on:click=move |_| {
                                                            if state.get().expanded.as_deref() == Some(path.as_str()) {
                                                                state.update(|s| s.expanded = None);
                                                            } else {
                                                                expand_note(path.clone(), state);
                                                            }
                                                        }
                                                    >
                                                        <div class="flex items-center gap-2">
                                                            <span class="text-sm font-medium truncate">{name.clone()}</span>
                                                            <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">{note.version_count} versions</span>
                                                        </div>
                                                        <div class="text-xs text-gray-500 mt-0.5 truncate">
                                                            {note.path.clone()}
                                                            {note.last_snapshot_at.map(|t| format!(" — last saved {}", absolute_time(&t))).unwrap_or_default()}
                                                        </div>
                                                    </button>
                                                </div>
                                                {move || if is_expanded() {
                                                    let versions = state.get().versions.clone();
                                                    let expanded_path = state.get().expanded.clone().unwrap_or_default();
                                                    view! {
                                                        <div class="border-t border-gray-800 bg-gray-900/40">
                                                            <div class="px-3 py-2 text-xs text-gray-400">"Versions (newest first):"</div>
                                                            <div class="divide-y divide-gray-800">
                                                                {versions.into_iter().rev().map(|version| {
                                                                    let id = version.id.clone();
                                                                    // Fresh clones for each button so no `move` closure steals `id` or
                                                                    // `expanded_path` from the enclosing `Fn`/`FnMut` closures.
                                                                    let id_check = id.clone();
                                                                    let id_select = id.clone();
                                                                    let id_restore = id.clone();
                                                                    let id_dup = id.clone();
                                                                    let expanded_dup = expanded_path.clone();
                                                                    let is_selected = move || state.get().selected_version.as_deref() == Some(id_check.as_str());
                                                                    view! {
                                                                        <div class=move || format!("px-3 py-2 flex items-center gap-2 {}", if is_selected() { "bg-blue-900/20" } else { "" })>
                                                                            <button
                                                                                class="flex-1 text-left"
                                                                                on:click=move |_| {
                                                                                    state.update(|s| s.selected_version = Some(id_select.clone()));
                                                                                }
                                                                            >
                                                                                <div class="text-sm">{version.summary.clone().unwrap_or_else(|| "Untitled version".to_string())}</div>
                                                                                <div class="text-xs text-gray-500">{absolute_time(&version.created_at)}</div>
                                                                            </button>
                                                                            <Button
                                                                                size=ButtonSize::Sm
                                                                                on_click=Callback::new(move |_| {
                                                                                    state.update(|s| s.selected_version = Some(id_restore.clone()));
                                                                                    state.update(|s| s.confirm_restore = true);
                                                                                })
                                                                            >
                                                                                "Restore"
                                                                            </Button>
                                                                            <Button
                                                                                size=ButtonSize::Sm
                                                                                variant=ButtonVariant::Outline
                                                                                on_click=Callback::new(move |_| {
                                                                                    // Clone into locals so the callback stays FnMut (the
                                                                                    // inner async block can then own them).
                                                                                    let expanded_arg = expanded_dup.clone();
                                                                                    let id_arg = id_dup.clone();
                                                                                    let dest_path = {
                                                                                        let base = Path::new(&expanded_arg);
                                                                                        base.file_stem().map(|s| format!("{}-copy.md", s.to_string_lossy())).unwrap_or_else(|| "copy.md".to_string())
                                                                                    };
                                                                                    let toasts_dup = toasts;
                                                                                    spawn_local(async move {
                                                                                        let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                                                                                            "path": expanded_arg,
                                                                                            "id": id_arg,
                                                                                            "dest": dest_path,
                                                                                        })).unwrap();
                                                                                        let result = crate::ipc::tauri_invoke("versions_duplicate", args).await;
                                                                                        match serde_wasm_bindgen::from_value::<()>(result) {
                                                                                            Ok(()) => toasts_dup.success("Duplicated", "Created a new note from this version."),
                                                                                            Err(e) => toasts_dup.error("Duplicate", e.to_string()),
                                                                                        }
                                                                                    });
                                                                                })
                                                                            >
                                                                                "Duplicate"
                                                                            </Button>
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
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            <ConfirmDialog
                open=restore_open
                title="Restore this version?".to_string()
                message="The note will be replaced with this snapshot. The current content is snapshotted first, and you can undo the restore.".to_string()
                confirm_label="Restore"
                cancel_label="Cancel"
                danger=false
                on_confirm=confirm_restore
                on_cancel=Callback::new(move |_| state.update(|s| s.confirm_restore = false))
            />
        </div>
    }
}
