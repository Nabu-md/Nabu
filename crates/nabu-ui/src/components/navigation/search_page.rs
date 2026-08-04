//! # Search page — dedicated full-text search
//!
//! Full-width results with snippets and highlighted matches, grouped by
//! folder, plus:
//!
//! - filters (folder) and sorting (relevance / modified / title)
//! - recent searches (persisted) with one-click re-run
//! - saved searches (persisted) with save / run / remove
//! - search history is recorded automatically

use crate::components::navigation::state::{
    clear_recent_searches, record_recent_search, remove_saved_search, save_search, use_nav,
};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::selection::{Select, SelectOption};
use crate::components::workspace::use_workspace;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// One search result (mirrors the backend `SearchHit`).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub folder: String,
    pub snippet: String,
    pub match_start: usize,
    pub match_end: usize,
    pub modified_at: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SortBy {
    Relevance,
    Modified,
    Title,
}

/// Splits a snippet into (before, match, after) for `<mark>` highlighting.
fn split_snippet(snippet: &str, start: usize, end: usize) -> (String, String, String) {
    let chars: Vec<char> = snippet.chars().collect();
    let start = start.min(chars.len());
    let end = end.clamp(start, chars.len());
    let before: String = chars[..start].iter().collect();
    let matched: String = chars[start..end].iter().collect();
    let after: String = chars[end..].iter().collect();
    (before, matched, after)
}

/// The Search page view.
#[component]
pub fn SearchPage() -> impl IntoView {
    let nav = use_nav();
    let ws = use_workspace();
    let toasts = use_toast();

    let (query, set_query) = signal(String::new());
    let (results, set_results) = signal(Vec::<SearchHit>::new());
    let (is_searching, set_is_searching) = signal(false);
    let sort_by = RwSignal::new(SortBy::Relevance);
    let folder_filter = RwSignal::new(String::new());
    let (save_name, set_save_name) = signal(String::new());
    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus the search input when the page mounts / query prefill changes.
    Effect::new(move |_| {
        let prefill = nav.search_query.get();
        if !prefill.is_empty() {
            set_query.set(prefill.clone());
            nav.search_query.set(String::new());
        }
        set_timeout(
            move || {
                if let Some(el) = input_ref.get() {
                    let _ = el.focus();
                }
            },
            std::time::Duration::from_millis(50),
        );
    });

    // Debounced search: 250 ms after the last keystroke. `set_timeout` does
    // not cancel previously scheduled timers, so a staleness guard inside the
    // callback makes stale (partial) queries a no-op — only the settled query
    // records into recent searches and hits the backend.
    let (dirty, set_dirty) = signal(0u32);
    Effect::new(move |_| {
        let _ = dirty.get();
        let q = query.get();
        let query_owned = q.clone();
        if query_owned.trim().is_empty() {
            set_results.set(Vec::new());
            set_is_searching.set(false);
            return;
        }
        let ws = ws;
        set_timeout(
            move || {
                let ws = ws;
                let query_final = query_owned;
                // Bail if the user typed something newer while we waited.
                if query.get() != query_final {
                    return;
                }
                set_is_searching.set(true);
                record_recent_search(nav, &query_final);
                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                        "query": query_final,
                    }))
                    .unwrap();
                    let result = crate::ipc::tauri_invoke("notes_search", args).await;
                    if let Ok(hits) = serde_wasm_bindgen::from_value::<Vec<SearchHit>>(result) {
                        set_results.set(hits);
                    }
                    set_is_searching.set(false);
                    let _ = ws;
                });
            },
            std::time::Duration::from_millis(250),
        );
    });

    // Folders available for the filter (from the vault index).
    let folders = Memo::new(move |_| {
        let mut folders = std::collections::HashSet::new();
        for note in nav.notes_index.get() {
            if !note.folder.is_empty() {
                folders.insert(note.folder.clone());
            }
        }
        let mut folders: Vec<String> = folders.into_iter().collect();
        folders.sort();
        folders
    });
    // Flat `SelectOption` list for the folder filter (reactive).
    let folder_options = Memo::new(move |_| {
        let mut opts = vec![SelectOption::new("all", "All folders")];
        for f in folders.get() {
            opts.push(SelectOption::new(f.clone(), f));
        }
        opts
    });

    // Filtered + sorted results.
    let visible = Memo::new(move |_| {
        let mut hits = results.get();
        let folder = folder_filter.get();
        if !folder.is_empty() {
            hits.retain(|h| h.folder == folder);
        }
        match sort_by.get() {
            SortBy::Relevance => {}
            SortBy::Modified => hits.sort_by(|a, b| b.modified_at.cmp(&a.modified_at)),
            SortBy::Title => hits.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        }
        hits
    });

    // Group results by folder for display.
    let grouped = Memo::new(move |_| {
        let hits = visible.get();
        let mut groups: Vec<(String, Vec<SearchHit>)> = Vec::new();
        for hit in hits {
            let folder = if hit.folder.is_empty() {
                "Vault root".to_string()
            } else {
                hit.folder.clone()
            };
            match groups.iter_mut().find(|(f, _)| *f == folder) {
                Some((_, list)) => list.push(hit),
                None => groups.push((folder, vec![hit])),
            }
        }
        groups
    });

    // Both the recent-search chips and the saved-search chips re-run a query.
    // `Callback` is cloneable, so each button can capture its own copy.
    // Recording into recent searches is owned by the debounced effect above
    // (which dedupes), so no direct call here.
    let run_search = Callback::new(move |q: String| {
        set_query.set(q);
        set_dirty.update(|v| *v = v.wrapping_add(1));
    });

    view! {
        <div class="search-page">
            <div class="search-header">
                <h1 class="dashboard-title">{render_icon_view(Icon::Search)} Search</h1>
                <p class="search-hint">"Search every note in the vault — case-insensitive full-text matching."</p>
            </div>

            <div class="search-bar panel">
                <input
                    node_ref=input_ref
                    class="search-input"
                    type="text"
                    placeholder="Search notes…  (try a keyword, folder name or phrase)"
                    prop:value=query
                    on:input=move |ev| {
                        set_query.set(event_target_value(&ev));
                        set_dirty.update(|v| *v = v.wrapping_add(1));
                    }
                />
                {move || if is_searching.get() {
                    view! {
                        <span class="search-spinner" aria-hidden="true">
                            <crate::components::ui::feedback::Spinner size=crate::components::ui::feedback::SpinnerSize::Sm label="Searching" />
                        </span>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
                <div class="search-controls">
                    <Select
                        label=""
                        options=folder_options.get()
                        value=folder_filter
                        on_change=Callback::new(move |v| folder_filter.set(v))
                    />
                    <Select
                        label=""
                        options=vec![
                            SelectOption::new("relevance", "Best match"),
                            SelectOption::new("modified", "Recently modified"),
                            SelectOption::new("title", "Title A→Z"),
                        ]
                        value=derive_sort_field(sort_by)
                        on_change=Callback::new(move |v: String| {
                            sort_by.set(match v.as_str() {
                                "modified" => SortBy::Modified,
                                "title" => SortBy::Title,
                                _ => SortBy::Relevance,
                            });
                        })
                    />
                </div>
            </div>

            // Recent + saved searches
            <div class="search-meta">
                <div class="search-chips">
                    <span class="search-meta-label">"Recent:"</span>
                    {move || {
                        let searches = nav.recent_searches.get();
                        if searches.is_empty() {
                            view! { <span class="search-meta-empty">"No recent searches yet"</span> }.into_any()
                        } else {
                            searches.into_iter().map(|s| {
                                let q = s.clone();
                                let run = run_search;
                                view! {
                                    <button
                                        type="button"
                                        class="dash-chip"
                                        on:click=move |_| run.run(q.clone())
                                    >
                                        {s}
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        on:click=move |_| clear_recent_searches(nav)
                    >
                        "Clear"
                    </button>
                </div>

                <div class="search-save-row">
                    <input
                        class="input text-sm"
                        type="text"
                        placeholder="Save this search as…"
                        prop:value=save_name
                        on:input=move |ev| set_save_name.set(event_target_value(&ev))
                    />
                    <button
                        type="button"
                        class="btn btn-sm"
                        on:click=move |_| {
                            save_search(nav, &save_name.get(), &query.get());
                            set_save_name.set(String::new());
                            toasts.success("Search saved", "You can re-run it from Saved searches.");
                        }
                    >
                        "Save search"
                    </button>
                </div>

                {move || {
                    let saved = nav.saved_searches.get();
                    if saved.is_empty() {
                        return view! {}.into_any();
                    }
                    view! {
                        <div class="search-chips">
                            <span class="search-meta-label">"Saved:"</span>
                            {saved.into_iter().map(|s| {
                                let name = s.name.clone();
                                let name_for_remove = name.clone();
                                let query = s.query.clone();
                                let run = run_search;
                                view! {
                                    <span class="dash-chip saved-chip">
                                        <button
                                            type="button"
                                            class="saved-chip-run"
                                            on:click=move |_| run.run(query.clone())
                                        >
                                            {render_icon_view(Icon::Bookmark)} {name}
                                        </button>
                                        <button
                                            type="button"
                                            class="saved-chip-remove"
                                            aria-label="Remove saved search"
                                            on:click=move |_| remove_saved_search(nav, &name_for_remove)
                                        >
                                            {render_icon_view(Icon::X)}
                                        </button>
                                    </span>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>

            // Results
            <div class="search-results">
                {move || {
                    if is_searching.get() && results.get().is_empty() {
                        view! {
                            <crate::components::ui::feedback::SkeletonList rows=5 />
                        }.into_any()
                    } else {
                    let groups = grouped.get();
                    if groups.is_empty() {
                        if query.get().trim().is_empty() {
                            view! {
                                <div class="dash-empty">
                                    "Type a query above to search your vault. Recent and saved searches appear here too."
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="dash-empty">
                                    {format!("No results for “{}”", query.get().trim())}
                                </div>
                            }.into_any()
                        }
                    } else {
                        let total: usize = groups.iter().map(|(_, l)| l.len()).sum();
                        view! {
                            <div class="search-count">{total} result(s)</div>
                            {groups.into_iter().map(|(folder, hits)| {
                                let folder_display = folder.clone();
                                let hits_clone = hits.clone();
                                view! {
                                    <div class="search-group">
                                        <div class="palette-category">{render_icon_view(Icon::Folder)} {folder_display}</div>
                                        {hits_clone.into_iter().map(|hit| {
                                            let (before, matched, after) = split_snippet(&hit.snippet, hit.match_start, hit.match_end);
                                            let path = hit.path.clone();
                                            let title = hit.title.clone();
                                            let folder = hit.folder.clone();
                                            view! {
                                                <div
                                                    class="search-hit"
                                                    on:click=move |_| {
                                                        crate::components::workspace::open_tab(ws, &path);
                                                    }
                                                >
                                                    <div class="search-hit-title">{render_icon_view(Icon::FileText)} {title}</div>
                                                    <div class="search-hit-path">{folder}</div>
                                                    <div class="search-hit-snippet">
                                                        {before}
                                                        {if !matched.is_empty() {
                                                            view! { <mark class="search-mark">{matched}</mark> }.into_any()
                                                        } else {
                                                            view! {}.into_any()
                                                        }}
                                                        {after}
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }
                            }).collect_view()}
                        }.into_any()
                    }
                    }
                }}
            </div>
        </div>
    }
}

fn derive_sort_field(sort_by: RwSignal<SortBy>) -> RwSignal<String> {
    let signal = RwSignal::new(match sort_by.get_untracked() {
        SortBy::Relevance => "relevance".to_string(),
        SortBy::Modified => "modified".to_string(),
        SortBy::Title => "title".to_string(),
    });
    Effect::new(move |_| {
        let current = match sort_by.get() {
            SortBy::Relevance => "relevance".to_string(),
            SortBy::Modified => "modified".to_string(),
            SortBy::Title => "title".to_string(),
        };
        if signal.get_untracked() != current {
            signal.set(current);
        }
    });
    signal
}
