//! # Recovery Manager / Snapshot Browser (Dioxus)
//!
//! A dedicated screen for inspecting every recoverable snapshot across the
//! vault. Lists all notes that have version history, shows retention
//! summaries, and provides a per-note timeline with restore / duplicate /
//! diff actions (reusing the version timeline renderer from
//! [`crate::components::recovery::version_history`]).
//!
//! Migration notes (LePtOS → Dioxus):
//! - `RwSignal<T>` → `Signal<T>` (with `mut` binding for `set`/`with_mut`)
//! - `state.get()` / `state.update(|s| …)` → `state.read()` / `state.with_mut(|s| …)`
//! - `Effect::new(move |_| { ... })` → `use_effect(move || { ... })`
//! - `Callback::new(closure)` + `.run(arg)` → `Callback::new(closure)` + `.call(arg)`
//! - `view!` / `.into_any()` / `collect_view()` → `rsx!` / `for` / `Element`
//! - `event_target_value(&ev)` → `ev.value()`
//! - `class=move || format!(...)` → `class: { format!(...) }`
//! - `move || { … }` reactive blocks → compute during render

use crate::components::recovery::version_history::{absolute_time, NoteSummary, VersionMeta};
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::Icon;
use crate::components::ui::info::EmptyState;
use dioxus::prelude::*;
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

/// Fetches the list of notes that have snapshots.
fn fetch_notes(mut state: Signal<ManagerState>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("versions_all", empty_args).await;
        if let Ok(notes) = serde_wasm_bindgen::from_value::<Vec<NoteSummary>>(result) {
            state.with_mut(|s| s.notes = notes);
        }
    });
}

/// Expands a note to show its version timeline.
fn expand_note(path: String, mut state: Signal<ManagerState>) {
    spawn_local(async move {
        let args =
            serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() })).unwrap();
        let result = crate::ipc::tauri_invoke("versions_list", args).await;
        match serde_wasm_bindgen::from_value::<Vec<VersionMeta>>(result) {
            Ok(versions) => {
                let newest = versions.last().map(|v| v.id.clone());
                state.with_mut(|s| {
                    s.expanded = Some(path);
                    s.versions = versions;
                    s.selected_version = newest;
                });
            }
            Err(_) => state.with_mut(|s| {
                s.expanded = None;
                s.versions.clear();
            }),
        }
    });
}

/// The Recovery Manager screen (ViewMode::Recovery).
#[component]
pub fn RecoveryManager() -> Element {
    let mut state = use_signal(|| ManagerState::default());
    let toasts = use_toast();

    let mut restore_open = use_signal(|| false);
    use_effect(move || {
        restore_open.set(state.read().confirm_restore);
    });

    fetch_notes(state);

    rsx! {
        div { class: "recovery-manager flex h-full bg-gray-950 text-gray-100 overflow-hidden" }

        div { class: "flex-1 overflow-y-auto p-4" }
        div { class: "max-w-4xl mx-auto space-y-4" }

        div {}
        h2 { class: "text-lg font-semibold text-gray-50", "\"Recovery Manager\"" }
        p { class: "text-xs text-gray-500", "\"Every note with version history, snapshotted automatically on save. Restore or duplicate any version — nothing is lost.\"" }

        input {
            r#type: "text",
            placeholder: "Search notes with snapshots…",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            value: "{state.read().search}",
            oninput: move |ev: FormEvent| {
                state.with_mut(|s| s.search = ev.value());
            },
        }

        {if {
            let s = state.read();
            let q = s.search.to_lowercase();
            if q.is_empty() {
                s.notes.clone()
            } else {
                s.notes
                    .iter()
                    .filter(|n| n.path.to_lowercase().contains(&q))
                    .cloned()
                    .collect()
            }
        }
        .is_empty() {
            rsx! {
                EmptyState {
                    icon: Icon::LifeBuoy,
                    title: "Nothing to recover yet".to_string(),
                    description: "Notes appear here once they have been saved at least once.".to_string(),
                }
            }
        } else {
            // ── Note list ──
            let filtered: Vec<NoteSummary> = {
                let s = state.read();
                let q = s.search.to_lowercase();
                if q.is_empty() {
                    s.notes.clone()
                } else {
                    s.notes
                        .iter()
                        .filter(|n| n.path.to_lowercase().contains(&q))
                        .cloned()
                        .collect()
                }
            };
            rsx! {
                div { class: "divide-y divide-gray-800 rounded-lg border border-gray-800" }
                for note in &filtered {
                    {
                        let path = note.path.clone();
                        let path_check = path.clone();
                        let name = Path::new(&path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        let is_expanded = {
                            let s = state.read();
                            s.expanded.as_deref() == Some(path_check.as_str())
                        };
                        let versions_snapshot = state.read().versions.clone();
                        let selected_version = state.read().selected_version.clone();
                        rsx! {
                            div {}
                            div {
                                class: "flex items-center gap-2 px-3 py-2 hover:bg-gray-800/40 transition-colors",
                            }
                            button {
                                class: "flex-1 text-left",
                                onclick: move |_: MouseEvent| {
                                    let s = state.read();
                                    if s.expanded.as_deref() == Some(path.as_str()) {
                                        drop(s);
                                        state.with_mut(|sn| sn.expanded = None);
                                    } else {
                                        drop(s);
                                        expand_note(path.clone(), state);
                                    }
                                },
                                div { class: "flex items-center gap-2" }
                                span { class: "text-sm font-medium truncate", "{name}" }
                                span { class: "text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300", "{note.version_count} versions" }
                            }
                            div { class: "text-xs text-gray-500 mt-0.5 truncate" }
                            "{note.path.clone()}"
                            {if let Some(t) = &note.last_snapshot_at {
                                rsx! { " — last saved {absolute_time(t)}" }
                            } else {
                                rsx! {}
                            }}
                            if is_expanded {
                                div {
                                    class: "border-t border-gray-800 bg-gray-900/40",
                                }
                                div { class: "px-3 py-2 text-xs text-gray-400", "\"Versions (newest first):\"" }
                                div { class: "divide-y divide-gray-800" }
                                for version in versions_snapshot.iter().rev() {
                                    {
                                        let id = version.id.clone();
                                        let id_check = id.clone();
                                        let id_select = id.clone();
                                        let id_restore = id.clone();
                                        let id_dup = id.clone();
                                        let expanded_dup = path.clone();
                                        let is_sel = selected_version.as_deref() == Some(id_check.as_str());
                                        rsx! {
                                            div {
                                                class: { format!(
                                                    "px-3 py-2 flex items-center gap-2 {}",
                                                    if is_sel { "bg-blue-900/20" } else { "" }
                                                ) },
                                            }
                                            button {
                                                class: "flex-1 text-left",
                                                onclick: move |_: MouseEvent| {
                                                    state.with_mut(|s| {
                                                        s.selected_version = Some(id_select.clone());
                                                    });
                                                },
                                                div { class: "text-sm", "{version.summary.clone().unwrap_or_else(|| \"Untitled version\".to_string())}" }
                                                div { class: "text-xs text-gray-500", "{absolute_time(&version.created_at)}" }
                                            }
                                            Button {
                                                size: ButtonSize::Sm,
                                                on_click: move |_: MouseEvent| {
                                                    state.with_mut(|s| {
                                                        s.selected_version = Some(id_restore.clone());
                                                        s.confirm_restore = true;
                                                    });
                                                },
                                                {"Restore"}
                                            }
                                            Button {
                                                size: ButtonSize::Sm,
                                                variant: ButtonVariant::Outline,
                                                on_click: move |_: MouseEvent| {
                                                    let expanded_arg = expanded_dup.clone();
                                                    let id_arg = id_dup.clone();
                                                    let dest_path = {
                                                        let base = Path::new(&expanded_arg);
                                                        base.file_stem()
                                                            .map(|s| format!("{}-copy.md", s.to_string_lossy()))
                                                            .unwrap_or_else(|| "copy.md".to_string())
                                                    };
                                                    let toasts_dup = toasts;
                                                    spawn_local(async move {
                                                        let args = serde_wasm_bindgen::to_value(
                                                            &serde_json::json!({
                                                                "path": expanded_arg,
                                                                "id": id_arg,
                                                                "dest": dest_path,
                                                            }),
                                                        )
                                                        .unwrap();
                                                        let result = crate::ipc::tauri_invoke(
                                                            "versions_duplicate",
                                                            args,
                                                        )
                                                        .await;
                                                        match serde_wasm_bindgen::from_value::<()>(result) {
                                                            Ok(()) => toasts_dup
                                                                .success("Duplicated", "Created a new note from this version."),
                                                            Err(e) => toasts_dup.error("Duplicate", e.to_string()),
                                                        }
                                                    });
                                                },
                                                {"Duplicate"}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }}

        ConfirmDialog {
            open: restore_open,
            title: "Restore this version?".to_string(),
            message: "The note will be replaced with this snapshot. The current content is snapshotted first, and you can undo the restore.".to_string(),
            confirm_label: "Restore",
            cancel_label: "Cancel",
            danger: false,
            on_confirm: move |_: ()| {
                state.with_mut(|s| s.confirm_restore = false);
                let s = state.read();
                let path = s.expanded.clone();
                let id = s.selected_version.clone();
                drop(s);
                if let (Some(path), Some(id)) = (path, id) {
                    let toasts_c = toasts;
                    spawn_local(async move {
                        let args = serde_wasm_bindgen::to_value(
                            &serde_json::json!({ "path": path, "id": id }),
                        )
                        .unwrap();
                        let result = crate::ipc::tauri_invoke("versions_restore", args).await;
                        match serde_wasm_bindgen::from_value::<()>(result) {
                            Ok(()) => {
                                toasts_c
                                    .success("Restored", "The note was restored to this version.");
                            }
                            Err(e) => toasts_c.error("Restore", e.to_string()),
                        }
                    });
                }
            },
            on_cancel: move |_: ()| {
                state.with_mut(|s| s.confirm_restore = false);
            },
        }
    }
}
