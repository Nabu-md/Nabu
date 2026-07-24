use leptos::prelude::*;

#[component]
pub fn RightInspector() -> impl IntoView {
    let (active_tab, set_active_tab) = signal("Tags".to_string());
    let tabs = vec!["🏷️", "🔗", "➡️", "📋"];

    view! {
        <div class="w-64 border-l border-gray-700 bg-gray-900 h-screen flex flex-col">
            <div class="flex border-b border-gray-700">
                {tabs.iter().map(|t| {
                    let tab = t.to_string();
                    let tab_for_class = tab.clone();
                    let tab_for_click = tab.clone();
                    view! {
                        <button 
                            class=move || format!("flex-1 p-2 text-center {}", if active_tab.get() == tab_for_class { "bg-gray-800" } else { "" })
                            on:click=move |_| set_active_tab.set(tab_for_click.clone())
                        >
                            {tab}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="flex-1 p-4 text-gray-300 text-sm">
                {move || match active_tab.get().as_str() {
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
