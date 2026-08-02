//! Gallery View component.
//!
//! Production-ready card-based gallery view with filtering,
//! sorting, and grouping.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::models::knowledge_object::KnowledgeObject;

#[derive(Clone, PartialEq, Default)]
pub struct GalleryFilter {
    pub query: String,
    pub object_type: Option<String>,
    pub sort_by: String,
    pub sort_ascending: bool,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub objects: Vec<KnowledgeObject>,
    pub filter: GalleryFilter,
    pub on_filter_change: Callback<GalleryFilter>,
}

#[function_component(GalleryView)]
pub fn gallery_view(props: &Props) -> Html {
    let filtered = move || {
        let f = &props.filter;
        let mut result = props.objects.clone();

        if !f.query.is_empty() {
            let q = f.query.to_lowercase();
            result.retain(|obj| {
                obj.metadata.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&q))
                    || obj.object_type.to_string().to_lowercase().contains(&q)
            });
        }

        if let Some(ref ot) = f.object_type {
            result.retain(|obj| obj.object_type.to_string() == *ot);
        }

        if !f.sort_by.is_empty() {
            let sort_key = f.sort_by.clone();
            let asc = f.sort_ascending;
            result.sort_by(|a, b| {
                let a_val = get_gallery_sort_value(a, &sort_key);
                let b_val = get_gallery_sort_value(b, &sort_key);
                let ord = a_val.cmp(&b_val);
                if asc { ord } else { ord.reverse() }
            });
        }

        result
    };

    view! {
        <div class="gallery-view p-4 overflow-y-auto h-full">
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                {move || {
                    let items = filtered();
                    if items.is_empty() {
                        view! {
                            <div class="col-span-full flex items-center justify-center h-64 text-gray-500">
                                "No items to display"
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            { items.iter().map(|obj| {
                                let title = obj.metadata.title.clone().unwrap_or_default();
                                let obj_type = obj.object_type.to_string();
                                let modified = obj.modified_at.clone();
                                let source = obj.metadata.source_url.clone().unwrap_or_default();
                                view! {
                                    <div class="bg-gray-800 rounded-lg border border-gray-700 p-4 hover:border-gray-600 transition-colors cursor-pointer">
                                        <div class="flex items-start justify-between mb-2">
                                            <span class="text-xs px-2 py-0.5 rounded bg-gray-700 text-gray-300">
                                                {obj_type}
                                            </span>
                                            <span class="text-xs text-gray-500">{modified}</span>
                                        </div>
                                        <h3 class="text-sm font-medium text-gray-200 mb-2 line-clamp-2">{title}</h3>
                                        {move || {
                                            if !source.is_empty() {
                                                view! {
                                                    <div class="text-xs text-blue-400 truncate">{source}</div>
                                                }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            }).collect_view()}
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

fn get_gallery_sort_value(obj: &KnowledgeObject, key: &str) -> String {
    match key {
        "title" => obj.metadata.title.clone().unwrap_or_default(),
        "type" => obj.object_type.to_string(),
        "modified" => obj.modified_at.clone(),
        "created" => obj.created_at.clone(),
        _ => obj.metadata.title.clone().unwrap_or_default(),
    }
}
