//! Template Picker component.
//!
//! Searchable list of templates. Views are projections of existing
//! KnowledgeObjects — views never own data.

use crate::models::template::Template;
use leptos::prelude::*;

#[component]
pub fn TemplatePicker(templates: Vec<Template>, on_select: Callback<Template>) -> impl IntoView {
    let (search, set_search) = signal(String::new());

    view! {
        <div class="template-picker space-y-2">
            <input
                type="text"
                placeholder="Search templates..."
                on:input=move |ev| set_search.set(event_target_value(&ev))
                class="w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
            />
            <div class="template-list space-y-1">
                {move || {
                    let query = search.get().to_lowercase();
                    let filtered: Vec<Template> = templates
                        .iter()
                        .filter(|t| t.name.to_lowercase().contains(&query))
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        view! {
                            <p class="text-sm text-gray-500 pt-1">"No templates found"</p>
                        }.into_any()
                    } else {
                        view! {
                            { filtered.iter().map(|template| {
                                let template = template.clone();
                                view! {
                                    <button
                                        class="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 rounded transition-colors"
                                        on:click=move |_| on_select.run(template.clone())
                                    >
                                        {template.name.clone()}
                                    </button>
                                }
                            }).collect_view()}
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
