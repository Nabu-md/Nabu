//! # Statistics & Insights — vault-wide metrics dashboard
//!
//! Displays comprehensive vault statistics: note count, folder count, tag
//! count, graph connections, orphan notes, writing streaks, recently
//! created/modified notes, vault growth (30-day histogram), and storage
//! usage. Data is computed on demand by the backend `statistics_get`
//! command — no persistent index to maintain.

use crate::components::navigation::state::use_nav;
use crate::components::workspace::{open_tab, use_workspace};
use leptos::prelude::*;
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

// ── Statistics Component ────────────────────────────────────────────

#[component]
pub fn StatisticsView() -> impl IntoView {
    let nav = use_nav();
    let workspace = use_workspace();

    let (stats, set_stats) = signal(VaultStatistics::default());
    let (loaded, set_loaded) = signal(false);
    let (load_error, set_load_error) = signal(None::<String>);
    let (tag_filter, set_tag_filter) = signal(String::new());

    let load_stats = Callback::new(move |_| {
        set_loaded.set(false);
        set_load_error.set(None);
        spawn_local(async move {
            let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("statistics_get", empty).await;
            match serde_wasm_bindgen::from_value::<VaultStatistics>(result) {
                Ok(s) => set_stats.set(s),
                Err(e) => set_load_error.set(Some(e.to_string())),
            }
            set_loaded.set(true);
        });
    });

    load_stats.run(());

    let open_note = move |path: String| {
        open_tab(workspace, &path);
        nav.view_mode.set(crate::components::navigation::state::ViewMode::Editor);
    };

    let format_bytes = |bytes: u64| -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    };

    let max_growth = move || {
        stats
            .get()
            .growth
            .iter()
            .map(|g| g.count)
            .max()
            .unwrap_or(1)
    };

    let filtered_tags = move || {
        let f = tag_filter.get().to_lowercase();
        let s = stats.get();
        if f.is_empty() {
            s.tags.clone()
        } else {
            s.tags.iter().filter(|t| t.tag.to_lowercase().contains(&f)).cloned().collect()
        }
    };

    view! {
        <div class="statistics-view h-full overflow-y-auto bg-gray-950 text-gray-100">
            <div class="max-w-6xl mx-auto p-6 space-y-6">
                // Header
                <div class="flex items-center justify-between">
                    <div>
                        <h1 class="text-xl font-semibold text-gray-200">"Statistics & Insights"</h1>
                        <p class="text-sm text-gray-500">"Vault-wide metrics and writing insights"</p>
                    </div>
                    <button
                        class="px-3 py-1.5 text-sm bg-gray-800 rounded hover:bg-gray-700 border border-gray-700"
                        on:click=move |_| load_stats.run(())
                    >
                        "↻ Refresh"
                    </button>
                </div>

                {move || {
                    if let Some(err) = load_error.get() {
                        view! {
                            <crate::components::ui::feedback::ErrorPanel
                                title="Couldn't load statistics".to_string()
                                message="Failed to compute vault statistics.".to_string()
                                details=err
                                recovery="Make sure your vault is accessible, then try again.".to_string()
                                on_retry=load_stats
                            />
                        }.into_any()
                    } else if !loaded.get() {
                        view! {
                            <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                                {(0..8).map(|_| view! {
                                    <crate::components::ui::feedback::Skeleton width="100%" height="80px" />
                                }).collect_view()}
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }
                }}

                {move || {
                    if !loaded.get() {
                        return view! {}.into_any();
                    }
                    let s = stats.get();

                    view! {
                        // Key metrics grid
                        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-blue-400">{s.note_count}</div>
                                <div class="text-xs text-gray-500 mt-1">"Notes"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-green-400">{s.folder_count}</div>
                                <div class="text-xs text-gray-500 mt-1">"Folders"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-purple-400">{s.tag_count}</div>
                                <div class="text-xs text-gray-500 mt-1">"Unique Tags"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-yellow-400">{s.total_tags}</div>
                                <div class="text-xs text-gray-500 mt-1">"Total Tag Uses"</div>
                            </div>
                        </div>

                        // Graph metrics
                        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-cyan-400">{s.graph_nodes}</div>
                                <div class="text-xs text-gray-500 mt-1">"Graph Nodes"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-indigo-400">{s.graph_edges}</div>
                                <div class="text-xs text-gray-500 mt-1">"Graph Connections"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-orange-400">{s.graph_orphans}</div>
                                <div class="text-xs text-gray-500 mt-1">"Orphan Notes"</div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <div class="text-2xl font-bold text-pink-400">{s.graph_clusters}</div>
                                <div class="text-xs text-gray-500 mt-1">"Clusters"</div>
                            </div>
                        </div>

                        // Writing streak & storage
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3">
                                <span class="text-3xl">"🔥"</span>
                                <div>
                                    <div class="text-2xl font-bold text-red-400">{s.writing_streak_days}</div>
                                    <div class="text-xs text-gray-500">"Day writing streak"</div>
                                </div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3">
                                <span class="text-3xl">"📅"</span>
                                <div>
                                    <div class="text-2xl font-bold text-green-400">{s.active_days_last_30}</div>
                                    <div class="text-xs text-gray-500">"Active days (last 30)"</div>
                                </div>
                            </div>
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center gap-3">
                                <span class="text-3xl">"💾"</span>
                                <div>
                                    <div class="text-2xl font-bold text-blue-400">{format_bytes(s.storage_bytes)}</div>
                                    <div class="text-xs text-gray-500">"Storage usage"</div>
                                </div>
                            </div>
                        </div>

                        // Vault growth histogram
                        <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                            <h3 class="text-sm font-semibold text-gray-300 mb-3">"Vault Growth (30 days)"</h3>
                            <div class="flex items-end gap-1 h-32">
                                {s.growth.iter().map(|point| {
                                    let max = max_growth();
                                    let height = if max > 0 { (point.count as f64 / max as f64 * 100.0) as u32 } else { 0 };
                                    let height = height.max(2);
                                    view! {
                                        <div class="flex-1 flex flex-col items-center justify-end group relative">
                                            <div class="w-full bg-blue-600 rounded-t hover:bg-blue-500 transition-colors"
                                                style=format!("height: {}%", height)
                                            ></div>
                                            <div class="absolute -top-6 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-gray-400 whitespace-nowrap">
                                                {format!("{}: {}", point.date, point.count)}
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                            <div class="flex justify-between mt-2 text-xs text-gray-600">
                                <span>{s.growth.first().map(|g| g.date.clone()).unwrap_or_default()}</span>
                                <span>{s.growth.last().map(|g| g.date.clone()).unwrap_or_default()}</span>
                            </div>
                        </div>

                        // Tags
                        <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                            <div class="flex items-center justify-between mb-3">
                                <h3 class="text-sm font-semibold text-gray-300">"Tags"</h3>
                                <input
                                    type="text"
                                    placeholder="Filter tags…"
                                    class="bg-gray-800 text-gray-100 rounded px-2 py-1 text-xs border border-gray-700"
                                    on:input=move |ev| set_tag_filter.set(event_target_value(&ev))
                                />
                            </div>
                            {move || {
                                let tags = filtered_tags();
                                if tags.is_empty() {
                                    view! { <div class="text-sm text-gray-500">"No tags found"</div> }.into_any()
                                } else {
                                    view! {
                                        <div class="flex flex-wrap gap-2">
                                            {tags.iter().take(100).map(|t| {
                                                view! {
                                                    <span class="px-2 py-1 text-xs bg-gray-800 rounded text-gray-300 border border-gray-700">
                                                        {format!("{} ({})", t.tag, t.count)}
                                                    </span>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>

                        // Recently modified
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <h3 class="text-sm font-semibold text-gray-300 mb-3">"Recently Modified"</h3>
                                <div class="space-y-1">
                                    {s.recently_modified.iter().take(10).map(|n| {
                                        let path = n.path.clone();
                                        let title = n.title.clone();
                                        view! {
                                            <div class="flex items-center justify-between py-1 cursor-pointer hover:bg-gray-800 rounded px-2"
                                                on:click=move |_| open_note(path.clone())
                                            >
                                                <span class="text-sm text-gray-300 truncate">{title}</span>
                                                <span class="text-xs text-gray-500">{format_bytes(n.size as u64)}</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>

                            <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
                                <h3 class="text-sm font-semibold text-gray-300 mb-3">"Recently Created"</h3>
                                <div class="space-y-1">
                                    {s.recently_created.iter().take(10).map(|n| {
                                        let path = n.path.clone();
                                        let title = n.title.clone();
                                        view! {
                                            <div class="flex items-center justify-between py-1 cursor-pointer hover:bg-gray-800 rounded px-2"
                                                on:click=move |_| open_note(path.clone())
                                            >
                                                <span class="text-sm text-gray-300 truncate">{title}</span>
                                                <span class="text-xs text-gray-500">{format_bytes(n.size as u64)}</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}