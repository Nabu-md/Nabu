use crate::components::file_tree::FileTree;
use crate::components::navigation::state::{use_nav, ViewMode};
use leptos::prelude::*;

/// Left sidebar — the vault file explorer plus organisational shortcuts.
///
/// Contains the interactive [`FileTree`] (real vault data, context menus,
/// inline rename, drag-and-drop, multi-select + batch actions) plus a quick
/// \"New note\" action, and a **Collections** section listing saved smart
/// folders and saved searches so query-powered organisation is always one
/// click away (Phase 13.2).
#[component]
pub fn LeftSidebar() -> impl IntoView {
    let nav = use_nav();

    // Clicking a smart folder opens the Smart Folders workspace.
    let open_smart_folders = move |_| {
        nav.view_mode.set(ViewMode::SmartFolders);
    };

    // Clicking a saved search opens the search page prefilled with the query.
    let run_saved_search = move |query: String| {
        nav.search_query.set(query);
        nav.view_mode.set(ViewMode::Search);
    };

    view! {
        <div class="w-64 border-r border-gray-700 bg-gray-900 h-full flex flex-col min-w-0">
            <div class="flex-1 min-h-0">
                <FileTree />
            </div>

            // Collections — smart folders + saved searches (virtual overlays)
            <div class="border-t border-gray-800 px-2 py-2 space-y-2 overflow-y-auto flex-none max-h-64">
                <div class="flex items-center justify-between px-1">
                    <span class="text-xs font-semibold uppercase tracking-wider text-gray-500">"Collections"</span>
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        title="Manage smart folders"
                        aria-label="Manage smart folders"
                        on:click=open_smart_folders
                    >
                        "🗂️"
                    </button>
                </div>

                // Smart folders
                {move || {
                    let folders = nav.smart_folders.get();
                    if folders.is_empty() {
                        view! {}.into_any()
                    } else {
                        view! {
                            <div class="space-y-0.5">
                                {folders.iter().take(6).map(|f| {
                                    let icon = f.icon.clone();
                                    let name = f.name.clone();
                                    let name_title = name.clone();
                                    let pinned = f.pinned;
                                    view! {
                                        <button
                                            type="button"
                                            class="w-full text-left px-2 py-1 rounded text-xs text-gray-300 hover:bg-gray-800 flex items-center gap-1.5"
                                            title=name_title
                                            on:click=move |_| nav.view_mode.set(ViewMode::SmartFolders)
                                        >
                                            <span aria-hidden="true">{icon}</span>
                                            <span class="flex-1 truncate">{name}</span>
                                            {if pinned {
                                                view! { <span class="text-[10px] text-yellow-500" aria-hidden="true">"📌"</span> }.into_any()
                                            } else { view! {}.into_any() }}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}

                // Saved searches
                {move || {
                    let searches = nav.saved_searches.get();
                    if searches.is_empty() {
                        view! {}.into_any()
                    } else {
                        view! {
                            <div class="space-y-0.5 border-t border-gray-800 pt-1.5">
                                {searches.iter().take(5).map(|s| {
                                    let name = s.name.clone();
                                    let query = s.query.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="w-full text-left px-2 py-1 rounded text-xs text-gray-400 hover:bg-gray-800 hover:text-gray-200 flex items-center gap-1.5"
                                            title=format!("Search: {}", query)
                                            on:click=move |_| run_saved_search(query.clone())
                                        >
                                            <span aria-hidden="true">"🔍"</span>
                                            <span class="flex-1 truncate">{name}</span>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}

                // Fallback hint when nothing is saved yet
                {move || if nav.smart_folders.get().is_empty() && nav.saved_searches.get().is_empty() {
                    view! {
                        <p class="text-[11px] text-gray-600 px-1">
                            "Save a search or create a smart folder to pin it here."
                        </p>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
            </div>
        </div>
    }
}
