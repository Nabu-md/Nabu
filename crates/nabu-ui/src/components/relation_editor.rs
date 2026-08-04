//! Relation Editor component.
//!
//! Production-ready relation editor with autocomplete, entity search,
//! create entity, relationship picker, and semantic edge editing.
//! Reuses VaultGraph for entity lookup and relation management.
//! Views are projections of existing KnowledgeObjects — views never own data.

use crate::models::graph::{GraphEdge, RelationType};
use crate::models::knowledge_object::KnowledgeObject;
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn RelationEditor(
    object: KnowledgeObject,
    relations: Vec<GraphEdge>,
    all_objects: Vec<KnowledgeObject>,
    on_add_relation: Callback<(Uuid, RelationType)>,
    on_remove_relation: Callback<Uuid>,
    on_create_entity: Callback<(String, RelationType)>,
) -> impl IntoView {
    let (search_query, set_search_query) = signal(String::new());
    let (selected_relation_type, set_selected_relation_type) = signal(RelationType::RelatedTo);
    let (_show_create, set_show_create) = signal(false);
    let (new_entity_name, set_new_entity_name) = signal(String::new());
    let (active_tab, set_active_tab) = signal(RelationTab::Existing);

    let all_objects_for_filter = all_objects.clone();
    let filtered_objects = move || {
        let query = search_query.get().to_lowercase();
        if query.is_empty() {
            all_objects_for_filter.clone()
        } else {
            all_objects_for_filter
                .iter()
                .filter(|obj| {
                    obj.metadata
                        .title
                        .as_ref()
                        .map_or(false, |t| t.to_lowercase().contains(&query))
                        || format!("{:?}", obj.object_type)
                            .to_lowercase()
                            .contains(&query)
                })
                .cloned()
                .collect()
        }
    };

    let object_id = object.id;
    let existing_relations = move || {
        relations
            .iter()
            .filter(|edge| edge.source == object_id)
            .cloned()
            .collect::<Vec<_>>()
    };

    let relation_types = vec![
        RelationType::BelongsTo,
        RelationType::WorksOn,
        RelationType::RelatedTo,
        RelationType::CreatedBy,
        RelationType::References,
        RelationType::MemberOf,
        RelationType::DependsOn,
    ];

    let add_relation = move |target_id: Uuid| {
        let rel_type = selected_relation_type.get();
        on_add_relation.run((target_id, rel_type));
    };

    let create_entity = move |_| {
        let name = new_entity_name.get();
        if !name.is_empty() {
            let rel_type = selected_relation_type.get();
            on_create_entity.run((name, rel_type));
            set_new_entity_name.set(String::new());
            set_show_create.set(false);
        }
    };

    view! {
        <div class="relation-editor space-y-4">
            // Header
            <div class="flex items-center justify-between">
                <h3 class="text-sm font-semibold text-gray-300">
                    {format!("Relations for {}", object.metadata.title.clone().unwrap_or_default())}
                </h3>
                <div class="flex gap-2">
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if active_tab.get() == RelationTab::Existing { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400 hover:text-gray-200" })
                        on:click=move |_| set_active_tab.set(RelationTab::Existing)
                    >
                        "Existing"
                    </button>
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if active_tab.get() == RelationTab::Search { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400 hover:text-gray-200" })
                        on:click=move |_| set_active_tab.set(RelationTab::Search)
                    >
                        "Search"
                    </button>
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if active_tab.get() == RelationTab::Create { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400 hover:text-gray-200" })
                        on:click=move |_| set_active_tab.set(RelationTab::Create)
                    >
                        "+ New Entity"
                    </button>
                </div>
            </div>

            // Existing relations tab
            {move || match active_tab.get() {
                RelationTab::Existing => view! {
                    <div class="space-y-2">
                        {let rels = existing_relations();
                            if rels.is_empty() {
                                view! { <p class="text-sm text-gray-500">"No relations yet"</p> }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-1">
                                        { rels.iter().map(|edge| {
                                            let target_id = edge.target;
                                            let rel_type = edge.relation.clone();
                                            let target_obj = all_objects.iter().find(|o| o.id == target_id);
                                            view! {
                                                <div class="flex items-center justify-between p-2 bg-gray-800 rounded border border-gray-700">
                                                    <div class="flex items-center gap-2">
                                                        <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">
                                                            {format!("{:?}", rel_type)}
                                                        </span>
                                                        <span class="text-sm text-gray-200">
                                                            {target_obj.map_or_else(|| format!("Entity {}", target_id), |o| o.metadata.title.clone().unwrap_or_default())}
                                                        </span>
                                                    </div>
                                                    <button
                                                        class="text-xs text-red-400 hover:text-red-300"
                                                        on:click=move |_| on_remove_relation.run(target_id)
                                                    >
                                                        "Remove"
                                                    </button>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }
                    </div>
                }.into_any(),

                RelationTab::Search => view! {
                    <div class="space-y-3">
                        // Search input
                        <input
                            type="text"
                            placeholder="Search entities..."
                            value={search_query.get()}
                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        />

                        // Relation type picker
                        <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500">"Relation:"</span>
                            <select
                                on:change=move |ev| {
                                    let rel = match event_target_value(&ev).as_str() {
                                        "BelongsTo" => RelationType::BelongsTo,
                                        "WorksOn" => RelationType::WorksOn,
                                        "RelatedTo" => RelationType::RelatedTo,
                                        "CreatedBy" => RelationType::CreatedBy,
                                        "References" => RelationType::References,
                                        "MemberOf" => RelationType::MemberOf,
                                        "DependsOn" => RelationType::DependsOn,
                                        _ => RelationType::RelatedTo,
                                    };
                                    set_selected_relation_type.set(rel);
                                }
                                class="bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700"
                            >
                                { relation_types.iter().map(|rt| {
                                    view! { <option value={format!("{:?}", rt)}>{format!("{:?}", rt)}</option> }
                                }).collect_view()}
                            </select>
                        </div>

                        // Search results
                        <div class="max-h-48 overflow-y-auto space-y-1">
                            {let results = filtered_objects();
                                if results.is_empty() {
                                    view! { <p class="text-sm text-gray-500">"No entities found"</p> }.into_any()
                                } else {
                                    view! {
                                        { results.iter().map(|obj| {
                                            let obj_id = obj.id;
                                            let title = obj.metadata.title.clone().unwrap_or_default();
                                            let obj_type = format!("{:?}", obj.object_type);
                                            view! {
                                                <div class="flex items-center justify-between p-2 bg-gray-800 rounded border border-gray-700 hover:border-gray-600 cursor-pointer"
                                                    on:click=move |_| add_relation(obj_id)>
                                                    <div>
                                                        <span class="text-sm text-gray-200">{title}</span>
                                                        <span class="text-xs text-gray-500 ml-2">{obj_type}</span>
                                                    </div>
                                                    <span class="text-xs text-blue-400">{crate::components::ui::icons::render_icon_view(crate::components::ui::icons::Icon::Plus)} Add</span>
                                                </div>
                                            }
                                        }).collect_view()}
                                    }.into_any()
                                }
                            }
                        </div>
                    </div>
                }.into_any(),

                RelationTab::Create => view! {
                    <div class="space-y-3">
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"New Entity Name"</label>
                            <input
                                type="text"
                                value={new_entity_name.get()}
                                on:input=move |ev| set_new_entity_name.set(event_target_value(&ev))
                                placeholder="Enter entity name..."
                                class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none mt-1"
                            />
                        </div>

                        <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500">"Relation:"</span>
                            <select
                                on:change=move |ev| {
                                    let rel = match event_target_value(&ev).as_str() {
                                        "BelongsTo" => RelationType::BelongsTo,
                                        "WorksOn" => RelationType::WorksOn,
                                        "RelatedTo" => RelationType::RelatedTo,
                                        "CreatedBy" => RelationType::CreatedBy,
                                        "References" => RelationType::References,
                                        "MemberOf" => RelationType::MemberOf,
                                        "DependsOn" => RelationType::DependsOn,
                                        _ => RelationType::RelatedTo,
                                    };
                                    set_selected_relation_type.set(rel);
                                }
                                class="bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700"
                            >
                                { relation_types.iter().map(|rt| {
                                    view! { <option value={format!("{:?}", rt)}>{format!("{:?}", rt)}</option> }
                                }).collect_view()}
                            </select>
                        </div>

                        <button
                            class="px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600"
                            on:click=create_entity
                        >
                            "Create Entity"
                        </button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RelationTab {
    Existing,
    Search,
    Create,
}
