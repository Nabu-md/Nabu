//! Reading Queue UI component.
//!
//! Production-ready reading queue with status management, priority,
//! progress tracking, filtering, search, and sorting.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Unread,
    Reading,
    Completed,
    Archived,
}

impl Default for QueueStatus {
    fn default() -> Self {
        Self::Unread
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePriority {
    Low,
    Normal,
    High,
}

impl Default for QueuePriority {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub title: String,
    pub object_type: String,
    pub status: QueueStatus,
    pub priority: QueuePriority,
    pub progress: f32, // 0.0 to 1.0
    pub source: String,
    pub modified_at: String,
    pub tags: Vec<String>,
    pub selected: bool,
}

#[derive(Clone, Default)]
pub struct QueueFilter {
    pub status: Option<QueueStatus>,
    pub priority: Option<QueuePriority>,
    pub search: String,
    pub sort_by: QueueSortField,
    pub sort_ascending: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueSortField {
    #[default]
    ModifiedAt,
    Title,
    Priority,
    Progress,
    Status,
}

// ── Reading Queue Component ────────────────────────────────────────

#[component]
pub fn ReadingQueue() -> impl IntoView {
    let (items, set_items) = signal(vec![]);
    let (filter, set_filter) = signal(QueueFilter::default());
    let (selected_ids, _set_selected_ids) = signal(Vec::<String>::new());
    let (loaded, set_loaded) = signal(false);
    let (load_error, set_load_error) = signal(None::<String>);
    let toasts = crate::components::ui::feedback::use_toast();

    // Load items on mount. `retry` is a plain fn so it can be re-run from the
    // error panel's Retry button.
    let retry = {
        let set_items = set_items;
        let set_loaded = set_loaded;
        let set_load_error = set_load_error;
        let toasts = toasts;
        Callback::new(move |_| {
            set_loaded.set(false);
            set_load_error.set(None);
            let set_items = set_items;
            let set_loaded = set_loaded;
            let set_load_error = set_load_error;
            let toasts = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "queue_get_all",
                    serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
                )
                .await;
                match serde_wasm_bindgen::from_value::<Vec<QueueItem>>(result) {
                    Ok(queue_items) => set_items.set(queue_items),
                    Err(e) => {
                        set_load_error.set(Some(e.to_string()));
                        toasts.error(
                            "Couldn't load the reading queue",
                            "Your reading queue could not be loaded — try again.",
                        );
                    }
                }
                set_loaded.set(true);
            });
        })
    };
    retry.run(());

    let filtered_items = move || {
        let f = filter.get();
        let mut result = items.get().clone();

        // Filter by status
        if let Some(status) = f.status {
            result.retain(|i| i.status == status);
        }

        // Filter by priority
        if let Some(priority) = f.priority {
            result.retain(|i| i.priority == priority);
        }

        // Filter by search
        if !f.search.is_empty() {
            let q = f.search.to_lowercase();
            result.retain(|i| {
                i.title.to_lowercase().contains(&q)
                    || i.object_type.to_lowercase().contains(&q)
                    || i.tags.iter().any(|t| t.to_lowercase().contains(&q))
            });
        }

        // Sort
        result.sort_by(|a, b| {
            let ord = match f.sort_by {
                QueueSortField::ModifiedAt => a.modified_at.cmp(&b.modified_at),
                QueueSortField::Title => a.title.cmp(&b.title),
                QueueSortField::Priority => a.priority.cmp(&b.priority),
                QueueSortField::Progress => a
                    .progress
                    .partial_cmp(&b.progress)
                    .unwrap_or(std::cmp::Ordering::Equal),
                QueueSortField::Status => a.status.cmp(&b.status),
            };
            if f.sort_ascending { ord } else { ord.reverse() }
        });

        result
    };

    let status_counts = move || {
        let all = items.get();
        (
            all.iter()
                .filter(|i| i.status == QueueStatus::Unread)
                .count(),
            all.iter()
                .filter(|i| i.status == QueueStatus::Reading)
                .count(),
            all.iter()
                .filter(|i| i.status == QueueStatus::Completed)
                .count(),
            all.iter()
                .filter(|i| i.status == QueueStatus::Archived)
                .count(),
        )
    };

    let selected_count = move || selected_ids.get().len();

    let toggle_select_item = move |id: String| {
        let mut items = items.get();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.selected = !item.selected;
        }
        set_items.set(items);
    };

    let set_status = move |id: String, status: QueueStatus| {
        let toasts = toasts;
        spawn_local(async move {
            let result = crate::ipc::tauri_invoke(
                "queue_set_status",
                serde_wasm_bindgen::to_value(&serde_json::json!({"id": id, "status": format!("{:?}", status).to_lowercase()})).unwrap(),
            ).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                toasts.error("Reading queue", "Could not update that item's status");
            }
        });
    };

    let set_priority = move |id: String, priority: QueuePriority| {
        let toasts = toasts;
        spawn_local(async move {
            let result = crate::ipc::tauri_invoke(
                "queue_set_priority",
                serde_wasm_bindgen::to_value(&serde_json::json!({"id": id, "priority": format!("{:?}", priority).to_lowercase()})).unwrap(),
            ).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                toasts.error("Reading queue", "Could not update that item's priority");
            }
        });
    };

    let set_progress = move |id: String, progress: f32| {
        let toasts = toasts;
        spawn_local(async move {
            let result = crate::ipc::tauri_invoke(
                "queue_set_progress",
                serde_wasm_bindgen::to_value(&serde_json::json!({"id": id, "progress": progress}))
                    .unwrap(),
            )
            .await;
            if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                toasts.error("Reading queue", "Could not update reading progress");
            }
        });
    };

    let batch_set_status = move |status: QueueStatus| {
        let ids: Vec<String> = items
            .get()
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            let toasts = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "queue_batch_set_status",
                    serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids, "status": format!("{:?}", status).to_lowercase()})).unwrap(),
                ).await;
                if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                    toasts.error("Reading queue", "Could not update the selected items");
                }
            });
        }
    };

    let on_filter_change = move |ev| {
        let mut f = filter.get();
        f.search = event_target_value(&ev);
        set_filter.set(f);
    };

    let on_status_filter = move |status: QueueStatus| {
        let mut f = filter.get();
        f.status = if f.status == Some(status) {
            None
        } else {
            Some(status)
        };
        set_filter.set(f);
    };

    let on_priority_filter = move |priority: QueuePriority| {
        let mut f = filter.get();
        f.priority = if f.priority == Some(priority) {
            None
        } else {
            Some(priority)
        };
        set_filter.set(f);
    };

    let on_sort_change = move |field: QueueSortField| {
        let mut f = filter.get();
        if f.sort_by == field {
            f.sort_ascending = !f.sort_ascending;
        } else {
            f.sort_by = field;
            f.sort_ascending = true;
        }
        set_filter.set(f);
    };

    view! {
        <div class="reading-queue flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            // Left panel: Queue list
            <div class="flex-none w-96 border-r border-gray-800 flex flex-col">
                // Header
                <div class="flex items-center justify-between px-4 py-3 border-b border-gray-800">
                    <h2 class="text-sm font-semibold text-gray-300">"Reading Queue"</h2>
                    <span class="text-xs text-gray-500">{move || format!("{} items", items.get().len())}</span>
                </div>

                // Search
                <div class="px-3 py-2 border-b border-gray-800">
                    <input
                        type="text"
                        placeholder="Search queue..."
                        class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        on:input=on_filter_change
                    />
                </div>

                // Status filter pills
                <div class="flex items-center gap-2 px-3 py-2 border-b border-gray-800">
                    {move || {
                        let (unread, reading, completed, archived) = status_counts();
                        view! {
                            <button
                                class=move || format!("px-2 py-0.5 text-xs rounded-full border transition-colors {}",
                                    if filter.get().status == Some(QueueStatus::Unread) { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400 hover:text-gray-200" })
                                on:click=move |_| on_status_filter(QueueStatus::Unread)
                            >
                                {format!("Unread {}", unread)}
                            </button>
                            <button
                                class=move || format!("px-2 py-0.5 text-xs rounded-full border transition-colors {}",
                                    if filter.get().status == Some(QueueStatus::Reading) { "bg-yellow-900/50 border-yellow-600 text-yellow-300" } else { "border-gray-700 text-gray-400 hover:text-gray-200" })
                                on:click=move |_| on_status_filter(QueueStatus::Reading)
                            >
                                {format!("Reading {}", reading)}
                            </button>
                            <button
                                class=move || format!("px-2 py-0.5 text-xs rounded-full border transition-colors {}",
                                    if filter.get().status == Some(QueueStatus::Completed) { "bg-green-900/50 border-green-600 text-green-300" } else { "border-gray-700 text-gray-400 hover:text-gray-200" })
                                on:click=move |_| on_status_filter(QueueStatus::Completed)
                            >
                                {format!("Done {}", completed)}
                            </button>
                            <button
                                class=move || format!("px-2 py-0.5 text-xs rounded-full border transition-colors {}",
                                    if filter.get().status == Some(QueueStatus::Archived) { "bg-gray-800 border-gray-600 text-gray-400" } else { "border-gray-700 text-gray-400 hover:text-gray-200" })
                                on:click=move |_| on_status_filter(QueueStatus::Archived)
                            >
                                {format!("Archived {}", archived)}
                            </button>
                        }.into_any()
                    }}
                </div>

                // Priority filter
                <div class="flex items-center gap-2 px-3 py-1.5 border-b border-gray-800">
                    <span class="text-xs text-gray-500">"Priority:"</span>
                    <button
                        class=move || format!("px-2 py-0.5 text-xs rounded border {}",
                            if filter.get().priority == Some(QueuePriority::High) { "bg-red-900/50 border-red-600 text-red-300" } else { "border-gray-700 text-gray-400" })
                        on:click=move |_| on_priority_filter(QueuePriority::High)
                    >
                        "High"
                    </button>
                    <button
                        class=move || format!("px-2 py-0.5 text-xs rounded border {}",
                            if filter.get().priority == Some(QueuePriority::Normal) { "bg-gray-700 border-gray-500 text-gray-300" } else { "border-gray-700 text-gray-400" })
                        on:click=move |_| on_priority_filter(QueuePriority::Normal)
                    >
                        "Normal"
                    </button>
                    <button
                        class=move || format!("px-2 py-0.5 text-xs rounded border {}",
                            if filter.get().priority == Some(QueuePriority::Low) { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400" })
                        on:click=move |_| on_priority_filter(QueuePriority::Low)
                    >
                        "Low"
                    </button>
                </div>

                // Batch actions
                {move || {
                    let count = selected_count();
                    if count > 0 {
                        view! {
                            <div class="flex items-center gap-2 px-3 py-2 bg-blue-900/30 border-b border-blue-800/30">
                                <span class="text-xs text-blue-400">{count} selected</span>
                                <button class="px-2 py-1 text-xs bg-green-700 rounded hover:bg-green-600" on:click=move |_| batch_set_status(QueueStatus::Reading)>"Mark Reading"</button>
                                <button class="px-2 py-1 text-xs bg-blue-700 rounded hover:bg-blue-600" on:click=move |_| batch_set_status(QueueStatus::Completed)>"Mark Done"</button>
                                <button class="px-2 py-1 text-xs bg-gray-700 rounded hover:bg-gray-600" on:click=move |_| batch_set_status(QueueStatus::Archived)>"Archive"</button>
                            </div>
                        }.into_any()
                    } else { view! {}.into_any() }
                }}

                // Queue list
                <div class="flex-1 overflow-y-auto">
                    {move || {
                        if let Some(err) = load_error.get() {
                            view! {
                                <div class="p-4">
                                    <crate::components::ui::feedback::ErrorPanel
                                        title="Couldn't load the reading queue".to_string()
                                        message="Something went wrong while reading your queue.".to_string()
                                        details=err
                                        recovery="Check that your vault is accessible, then try again.".to_string()
                                        on_retry=retry
                                    />
                                </div>
                            }.into_any()
                        } else if !loaded.get() {
                            view! {
                                <div class="p-4">
                                    <crate::components::ui::feedback::SkeletonList rows=6 />
                                </div>
                            }.into_any()
                        } else if filtered_items().is_empty() {
                            view! {
                                <div class="h-full flex items-center justify-center p-6">
                                    <crate::components::ui::info::EmptyState
                                        icon=crate::components::ui::icons::Icon::BookOpen
                                        title="Nothing in the queue".to_string()
                                        description="Add documents or web articles to your reading queue and track progress here.".to_string()
                                    ></crate::components::ui::info::EmptyState>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800">
                                    { filtered_items().iter().map(|item| {
                                        let id = item.id.clone();
                                        let title = item.title.clone();
                                        let status = item.status;
                                        let priority = item.priority;
                                        let progress = item.progress;
                                        let object_type = item.object_type.clone();
                                        let selected = item.selected;
                                        view! {
                                            <div class=move || format!(
                                                "queue-item px-3 py-2 cursor-pointer hover:bg-gray-800 border-l-2 {} {}",
                                                if selected { "bg-gray-800 border-l-blue-500" } else { "border-l-transparent" },
                                                match priority {
                                                    QueuePriority::High => "border-l-red-500",
                                                    QueuePriority::Normal => "",
                                                    QueuePriority::Low => "border-l-blue-500",
                                                }
                                            )
                                                on:click=move |_| toggle_select_item(id.clone())>
                                                <div class="flex items-center justify-between">
                                                    <span class="text-sm font-medium truncate max-w-48">{title}</span>
                                                    <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">
                                                        {move || format!("{:?}", status)}
                                                    </span>
                                                </div>
                                                <div class="flex items-center gap-2 mt-1">
                                                    <span class="text-xs text-gray-500">{object_type}</span>
                                                    // Progress bar
                                                    <div class="flex-1 h-1 bg-gray-700 rounded-full overflow-hidden">
                                                        <div class=move || format!("h-full bg-blue-500 rounded-full transition-all {}", format!("width: {}%", (progress * 100.0) as u32))></div>
                                                    </div>
                                                    <span class="text-xs text-gray-500">{format!("{}%", (progress * 100.0) as u32)}</span>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                // Footer with sort controls
                <div class="flex items-center justify-between px-3 py-2 border-t border-gray-800 text-xs text-gray-500">
                    <span>{move || format!("{} items", filtered_items().len())}</span>
                    <div class="flex gap-2">
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(QueueSortField::ModifiedAt)>"Sort by Date"</button>
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(QueueSortField::Title)>"Sort by Title"</button>
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(QueueSortField::Priority)>"Sort by Priority"</button>
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(QueueSortField::Progress)>"Sort by Progress"</button>
                    </div>
                </div>
            </div>

            // Right panel: Detail view
            <div class="flex-1 flex flex-col overflow-hidden">
                {move || {
                    let items_guard = items.get();
                    let selected = items_guard.iter().find(|i| i.selected);
                    match selected {
                        Some(item) => view! {
                            <div class="flex h-full">
                                <div class="flex-1 overflow-y-auto p-4">
                                    <div class="space-y-4">
                                        <h2 class="text-xl font-semibold">{item.title.clone()}</h2>
                                        <div class="grid grid-cols-2 gap-4">
                                            <div>
                                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Status"</label>
                                                <div class="mt-1">
                                                    <select
                                                        class="bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700"
                                                        on:change={let id = item.id.clone(); move |ev| {
                                                            let status = match event_target_value(&ev).as_str() {
                                                                "reading" => QueueStatus::Reading,
                                                                "completed" => QueueStatus::Completed,
                                                                "archived" => QueueStatus::Archived,
                                                                _ => QueueStatus::Unread,
                                                            };
                                                            set_status(id.clone(), status);
                                                        }}
                                                    >
                                                        <option value="unread" selected={item.status == QueueStatus::Unread}>"Unread"</option>
                                                        <option value="reading" selected={item.status == QueueStatus::Reading}>"Reading"</option>
                                                        <option value="completed" selected={item.status == QueueStatus::Completed}>"Completed"</option>
                                                        <option value="archived" selected={item.status == QueueStatus::Archived}>"Archived"</option>
                                                    </select>
                                                </div>
                                            </div>
                                            <div>
                                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Priority"</label>
                                                <div class="mt-1">
                                                    <select
                                                        class="bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700"
                                                        on:change={let id = item.id.clone(); move |ev| {
                                                            let priority = match event_target_value(&ev).as_str() {
                                                                "high" => QueuePriority::High,
                                                                "low" => QueuePriority::Low,
                                                                _ => QueuePriority::Normal,
                                                            };
                                                            set_priority(id.clone(), priority);
                                                        }}
                                                    >
                                                        <option value="normal" selected={item.priority == QueuePriority::Normal}>"Normal"</option>
                                                        <option value="high" selected={item.priority == QueuePriority::High}>"High"</option>
                                                        <option value="low" selected={item.priority == QueuePriority::Low}>"Low"</option>
                                                    </select>
                                                </div>
                                            </div>
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Progress"</label>
                                            <div class="mt-1 flex items-center gap-3">
                                                <input
                                                    type="range"
                                                    min="0"
                                                    max="100"
                                                    value={format!("{}", (item.progress * 100.0) as u32)}
                                                    class="flex-1"
                                                    on:input={let id = item.id.clone(); move |ev| {
                                                        let progress = event_target_value(&ev).parse::<f32>().unwrap_or(0.0) / 100.0;
                                                        set_progress(id.clone(), progress);
                                                    }}
                                                />
                                                <span class="text-sm text-gray-400">{format!("{}%", (item.progress * 100.0) as u32)}</span>
                                            </div>
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Type"</label>
                                            <p class="text-sm text-gray-300 mt-1">{item.object_type.clone()}</p>
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Source"</label>
                                            <p class="text-sm text-gray-300 mt-1">{item.source.clone()}</p>
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Tags"</label>
                                            <div class="flex flex-wrap gap-1 mt-1">
                                                { item.tags.iter().map(|tag| view! { <span class="px-2 py-0.5 text-xs bg-gray-700 rounded text-gray-300">{tag.clone()}</span> }).collect_view()}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }.into_any(),
                        None => view! { <div class="flex items-center justify-center h-full text-gray-500">"Select an item to view details"</div> }.into_any(),
                    }
                }}
            </div>
        </div>
    }
}
