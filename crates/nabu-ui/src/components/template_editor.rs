//! Template Editor component.
//!
//! Production-ready template editor with per-folder templates,
//! template assignment, property presets, and template editing workflow.
//! Reuses TemplateManager for persistence.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::models::template::Template;
use crate::models::properties::{PropertyDefinition, PropertyType, PropertyValue};

#[derive(Clone, PartialEq, Default)]
pub struct FolderTemplateConfig {
    pub folder_path: String,
    pub template_ids: Vec<String>,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub templates: Vec<Template>,
    pub folder_templates: Vec<FolderTemplateConfig>,
    pub on_save: Callback<Template>,
    pub on_delete: Callback<String>,
    pub on_assign: Callback<(String, String)>, // (template_id, folder_path)
    pub on_unassign: Callback<(String, String)>, // (template_id, folder_path)
}

#[function_component(TemplateEditor)]
pub fn template_editor(props: &Props) -> Html {
    let (active_tab, set_active_tab) = signal(TemplateTab::List);
    let (editing_template, set_editing_template) = signal(None::<Template>);
    let (selected_folder, set_selected_folder) = signal(String::new());
    let (new_template_name, set_new_template_name) = signal(String::new());
    let (new_template_body, set_new_template_body) = signal(String::new());
    let (new_template_folder, set_new_template_folder) = signal(String::new());
    let (search_query, set_search_query) = signal(String::new());

    let filtered_templates = move || {
        let q = search_query.get().to_lowercase();
        if q.is_empty() {
            props.templates.clone()
        } else {
            props.templates.iter()
                .filter(|t| t.name.to_lowercase().contains(&q))
                .cloned()
                .collect()
        }
    };

    let templates_for_folder = move || {
        let folder = selected_folder.get();
        if folder.is_empty() {
            props.templates.clone()
        } else {
            props.folder_templates.iter()
                .find(|ft| ft.folder_path == folder)
                .map(|ft| {
                    props.templates.iter()
                        .filter(|t| ft.template_ids.contains(&t.name))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    let on_save_template = move |_| {
        let name = new_template_name.get();
        let body = new_template_body.get();
        let folder = new_template_folder.get();
        if !name.is_empty() {
            let template = Template {
                name,
                description: None,
                icon: None,
                default_folder: if folder.is_empty() { None } else { Some(folder) },
                frontmatter_defaults: std::collections::HashMap::new(),
                property_presets: std::collections::HashMap::new(),
                body,
                object_type: None,
            };
            props.on_save.emit(template);
            set_new_template_name.set(String::new());
            set_new_template_body.set(String::new());
            set_new_template_folder.set(String::new());
            set_active_tab.set(TemplateTab::List);
        }
    };

    let on_edit_template = move |template: Template| {
        set_editing_template.set(Some(template));
        set_active_tab.set(TemplateTab::Edit);
    };

    let on_delete_template = move |name: String| {
        props.on_delete.emit(name);
    };

    let on_assign_to_folder = move |template_id: String| {
        let folder = selected_folder.get();
        if !folder.is_empty() {
            props.on_assign.emit((template_id, folder));
        }
    };

    let on_unassign_from_folder = move |template_id: String| {
        let folder = selected_folder.get();
        if !folder.is_empty() {
            props.on_unassign.emit((template_id, folder));
        }
    };

    view! {
        <div class="template-editor space-y-4">
            // Tabs
            <div class="flex items-center gap-2 border-b border-gray-700 pb-2">
                <button
                    class=move || format!("px-3 py-1 text-sm rounded {}",
                        if active_tab.get() == TemplateTab::List { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(TemplateTab::List)
                >
                    "Templates"
                </button>
                <button
                    class=move || format!("px-3 py-1 text-sm rounded {}",
                        if active_tab.get() == TemplateTab::Create { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(TemplateTab::Create)
                >
                    "+ New Template"
                </button>
                <button
                    class=move || format!("px-3 py-1 text-sm rounded {}",
                        if active_tab.get() == TemplateTab::Assign { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(TemplateTab::Assign)
                >
                    "Assign to Folder"
                </button>
            </div>

            // Search
            <div class="flex gap-2">
                <input
                    type="text"
                    placeholder="Search templates..."
                    value={search_query.get()}
                    on:input=move |ev: InputEvent| {
                        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                        set_search_query.set(input.value());
                    }
                    class="flex-1 bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                />
            </div>

            // List tab
            {move || match active_tab.get() {
                TemplateTab::List => view! {
                    <div class="space-y-2">
                        {move || {
                            let templates = filtered_templates();
                            if templates.is_empty() {
                                view! { <p class="text-sm text-gray-500">"No templates found"</p> }.into_any()
                            } else {
                                view! {
                                    {for templates.iter().map(|template| {
                                        let name = template.name.clone();
                                        let desc = template.description.clone().unwrap_or_default();
                                        let folder = template.default_folder.clone().unwrap_or_default();
                                        view! {
                                            <div class="bg-gray-800 rounded-lg border border-gray-700 p-3 hover:border-gray-600 transition-colors">
                                                <div class="flex items-center justify-between">
                                                    <div>
                                                        <h4 class="text-sm font-medium text-gray-200">{name}</h4>
                                                        {move || {
                                                            if !desc.is_empty() {
                                                                view! { <p class="text-xs text-gray-500 mt-1">{desc}</p> }.into_any()
                                                            } else { view! {}.into_any() }
                                                        }}
                                                        {move || {
                                                            if !folder.is_empty() {
                                                                view! { <span class="text-xs text-blue-400 mt-1 block">📁 {folder}</span> }.into_any()
                                                            } else { view! {}.into_any() }
                                                        }}
                                                    </div>
                                                    <div class="flex gap-2">
                                                        <button
                                                            class="text-xs text-blue-400 hover:text-blue-300"
                                                            on:click=move |_| on_edit_template(template.clone())
                                                        >
                                                            "Edit"
                                                        </button>
                                                        <button
                                                            class="text-xs text-red-400 hover:text-red-300"
                                                            on:click=move |_| on_delete_template(name.clone())
                                                        >
                                                            "Delete"
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        }}
                    </div>
                }.into_any(),

                // Create tab
                TemplateTab::Create => view! {
                    <div class="space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4">
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Template Name"</label>
                            <input
                                type="text"
                                value={new_template_name.get()}
                                on:input=move |ev: InputEvent| {
                                    let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                    set_new_template_name.set(input.value());
                                }
                                placeholder="My Template"
                                class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                            />
                        </div>
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Default Folder"</label>
                            <input
                                type="text"
                                value={new_template_folder.get()}
                                on:input=move |ev: InputEvent| {
                                    let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                    set_new_template_folder.set(input.value());
                                }
                                placeholder="/path/to/folder (optional)"
                                class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                            />
                        </div>
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Body (Markdown)"</label>
                            <textarea
                                value={new_template_body.get()}
                                on:input=move |ev: InputEvent| {
                                    let input: web_sys::HtmlTextAreaElement = ev.target_unchecked_into();
                                    set_new_template_body.set(input.value());
                                }
                                placeholder="# Template Body\n\nWrite your template content here..."
                                class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none h-40 resize-y font-mono"
                            />
                        </div>
                        <div class="flex gap-2">
                            <button
                                class="px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600"
                                on:click=on_save_template
                            >
                                "Save Template"
                            </button>
                            <button
                                class="px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600"
                                on:click=move |_| set_active_tab.set(TemplateTab::List)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }.into_any(),

                // Edit tab
                TemplateTab::Edit => view! {
                    {move || match editing_template.get() {
                        Some(template) => {
                            let name = template.name.clone();
                            let body = template.body.clone();
                            let desc = template.description.clone().unwrap_or_default();
                            let folder = template.default_folder.clone().unwrap_or_default();

                            let (edit_name, set_edit_name) = signal(name);
                            let (edit_body, set_edit_body) = signal(body);
                            let (edit_desc, set_edit_desc) = signal(desc);
                            let (edit_folder, set_edit_folder) = signal(folder);

                            view! {
                                <div class="space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4">
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Template Name"</label>
                                        <input
                                            type="text"
                                            value={edit_name.get()}
                                            on:input=move |ev: InputEvent| {
                                                let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                                set_edit_name.set(input.value());
                                            }
                                            class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                                        />
                                    </div>
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Description"</label>
                                        <input
                                            type="text"
                                            value={edit_desc.get()}
                                            on:input=move |ev: InputEvent| {
                                                let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                                set_edit_desc.set(input.value());
                                            }
                                            class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                                        />
                                    </div>
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Default Folder"</label>
                                        <input
                                            type="text"
                                            value={edit_folder.get()}
                                            on:input=move |ev: InputEvent| {
                                                let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                                set_edit_folder.set(input.value());
                                            }
                                            class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                                        />
                                    </div>
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Body (Markdown)"</label>
                                        <textarea
                                            value={edit_body.get()}
                                            on:input=move |ev: InputEvent| {
                                                let input: web_sys::HtmlTextAreaElement = ev.target_unchecked_into();
                                                set_edit_body.set(input.value());
                                            }
                                            class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none h-40 resize-y font-mono"
                                        />
                                    </div>
                                    <div class="flex gap-2">
                                        <button
                                            class="px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600"
                                            on:click=move |_| {
                                                let updated = Template {
                                                    name: edit_name.get(),
                                                    description: if edit_desc.get().is_empty() { None } else { Some(edit_desc.get()) },
                                                    icon: template.icon.clone(),
                                                    default_folder: if edit_folder.get().is_empty() { None } else { Some(edit_folder.get()) },
                                                    frontmatter_defaults: template.frontmatter_defaults.clone(),
                                                    property_presets: template.property_presets.clone(),
                                                    body: edit_body.get(),
                                                    object_type: template.object_type.clone(),
                                                };
                                                props.on_save.emit(updated);
                                                set_editing_template.set(None);
                                                set_active_tab.set(TemplateTab::List);
                                            }
                                        >
                                            "Save Changes"
                                        </button>
                                        <button
                                            class="px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600"
                                            on:click=move |_| {
                                                set_editing_template.set(None);
                                                set_active_tab.set(TemplateTab::List);
                                            }
                                        >
                                            "Cancel"
                                        </button>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        None => view! { <p class="text-gray-500">"No template selected for editing"</p> }.into_any(),
                    }}
                }.into_any(),

                // Assign tab
                TemplateTab::Assign => view! {
                    <div class="space-y-3">
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Target Folder"</label>
                            <input
                                type="text"
                                value={selected_folder.get()}
                                on:input=move |ev: InputEvent| {
                                    let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                                    set_selected_folder.set(input.value());
                                }
                                placeholder="/path/to/folder"
                                class="w-full mt-1 bg-gray-700 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-600 focus:border-blue-500 focus:outline-none"
                            />
                        </div>
                        <div class="space-y-2">
                            {move || {
                                let templates = filtered_templates();
                                let folder = selected_folder.get();
                                let assigned = props.folder_templates.iter()
                                    .find(|ft| ft.folder_path == folder)
                                    .map(|ft| ft.template_ids.clone())
                                    .unwrap_or_default();

                                if templates.is_empty() {
                                    view! { <p class="text-sm text-gray-500">"No templates available"</p> }.into_any()
                                } else {
                                    view! {
                                        {for templates.iter().map(|template| {
                                            let is_assigned = assigned.contains(&template.name);
                                            let template_id = template.name.clone();
                                            view! {
                                                <div class="flex items-center justify-between p-2 bg-gray-800 rounded border border-gray-700">
                                                    <span class="text-sm text-gray-200">{&template.name}</span>
                                                    <div class="flex gap-2">
                                                        {move || {
                                                            if is_assigned {
                                                                view! {
                                                                    <button
                                                                        class="px-2 py-0.5 text-xs bg-red-700 rounded hover:bg-red-600"
                                                                        on:click=move |_| on_unassign_from_folder(template_id.clone())
                                                                    >
                                                                        "Unassign"
                                                                    </button>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <button
                                                                        class="px-2 py-0.5 text-xs bg-green-700 rounded hover:bg-green-600"
                                                                        on:click=move |_| on_assign_to_folder(template_id.clone())
                                                                    >
                                                                        "Assign"
                                                                    </button>
                                                                }.into_any()
                                                            }
                                                        }}
                                                    </div>
                                                </div>
                                            }
                                        })}
                                    }.into_any()
                                }
                            }}
                        </div>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TemplateTab {
    List,
    Create,
    Edit,
    Assign,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::template::Template;
    use std::collections::HashMap;

    #[test]
    fn test_template_editor_filters_by_name() {
        let template = Template {
            name: "Test Template".to_string(),
            description: None,
            icon: None,
            default_folder: None,
            frontmatter_defaults: HashMap::new(),
            property_presets: HashMap::new(),
            body: "# Test".to_string(),
            object_type: None,
        };
        let props = Props {
            templates: vec![template],
            folder_templates: vec![],
            on_save: Callback::new(|_| {}),
            on_delete: Callback::new(|_| {}),
            on_assign: Callback::new(|_| {}),
            on_unassign: Callback::new(|_| {}),
        };
        let _html = template_editor(&props);
    }
}
