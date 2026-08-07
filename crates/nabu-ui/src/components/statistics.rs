//! # Statistics & Insights — Dioxus migration
//!
//! Displays comprehensive vault statistics: note count, folder count, tag
//! count, graph connections, orphan notes, writing streaks, recently
//! created/modified notes, vault growth (30-day histogram), and storage
//! usage. Data is computed on demand by the backend `statistics_get`
//! command — no persistent index to maintain.

use crate::components::contexts::{open_tab, use_nav, use_workspace};
use crate::components::ui::feedback::{ErrorPanel, Skeleton};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types (mirror backend `VaultStatistics`) ───────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TagStat {
    tag: String,
    count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GrowthPoint {
    date: String,
    count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RecentNoteStat {
    path: String,
    title: String,
    folder: String,
    modified_at: String,
    created_at: Option<String>,
    size: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct VaultStatistics {
    note_count: usize,
    folder_count: usize,
    tag_count: usize,
    total_tags: usize,
    graph_nodes: usize,
    graph_edges: usize,
    graph_orphans: usize,
    graph_clusters: usize,
    tags: Vec<TagStat>,
    recently_created: Vec<RecentNoteStat>,
    recently_modified: Vec<RecentNoteStat>,
    growth: Vec<GrowthPoint>,
    storage_bytes: u64,
    writing_streak_days: usize,
    active_days_last_30: usize,
}

/// Human-readable byte size.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Triggers a settings save so the theme change is persisted.
fn save_theme(theme: &str) {
    let args = serde_wasm_bindgen::to_value(
        &serde_json::json!({ "key": "theme", "value": theme }),
    )
    .unwrap();
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("settings_set", args).await;
    });
}

/// Loads vault statistics from the backend.
fn reload_stats(
    stats: Signal<VaultStatistics>,
    loaded: Signal<bool>,
    error: Signal<Option<String>>,
) {
    let mut stats = stats;
    let mut loaded = loaded;
    let mut error = error;
    loaded.set(false);
    error.set(None);
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("statistics_get", empty).await;
        match serde_wasm_bindgen::from_value::<VaultStatistics>(result) {
            Ok(s) => stats.set(s),
            Err(e) => error.set(Some(e.to_string())),
        }
        loaded.set(true);
    });
}

#[component]
pub fn StatisticsView() -> Element {
    let nav: crate::components::navigation::state::NavContext = use_nav();
    let workspace = use_workspace();

    let mut stats = use_signal(VaultStatistics::default);
    let mut loaded = use_signal(|| false);
    let mut load_error = use_signal(|| None::<String>);
    let mut tag_filter = use_signal(String::new);

    // Initial load — mirrors LePtOS `load_stats.run(())` on mount.
    let mut loaded_once = use_signal(|| false);
    if !*loaded_once.read() {
        loaded_once.set(true);
        reload_stats(stats, loaded, load_error);
    }

    let open_note = move |path: String| {
        let ws = workspace;
        open_tab(ws, &path);
    };

    rsx! {
        div { class: "statistics-view h-full overflow-y-auto bg-gray-950 text-gray-100" }
        div { class: "max-w-6xl mx-auto p-6 space-y-6" }

        // Header
        div { class: "flex items-center justify-between" }
        div {
            h1 { class: "text-xl font-semibold text-gray-200", "Statistics & Insights" }
            p { class: "text-sm text-gray-500", "Vault-wide metrics and writing insights" }
        }
        button {
            class: "px-3 py-1.5 text-sm bg-gray-800 rounded hover:bg-gray-700 border border-gray-700",
            onclick: move |_: MouseEvent| reload_stats(stats, loaded, load_error),
            {render_icon_view(Icon::RefreshCw)}
            " Refresh"
        }

        // Error / loading state
        {move || {
            let err = load_error.read().clone();
            if let Some(err) = err {
                rsx! {
                    ErrorPanel {
                        title: "Couldn't load statistics".to_string(),
                        message: "Failed to compute vault statistics.".to_string(),
                        details: err,
                        recovery: "Make sure your vault is accessible, then try again.".to_string(),
                        on_retry: move |_: ()| reload_stats(stats, loaded, load_error),
                    }
                }
            } else if !loaded.read() {
                rsx! {
                    div { class: "grid grid-cols-2 md:grid-cols-4 gap-4" }
                    for _ in 0..8 {
                        Skeleton { width: "100%", height: "80px" }
                    }
                }
            }
        }}

        // Main content (only when loaded)
        {move || {
            if !loaded.read() {
                return rsx! {};
            }
            let s = stats.read().clone();
            let max_growth = s.growth.iter().map(|g| g.count).max().unwrap_or(1);
            let query = tag_filter.read().to_lowercase();
            let filtered_tags: Vec<TagStat> = if query.is_empty() {
                s.tags.clone()
            } else {
                s.tags.iter()
                    .filter(|t| t.tag.to_lowercase().contains(&query))
                    .cloned()
                    .collect()
            };

            rsx! {
                // Key metrics grid
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4" }
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-blue-400", "{s.note_count}" }
                div { class: "text-xs text-gray-500 mt-1", "Notes" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-green-400", "{s.folder_count}" }
                div { class: "text-xs text-gray-500 mt-1", "Folders" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-purple-400", "{s.tag_count}" }
                div { class: "text-xs text-gray-500 mt-1", "Unique Tags" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-yellow-400", "{s.total_tags}" }
                div { class: "text-xs text-gray-500 mt-1", "Total Tag Uses" }

                // Graph metrics
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4" }
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-cyan-400", "{s.graph_nodes}" }
                div { class: "text-xs text-gray-500 mt-1", "Graph Nodes" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-indigo-400", "{s.graph_edges}" }
                div { class: "text-xs text-gray-500 mt-1", "Graph Connections" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-orange-400", "{s.graph_orphans}" }
                div { class: "text-xs text-gray-500 mt-1", "Orphan Notes" }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "text-2xl font-bold text-pink-400", "{s.graph_clusters}" }
                div { class: "text-xs text-gray-500 mt-1", "Clusters" }

                // Writing streak & storage
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4" }
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3" }
                span { class: "text-3xl", {render_icon_view(Icon::Flame)} }
                div {
                    div { class: "text-2xl font-bold text-red-400", "{s.writing_streak_days}" }
                    div { class: "text-xs text-gray-500", "Day writing streak" }
                }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3" }
                span { class: "text-3xl", {render_icon_view(Icon::Calendar)} }
                div {
                    div { class: "text-2xl font-bold text-green-400", "{s.active_days_last_30}" }
                    div { class: "text-xs text-gray-500", "Active days (last 30)" }
                }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3" }
                span { class: "text-3xl", {render_icon_view(Icon::HardDrive)} }
                div {
                    div { class: "text-2xl font-bold text-blue-400", "{format_bytes(s.storage_bytes)}" }
                    div { class: "text-xs text-gray-500", "Storage usage" }
                }

                // Vault growth histogram
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Vault Growth (30 days)" }
                div { class: "flex items-end gap-1 h-32" }
                for point in &s.growth {
                    {
                        let pct = if max_growth > 0 {
                            (point.count as f64 / max_growth as f64 * 100.0) as u32
                        } else {
                            0
                        };
                        let height = pct.max(2);
                        let label = format!("{}: {}", point.date, point.count);
                        rsx! {
                            div { class: "flex-1 flex flex-col items-center justify-end group relative" }
                            div {
                                class: "w-full bg-blue-600 rounded-t hover:bg-blue-500 transition-colors",
                                style: "height: {height}%",
                            }
                            div { class: "absolute -top-6 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-gray-400 whitespace-nowrap", "{label}" }
                        }
                    }
                }
                div { class: "flex justify-between mt-2 text-xs text-gray-600" }
                span { "{s.growth.first().map(|g| g.date.clone()).unwrap_or_default()}" }
                span { "{s.growth.last().map(|g| g.date.clone()).unwrap_or_default()}" }

                // Tags
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                div { class: "flex items-center justify-between mb-3" }
                h3 { class: "text-sm font-semibold text-gray-300", "Tags" }
                input {
                    r#type: "text",
                    placeholder: "Filter tags…",
                    class: "bg-gray-800 text-gray-100 rounded px-2 py-1 text-xs border border-gray-700",
                    value: "{tag_filter.read()}",
                    oninput: move |ev: FormEvent| tag_filter.set(ev.value()),
                }
                {if filtered_tags.is_empty() {
                    rsx! { div { class: "text-sm text-gray-500", "No tags found" } }
                } else {
                    rsx! {
                        div { class: "flex flex-wrap gap-2" }
                        for t in &filtered_tags {
                            {
                                let label = format!("{} ({})", t.tag, t.count);
                                rsx! {
                                    span {
                                        class: "px-2 py-1 text-xs bg-gray-800 rounded text-gray-300 border border-gray-700",
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }}

                // Recently modified & recently created
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4" }
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Recently Modified" }
                div { class: "space-y-1" }
                for n in &s.recently_modified {
                    {
                        let path = n.path.clone();
                        let title = n.title.clone();
                        let size = n.size;
                        rsx! {
                            div {
                                class: "flex items-center justify-between py-1 cursor-pointer hover:bg-gray-800 rounded px-2",
                                onclick: move |_: MouseEvent| open_note(path),
                                span { class: "text-sm text-gray-300 truncate", "{title}" }
                                span { class: "text-xs text-gray-500", "{format_bytes(size as u64)}" }
                            }
                        }
                    }
                }

                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Recently Created" }
                div { class: "space-y-1" }
                for n in &s.recently_created {
                    {
                        let path = n.path.clone();
                        let title = n.title.clone();
                        let size = n.size;
                        rsx! {
                            div {
                                class: "flex items-center justify-between py-1 cursor-pointer hover:bg-gray-800 rounded px-2",
                                onclick: move |_: MouseEvent| open_note(path),
                                span { class: "text-sm text-gray-300 truncate", "{title}" }
                                span { class: "text-xs text-gray-500", "{format_bytes(size as u64)}" }
                            }
                        }
                    }
                }
            }
        }}
    }
}
