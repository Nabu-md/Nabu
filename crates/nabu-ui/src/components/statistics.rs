//! # Statistics & Insights — Dioxus migration
//!
//! Displays comprehensive vault statistics: note count, folder count, tag
//! count, graph connections, orphan notes, writing streaks, recently
//! created/modified notes, vault growth (30-day histogram), and storage
//! usage. Data is computed on demand by the backend `statistics_get`
//! command — no persistent index to maintain.
//!
//! Runtime metrics (timers, counters, gauges) are consumed from the
//! centralized [`crate::metrics::MetricsContext`] — see that module for the
//! end-to-end metrics pipeline (backend → IPC → frontend store → UI).

use crate::components::contexts::{open_tab, use_nav, use_workspace};
use crate::components::ui::feedback::{ErrorPanel, Skeleton};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::metrics::MetricsContext;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types (mirror backend `VaultStatistics`) ──────────────────────────

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

// ── Worker Pool Snapshot Type ────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PoolHealthSnapshot {
    worker_count: usize,
    shutting_down: bool,
    pending_jobs: usize,
    running_jobs: usize,
    active_workers: usize,
    is_throttled: bool,
    is_full: bool,
    lifecycle_stage: String,
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
#[allow(dead_code)]
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

/// Loads worker pool health from the backend.
fn reload_pool_health(
    pool: Signal<PoolHealthSnapshot>,
    loaded: Signal<bool>,
    error: Signal<Option<String>>,
) {
    let mut pool = pool;
    let mut loaded = loaded;
    let mut error = error;
    loaded.set(false);
    error.set(None);
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("pool_health", empty).await;
        match serde_wasm_bindgen::from_value::<PoolHealthSnapshot>(result) {
            Ok(p) => pool.set(p),
            Err(e) => error.set(Some(e.to_string())),
        }
        loaded.set(true);
    });
}

#[component]
pub fn StatisticsView() -> Element {
    let _nav: crate::components::navigation::state::NavContext = use_nav();
    let workspace = use_workspace();

    // Vault statistics (owned by this view — computed on demand).
    let stats = use_signal(VaultStatistics::default);
    let loaded = use_signal(|| false);
    let load_error = use_signal(|| None::<String>);
    let mut tag_filter = use_signal(String::new);

    // Runtime metrics — consumed from the centralized MetricsContext.
    let metrics_ctx: MetricsContext = use_context::<MetricsContext>();

    // Worker pool health (owned by this view).
    let pool = use_signal(PoolHealthSnapshot::default);
    let pool_loaded = use_signal(|| false);
    let pool_error = use_signal(|| None::<String>);

    // Initial load — mirrors LePtOS `load_stats.run(())` on mount.
    let mut loaded_once = use_signal(|| false);
    if !*loaded_once.read() {
        loaded_once.set(true);
        reload_stats(stats, loaded, load_error);
        reload_pool_health(pool, pool_loaded, pool_error);
    }

    let open_note = move |path: String| {
        let ws = workspace;
        open_tab(ws, &path);
    };

    // Manual refresh for stats and pool; metrics are refreshed centrally.
    let refresh_all = move |_: MouseEvent| {
        reload_stats(stats, loaded, load_error);
        reload_pool_health(pool, pool_loaded, pool_error);
        crate::metrics::reload_metrics(metrics_ctx);
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
            onclick: refresh_all,
            {render_icon_view(Icon::RefreshCw)}
            " Refresh"
        }

        // Error / loading state for vault statistics
        {if let Some(err) = load_error.read().clone() {
            rsx! {
                ErrorPanel {
                    title: "Couldn't load statistics".to_string(),
                    message: "Failed to compute vault statistics.".to_string(),
                    details: err,
                    recovery: "Make sure your vault is accessible, then try again.".to_string(),
                    on_retry: move |_: ()| reload_stats(stats, loaded, load_error),
                }
            }
        } else if !*loaded.read() {
            rsx! {
                div { class: "grid grid-cols-2 md:grid-cols-4 gap-4" }
                for _ in 0..8 {
                    Skeleton { width: "100%", height: "80px" }
                }
            }
        } else {
            rsx! {}
        }}

        // Main content (only when loaded)
        {if !*loaded.read() {
            rsx! {}
        } else {
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
                                onclick: move |_: MouseEvent| open_note(path.clone()),
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
                                onclick: move |_: MouseEvent| open_note(path.clone()),
                                span { class: "text-sm text-gray-300 truncate", "{title}" }
                                span { class: "text-xs text-gray-500", "{format_bytes(size as u64)}" }
                            }
                        }
                    }
                }

                // Worker Pool Health
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Worker Pool Health" }
                {if !*pool_loaded.read() {
                    rsx! {
                        div { class: "space-y-1" }
                        for _ in 0..4 {
                            Skeleton { width: "100%", height: "24px" }
                        }
                    }
                } else {
                    let p = pool.read().clone();
                    rsx! {
                        div { class: "grid grid-cols-2 md:grid-cols-4 gap-3" }
                        div { class: "flex flex-col" }
                        div { class: "text-xl font-bold text-blue-400", "{p.worker_count}" }
                        div { class: "text-xs text-gray-500", "Workers" }

                        div { class: "flex flex-col" }
                        div { class: "text-xl font-bold text-green-400", "{p.active_workers}" }
                        div { class: "text-xs text-gray-500", "Active" }

                        div { class: "flex flex-col" }
                        div { class: "text-xl font-bold text-yellow-400", "{p.pending_jobs}" }
                        div { class: "text-xs text-gray-500", "Pending" }

                        div { class: "flex flex-col" }
                        div { class: "text-xl font-bold text-purple-400", "{p.running_jobs}" }
                        div { class: "text-xs text-gray-500", "Running" }

                        div { class: "mt-3 text-xs text-gray-500", "Lifecycle: {p.lifecycle_stage}" }
                        {if p.is_throttled {
                            rsx! { span { class: "text-xs text-orange-400", "Throttled" } }
                        } else {
                            rsx! { span { class: "text-xs text-gray-500", "Normal capacity" } }
                        }}
                    }
                }}

                // Performance Metrics — sourced from the centralized MetricsContext
                div { class: "bg-gray-900 border border-gray-800 rounded-lg p-4" }
                h3 { class: "text-sm font-semibold text-gray-300 mb-3", "Performance Metrics" }
                {if metrics_ctx.loading.read().to_owned() {
                    rsx! {
                        div { class: "space-y-1" }
                        for _ in 0..6 {
                            Skeleton { width: "100%", height: "24px" }
                        }
                    }
                } else if let Some(err) = metrics_ctx.error.read().as_ref() {
                    rsx! {
                        div { class: "text-sm text-yellow-400", "Metrics unavailable ({err})" }
                    }
                } else {
                    let m = metrics_ctx.metrics.read().clone();
                    rsx! {
                        // Timers
                        {if !m.timers.is_empty() {
                            rsx! {
                                div { class: "mb-3" }
                                h4 { class: "text-xs font-semibold text-gray-500 uppercase mb-2", "Timers" }
                                table { class: "w-full text-xs" }
                                thead {
                                    tr {
                                        th { class: "text-left text-gray-600 pb-1", "Operation" }
                                        th { class: "text-right text-gray-600 pb-1", "Count" }
                                        th { class: "text-right text-gray-600 pb-1", "Avg (ms)" }
                                        th { class: "text-right text-gray-600 pb-1", "p50" }
                                        th { class: "text-right text-gray-600 pb-1", "p90" }
                                        th { class: "text-right text-gray-600 pb-1", "Max" }
                                    }
                                }
                                tbody {
                                    for t in &m.timers {
                                        {
                                            let key = t.key.clone();
                                            let s = t.stats.clone();
                                            let avg = format!("{:.1}", s.avg_ms);
                                            let p50 = format!("{:.1}", s.p50_ms);
                                            let p90 = format!("{:.1}", s.p90_ms);
                                            let max = format!("{:.1}", s.max_ms);
                                            rsx! {
                                                tr {
                                                    td { class: "text-gray-400 py-1", "{key}" }
                                                    td { class: "text-right text-gray-500", "{s.count}" }
                                                    td { class: "text-right text-gray-500", "{avg}" }
                                                    td { class: "text-right text-gray-500", "{p50}" }
                                                    td { class: "text-right text-gray-500", "{p90}" }
                                                    td { class: "text-right text-gray-500", "{max}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else { rsx!{} }}
                        // Counters
                        {if !m.counters.is_empty() {
                            rsx! {
                                div { class: "mt-3" }
                                h4 { class: "text-xs font-semibold text-gray-500 uppercase mb-2", "Counters" }
                                table { class: "w-full text-xs" }
                                tbody {
                                    for c in &m.counters {
                                        {
                                            let key = c.key.clone();
                                            let val = c.value;
                                            rsx! {
                                                tr {
                                                    td { class: "text-gray-400 py-1", "{key}" }
                                                    td { class: "text-right text-gray-500", "{val}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else { rsx!{} }}
                        // Gauges
                        {if !m.gauges.is_empty() {
                            rsx! {
                                div { class: "mt-3" }
                                h4 { class: "text-xs font-semibold text-gray-500 uppercase mb-2", "Gauges" }
                                table { class: "w-full text-xs" }
                                tbody {
                                    for g in &m.gauges {
                                        {
                                            let key = g.key.clone();
                                            let val = g.value;
                                            rsx! {
                                                tr {
                                                    td { class: "text-gray-400 py-1", "{key}" }
                                                    td { class: "text-right text-gray-500", "{val}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else { rsx!{} }}
                    }
                }}
            }
        }}
    }
}
