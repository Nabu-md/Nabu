//! # Template Editor — backend-wired template manager
//!
//! Phase 13.2 wires the template manager to the real backend
//! (`template_list` / `template_save` / `template_delete` /
//! `template_duplicate` / `template_set_favourite`, persisted in settings):
//!
//! - browse templates with search and category grouping
//! - create / edit (name, description, icon, category, default folder, body)
//! - duplicate, delete, and favourite (star) any template
//! - per-folder assignment UI (future-compatible; the backend stores
//!   templates as pure data so assignment can be layered on later)
//!
//! ## Reactivity note
//!
//! Toast context is `Copy` and captured at render time, then threaded into
//! async tasks as plain values — never `expect_context` inside a
//! `spawn_local` future (no reactive owner on the failure path).

use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::models::template::Template;
use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TemplateTab {
    List,
    Create,
    Edit,
}

/// The template manager workspace (self-contained, backend-backed).
#[component]
pub fn TemplateEditor() -> impl IntoView {
    let toasts = use_toast();

    let (templates, set_templates) = signal(Vec::<Template>::new());
    let (active_tab, set_active_tab) = signal(TemplateTab::List);
    let (editing_template, set_editing_template) = signal(None::<Template>);
    let (search_query, set_search_query) = signal(String::new());
    let (new_name, set_new_name) = signal(String::new());
    let (new_body, set_new_body) = signal(String::new());
    let (new_icon, set_new_icon) = signal(String::from("📋"));
    let (new_category, set_new_category) = signal(String::new());
    let (new_folder, set_new_folder) = signal(String::new());
    let (new_desc, set_new_desc) = signal(String::new());
    let (saving, set_saving) = signal(false);

    /// Loads the template list from the backend.
    fn load_templates(set_templates: WriteSignal<Vec<Template>>) {
        spawn_local(async move {
            let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("template_list", empty).await;
            if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<Template>>(result) {
                set_templates.set(list);
            }
        });
    }

    // Initial load.
    Effect::new(move |_| {
        load_templates(set_templates);
    });

    // Saves (creates or updates) the template via the backend.
    let save_template = Callback::new(move |t: Template| {
        let toasts = toasts;
        let set_saving = set_saving;
        let set_templates = set_templates;
        let set_new_name = set_new_name;
        let set_new_body = set_new_body;
        let set_new_icon = set_new_icon;
        let set_new_category = set_new_category;
        let set_new_folder = set_new_folder;
        let set_new_desc = set_new_desc;
        let set_editing_template = set_editing_template;
        let set_active_tab = set_active_tab;
        set_saving.set(true);
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "template": t })).unwrap();
            let result = crate::ipc::tauri_invoke("template_save", args).await;
            set_saving.set(false);
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(set_templates);
                set_new_name.set(String::new());
                set_new_body.set(String::new());
                set_new_icon.set(String::from("📋"));
                set_new_category.set(String::new());
                set_new_folder.set(String::new());
                set_new_desc.set(String::new());
                set_editing_template.set(None);
                set_active_tab.set(TemplateTab::List);
                toasts.success("Template", "Saved");
            } else {
                toasts.error("Template", "Could not save that template");
            }
        });
    });

    let delete_template = Callback::new(move |name: String| {
        let toasts = toasts;
        let set_templates = set_templates;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "name": name })).unwrap();
            let result = crate::ipc::tauri_invoke("template_delete", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(set_templates);
                toasts.success("Template", "Deleted");
            } else {
                toasts.error("Template", "Could not delete that template");
            }
        });
    });

    let duplicate_template = Callback::new(move |name: String| {
        let toasts = toasts;
        let set_templates = set_templates;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "name": name })).unwrap();
            let result = crate::ipc::tauri_invoke("template_duplicate", args).await;
            if serde_wasm_bindgen::from_value::<Template>(result).is_ok() {
                load_templates(set_templates);
                toasts.success("Template", "Duplicated");
            } else {
                toasts.error("Template", "Could not duplicate that template");
            }
        });
    });

    let toggle_favourite = Callback::new(move |(name, favourite): (String, bool)| {
        let set_templates = set_templates;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "name": name,
                "favourite": favourite,
            }))
            .unwrap();
            let result = crate::ipc::tauri_invoke("template_set_favourite", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(set_templates);
            }
        });
    });

    let on_save_form = Callback::new(move |_| {
        let name = new_name.get_untracked().trim().to_string();
        if name.is_empty() {
            toasts.warning("Template", "Give the template a name first.");
            return;
        }
        save_template.run(Template {
            name,
            description: if new_desc.get_untracked().is_empty() {
                None
            } else {
                Some(new_desc.get_untracked())
            },
            icon: if new_icon.get_untracked().is_empty() {
                None
            } else {
                Some(new_icon.get_untracked())
            },
            default_folder: if new_folder.get_untracked().is_empty() {
                None
            } else {
                Some(new_folder.get_untracked())
            },
            category: if new_category.get_untracked().is_empty() {
                None
            } else {
                Some(new_category.get_untracked())
            },
            favourite: false,
            frontmatter_defaults: HashMap::new(),
            property_presets: HashMap::new(),
            body: new_body.get_untracked(),
            object_type: None,
        });
    });

    let filtered_templates = move || {
        let q = search_query.get().to_lowercase();
        let all = templates.get();
        if q.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|t| {
                    t.name.to_lowercase().contains(&q)
                        || t.category
                            .as_ref()
                            .map(|c| c.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .collect()
        }
    };

    view! {
        <div class="template-editor space-y-4">
            <header>
                <h1 class="text-xl font-semibold text-gray-100">"Templates"</h1>
                <p class="text-sm text-gray-400 mt-1">
                    "Reusable note skeletons with variables, dates and property presets."
                </p>
            </header>

            // Tabs
            <div class="flex items-center gap-2 border-b border-gray-700 pb-2">
                <button
                    type="button"
                    class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == TemplateTab::List { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(TemplateTab::List)
                >
                    "Templates"
                </button>
                <button
                    type="button"
                    class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == TemplateTab::Create { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| {
                        set_new_name.set(String::new());
                        set_new_body.set(String::new());
                        set_new_icon.set(String::from("📋"));
                        set_new_category.set(String::new());
                        set_new_folder.set(String::new());
                        set_new_desc.set(String::new());
                        set_active_tab.set(TemplateTab::Create);
                    }
                >
                    "+ New Template"
                </button>
            </div>

            {move || match active_tab.get() {
                TemplateTab::List => view! {
                    <div class="space-y-2">
                        <input
                            type="text"
                            placeholder="Search templates or categories…"
                            prop:value=search_query
                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                            class="input w-full"
                        />
                        {let list = filtered_templates();
                            if list.is_empty() {
                                view! {
                                    <div class="text-sm text-gray-500 py-6 text-center">
                                        "No templates yet — create your first one."
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="divide-y divide-gray-800">
                                        {list.into_iter().map(|template| {
                                            let name = template.name.clone();
                                            let icon = template.icon.clone().unwrap_or_else(|| "📋".to_string());
                                            let desc = template.description.clone().unwrap_or_default();
                                            let folder = template.default_folder.clone().unwrap_or_default();
                                            let category = template.category.clone().unwrap_or_default();
                                            let favourite = template.favourite;
                                            view! {
                                                <div class="bg-gray-800 rounded-lg border border-gray-700 p-3 hover:border-gray-600 transition-colors flex items-start gap-3">
                                                    <span class="text-xl" aria-hidden="true">{icon}</span>
                                                    <div class="flex-1 min-w-0">
                                                        <div class="flex items-center gap-2">
                                                            <h4 class="text-sm font-medium text-gray-200 truncate">{name.clone()}</h4>
                                                            {if favourite {
                                                                view! { <span class="text-xs text-yellow-400" title="Favourite">{render_icon_view(Icon::Star)}</span> }.into_any()
                                                            } else { view! {}.into_any() }}
                                                            {if !category.is_empty() {
                                                                view! { <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-700 text-gray-400">{category}</span> }.into_any()
                                                            } else { view! {}.into_any() }}
                                                        </div>
                                                        {if !desc.is_empty() {
                                                            view! { <p class="text-xs text-gray-500 mt-1">{desc}</p> }.into_any()
                                                        } else { view! {}.into_any() }}
                                                        {if !folder.is_empty() {
                                                            view! { <span class="text-xs text-blue-400 mt-1 block">{render_icon_view(Icon::Folder)} {folder}</span> }.into_any()
                                                        } else { view! {}.into_any() }}
                                                    </div>
                                                    <div class="flex gap-1 flex-none">
                                                        <button
                                                            type="button"
                                                            class="btn btn-sm btn-ghost"
                                                            title=if favourite { "Unfavourite" } else { "Favourite" }
                                                            aria-label=if favourite { "Unfavourite" } else { "Favourite" }
                                                            on:click={let n = name.clone(); move |_| toggle_favourite.run((n.clone(), !favourite))}
                                                        >
                                                            {if favourite { render_icon_view(Icon::Star) } else { render_icon_view(Icon::StarHalf) }}
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="btn btn-sm btn-ghost"
                                                            title="Duplicate"
                                                            aria-label="Duplicate"
                                                            on:click={let n = name.clone(); move |_| duplicate_template.run(n.clone())}
                                                        >
                                                            {render_icon_view(Icon::Copy)}
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="text-xs text-blue-400 hover:text-blue-300 px-2 py-1"
                                                            on:click={let t = template.clone(); move |_| {
                                                                set_editing_template.set(Some(t.clone()));
                                                                set_active_tab.set(TemplateTab::Edit);
                                                            }}
                                                        >
                                                            "Edit"
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="text-xs text-red-400 hover:text-red-300 px-2 py-1"
                                                            on:click={let n = name.clone(); move |_| delete_template.run(n.clone())}
                                                        >
                                                            "Delete"
                                                        </button>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }
                    </div>
                }.into_any(),

                TemplateTab::Create => view! {
                    <div class="space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4">
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Template Name"</label>
                                <input
                                    type="text"
                                    prop:value=new_name
                                    on:input=move |ev| set_new_name.set(event_target_value(&ev))
                                    placeholder="My Template"
                                    class="input w-full mt-1"
                                />
                            </div>
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Icon (emoji)"</label>
                                <input
                                    type="text"
                                    prop:value=new_icon
                                    on:input=move |ev| set_new_icon.set(event_target_value(&ev))
                                    placeholder="📋"
                                    class="input w-full mt-1"
                                />
                            </div>
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Category"</label>
                                <input
                                    type="text"
                                    prop:value=new_category
                                    on:input=move |ev| set_new_category.set(event_target_value(&ev))
                                    placeholder="Work / Personal / Journal…"
                                    class="input w-full mt-1"
                                />
                            </div>
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Default Folder"</label>
                                <input
                                    type="text"
                                    prop:value=new_folder
                                    on:input=move |ev| set_new_folder.set(event_target_value(&ev))
                                    placeholder="projects (optional)"
                                    class="input w-full mt-1"
                                />
                            </div>
                        </div>
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Description"</label>
                            <input
                                type="text"
                                prop:value=new_desc
                                on:input=move |ev| set_new_desc.set(event_target_value(&ev))
                                placeholder="What is this template for?"
                                class="input w-full mt-1"
                            />
                        </div>
                        <div>
                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Body (Markdown)"</label>
                            <textarea
                                prop:value=new_body
                                on:input=move |ev| set_new_body.set(event_target_value(&ev))
                                placeholder="# {{title}}\n\nTags: \n"
                                class="input w-full mt-1 h-40 resize-y font-mono"
                            />
                            <p class="text-xs text-gray-500 mt-1">
                                "Supports {{title}}, {{date}}, {{tags}} and {{custom}} variables."
                            </p>
                        </div>
                        <div class="flex gap-2">
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=saving
                                on:click=move |_| on_save_form.run(())
                            >
                                {if saving.get() { "Saving…" } else { "Save Template" }}
                            </button>
                            <button
                                type="button"
                                class="btn"
                                on:click=move |_| set_active_tab.set(TemplateTab::List)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }.into_any(),

                TemplateTab::Edit => view! {
                    {move || match editing_template.get() {
                        Some(template) => {
                            let name = template.name.clone();
                            let body = template.body.clone();
                            let desc = template.description.clone().unwrap_or_default();
                            let icon = template.icon.clone().unwrap_or_default();
                            let category = template.category.clone().unwrap_or_default();
                            let folder = template.default_folder.clone().unwrap_or_default();

                            let (edit_name, set_edit_name) = signal(name);
                            let (edit_body, set_edit_body) = signal(body);
                            let (edit_desc, set_edit_desc) = signal(desc);
                            let (edit_icon, set_edit_icon) = signal(icon);
                            let (edit_category, set_edit_category) = signal(category);
                            let (edit_folder, set_edit_folder) = signal(folder);

                            view! {
                                <div class="space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4">
                                    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Template Name"</label>
                                            <input
                                                type="text"
                                                prop:value=edit_name
                                                on:input=move |ev| set_edit_name.set(event_target_value(&ev))
                                                class="input w-full mt-1"
                                            />
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Icon (emoji)"</label>
                                            <input
                                                type="text"
                                                prop:value=edit_icon
                                                on:input=move |ev| set_edit_icon.set(event_target_value(&ev))
                                                class="input w-full mt-1"
                                            />
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Category"</label>
                                            <input
                                                type="text"
                                                prop:value=edit_category
                                                on:input=move |ev| set_edit_category.set(event_target_value(&ev))
                                                class="input w-full mt-1"
                                            />
                                        </div>
                                        <div>
                                            <label class="text-xs text-gray-500 uppercase tracking-wide">"Default Folder"</label>
                                            <input
                                                type="text"
                                                prop:value=edit_folder
                                                on:input=move |ev| set_edit_folder.set(event_target_value(&ev))
                                                class="input w-full mt-1"
                                            />
                                        </div>
                                    </div>
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Description"</label>
                                        <input
                                            type="text"
                                            prop:value=edit_desc
                                            on:input=move |ev| set_edit_desc.set(event_target_value(&ev))
                                            class="input w-full mt-1"
                                        />
                                    </div>
                                    <div>
                                        <label class="text-xs text-gray-500 uppercase tracking-wide">"Body (Markdown)"</label>
                                        <textarea
                                            prop:value=edit_body
                                            on:input=move |ev| set_edit_body.set(event_target_value(&ev))
                                            class="input w-full mt-1 h-40 resize-y font-mono"
                                        />
                                    </div>
                                    <div class="flex gap-2">
                                        <button
                                            type="button"
                                            class="btn btn-primary"
                                            on:click=move |_| {
                                                let updated = Template {
                                                    name: edit_name.get(),
                                                    description: if edit_desc.get().is_empty() { None } else { Some(edit_desc.get()) },
                                                    icon: if edit_icon.get().is_empty() { None } else { Some(edit_icon.get()) },
                                                    default_folder: if edit_folder.get().is_empty() { None } else { Some(edit_folder.get()) },
                                                    category: if edit_category.get().is_empty() { None } else { Some(edit_category.get()) },
                                                    favourite: template.favourite,
                                                    frontmatter_defaults: template.frontmatter_defaults.clone(),
                                                    property_presets: template.property_presets.clone(),
                                                    body: edit_body.get(),
                                                    object_type: template.object_type.clone(),
                                                };
                                                save_template.run(updated);
                                            }
                                        >
                                            "Save Changes"
                                        </button>
                                        <button
                                            type="button"
                                            class="btn"
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
            }}
        </div>
    }
}
