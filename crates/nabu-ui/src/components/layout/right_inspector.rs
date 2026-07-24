use leptos::prelude::*;

#[component]
    let (active_tab, set_active_tab) = signal("Tags".to_string());
    let tabs = vec!["🏷️", "🔗", "➡️", "📋"];

    view! {
        <div class="w-64 border-l border-gray-700 bg-gray-900 h-screen flex flex-col">
            <div class="flex border-b border-gray-700">
                {tabs.iter().map(|tab| {
                    let tab = tab.to_string();
                    view! {
                        <button 
                            class=move || format!("flex-1 p-2 text-center {}", if active_tab.get() == tab { "bg-gray-800" } else { "" })
                            on:click=move |_| set_active_tab.set(tab.clone())
                        >
                            {tab}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="flex-1 p-4 text-gray-300 text-sm">
                {move || match active_tab.get() {
                    "🏷️" => view! { "Tags: #work, #project" }.into_any(),
                    "🔗" => view! { "Backlinks: Note A" }.into_any(),
                    "➡️" => view! { "Outgoing: Note B" }.into_any(),
                    "📋" => view! { "Outline: H1, H2" }.into_any(),
                    _ => view! {}.into_any(),
                }}
            </div>
        </div>
    }
}
