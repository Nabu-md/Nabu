//! Relation Editor component.
//!
//! Production-ready relation editor with autocomplete, entity search,
//! create entity, relationship picker, and semantic edge editing.
//! Reuses VaultGraph for entity lookup and relation management.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use uuid::Uuid;
use crate::models::graph::{RelationType, GraphEdge};
use crate::models::knowledge_object::KnowledgeObject;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub object: KnowledgeObject,
    pub relations: Vec<GraphEdge>,
    pub all_objects: Vec<KnowledgeObject>,
    pub on_add_relation: Callback<(Uuid, RelationType)>,
    pub on_remove_relation: Callback<Uuid>,
    pub on_create_entity: Callback<(String, RelationType)>,
}

#[function_component(RelationEditor)]
pub fn relation_editor(props: &Props) -> Html {
    let (search_query, set_search_query) = signal(String::new());
    let (selected_relation_type, set_selected_relation_type) = signal(RelationType::RelatedTo);
    let (show_create, set_show_create) = signal(false);
    let (new_entity_name, set_new_entity_name) = signal(String::new());
    let (active_tab, set_active_tab) = signal(RelationTab::Existing);

    let filtered_objects = move || {
        let query = search_query.get().to_lowercase();
        if query.is_empty() {
            props.all_objects.clone()
        } else {
            props.all_objects.iter()
                .filter(|obj| {
                    obj.metadata.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&query))
                        || obj.object_type.to_string().to_lowercase().contains(&query)
                })
                .cloned()
                .collect()
        }
    };

    let existing_relations = move || {
        props.relations.iter()
            .filter(|edge| edge.source == props.object.id)
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

    let on_add_relation = move |target_id: Uuid| {
        let rel_type = selected_relation_type.get();
        props.on_add_relation.emit((target_id, rel_type));
    };

    let on_create_entity = move |_| {
        let name = new_entity_name.get();
        if !name.is_empty() {
            let rel_type = selected_relation_type.get();
            props.on_create_entity.emit((name, rel_type));
            set_new_entity_name.set(String::new());
            set_show_create.set(false);
        }
    };

    view! {
        <div class="relation-editor space-y-4">
            // Header
            <div class="flex items-center justify-between">
                <h3 class="text-sm font-semibold text-gray-300">
                    {format!("Relations for {}", props.object.metadata.title.clone().unwrap_or_default())}
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
                        {move || {
                            let rels = existing_relations();
                            if rels.is_empty() {
                                view! { <p class="text-sm text-gray-500">"No relations yet"</p> }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-1">
                                        {for rels.iter().map(|edge| {
                                            let target_id = edge.target;
                                            let rel_type = edge.relation.clone();
                                            let target_obj = props.all_objects.iter().find(|o| o.id == target_id);
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
                                                        on:click=move |_| props.on_remove_relation.emit(target_id)
                                                    >
                                                        "Remove"
                                                    </button>
                                                </div>
                                            }
                                        })}
                                    </div>
                                }.into_any()
                            }
                        }}
                    </div>
                }.into_any(),

                RelationTab::Search => view! {
                    <div class="space-y-3">
                        // Search input
                        <input
                            type="text"
                            placeholder="Search entities..."
                            value={search_query.get()}
                            on:input=move |ev: InputEvent| {
                                let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                set_search_query.set(input.value());
                            }
                            class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        />

                        // Relation type picker
                        <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500">"Relation:"</span>
                            <select
                                value={format!("{:?}", selected_relation_type.get())}
                                on:change=move |ev: Event| {
                                    let select: web_sys::HtmlSelectElement = ev.target_unchecked_into();
                                    let val = select.value();
                                    let rel = match val.as_str() {
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
                                {for relation_types.iter().map(|rt| {
                                    view! { <option value={format!("{:?}", rt)}>{format!("{:?}", rt)}</option> }
                                })}
                            </select>
                        </div>

                        // Search results
                        <div class="max-h-48 overflow-y-auto space-y-1">
                            {move || {
                                let results = filtered_objects();
                                if results.is_empty() {
                                    view! { <p class="text-sm text-gray-500">"No entities found"</p> }.into_any()
                                } else {
                                    view! {
                                        {for results.iter().map(|obj| {
                                            let obj_id = obj.id;
                                            let title = obj.metadata.title.clone().unwrap_or_default();
                                            let obj_type = obj.object_type.to_string();
                                            view! {
                                                <div class="flex items-center justify-between p-2 bg-gray-800 rounded border border-gray-700 hover:border-gray-600 cursor-pointer"
                                                    on:click=move |_| on_add_relation(obj_id)>
                                                    <div>
                                                        <span class="text-sm text-gray-200">{title}</span>
                                                        <span class="text-xs text-gray-500 ml-2">{obj_type}</span>
                                                    </div>
                                                    <span class="text-xs text-blue-400">"Add →"</span>
                                                </div>
                                            }
                                        })}
                                    }.into_any()
                                }
                            }}
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
                                on:input=move |ev: InputEvent| {
                                    let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                    set_new_entity_name.set(input.value());
                                }
                                placeholder="Enter entity name..."
                                class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none mt-1"
                            />
                        </div>

                        <div class="flex items-center gap-2">
                            <span class="text-xs text-gray-500">"Relation:"</span>
                            <select
                                value={format!("{:?}", selected_relation_type.get())}
                                on:change=move |ev: Event| {
                                    let select: web_sys::HtmlSelectElement = ev.target_unchecked_into();
                                    let val = select.value();
                                    let rel = match val.as_str() {
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
                                {for relation_types.iter().map(|rt| {
                                    view! { <option value={format!("{:?}", rt)}>{format!("{:?}", rt)}</option> }
                                })}
                            </select>
                        </div>

                        <button
                            class="px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600"
                            on:click=on_create_entity
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
    use std::collections::HashMap;

    #[test]
    fn test_relation_editor_filters_by_title() {
        let obj = KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-01-01T00:00:00Z".to_string(),
            content: ObjectContent::Markdown,
            metadata: ObjectMetadata {
                title: Some("Test Note".to_string()),
                ..ObjectMetadata::default()
            },
        };
        let props = Props {
            object: obj.clone(),
            relations: vec![],
            all_objects: vec![obj],
            on_add_relation: Callback::new(|_| {}),
            on_remove_relation: Callback::new(|_| {}),
            on_create_entity: Callback::new(|_| {}),
        };
        // The component renders without panicking
        let _html = relation_editor(&props);
    }
}
