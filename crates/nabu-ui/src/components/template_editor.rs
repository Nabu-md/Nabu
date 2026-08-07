//! # Template Editor — backend-wired template manager (Dioxus)
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
//! Migration notes (LePtOS → Dioxus):
//! - `signal(x)` → `use_signal(|| x)`
//! - `state.get()` / `set_s.set(x)` / `state.update(|s| …)` → `signal.read()` / `signal.set(x)` (mut) / `signal.with_mut(|s| …)` (mut)
//! - `Effect::new(move |_| { ... })` → `use_effect(move || { ... })`
//! - `Callback::new(closure)` + `.run(arg)` → `Callback::new(closure)` + `.call(arg)`
//! - `view!` / `.into_any()` / `collect_view()` → `rsx!` / `for` / `Element`
//! - `event_target_value(&ev)` → `ev.value()`
//! - `class=move || format!(...)` → `class: { format!(...) }`
//! - `prop:value=` → `value: "{...}"` + `oninput:`
//! - `on:click=` → `onclick:`
//! - `move || { … view!{} … }` reactive blocks → compute during render

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::models::template::Template;
use dioxus::prelude::*;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TemplateTab {
    List,
    Create,
    Edit,
}

/// Loads the template list from the backend.
fn load_templates(mut templates: Signal<Vec<Template>>) {
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("template_list", empty).await;
        if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<Template>>(result) {
            templates.set(list);
        }
    });
}

/// The template manager workspace (self-contained, backend-backed).
#[component]
pub fn TemplateEditor() -> Element {
    let toasts = use_toast();

    let templates = use_signal(|| Vec::<Template>::new());
    let mut active_tab = use_signal(|| TemplateTab::List);
    let mut editing_template = use_signal(|| None::<Template>);
    let mut search_query = use_signal(String::new);
    let mut new_name = use_signal(String::new);
    let mut new_body = use_signal(String::new);
    let mut new_icon = use_signal(|| String::from("📋"));
    let mut new_category = use_signal(String::new);
    let mut new_folder = use_signal(String::new);
    let mut new_desc = use_signal(String::new);
    let mut saving = use_signal(|| false);

    // Edit-form local signals (always created; populated when entering Edit mode).
    let mut edit_name = use_signal(String::new);
    let mut edit_body = use_signal(String::new);
    let mut edit_desc = use_signal(String::new);
    let mut edit_icon = use_signal(String::new);
    let mut edit_category = use_signal(String::new);
    let mut edit_folder = use_signal(String::new);

    // Initial load.
    use_effect(move || {
        load_templates(templates);
    });

    // Saves (creates or updates) the template via the backend.
    let save_template = Callback::new(move |t: Template| {
        let toasts_s = toasts;
        let mut set_saving = saving;
        let templates_s = templates;
        let mut set_new_name = new_name;
        let mut set_new_body = new_body;
        let mut set_new_icon = new_icon;
        let mut set_new_category = new_category;
        let mut set_new_folder = new_folder;
        let mut set_new_desc = new_desc;
        let mut set_editing_template = editing_template;
        let mut set_active_tab = active_tab;
        set_saving.set(true);
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "template": t })).unwrap();
            let result = crate::ipc::tauri_invoke("template_save", args).await;
            set_saving.set(false);
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(templates_s);
                set_new_name.set(String::new());
                set_new_body.set(String::new());
                set_new_icon.set(String::from("📋"));
                set_new_category.set(String::new());
                set_new_folder.set(String::new());
                set_new_desc.set(String::new());
                set_editing_template.set(None);
                set_active_tab.set(TemplateTab::List);
                toasts_s.success("Template", "Saved");
            } else {
                toasts_s.error("Template", "Could not save that template");
            }
        });
    });

    let delete_template = Callback::new(move |name: String| {
        let toasts_d = toasts;
        let templates_d = templates;
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "name": name })).unwrap();
            let result = crate::ipc::tauri_invoke("template_delete", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(templates_d);
                toasts_d.success("Template", "Deleted");
            } else {
                toasts_d.error("Template", "Could not delete that template");
            }
        });
    });

    let duplicate_template = Callback::new(move |name: String| {
        let toasts_dup = toasts;
        let templates_dup = templates;
        spawn_local(async move {
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "name": name })).unwrap();
            let result = crate::ipc::tauri_invoke("template_duplicate", args).await;
            if serde_wasm_bindgen::from_value::<Template>(result).is_ok() {
                load_templates(templates_dup);
                toasts_dup.success("Template", "Duplicated");
            } else {
                toasts_dup.error("Template", "Could not duplicate that template");
            }
        });
    });

    let toggle_favourite = Callback::new(move |(name, favourite): (String, bool)| {
        let templates_f = templates;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "name": name,
                "favourite": favourite,
            }))
            .unwrap();
            let result = crate::ipc::tauri_invoke("template_set_favourite", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                load_templates(templates_f);
            }
        });
    });

    // Compute filtered templates during render.
    let query = search_query.read().to_lowercase();
    let filtered: Vec<Template> = if query.is_empty() {
        templates.read().clone()
    } else {
        templates
            .read()
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query)
                    || t.category
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    };

    rsx! {
        div { class: "template-editor space-y-4" }

        // Header
        div {}
        h1 { class: "text-xl font-semibold text-gray-100", "\"Templates\"" }
        p { class: "text-sm text-gray-400 mt-1", "\"Reusable note skeletons with variables, dates and property presets.\"" }

        // Tabs
        div { class: "flex items-center gap-2 border-b border-gray-700 pb-2" }
        button {
            r#type: "button",
            class:  format!("px-3 py-1 text-sm rounded {}", if *active_tab.read() == TemplateTab::List { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" }),
            onclick: move |_: MouseEvent| { active_tab.set(TemplateTab::List); },
            "\"Templates\""
        }
        button {
            r#type: "button",
            class:  format!("px-3 py-1 text-sm rounded {}", if *active_tab.read() == TemplateTab::Create { "bg-blue-700 text-white" } else { "text-gray-400 hover:text-gray-200" }),
            onclick: move |_: MouseEvent| {
                new_name.set(String::new());
                new_body.set(String::new());
                new_icon.set(String::from("📋"));
                new_category.set(String::new());
                new_folder.set(String::new());
                new_desc.set(String::new());
                active_tab.set(TemplateTab::Create);
            },
            "\"+ New Template\""
        }

        // Tab content
        match *active_tab.read() {
            TemplateTab::List => rsx! {
                div { class: "space-y-2" }
                input {
                    r#type: "text",
                    placeholder: "Search templates or categories…",
                    class: "input w-full",
                    value: "{search_query.read()}",
                    oninput: move |ev: FormEvent| { search_query.set(ev.value()); },
                }
                {if filtered.is_empty() {
                    rsx! {
                        div { class: "text-sm text-gray-500 py-6 text-center", "\"No templates yet — create your first one.\"" }
                    }
                } else {
                    rsx! {
                        div { class: "divide-y divide-gray-800" }
                        for template in &filtered {
                            {
                                let name = template.name.clone();
                                let fav_name = name.clone();
                                let dup_name = name.clone();
                                let del_name = name.clone();
                                let icon = template.icon.clone().unwrap_or_else(|| "📋".to_string());
                                let desc = template.description.clone().unwrap_or_default();
                                let folder = template.default_folder.clone().unwrap_or_default();
                                let category = template.category.clone().unwrap_or_default();
                                let favourite = template.favourite;
                                let template_for_edit = template.clone();
                                let del_t = delete_template;
                                let dup_t = duplicate_template;
                                let fav_t = toggle_favourite;
                                rsx! {
                                    div {
                                        class: "bg-gray-800 rounded-lg border border-gray-700 p-3 hover:border-gray-600 transition-colors flex items-start gap-3",
                                    }
                                    span { class: "text-xl", "aria-hidden": "true", "{icon}" }
                                    div { class: "flex-1 min-w-0" }
                                    div { class: "flex items-center gap-2" }
                                    h4 { class: "text-sm font-medium text-gray-200 truncate", "{name}" }
                                    {if favourite {
                                        rsx! {
                                            span { class: "text-xs text-yellow-400", title: "Favourite", {render_icon_view(Icon::Star)} }
                                        }
                                    } else { rsx! {} }}
                                    {if !category.is_empty() {
                                        rsx! {
                                            span { class: "text-[10px] px-1.5 py-0.5 rounded-full bg-gray-700 text-gray-400", "{category}" }
                                        }
                                    } else { rsx! {} }}

                                    {if !desc.is_empty() {
                                        rsx! {
                                            p { class: "text-xs text-gray-500 mt-1", "{desc}" }
                                        }
                                    } else { rsx! {} }}
                                    {if !folder.is_empty() {
                                        rsx! {
                                            span { class: "text-xs text-blue-400 mt-1 block", {render_icon_view(Icon::Folder)} " {folder}" }
                                        }
                                    } else { rsx! {} }}

                                    div { class: "flex gap-1 flex-none" }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-sm btn-ghost",
                                        title: if favourite { "Unfavourite" } else { "Favourite" },
                                        "aria-label": if favourite { "Unfavourite" } else { "Favourite" },
                                        onclick: move |_: MouseEvent| {
                                            fav_t.call((fav_name.clone(), !favourite));
                                        },
                                        {if favourite { render_icon_view(Icon::Star) } else { render_icon_view(Icon::StarHalf) }}
                                    }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-sm btn-ghost",
                                        title: "Duplicate",
                                        "aria-label": "Duplicate",
                                        onclick: move |_: MouseEvent| {
                                            dup_t.call(dup_name.clone());
                                        },
                                        {render_icon_view(Icon::Copy)}
                                    }
                                    button {
                                        r#type: "button",
                                        class: "text-xs text-blue-400 hover:text-blue-300 px-2 py-1",
                                        onclick: move |_: MouseEvent| {
                                            let t = template_for_edit.clone();
                                            edit_name.set(t.name.clone());
                                            edit_body.set(t.body.clone());
                                            edit_desc.set(t.description.clone().unwrap_or_default());
                                            edit_icon.set(t.icon.clone().unwrap_or_default());
                                            edit_category.set(t.category.clone().unwrap_or_default());
                                            edit_folder.set(t.default_folder.clone().unwrap_or_default());
                                            editing_template.set(Some(t.clone()));
                                            active_tab.set(TemplateTab::Edit);
                                        },
                                        "\"Edit\""
                                    }
                                    button {
                                        r#type: "button",
                                        class: "text-xs text-red-400 hover:text-red-300 px-2 py-1",
                                        onclick: move |_: MouseEvent| {
                                            del_t.call(del_name.clone());
                                        },
                                        "\"Delete\""
                                    }
                                }
                            }
                        }
                    }
                }}
            },

            TemplateTab::Create => rsx! {
                div { class: "space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4" }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-3" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Template Name" }
                input {
                    r#type: "text",
                    class: "input w-full mt-1",
                    value: "{new_name.read()}",
                    oninput: move |ev: FormEvent| { new_name.set(ev.value()); },
                    placeholder: "My Template",
                }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Icon (emoji)" }
                input {
                    r#type: "text",
                    class: "input w-full mt-1",
                    value: "{new_icon.read()}",
                    oninput: move |ev: FormEvent| { new_icon.set(ev.value()); },
                    placeholder: "📋",
                }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Category" }
                input {
                    r#type: "text",
                    class: "input w-full mt-1",
                    value: "{new_category.read()}",
                    oninput: move |ev: FormEvent| { new_category.set(ev.value()); },
                    placeholder: "Work / Personal / Journal…",
                }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Default Folder" }
                input {
                    r#type: "text",
                    class: "input w-full mt-1",
                    value: "{new_folder.read()}",
                    oninput: move |ev: FormEvent| { new_folder.set(ev.value()); },
                    placeholder: "projects (optional)",
                }

                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Description" }
                input {
                    r#type: "text",
                    class: "input w-full mt-1",
                    value: "{new_desc.read()}",
                    oninput: move |ev: FormEvent| { new_desc.set(ev.value()); },
                    placeholder: "What is this template for?",
                }

                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Body (Markdown)" }
                textarea {
                    class: "input w-full mt-1 h-40 resize-y font-mono",
                    value: "{new_body.read()}",
                    oninput: move |ev: FormEvent| { new_body.set(ev.value()); },
                    placeholder: "# {{title}}\n\nTags: \n",
                }
                p { class: "text-xs text-gray-500 mt-1", "\"Supports {{title}}, {{date}}, {{tags}} and {{custom}} variables.\"" }

                div { class: "flex gap-2" }
                Button {
                    variant: ButtonVariant::Primary,
                    on_click: move |_: MouseEvent| {
                        let name = new_name.read().trim().to_string();
                        if name.is_empty() {
                            toasts.warning("Template", "Give the template a name first.");
                            return;
                        }
                        let template_to_save = Template {
                            name,
                            description: {
                                let d = new_desc.read();
                                if d.is_empty() { None } else { Some(d.clone()) }
                            },
                            icon: {
                                let i = new_icon.read();
                                if i.is_empty() { None } else { Some(i.clone()) }
                            },
                            default_folder: {
                                let f = new_folder.read();
                                if f.is_empty() { None } else { Some(f.clone()) }
                            },
                            category: {
                                let c = new_category.read();
                                if c.is_empty() { None } else { Some(c.clone()) }
                            },
                            favourite: false,
                            frontmatter_defaults: HashMap::new(),
                            property_presets: HashMap::new(),
                            body: new_body.read().clone(),
                            object_type: None,
                        };
                        save_template.call(template_to_save);
                    },
                    disabled: *saving.read(),
                    {if *saving.read() { "Saving…" } else { "Save Template" }}
                }
                Button {
                    on_click: move |_: MouseEvent| { active_tab.set(TemplateTab::List); },
                    "\"Cancel\""
                }
            },

            TemplateTab::Edit => rsx! {
                {if let Some(template) = editing_template.read().clone() {
                    rsx! {
                        div { class: "space-y-3 bg-gray-800 rounded-lg border border-gray-700 p-4" }
                        div { class: "grid grid-cols-1 md:grid-cols-2 gap-3" }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Template Name" }
                        input {
                            r#type: "text",
                            class: "input w-full mt-1",
                            value: "{edit_name.read()}",
                            oninput: move |ev: FormEvent| { edit_name.set(ev.value()); },
                        }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Icon (emoji)" }
                        input {
                            r#type: "text",
                            class: "input w-full mt-1",
                            value: "{edit_icon.read()}",
                            oninput: move |ev: FormEvent| { edit_icon.set(ev.value()); },
                        }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Category" }
                        input {
                            r#type: "text",
                            class: "input w-full mt-1",
                            value: "{edit_category.read()}",
                            oninput: move |ev: FormEvent| { edit_category.set(ev.value()); },
                        }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Default Folder" }
                        input {
                            r#type: "text",
                            class: "input w-full mt-1",
                            value: "{edit_folder.read()}",
                            oninput: move |ev: FormEvent| { edit_folder.set(ev.value()); },
                        }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Description" }
                        input {
                            r#type: "text",
                            class: "input w-full mt-1",
                            value: "{edit_desc.read()}",
                            oninput: move |ev: FormEvent| { edit_desc.set(ev.value()); },
                        }
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Body (Markdown)" }
                        textarea {
                            class: "input w-full mt-1 h-40 resize-y font-mono",
                            value: "{edit_body.read()}",
                            oninput: move |ev: FormEvent| { edit_body.set(ev.value()); },
                        }
                        div { class: "flex gap-2" }
                        Button {
                            variant: ButtonVariant::Primary,
                            on_click: move |_: MouseEvent| {
                                let updated = Template {
                                    name: edit_name.read().clone(),
                                    description: {
                                        let d = edit_desc.read();
                                        if d.is_empty() { None } else { Some(d.clone()) }
                                    },
                                    icon: {
                                        let i = edit_icon.read();
                                        if i.is_empty() { None } else { Some(i.clone()) }
                                    },
                                    default_folder: {
                                        let f = edit_folder.read();
                                        if f.is_empty() { None } else { Some(f.clone()) }
                                    },
                                    category: {
                                        let c = edit_category.read();
                                        if c.is_empty() { None } else { Some(c.clone()) }
                                    },
                                    favourite: template.favourite,
                                    frontmatter_defaults: template.frontmatter_defaults.clone(),
                                    property_presets: template.property_presets.clone(),
                                    body: edit_body.read().clone(),
                                    object_type: template.object_type.clone(),
                                };
                                save_template.call(updated);
                            },
                            {"Save Changes"}
                        }
                        Button {
                            on_click: move |_: MouseEvent| {
                                editing_template.set(None);
                                active_tab.set(TemplateTab::List);
                            },
                            "\"Cancel\""
                        }
                    }
                } else {
                    rsx! {
                        p { class: "text-gray-500", "\"No template selected for editing\"" }
                    }
                }}
            },
        }
    }
}
