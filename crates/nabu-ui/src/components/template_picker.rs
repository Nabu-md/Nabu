//! Template Picker component.
//!
//! Searchable list of templates. Views are projections of existing
//! KnowledgeObjects — views never own data.
//!
//! Dioxus 0.6 migration: `leptos::view!` → `dioxus::rsx!`, `signal(x)` →
//! `use_signal(|| x)`, `Callback<T>` → `dioxus::prelude::Callback<T>`,
//! `event_target_value` → `ev.value()`, `collect_view()` dropped (iterators
//! are consumed directly by `rsx!` `for`).

use crate::models::template::Template;
use dioxus::prelude::*;

/// Searchable template picker. Callers receive the selected `Template` via
/// the `on_select` callback.
#[component]
pub fn TemplatePicker(
    templates: Vec<Template>,
    on_select: Callback<Template>,
) -> Element {
    let mut search = use_signal(String::new);

    // Re-evaluate on every render; Dioxus re-renders when `search` changes.
    let query = search.read().to_lowercase();
    let filtered: Vec<Template> = if query.is_empty() {
        templates.clone()
    } else {
        templates
            .iter()
            .filter(|t| t.name.to_lowercase().contains(&query))
            .cloned()
            .collect()
    };

    rsx! {
        div { class: "template-picker space-y-2" }
        input {
            r#type: "text",
            placeholder: "Search templates...",
            class: "w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
            value: "{search.read()}",
            oninput: move |ev: FormEvent| {
                search.set(ev.value());
            },
        }
        div { class: "template-list space-y-1" }

        if filtered.is_empty() {
            p { class: "text-sm text-gray-500 pt-1", "No templates found" }
        }

        for template in filtered {
            {
                let t = template;
                rsx! {
                    button {
                        class: "w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 rounded transition-colors",
                        onclick: move |_| {
                            let _ = on_select.call(t.clone());
                        },
                        "{t.name}"
                    }
                }
            }
        }
    }
}
