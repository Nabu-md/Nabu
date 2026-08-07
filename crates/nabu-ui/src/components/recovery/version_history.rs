//! # Version History screen (Dioxus)
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
//! Migration notes (LePtOS → Dioxus):
//! - `RwSignal<T>` → `Signal<T>` (with `mut` on the binding for `set`/`with_mut`)
//! - `state.get()` / `state.update(|s| …)` → `state.read()` / `state.with_mut(|s| …)`
//! - `Effect::new(move |_| { ... })` → `use_effect(move || { ... })`
//! - `Callback::new(closure)` + `.run(arg)` → `Callback::new(closure)` + `.call(arg)`
//! - `view!` / `.into_any()` / `collect_view()` → `rsx!` / `for` / `Element`
//! - `event_target_value(&ev)` → `ev.value()`
//! - `class=move || format!(...)` → `class: { format!(...) }`
//! - `move || { … view!{} … }` reactive blocks → compute during render

use crate::components::recovery::diff_view::{DiffRow, DiffView};
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::dialog::{ConfirmDialog, PromptDialog};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use dioxus::prelude::*;
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

/// Short relative timestamp for display ("5m ago", "3d ago").
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

/// Absolute timestamp formatted directly (no Local clock, which is disabled
/// for the wasm build — see Cargo.toml).
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
fn fetch_all_notes(mut state: Signal<VersionState>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("versions_all", empty_args).await;
        if let Ok(summaries) = serde_wasm_bindgen::from_value::<Vec<NoteSummary>>(result) {
            state.with_mut(|s| {
                s.notes = summaries;
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
fn load_versions(path: String, mut state: Signal<VersionState>) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("versions_list", args).await;
        match serde_wasm_bindgen::from_value::<Vec<VersionMeta>>(result) {
            Ok(versions) => {
                let newest = versions.last().map(|v| v.id.clone());
                state.with_mut(|s| {
                    s.versions = versions;
                    s.selected_version = newest.clone();
                    s.diff = None;
                });
                if let Some(id) = newest {
                    preview_version(path, id, state);
                }
            }
            Err(_) => state.with_mut(|s| {
                s.versions.clear();
                s.preview_content = None;
                s.diff = None;
            }),
        }
    });
}

/// Loads the content of one version into the preview pane.
fn preview_version(path: String, id: String, mut state: Signal<VersionState>) {
    spawn_local(async move {
        let args =
            serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path, "id": id.clone() }))
                .unwrap();
        let result = crate::ipc::tauri_invoke("versions_get", args).await;
        if let Ok(content) = serde_wasm_bindgen::from_value::<String>(result) {
            state.with_mut(|s| {
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
    mut state: Signal<VersionState>,
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
            Ok(rows) => state.with_mut(|s| s.diff = Some(rows)),
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
pub fn VersionHistory() -> Element {
    let mut state = use_signal(|| VersionState::default());
    let toasts = use_toast();

    // Dialog open signals — synced from state.
    let mut restore_open = use_signal(|| false);
    let mut duplicate_open = use_signal(|| false);
    use_effect(move || {
        restore_open.set(state.read().confirm_restore);
        duplicate_open.set(state.read().duplicate_open);
    });

    fetch_all_notes(state);

    // ── Read state for rendering ───────────────────────────────────────────
    let notes = state.read().notes.clone();
    let selected_note_name = state
        .read()
        .selected_note
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Select a note".to_string());
    let versions: Vec<VersionMeta> = if state.read().versions.is_empty() {
        Vec::new()
    } else {
        state.read().versions.clone()
    };
    let diff_rows = state.read().diff.clone();
    let preview_content = state.read().preview_content.clone();

    rsx! {
        div {
            class: "version-history flex h-full bg-gray-950 text-gray-100 overflow-hidden",

            // ── Left: snapshot browser ──
            div {
                class: "flex-none w-72 border-r border-gray-800 flex flex-col min-w-0",
            }
            div { class: "px-4 py-3 border-b border-gray-800" }
            h2 { class: "text-base font-semibold text-gray-50", "Version History" }
            p { class: "text-xs text-gray-500", format!("{} notes with snapshots", notes.len()) }

            div { class: "flex-1 overflow-y-auto" }
            {if notes.is_empty() {
                rsx! {
                    EmptyState {
                        icon: Icon::History,
                        title: "No snapshots yet".to_string(),
                        description: "Save a note and it will appear here with version history.".to_string(),
                    }
                }
            } else {
                rsx! {
                    div { class: "divide-y divide-gray-800" }
                    for note in &notes {
                        {
                            let path = note.path.clone();
                            let name = Path::new(&path)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.clone());
                            let count = note.version_count;
                            let last = note.last_snapshot_at.clone();
                            let path_check = path.clone();
                            let s = state;
                            let is_selected = {
                                let s = state.read();
                                s.selected_note.as_deref() == Some(path_check.as_str())
                            };
                            rsx! {
                                button {
                                    class: { format!(
                                        "w-full text-left px-3 py-2 hover:bg-gray-800/60 transition-colors {}",
                                        if is_selected {
                                            "bg-gray-800 border-l-2 border-l-blue-500"
                                        } else {
                                            "border-l-2 border-l-transparent"
                                        }
                                    ) },
                                    onclick: move |_: MouseEvent| {
                                        s.with_mut(|sn| {
                                            sn.selected_note = Some(path.clone());
                                        });
                                        load_versions(path.clone(), s);
                                    },
                                    div { class: "flex items-center justify-between gap-2" }
                                    span { class: "text-sm font-medium truncate", "{name}" }
                                    span { class: "text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300", "{count}" }
                                }
                                div { class: "text-xs text-gray-500 mt-0.5 truncate" }
                                {if let Some(at) = &last {
                                    {relative_time(at)}
                                } else {
                                    rsx! {}
                                }}
                            }
                        }
                    }
                }
            }}

            // ── Middle: version timeline ──
            div {
                class: "flex-none w-80 border-r border-gray-800 flex flex-col min-w-0",
            }
            div {
                class: "px-4 py-3 border-b border-gray-800 flex items-center justify-between gap-2",
            }
            h3 {
                class: "text-sm font-semibold text-gray-300 truncate",
                "{selected_note_name}",
            }
            Button {
                size: ButtonSize::Sm,
                on_click: move |_: MouseEvent| {
                    if let Some(path) = state.read().selected_note.clone() {
                        let args = serde_wasm_bindgen::to_value(
                            &serde_json::json!({ "path": path.clone() }),
                        )
                        .unwrap();
                        let toasts_snap = toasts;
                        spawn_local(async move {
                            let result = crate::ipc::tauri_invoke("snapshot_create", args).await;
                            match serde_wasm_bindgen::from_value::<VersionMeta>(result) {
                                Ok(_meta) => {
                                    toasts_snap.success(
                                        "Snapshot created",
                                        format!("Captured a snapshot of '{}'", path),
                                    );
                                    load_versions(path, state);
                                }
                                Err(_) => toasts_snap.error(
                                    "Snapshot",
                                    "Could not create a snapshot",
                                ),
                            }
                        });
                    } else {
                        toasts.info("Snapshot", "Select a note first");
                    }
                },
                title: "Capture a manual snapshot of the current content",
                {"Snapshot"}
            }

            div { class: "flex-1 overflow-y-auto" }
            {if versions.is_empty() {
                rsx! {
                    div {
                        class: "px-4 py-3 text-xs text-gray-500",
                        "No versions recorded yet.",
                    }
                }
            } else {
                rsx! {
                    div { class: "divide-y divide-gray-800" }
                    for version in versions.iter().rev() {
                        {
                            let id = version.id.clone();
                            let id_check = id.clone();
                            let s = state;
                            let is_selected = {
                                let s = state.read();
                                s.selected_version.as_deref() == Some(id_check.as_str())
                            };
                            let summary_text =
                                version.summary.clone().unwrap_or_else(|| "Untitled version".to_string());
                            let created_abs = absolute_time(&version.created_at);
                            let created_rel = relative_time(&version.created_at);
                            let size_text = human_size(version.size);
                            let char_text = format!("{} chars", version.char_count);
                            rsx! {
                                button {
                                    key: "{id}",
                                    class: { format!(
                                        "w-full text-left px-3 py-2 hover:bg-gray-800/60 transition-colors {}",
                                        if is_selected {
                                            "bg-gray-800 border-l-2 border-l-blue-500"
                                        } else {
                                            "border-l-2 border-l-transparent"
                                        }
                                    ) },
                                    onclick: move |_: MouseEvent| {
                                        if let Some(p) = s.read().selected_note.clone() {
                                            preview_version(p, id.clone(), s);
                                        }
                                    },
                                    div { class: "flex items-center justify-between gap-2" }
                                    span { class: "text-sm font-medium", "{summary_text}" }
                                    {if version.manual {
                                        rsx! {
                                            span {
                                                class: "text-xs px-1.5 py-0.5 rounded bg-blue-900/50 text-blue-300",
                                                "Manual",
                                            }
                                        }
                                    } else {
                                        rsx! {}
                                    }}
                                }
                                div { class: "text-xs text-gray-500 mt-0.5" }
                                "{created_abs}"
                                span { class: "mx-1", "•" }
                                "{created_rel}"
                                div { class: "text-xs text-gray-600" }
                                "{size_text}"
                                span { class: "mx-1", "•" }
                                "{char_text}"
                            }
                        }
                    }
                }
            }}

            // ── Right: preview + diff ──
            div { class: "flex-1 overflow-y-auto p-4 min-w-0" }
            {if let Some(rows) = &diff_rows {
                rsx! {
                    div { class: "space-y-3" }
                    div { class: "flex items-center justify-between gap-2" }
                    h3 { class: "text-sm font-semibold text-gray-300", "Diff" }
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Ghost,
                        on_click: move |_: MouseEvent| {
                            state.with_mut(|s| s.diff = None);
                        },
                        {render_icon_view(Icon::X)} " Close"
                    }
                    DiffView {
                        rows: rows.clone(),
                        old_label: "Version".to_string(),
                        new_label: "Current / Other".to_string(),
                    }
                }
            } else if let Some(content) = &preview_content {
                rsx! {
                    div { class: "space-y-3" }
                    div { class: "flex items-center justify-between gap-2 flex-wrap" }
                    h3 { class: "text-sm font-semibold text-gray-300", "Preview" }
                    div { class: "flex items-center gap-2 flex-wrap" }
                    Button {
                        size: ButtonSize::Sm,
                        on_click: move |_: MouseEvent| {
                            if let Some(path) = state.read().selected_note.clone() {
                                if let Some(from) = state.read().selected_version.clone() {
                                    let toasts_df = toasts;
                                    fetch_diff(path, from, None, state, toasts_df);
                                }
                            }
                        },
                        "Diff vs current"
                    }
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Destructive,
                        on_click: move |_: MouseEvent| {
                            state.with_mut(|s| s.confirm_restore = true);
                        },
                        "Restore this version"
                    }
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Outline,
                        on_click: move |_: MouseEvent| {
                            state.with_mut(|s| s.duplicate_open = true);
                        },
                        "Duplicate…"
                    }
                    pre {
                        class: "text-xs text-gray-300 bg-gray-900 p-3 rounded-lg overflow-auto max-h-96 whitespace-pre-wrap border border-gray-800",
                        "{content}",
                    }
                }
            } else {
                rsx! {
                    EmptyState {
                        icon: Icon::Eye,
                        title: "Select a note and a version".to_string(),
                        description: "Preview the content, compare revisions, restore, or duplicate it.".to_string(),
                    }
                }
            }}
            }

            // ── Dialogs ──
            ConfirmDialog {
                open: restore_open,
                title: "Restore this version?".to_string(),
                message: "The note will be replaced with this snapshot. The current content is snapshotted first, and you can undo the restore.".to_string(),
                confirm_label: "Restore",
                cancel_label: "Cancel",
                danger: false,
                on_confirm: move |_: ()| {
                    state.with_mut(|s| s.confirm_restore = false);
                    let path = {
                        let s = state.read();
                        s.selected_note.clone()
                    };
                    let id = {
                        let s = state.read();
                        s.selected_version.clone()
                    };
                    if let (Some(path), Some(id)) = (path, id) {
                        let toasts_c = toasts;
                        spawn_local(async move {
                            let args = serde_wasm_bindgen::to_value(
                                &serde_json::json!({ "path": path.clone(), "id": id }),
                            )
                            .unwrap();
                            let result =
                                crate::ipc::tauri_invoke("versions_restore", args).await;
                            match serde_wasm_bindgen::from_value::<()>(result) {
                                Ok(()) => {
                                    toasts_c.success(
                                        "Restored",
                                        "The note was restored to this version.",
                                    );
                                    let _ = path.clone();
                                    load_versions(path, state);
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

            PromptDialog {
                open: duplicate_open,
                title: "Duplicate version as new note".to_string(),
                message: "Enter the new note path (e.g. copy-of-note.md):".to_string(),
                confirm_label: "Duplicate",
                cancel_label: "Cancel",
                on_submit: move |dest: String| {
                    state.with_mut(|s| s.duplicate_open = false);
                    let dest = dest.trim().to_string();
                    if dest.is_empty() {
                        return;
                    }
                    let path = {
                        let s = state.read();
                        s.selected_note.clone()
                    };
                    let id = {
                        let s = state.read();
                        s.selected_version.clone()
                    };
                    if let (Some(path), Some(id)) = (path, id) {
                        let toasts_d = toasts;
                        spawn_local(async move {
                            let args = serde_wasm_bindgen::to_value(
                                &serde_json::json!({ "path": path, "id": id, "dest": dest }),
                            )
                            .unwrap();
                            let result =
                                crate::ipc::tauri_invoke("versions_duplicate", args).await;
                            match serde_wasm_bindgen::from_value::<()>(result) {
                                Ok(()) => toasts_d
                                    .success("Duplicated", "Created a new note from this version."),
                                Err(e) => toasts_d.error("Duplicate", e.to_string()),
                            }
                        });
                    }
                },
                on_cancel: move |_: ()| {
                    state.with_mut(|s| s.duplicate_open = false);
                },
            }
        }
    }
}
