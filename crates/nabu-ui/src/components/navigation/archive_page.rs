//! # Archive — non-destructive archiving workspace
//!
//! Phase 13.2: archived notes live in the reserved `archive/` folder — hidden
//! from normal navigation and the file tree, but still full-text searchable.
//! This screen lists everything archived (with its original location) and
//! offers one-click restore back to its original path.
//!
//! ## Reactivity note
//!
//! Toast and workspace contexts are `Copy` and captured at render time, then
//! threaded into async tasks as plain values — never `expect_context` inside a
//! `spawn_local` future (no reactive owner on the failure path).

use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::info::EmptyState;
use crate::components::workspace::{refresh_tree, use_workspace};
use crate::models::organisation::ArchiveEntry;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// The Archive workspace.
#[component]
pub fn ArchivePage() -> impl IntoView {
    let ws = use_workspace();
    let toasts = use_toast();

    let (entries, set_entries) = signal(Vec::<ArchiveEntry>::new());
    let (loading, set_loading) = signal(true);

    // Load the archive list.
    let load = Callback::new(move |_| {
        set_loading.set(true);
        spawn_local(async move {
            let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("archive_list", empty).await;
            set_loading.set(false);
            if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<ArchiveEntry>>(result) {
                set_entries.set(list);
            } else {
                set_entries.set(Vec::new());
                toasts.error("Archive", "Could not load the archive.");
            }
        });
    });

    // Load once on mount.
    Effect::new(move |_| {
        load.run(());
    });

    // Restore an archived note to its original location.
    let restore = Callback::new(move |archive_path: String| {
        let toasts = toasts;
        let ws = ws;
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "archive_path": archive_path }))
                    .unwrap();
            let result = crate::ipc::tauri_invoke("archive_restore", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                set_entries.update(|l| l.retain(|e| e.archive_path != archive_path));
                refresh_tree(ws);
                toasts.success("Restored", "Note moved back to its original location.");
            } else {
                toasts.error("Restore", "Could not restore that note.");
            }
        });
    });

    // Display title of the folder the note was archived from.
    let original_folder = move |e: &ArchiveEntry| -> String {
        if e.folder.is_empty() {
            "Vault root".to_string()
        } else {
            e.folder.clone()
        }
    };

    view! {
        <div class="space-y-6">
            <header>
                <h1 class="text-xl font-semibold text-gray-100">"Archive"</h1>
                <p class="text-sm text-gray-400 mt-1">
                    "Archived notes are hidden from navigation but remain searchable. Restore them here — nothing is deleted."
                </p>
            </header>

            {move || if loading.get() {
                view! {
                    <div class="text-sm text-gray-500">"Loading archive…"</div>
                }.into_any()
            } else if entries.get().is_empty() {
                view! {
                    <EmptyState
                        icon="🗃️"
                        title="Archive is empty".to_string()
                        description="Notes you archive (right-click a note in the file tree → Archive) will appear here.".to_string()
                    />
                }.into_any()
            } else {
                view! {
                    <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                        {entries.get().into_iter().map(|entry| {
                            let archive_path = entry.archive_path.clone();
                            let title = entry.title.clone();
                            let folder = original_folder(&entry);
                            let modified = entry.modified_at.clone();
                            view! {
                                <div class="rounded-md border border-gray-800 p-3 flex flex-col gap-2 hover:border-gray-600 transition-colors">
                                    <div class="flex items-center gap-2 min-w-0">
                                        <span aria-hidden="true">"📄"</span>
                                        <span class="text-sm font-medium text-gray-200 truncate" title=title.clone()>{title.clone()}</span>
                                    </div>
                                    <div class="text-xs text-gray-500 truncate">"Original: " {folder}</div>
                                    {if !modified.is_empty() {
                                        view! { <div class="text-[11px] text-gray-600">"Modified: " {modified}</div> }.into_any()
                                    } else {
                                        view! {}.into_any()
                                    }}
                                    <div class="flex gap-2 mt-1">
                                        <Button
                                            size=ButtonSize::Sm
                                            variant=ButtonVariant::Primary
                                            on_click=Callback::new(move |_| restore.run(archive_path.clone()))
                                        >
                                            "↩ Restore"
                                        </Button>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
