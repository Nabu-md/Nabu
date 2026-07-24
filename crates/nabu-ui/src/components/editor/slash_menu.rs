use leptos::prelude::*;

#[component]
pub fn SlashMenu(on_select: Callback<String>) -> impl IntoView {
    let items = vec![
        "# Heading 1", "## Heading 2", "### Heading 3",
        "📋 Kanban Board", "📷 Vision OCR Scan",
        "📦 Code Block / Sandbox", "💡 Callout Box"
    ];
    let (active_index, _set_active_index) = signal(0);

    view! {
        <div class="absolute bg-gray-800 border border-gray-700 rounded shadow-lg z-10 w-48">
            {items.iter().enumerate().map(|(i, item)| {
                let item_clone = (*item).to_string();
                view! {
                    <div 
                        class=move || if active_index.get() == i { "p-2 bg-gray-700 text-white cursor-pointer" } else { "p-2 text-gray-300 cursor-pointer" }
                        on:click=move |_| on_select.run(item_clone.to_string())
                    >
                        {item.to_string()}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}
