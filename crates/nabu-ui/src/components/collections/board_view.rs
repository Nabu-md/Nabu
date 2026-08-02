//! Board View component.
//!
//! Production-ready Kanban-style board view with columns,
//! filtering, sorting, grouping, and drag-and-drop support.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::models::knowledge_object::KnowledgeObject;

#[derive(Clone, PartialEq, Default)]
pub struct BoardColumn {
    pub id: String,
    pub title: String,
    pub items: Vec<KnowledgeObject>,
}

#[derive(Clone, PartialEq, Default)]
pub struct BoardFilter {
    pub query: String,
    pub object_type: Option<String>,
    pub group_by: String, // "status", "type", "priority"
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub objects: Vec<KnowledgeObject>,
    pub columns: Vec<BoardColumn>,
    pub filter: BoardFilter,
    pub on_filter_change: Callback<BoardFilter>,
    pub on_move_item: Callback<(String, String)>, // (item_id, target_column_id)
}

#[function_component(BoardView)]
pub fn board_view(props: &Props) -> Html {
    let filtered = move || {
        let f = &props.filter;
        let mut result = props.objects.clone();

        if !f.query.is_empty() {
            let q = f.query.to_lowercase();
            result.retain(|obj| {
                obj.metadata.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&q))
            });
        }

        if let Some(ref ot) = f.object_type {
            result.retain(|obj| obj.object_type.to_string() == *ot);
        }

        result
    };

    let grouped = move || {
        let f = &props.filter;
        let items = filtered();
        let mut groups: std::collections::HashMap<String, Vec<KnowledgeObject>> = std::collections::HashMap::new();

        for obj in items {
            let key = match f.group_by.as_str() {
                "type" => obj.object_type.to_string(),
                "priority" => {
                    obj.metadata.custom.get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("normal")
                        .to_string()
                }
                _ => obj.metadata.custom.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string(),
            };
            groups.entry(key).or_default().push(obj);
        }

        groups
    };

    let on_drag_start = move |ev: DragEvent, item_id: String| {
        ev.data_transfer().unwrap().set_data("text/plain", &item_id).unwrap();
    };

    let on_drop = move |ev: DragEvent, column_id: String| {
        ev.prevent_default();
        let data = ev.data_transfer().unwrap().get_data("text/plain").unwrap();
        props.on_move_item.emit((data, column_id));
    };

    let on_drag_over = move |ev: DragEvent| {
        ev.prevent_default();
    };

    view! {
        <div class="board-view flex gap-4 overflow-x-auto p-4 h-full">
            {move || {
                let groups = grouped();
                let columns = props.columns.clone();

                if columns.is_empty() {
                    // Auto-generate columns from grouped data
                    view! {
                        <div class="flex gap-4 overflow-x-auto h-full">
                            { groups.iter().map(|(group_key, items)| {
                                let column_id = group_key.clone();
                                view! {
                                    <div
                                        class="flex-none w-72 bg-gray-800 rounded-lg border border-gray-700 flex flex-col max-h-full"
                                        on:dragover=on_drag_over
                                        on:drop=move |ev| on_drop(ev, column_id.clone())
                                    >
                                        <div class="px-3 py-2 border-b border-gray-700 flex items-center justify-between">
                                            <span class="text-sm font-medium text-gray-300">{group_key}</span>
                                            <span class="text-xs text-gray-500">{items.len()}</span>
                                        </div>
                                        <div class="flex-1 overflow-y-auto p-2 space-y-2">
                                            { items.iter().map(|obj| {
                                                let obj_id = obj.id.to_string();
                                                let title = obj.metadata.title.clone().unwrap_or_default();
                                                view! {
                                                    <div
                                                        class="bg-gray-700 rounded p-2 border border-gray-600 cursor-grab hover:border-gray-500 transition-colors"
                                                        draggable="true"
                                                        on:dragstart=move |ev| on_drag_start(ev, obj_id.clone())
                                                    >
                                                        <div class="text-sm font-medium text-gray-200">{title}</div>
                                                        <div class="text-xs text-gray-500 mt-1">{obj.object_type.to_string()}</div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="flex gap-4 overflow-x-auto h-full">
                            { columns.iter().map(|col| {
                                let column_id = col.id.clone();
                                let items = groups.get(&col.id).cloned().unwrap_or_default();
                                view! {
                                    <div
                                        class="flex-none w-72 bg-gray-800 rounded-lg border border-gray-700 flex flex-col max-h-full"
                                        on:dragover=on_drag_over
                                        on:drop=move |ev| on_drop(ev, column_id.clone())
                                    >
                                        <div class="px-3 py-2 border-b border-gray-700 flex items-center justify-between">
                                            <span class="text-sm font-medium text-gray-300">{&col.title}</span>
                                            <span class="text-xs text-gray-500">{items.len()}</span>
                                        </div>
                                        <div class="flex-1 overflow-y-auto p-2 space-y-2">
                                            { items.iter().map(|obj| {
                                                let obj_id = obj.id.to_string();
                                                let title = obj.metadata.title.clone().unwrap_or_default();
                                                view! {
                                                    <div
                                                        class="bg-gray-700 rounded p-2 border border-gray-600 cursor-grab hover:border-gray-500 transition-colors"
                                                        draggable="true"
                                                        on:dragstart=move |ev| on_drag_start(ev, obj_id.clone())
                                                    >
                                                        <div class="text-sm font-medium text-gray-200">{title}</div>
                                                        <div class="text-xs text-gray-500 mt-1">{obj.object_type.to_string()}</div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
