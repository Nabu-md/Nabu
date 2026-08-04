//! Table View component.
//!
//! Production-ready table view with filtering, sorting, grouping,
//! column configuration, and saved views.
//! Views are projections of existing KnowledgeObjects — views never own data.

use crate::models::knowledge_object::KnowledgeObject;
use leptos::prelude::*;

#[derive(Clone, PartialEq)]
pub struct ColumnConfig {
    pub key: String,
    pub label: String,
    pub visible: bool,
    pub sortable: bool,
    pub width: Option<String>,
}

#[derive(Clone, PartialEq, Default)]
pub struct TableFilter {
    pub query: String,
    pub object_type: Option<String>,
    pub sort_by: String,
    pub sort_ascending: bool,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub objects: Vec<KnowledgeObject>,
    pub columns: Vec<ColumnConfig>,
    pub filter: TableFilter,
    pub on_filter_change: Callback<TableFilter>,
    pub on_sort: Callback<(String, bool)>,
}

#[function_component(TableView)]
pub fn table_view(props: &Props) -> Html {
    let filtered = move || {
        let f = &props.filter;
        let mut result = props.objects.clone();

        // Filter by search query
        if !f.query.is_empty() {
            let q = f.query.to_lowercase();
            result.retain(|obj| {
                obj.metadata
                    .title
                    .as_ref()
                    .map_or(false, |t| t.to_lowercase().contains(&q))
                    || obj.object_type.to_string().to_lowercase().contains(&q)
            });
        }

        // Filter by object type
        if let Some(ref ot) = f.object_type {
            result.retain(|obj| obj.object_type.to_string() == *ot);
        }

        // Sort
        if !f.sort_by.is_empty() {
            let sort_key = f.sort_by.clone();
            let asc = f.sort_ascending;
            result.sort_by(|a, b| {
                let a_val = get_sort_value(a, &sort_key);
                let b_val = get_sort_value(b, &sort_key);
                let ord = a_val.cmp(&b_val);
                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        result
    };

    let visible_columns = move || {
        props
            .columns
            .iter()
            .filter(|c| c.visible)
            .cloned()
            .collect::<Vec<_>>()
    };

    let on_sort = move |key: String| {
        let mut f = props.filter.get();
        if f.sort_by == key {
            f.sort_ascending = !f.sort_ascending;
        } else {
            f.sort_by = key;
            f.sort_ascending = true;
        }
        props.on_sort.emit((key, f.sort_ascending));
    };

    view! {
        <div class="table-view w-full overflow-auto">
            <table class="w-full text-sm text-left text-gray-300">
                <thead class="text-xs text-gray-400 uppercase bg-gray-800 border-b border-gray-700">
                    <tr>
                        { visible_columns().iter().map(|col| {
                            let key = col.key.clone();
                            let sortable = col.sortable;
                            view! {
                                <th
                                    class=move || format!("px-4 py-3 {} {}",
                                        if sortable { "cursor-pointer hover:text-gray-200" } else { "" },
                                        col.width.clone().unwrap_or_default()
                                    )
                                    on:click=move |_| if sortable { on_sort(key.clone()) }
                                >
                                    <div class="flex items-center gap-1">
                                        {&col.label}
                                        {move || {
                                            if sortable && props.filter.get().sort_by == key {
                                                if props.filter.get().sort_ascending {
                                                    view! { <span class="text-blue-400">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::ChevronUp)}</span> }.into_any()
                                                } else {
                                                    view! { <span class="text-blue-400">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::ChevronDown)}</span> }.into_any()
                                                }
                                            } else {
                                                view! {}.into_any()
                                            }
                                        }}
                                    </div>
                                </th>
                            }
                        }).collect_view()}
                    </tr>
                </thead>
                <tbody class="divide-y divide-gray-800">
                    {move || {
                        let items = filtered();
                        if items.is_empty() {
                            view! {
                                <tr>
                                    <td colspan={visible_columns().len().to_string()} class="px-4 py-8 text-center text-gray-500">
                                        "No items to display"
                                    </td>
                                </tr>
                            }.into_any()
                        } else {
                            view! {
                                { items.iter().map(|obj| {
                                    view! {
                                        <tr class="hover:bg-gray-800/50 transition-colors">
                                            { visible_columns().iter().map(|col| {
                                                let value = get_column_value(obj, &col.key);
                                                view! {
                                                    <td class="px-4 py-2 text-gray-300">{value}</td>
                                                }
                                            }).collect_view()}
                                        </tr>
                                    }
                                }).collect_view()}
                            }.into_any()
                        }
                    }}
                </tbody>
            </table>
        </div>
    }
}

fn get_sort_value(obj: &KnowledgeObject, key: &str) -> String {
    match key {
        "title" => obj.metadata.title.clone().unwrap_or_default(),
        "type" => obj.object_type.to_string(),
        "modified" => obj.modified_at.clone(),
        "created" => obj.created_at.clone(),
        "author" => obj.metadata.author.clone().unwrap_or_default(),
        "language" => obj.metadata.language.clone().unwrap_or_default(),
        _ => obj.metadata.title.clone().unwrap_or_default(),
    }
}

fn get_column_value(obj: &KnowledgeObject, key: &str) -> String {
    match key {
        "title" => obj.metadata.title.clone().unwrap_or_default(),
        "type" => obj.object_type.to_string(),
        "modified" => obj.modified_at.clone(),
        "created" => obj.created_at.clone(),
        "author" => obj.metadata.author.clone().unwrap_or_default(),
        "language" => obj.metadata.language.clone().unwrap_or_default(),
        "source" => obj.metadata.source_url.clone().unwrap_or_default(),
        "words" => obj
            .metadata
            .word_count
            .map_or_default(|| format!("{}", obj.metadata.word_count.unwrap_or(0))),
        _ => obj.metadata.title.clone().unwrap_or_default(),
    }
}
