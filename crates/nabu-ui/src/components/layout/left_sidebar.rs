//! # Left Sidebar — vault file explorer + organisational shortcuts
//!
//! Contains a FileTree placeholder (migrated in a later phase) plus a
//! **Collections** section listing saved smart folders and saved searches so
//! query-powered organisation is always one click away.

use crate::components::contexts::{use_nav, NavContext};
use crate::components::navigation::state::ViewMode;
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// Pre-computed render data for a smart folder row.
#[derive(Clone)]
struct SmartFolderRow {
    name: String,
    icon: String,
    pinned: bool,
}

/// Pre-computed render data for a saved-search row.
#[derive(Clone)]
struct SavedSearchRow {
    name: String,
    query: String,
}

/// Left sidebar — the vault file explorer plus organisational shortcuts.
#[component]
pub fn LeftSidebar() -> Element {
    let mut nav: NavContext = use_nav();

    // Clicking a saved search opens the search page prefilled with the query.
    let run_saved_search = move |query: String| {
        nav.search_query.set(query);
        nav.view_mode.set(ViewMode::Search);
    };

    // ── Pre-compute collections ──
    let smart_folders: Vec<SmartFolderRow> = nav
        .smart_folders
        .read()
        .iter()
        .map(|f| SmartFolderRow {
            name: f.name.clone(),
            icon: f.icon.clone(),
            pinned: f.pinned,
        })
        .collect();
    let saved_searches: Vec<SavedSearchRow> = nav
        .saved_searches
        .read()
        .iter()
        .map(|s| SavedSearchRow {
            name: s.name.clone(),
            query: s.query.clone(),
        })
        .collect();
    let smart_empty = smart_folders.is_empty();
    let search_empty = saved_searches.is_empty();
    let show_hint = smart_empty && search_empty;

    // Build smart folder item VNodes (avoids `let` inside rsx! for loops).
    let smart_items: Vec<Element> = smart_folders
        .iter()
        .map(|f| {
            let f_icon = f.icon.clone();
            let f_name = f.name.clone();
            let f_pinned = f.pinned;
            let nav_smart = nav;
            rsx! {
                button {
                    r#type: "button",
                    class: "w-full text-left px-2 py-1 rounded text-xs text-gray-300 hover:bg-gray-800 flex items-center gap-1.5",
                    title: "{f_name}",
                    onclick: move |_| {
                        let _ = (&f_icon, &f_pinned);
                        nav_smart.view_mode.set(ViewMode::SmartFolders);
                    },
                    span { "aria-hidden": "true", "{f_icon}" }
                    span { class: "flex-1 truncate", "{f_name}" }
                    if f_pinned {
                        span { class: "text-[10px] text-yellow-500", "aria-hidden": "true", {render_icon_view(Icon::MapPin)} }
                    }
                }
            }
        })
        .collect();

    // Build saved search item VNodes.
    let search_items: Vec<Element> = saved_searches
        .iter()
        .map(|s| {
            let s_query = s.query.clone();
            let s_name = s.name.clone();
            rsx! {
                button {
                    r#type: "button",
                    class: "w-full text-left px-2 py-1 rounded text-xs text-gray-400 hover:bg-gray-800 hover:text-gray-200 flex items-center gap-1.5",
                    title: "Search: {s_query}",
                    onclick: move |_| {
                        run_saved_search(s_query.clone());
                    },
                    span { "aria-hidden": "true", {render_icon_view(Icon::Search)} }
                    span { class: "flex-1 truncate", "{s_name}" }
                }
            }
        })
        .collect();

    rsx! {
        div {
            class: "left-sidebar sidebar-left w-64 border-r border-gray-700 bg-gray-900 h-full flex flex-col min-w-0 transition-[width] duration-slow ease-standard",

            // FileTree placeholder — migrated in a later phase.
            div {
                class: "flex-1 overflow-y-auto p-2",
                div { class: "empty-state" }
                div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(Icon::FolderTree)} }
                div { class: "empty-state-title", "File Tree" }
                div { class: "empty-state-desc", "The vault file explorer placeholder — migrated in a later phase." }
            }

            // Collections — smart folders + saved searches
            div {
                class: "border-t border-gray-800 px-2 py-2 space-y-2 overflow-y-auto flex-none max-h-64",
                span { class: "text-xs font-semibold uppercase tracking-wider text-gray-500", "Collections" }
                button {
                    r#type: "button",
                    class: "btn btn-sm btn-ghost",
                    title: "Manage smart folders",
                    "aria-label": "Manage smart folders",
                    onclick: move |_| {
                        nav.view_mode.set(ViewMode::SmartFolders);
                    },
                    {render_icon_view(Icon::FolderTree)}
                }

                if show_hint {
                    p { class: "text-[11px] text-gray-600 px-1",
                        "Save a search or create a smart folder to pin it here."
                    }
                }
                for item in smart_items {
                        {item}
                    }
                if !saved_searches.is_empty() {
                    div { class: "border-t border-gray-800 pt-1.5" }
                }
                for item in search_items {
                        {item}
                    }
            }
        }
    }
}
