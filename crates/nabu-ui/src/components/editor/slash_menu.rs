use crate::components::ui::menu::MenuItem;
use leptos::prelude::*;

#[component]
pub fn SlashMenu(on_select: Callback<String>) -> impl IntoView {
    let items = vec![
        "# Heading 1",
        "## Heading 2",
        "### Heading 3",
        "📋 Kanban Board",
        "📷 Vision OCR Scan",
        "📦 Code Block / Sandbox",
        "💡 Callout Box",
    ];

    view! {
        <div class="absolute bg-gray-800 border border-gray-700 rounded shadow-lg z-10 w-48 py-1">
            {items.into_iter().map(|item| {
                let label = item.to_string();
                let on_pick = on_select.clone();
                view! {
                    <MenuItem
                        label=label.clone()
                        on_select=Callback::new(move |_| on_pick.run(label.clone()))
                    />
                }
            }).collect_view()}
        </div>
    }
}
