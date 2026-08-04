//! # Version History screen
//!
//! Browse the snapshots of a note: list versions, preview their content, diff
//! two revisions, restore an old version (undoable), duplicate it as a new
//! note, and capture a manual snapshot.
//!
//! Layout:
//! - left: the snapshot browser — every note that has versions
//! - middle: the version timeline for the selected note
//! - right: preview + diff of the selected version vs. another / current
//!
//! ## Reactivity note
//!
//! Toast / history contexts are `Copy` and captured at render time, then
//! threaded into async tasks — never via `expect_context` inside a
//! `spawn_local` future.

use crate::components::recovery::diff_view::{DiffRow, DiffView};
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::dialog::{ConfirmDialog, PromptDialog};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use wasm_bindgen_futures::spawn_local;

/// Mirrors the backend `VersionMeta`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionMeta {
    pub id: String,
    pub created_at: String,
    pub size: usize,
    pub char_count: usize,
    pub summary: Option<String>,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub author: Option<String>,
}

/// Mirrors the backend `NoteSummary`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteSummary {
    pub path: String,
    pub version_count: usize,
    pub last_snapshot_at: Option<String>,
}

/// Relative time for display ("5m ago", "3d ago"). Uses the JS clock to avoid
/// chrono's wasm clock panic (see trash.rs).
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

/// Displays a short timestamp (used in the timeline). Formats the parsed UTC
/// instant directly — `chrono::Local` requires the `clock` feature, which is
/// deliberately disabled for the wasm build (see the Cargo.toml note).
pub(crate) fn absolute_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.format("%b %e, %H:%M").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}

/// Human-readable byte size.
fn human_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Fetches the list of notes that have snapshots.
fn fetch_all_notes(state: RwSignal<VersionState>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("versions_all", empty_args).await;
        if let Ok(summaries) = serde_wasm_bindgen::from_value::<Vec<NoteSummary>>(result) {
            state.update(|s| {
                s.notes = summaries;
                // Keep the selection valid.
                if let Some(sel) = &s.selected_note {
                    if !s.notes.iter().any(|n| n.path == *sel) {
                        s.selected_note = None;
                        s.versions.clear();
                        s.preview_content = None;
                        s.selected_version = None;
                    }
                }
            });
        }
    });
}

/// Loads the version list for the selected note and auto-picks the newest.
fn load_versions(path: String, state: RwSignal<VersionState>) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("versions_list", args).await;
        match serde_wasm_bindgen::from_value::<Vec<VersionMeta>>(result) {
            Ok(versions) => {
                let newest = versions.last().map(|v| v.id.clone());
                state.update(|s| {
                    s.versions = versions;
                    s.selected_version = newest.clone();
                    s.diff = None;
                });
                if let Some(id) = newest {
                    preview_version(path.clone(), id, state);
                }
            }
            Err(_) => state.update(|s| {
                s.versions.clear();
                s.preview_content = None;
                s.diff = None;
            }),
        }
    });
}

/// Loads the content of one version into the preview pane.
fn preview_version(path: String, id: String, state: RwSignal<VersionState>) {
    spawn_local(async move {
        let args =
            serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path, "id": id })).unwrap();
        let result = crate::ipc::tauri_invoke("versions_get", args).await;
        if let Ok(content) = serde_wasm_bindgen::from_value::<String>(result) {
            state.update(|s| {
                s.selected_version = Some(id);
                s.preview_content = Some(content);
                s.diff = None;
            });
        }
    });
}

/// Fetches a diff between two versions (or a version and the live note when
/// `to_id` is `None`).
fn fetch_diff(
    path: String,
    from_id: String,
    to_id: Option<String>,
    state: RwSignal<VersionState>,
    toasts: crate::components::ui::feedback::ToastContext,
) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({
            "path": path,
            "id_a": from_id,
            "id_b": to_id,
        }))
        .unwrap();
        let result = crate::ipc::tauri_invoke("versions_diff", args).await;
        match serde_wasm_bindgen::from_value::<Vec<DiffRow>>(result) {
            Ok(rows) => state.update(|s| s.diff = Some(rows)),
            Err(_) => toasts.error("Diff", "Could not compute the diff"),
        }
    });
}

/// Internal state shared by the screen's reactive sections.
#[derive(Clone, Default)]
struct VersionState {
    notes: Vec<NoteSummary>,
    selected_note: Option<String>,
    versions: Vec<VersionMeta>,
    selected_version: Option<String>,
    preview_content: Option<String>,
    diff: Option<Vec<DiffRow>>,
    /// Dialog state: restore confirmation.
    confirm_restore: bool,
    /// Dialog state: duplicate prompt.
    duplicate_open: bool,
}

/// The Version History screen (ViewMode::History).
#[component]
pub fn VersionHistory() -> impl IntoView {
    let state = RwSignal::new(VersionState::default());
    let toasts = use_toast();

    // Dialog open signals — synced from state so the ConfirmDialog's internal
    // `open.set(false)` (overlay / Escape / cancel) stays consistent.
    let restore_open = RwSignal::new(false);
    let duplicate_open = RwSignal::new(false);
    Effect::new(move |_| {
        let s = state.get();
        restore_open.set(s.confirm_restore);
        duplicate_open.set(s.duplicate_open);
    });

    fetch_all_notes(state);

    let select_note = move |path: String| {
        state.update(|s| {
            s.selected_note = Some(path.clone());
        });
        load_versions(path, state);
    };

    let select_version = move |id: String| {
        if let Some(path) = state.get().selected_note.clone() {
            preview_version(path, id, state);
        }
    };

    let on_manual_snapshot = Callback::new(move |_: web_sys::MouseEvent| {
        let Some(path) = state.get().selected_note.clone() else {
            toasts.info("Snapshot", "Select a note first");
            return;
        };
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() }))
                    .unwrap();
            let result = crate::ipc::tauri_invoke("snapshot_create", args).await;
            match serde_wasm_bindgen::from_value::<VersionMeta>(result) {
                Ok(_meta) => {
                    toasts.success(
                        "Snapshot created",
                        format!("Captured a snapshot of '{}'", path),
                    );
                    // Refresh the timeline so the new version appears.
                    load_versions(path, state);
                }
                Err(_) => toasts.error("Snapshot", "Could not create a snapshot"),
            }
        });
    });

    let confirm_restore = Callback::new(move |_| {
        state.update(|s| s.confirm_restore = false);
        let Some(path) = state.get().selected_note.clone() else { return; };
        let Some(id) = state.get().selected_version.clone() else { return; };
        let toasts_confirm = toasts;
        let state_confirm = state;
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone(), "id": id }))
                    .unwrap();
            let result = crate::ipc::tauri_invoke("versions_restore", args).await;
            match serde_wasm_bindgen::from_value::<()>(result) {
                Ok(()) => {
                    toasts_confirm.success("Restored", "The note was restored to this version.");
                    // Refresh the timeline (the restore also snapshots the
                    // pre-restore content).
                    load_versions(path.clone(), state_confirm);
                }
                Err(e) => toasts_confirm.error("Restore", e.to_string()),
            }
        });
    });

    let duplicate_submit = Callback::new(move |dest: String| {
        state.update(|s| s.duplicate_open = false);
        let dest = dest.trim().to_string();
        if dest.is_empty() {
            return;
        }
        let Some(path) = state.get().selected_note.clone() else { return; };
        let Some(id) = state.get().selected_version.clone() else { return; };
        let toasts_dup = toasts;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "path": path,
                "id": id,
                "dest": dest,
            }))
            .unwrap();
            let result = crate::ipc::tauri_invoke("versions_duplicate", args).await;
            match serde_wasm_bindgen::from_value::<()>(result) {
                Ok(()) => toasts_dup.success("Duplicated", "Created a new note from this version."),
                Err(e) => toasts_dup.error("Duplicate", e.to_string()),
            }
        });
    });

    // The selected version drives the "diff vs current" quick action.
    let diff_vs_current = Callback::new(move |_| {
        let Some(path) = state.get().selected_note.clone() else { return; };
        let Some(from) = state.get().selected_version.clone() else { return; };
        fetch_diff(path, from, None, state, toasts);
    });

    view! {
        <div class="version-history flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            // ── Left: snapshot browser ──
            <div class="flex-none w-72 border-r border-gray-800 flex flex-col min-w-0">
                <div class="px-4 py-3 border-b border-gray-800">
                    <h2 class="text-base font-semibold text-gray-50">"Version History"</h2>
                    <p class="text-xs text-gray-500">{move || format!("{} notes with snapshots", state.get().notes.len())}</p>
                </div>
                <div class="flex-1 overflow-y-auto">
                    {move || {
                        let notes = state.get().notes.clone();
                        if notes.is_empty() {
                            view! {
                                <EmptyState
                                    icon=Icon::History
                                    title="No snapshots yet".to_string()
                                    description="Save a note and it will appear here with version history.".to_string()
                                ></EmptyState>
                            }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800">
                                    {notes.into_iter().map(|note| {
                                        let path = note.path.clone();
                                        let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
                                        let count = note.version_count;
                                        let last = note.last_snapshot_at.clone();
                                        // Clone for the closure so `path` stays usable in the click handler.
                                        let path_check = path.clone();
                                        let is_selected = move || state.get().selected_note.as_deref() == Some(path_check.as_str());
                                        view! {
                                            <button
                                                class=move || format!("w-full text-left px-3 py-2 hover:bg-gray-800/60 transition-colors {}", if is_selected() { "bg-gray-800 border-l-2 border-l-blue-500" } else { "border-l-2 border-l-transparent" })
                                                on:click=move |_| select_note(path.clone())
                                            >
                                                <div class="flex items-center justify-between gap-2">
                                                    <span class="text-sm font-medium truncate">{name}</span>
                                                    <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">{count}</span>
                                                </div>
                                                <div class="text-xs text-gray-500 mt-0.5 truncate">
                                                    {last.as_deref().map(relative_time).unwrap_or_default()}
                                                </div>
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            // ── Middle: version timeline ──
            <div class="flex-none w-80 border-r border-gray-800 flex flex-col min-w-0">
                <div class="px-4 py-3 border-b border-gray-800 flex items-center justify-between gap-2">
                    <h3 class="text-sm font-semibold text-gray-300 truncate">
                        {move || state.get().selected_note.as_deref().and_then(|p| Path::new(p).file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Select a note".to_string())}
                    </h3>
                    <Button size=ButtonSize::Sm on_click=on_manual_snapshot title="Capture a manual snapshot of the current content">
                        "Snapshot"
                    </Button>
                </div>
                <div class="flex-1 overflow-y-auto">
                    {move || {
                        let versions = state.get().versions.clone();
                        if versions.is_empty() {
                            view! { <div class="px-4 py-3 text-xs text-gray-500">"No versions recorded yet."</div> }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800">
                                    {versions.into_iter().rev().map(|version| {
                                        let id = version.id.clone();
                                        // Clone for the closure so `id` stays usable in the click handler.
                                        let id_check = id.clone();
                                        let is_selected = move || state.get().selected_version.as_deref() == Some(id_check.as_str());
                                        view! {
                                            <button
                                                class=move || format!("w-full text-left px-3 py-2 hover:bg-gray-800/60 transition-colors {}", if is_selected() { "bg-gray-800 border-l-2 border-l-blue-500" } else { "border-l-2 border-l-transparent" })
                                                on:click=move |_| select_version(id.clone())
                                            >
                                                <div class="flex items-center justify-between gap-2">
                                                    <span class="text-sm font-medium">{version.summary.clone().unwrap_or_else(|| "Untitled version".to_string())}</span>
                                                    {if version.manual {
                                                        view! { <span class="text-xs px-1.5 py-0.5 rounded bg-blue-900/50 text-blue-300">"Manual"</span> }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }}
                                                </div>
                                                <div class="text-xs text-gray-500 mt-0.5">
                                                    {absolute_time(&version.created_at)}
                                                    <span class="mx-1">"•"</span>
                                                    {relative_time(&version.created_at)}
                                                </div>
                                                <div class="text-xs text-gray-600">
                                                    {human_size(version.size)}
                                                    <span class="mx-1">"•"</span>
                                                    {format!("{} chars", version.char_count)}
                                                </div>
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            // ── Right: preview + diff ──
            <div class="flex-1 overflow-y-auto p-4 min-w-0">
                {move || {
                    if let Some(rows) = state.get().diff.clone() {
                        view! {
                            <div class="space-y-3">
                                <div class="flex items-center justify-between gap-2">
                                    <h3 class="text-sm font-semibold text-gray-300">"Diff"</h3>
                                    <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=Callback::new(move |_| state.update(|s| s.diff = None))>
                                        {render_icon_view(Icon::X)} Close
                                    </Button>
                                </div>
                                <DiffView rows=rows old_label="Version".to_string() new_label="Current / Other".to_string() />
                            </div>
                        }.into_any()
                    } else if let Some(content) = state.get().preview_content.clone() {
                        view! {
                            <div class="space-y-3">
                                <div class="flex items-center justify-between gap-2 flex-wrap">
                                    <h3 class="text-sm font-semibold text-gray-300">"Preview"</h3>
                                    <div class="flex items-center gap-2 flex-wrap">
                                        <Button size=ButtonSize::Sm on_click=diff_vs_current> "Diff vs current" </Button>
                                        <Button
                                            size=ButtonSize::Sm
                                            variant=ButtonVariant::Destructive
                                            on_click=Callback::new(move |_| state.update(|s| s.confirm_restore = true))
                                        >
                                            "Restore this version"
                                        </Button>
                                        <Button
                                            size=ButtonSize::Sm
                                            variant=ButtonVariant::Outline
                                            on_click=Callback::new(move |_| state.update(|s| s.duplicate_open = true))
                                        >
                                            "Duplicate…"
                                        </Button>
                                    </div>
                                </div>
                                <pre class="text-xs text-gray-300 bg-gray-900 p-3 rounded-lg overflow-auto max-h-96 whitespace-pre-wrap border border-gray-800">{content}</pre>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <EmptyState
                                icon=Icon::Eye
                                title="Select a note and a version".to_string()
                                description="Preview the content, compare revisions, restore, or duplicate it.".to_string()
                            ></EmptyState>
                        }.into_any()
                    }
                }}
            </div>

            // ── Dialogs ──
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

            <PromptDialog
                open=duplicate_open
                title="Duplicate version as new note".to_string()
                message="Enter the new note path (e.g. copy-of-note.md):".to_string()
                confirm_label="Duplicate"
                cancel_label="Cancel"
                on_submit=duplicate_submit
                on_cancel=Callback::new(move |_| state.update(|s| s.duplicate_open = false))
            />
        </div>
    }
}
