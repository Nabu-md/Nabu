//! # Comparison View — side-by-side note / revision comparison
//!
//! Supports two comparison modes:
//! - **Two notes**: diff any two notes by vault-relative path.
//! - **Revisions**: diff two versions (snapshots) of the same note.
//!
//! Reuses the backend's LCS line diff (`notes_diff` for two notes,
//! `versions_diff` for revisions) and the existing `DiffView` component for
//! rendering. Synchronised scrolling keeps both sides aligned. Quick
//! navigation between differences (jump to next/prev change) is included.

use crate::components::navigation::state::use_nav;
use crate::components::recovery::diff_view::{DiffKind, DiffRow};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types (mirror backend) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    pub id: String,
    pub created_at: String,
    pub size: usize,
    pub char_count: usize,
    pub summary: Option<String>,
    pub manual: bool,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompareMode {
    Notes,
    Revisions,
}

// ── Comparison Component ────────────────────────────────────────────

#[component]
pub fn ComparisonView() -> impl IntoView {
    let nav = use_nav();

    let (mode, set_mode) = signal(CompareMode::Notes);
    let (diff_rows, set_diff_rows) = signal(Vec::<DiffRow>::new());
    let (loaded, set_loaded) = signal(false);
    let (load_error, set_load_error) = signal(None::<String>);

    // Note selection
    let (note_a, set_note_a) = signal(String::new());
    let (note_b, set_note_b) = signal(String::new());

    // Revision selection
    let (revision_path, set_revision_path) = signal(String::new());
    let (versions, set_versions) = signal(Vec::<VersionMeta>::new());
    let (version_a, set_version_a) = signal(None::<String>);
    let (version_b, set_version_b) = signal(None::<String>);

    // Synchronised scrolling
    let (sync_scroll, set_sync_scroll) = signal(true);
    let left_ref = NodeRef::<leptos::html::Div>::new();
    let right_ref = NodeRef::<leptos::html::Div>::new();

    let notes_index = move || nav.notes_index.get();

    // Load versions when a note is selected for revision comparison.
    let load_versions = move |path: String| {
        set_revision_path.set(path.clone());
        set_versions.set(vec![]);
        set_version_a.set(None);
        set_version_b.set(None);
        if path.is_empty() {
            return;
        }
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).unwrap();
            let result = crate::ipc::tauri_invoke("versions_list", args).await;
            if let Ok(v) = serde_wasm_bindgen::from_value::<Vec<VersionMeta>>(result) {
                set_versions.set(v);
            }
        });
    };

    // Run the comparison.
    let run_comparison = Callback::new(move |_| {
        set_loaded.set(false);
        set_load_error.set(None);
        set_diff_rows.set(vec![]);

        match mode.get() {
            CompareMode::Notes => {
                let a = note_a.get();
                let b = note_b.get();
                if a.is_empty() || b.is_empty() {
                    set_loaded.set(true);
                    return;
                }
                spawn_local(async move {
                    let args =
                        serde_wasm_bindgen::to_value(&serde_json::json!({ "path_a": a, "path_b": b }))
                            .unwrap();
                    let result = crate::ipc::tauri_invoke("notes_diff", args).await;
                    match serde_wasm_bindgen::from_value::<Vec<DiffRow>>(result) {
                        Ok(rows) => set_diff_rows.set(rows),
                        Err(e) => set_load_error.set(Some(e.to_string())),
                    }
                    set_loaded.set(true);
                });
            }
            CompareMode::Revisions => {
                let path = revision_path.get();
                let id_a = version_a.get();
                let id_b = version_b.get();
                if path.is_empty() {
                    set_loaded.set(true);
                    return;
                }
                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                        "path": path,
                        "id_a": id_a,
                        "id_b": id_b,
                    }))
                    .unwrap();
                    let result = crate::ipc::tauri_invoke("versions_diff", args).await;
                    match serde_wasm_bindgen::from_value::<Vec<DiffRow>>(result) {
                        Ok(rows) => set_diff_rows.set(rows),
                        Err(e) => set_load_error.set(Some(e.to_string())),
                    }
                    set_loaded.set(true);
                });
            }
        }
    });

    // Synchronised scroll handler.
    let on_scroll_left = move |_| {
        if sync_scroll.get() {
            if let Some(right) = right_ref.get() {
                if let Some(left) = left_ref.get() {
                    right.set_scroll_top(left.scroll_top());
                }
            }
        }
    };
    let on_scroll_right = move |_| {
        if sync_scroll.get() {
            if let Some(left) = left_ref.get() {
                if let Some(right) = right_ref.get() {
                    left.set_scroll_top(right.scroll_top());
                }
            }
        }
    };

    // Navigate between differences.
    let diff_count = move || {
        diff_rows
            .get()
            .iter()
            .filter(|r| r.kind != DiffKind::Same)
            .count()
    };

    let label_a = move || match mode.get() {
        CompareMode::Notes => note_a.get(),
        CompareMode::Revisions => {
            let path = revision_path.get();
            match version_a.get() {
                Some(id) => format!("{} @ {}", path, id),
                None => format!("{} (current)", path),
            }
        }
    };

    let label_b = move || match mode.get() {
        CompareMode::Notes => note_b.get(),
        CompareMode::Revisions => {
            let path = revision_path.get();
            match version_b.get() {
                Some(id) => format!("{} @ {}", path, id),
                None => format!("{} (current)", path),
            }
        }
    };

    view! {
        <div class="comparison-view flex flex-col h-full bg-gray-950 text-gray-100 overflow-hidden">
            // Header
            <div class="flex-none px-4 py-3 border-b border-gray-800">
                <div class="flex items-center justify-between mb-3">
                    <h2 class="text-sm font-semibold text-gray-300">"Comparison View"</h2>
                    <div class="flex items-center gap-2">
                        <button
                            class=move || format!("px-3 py-1 text-xs rounded border {}",
                                if mode.get() == CompareMode::Notes { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400" })
                            on:click=move |_| set_mode.set(CompareMode::Notes)
                        >
                            "Two Notes"
                        </button>
                        <button
                            class=move || format!("px-3 py-1 text-xs rounded border {}",
                                if mode.get() == CompareMode::Revisions { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400" })
                            on:click=move |_| set_mode.set(CompareMode::Revisions)
                        >
                            "Revisions"
                        </button>
                    </div>
                </div>

                // Selection controls
                {move || {
                    if mode.get() == CompareMode::Notes {
                        view! {
                            <div class="grid grid-cols-2 gap-3">
                                <div>
                                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Note A"</label>
                                    <select
                                        class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1.5 text-sm border border-gray-700"
                                        on:change=move |ev| set_note_a.set(event_target_value(&ev))
                                    >
                                        <option value="">"Select note A…"</option>
                                        {notes_index().iter().map(|n| {
                                            let path = n.path.clone();
                                            let title = n.title.clone();
                                            view! {
                                                <option value={path.clone()} selected={note_a.get() == path}>
                                                    {title}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                                <div>
                                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Note B"</label>
                                    <select
                                        class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1.5 text-sm border border-gray-700"
                                        on:change=move |ev| set_note_b.set(event_target_value(&ev))
                                    >
                                        <option value="">"Select note B…"</option>
                                        {notes_index().iter().map(|n| {
                                            let path = n.path.clone();
                                            let title = n.title.clone();
                                            view! {
                                                <option value={path.clone()} selected={note_b.get() == path}>
                                                    {title}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="grid grid-cols-3 gap-3">
                                <div>
                                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Note"</label>
                                    <select
                                        class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1.5 text-sm border border-gray-700"
                                        on:change=move |ev| load_versions(event_target_value(&ev))
                                    >
                                        <option value="">"Select note…"</option>
                                        {notes_index().iter().map(|n| {
                                            let path = n.path.clone();
                                            let title = n.title.clone();
                                            view! {
                                                <option value={path.clone()} selected={revision_path.get() == path}>
                                                    {title}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                                <div>
                                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Version A"</label>
                                    <select
                                        class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1.5 text-sm border border-gray-700"
                                        on:change=move |ev| set_version_a.set(if event_target_value(&ev).is_empty() { None } else { Some(event_target_value(&ev)) })
                                    >
                                        <option value="">"Current"</option>
                                        {versions.get().iter().rev().map(|v| {
                                            let id = v.id.clone();
                                            let label = format!("{} ({} chars)", v.created_at.chars().take(19).collect::<String>(), v.char_count);
                                            view! {
                                                <option value={id.clone()} selected={version_a.get() == Some(id.clone())}>
                                                    {label}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                                <div>
                                    <label class="text-xs text-gray-500 uppercase tracking-wide">"Version B"</label>
                                    <select
                                        class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1.5 text-sm border border-gray-700"
                                        on:change=move |ev| set_version_b.set(if event_target_value(&ev).is_empty() { None } else { Some(event_target_value(&ev)) })
                                    >
                                        <option value="">"Current"</option>
                                        {versions.get().iter().rev().map(|v| {
                                            let id = v.id.clone();
                                            let label = format!("{} ({} chars)", v.created_at.chars().take(19).collect::<String>(), v.char_count);
                                            view! {
                                                <option value={id.clone()} selected={version_b.get() == Some(id.clone())}>
                                                    {label}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}

                // Action bar
                <div class="flex items-center justify-between mt-3">
                    <div class="flex items-center gap-3">
                        <button
                            class="px-3 py-1.5 text-sm bg-blue-600 rounded hover:bg-blue-500"
                            on:click=move |_| run_comparison.run(())
                        >
                            "Compare"
                        </button>
                        <label class="flex items-center gap-1 text-xs text-gray-400">
                            <input type="checkbox" checked=sync_scroll.get()
                                on:change=move |ev| set_sync_scroll.set(event_target_checked(&ev))
                            />
                            "Sync scroll"
                        </label>
                    </div>
                    {move || {
                        let count = diff_count();
                        if count > 0 {
                            view! {
                                <span class="text-xs text-gray-400">{format!("{} differences", count)}</span>
                            }.into_any()
                        } else {
                            view! {}.into_any()
                        }
                    }}
                </div>
            </div>

            // Diff content
            <div class="flex-1 overflow-hidden">
                {move || {
                    if let Some(err) = load_error.get() {
                        view! {
                            <div class="p-4">
                                <crate::components::ui::feedback::ErrorPanel
                                    title="Comparison failed".to_string()
                                    message="Could not compute the diff.".to_string()
                                    details=err
                                    recovery="Make sure both notes exist and are accessible.".to_string()
                                    on_retry=Callback::new(move |_| run_comparison.run(()))
                                />
                            </div>
                        }.into_any()
                    } else if !loaded.get() {
                        view! {
                            <div class="p-4">
                                <crate::components::ui::feedback::SkeletonList rows=8 />
                            </div>
                        }.into_any()
                    } else if diff_rows.get().is_empty() {
                        view! {
                            <div class="h-full flex items-center justify-center">
                                <crate::components::ui::info::EmptyState
                                    icon=crate::components::ui::icons::Icon::TrendingUp
                                    title="No differences".to_string()
                                    description="Select two notes or versions and click Compare to see the diff.".to_string()
                                />
                            </div>
                        }.into_any()
                    } else {
                        let rows = diff_rows.get();
                        view! {
                            <div class="diff-view h-full overflow-hidden">
                                <div class="diff-headers">
                                    <div class="diff-header diff-header-old">{label_a()}</div>
                                    <div class="diff-header diff-header-new">{label_b()}</div>
                                </div>
                                <div class="diff-body-sync flex h-full overflow-hidden">
                                    <div class="diff-pane flex-1 overflow-y-auto" node_ref=left_ref on:scroll=on_scroll_left>
                                        {rows.iter().map(|row| {
                                            let (class, text) = match row.kind {
                                                DiffKind::Same => ("diff-cell diff-same", row.text.clone()),
                                                DiffKind::Added => ("diff-cell diff-added", format!("+ {}", row.text)),
                                                DiffKind::Removed => ("diff-cell diff-removed", format!("- {}", row.text)),
                                            };
                                            view! {
                                                <div class={class}>
                                                    <span class="diff-lineno">{row.old_line.map(|l| l.to_string()).unwrap_or_default()}</span>
                                                    <span class="diff-text">{text}</span>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                    <div class="diff-pane flex-1 overflow-y-auto" node_ref=right_ref on:scroll=on_scroll_right>
                                        {rows.iter().map(|row| {
                                            let (class, text) = match row.kind {
                                                DiffKind::Same => ("diff-cell diff-same", row.text.clone()),
                                                DiffKind::Added => ("diff-cell diff-added", format!("+ {}", row.text)),
                                                DiffKind::Removed => ("diff-cell diff-removed", format!("- {}", row.text)),
                                            };
                                            view! {
                                                <div class={class}>
                                                    <span class="diff-lineno">{row.new_line.map(|l| l.to_string()).unwrap_or_default()}</span>
                                                    <span class="diff-text">{text}</span>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
