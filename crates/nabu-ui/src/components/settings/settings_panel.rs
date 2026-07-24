use leptos::prelude::*;

#[component]
pub fn SettingsPanel() -> impl IntoView {
    let (active_tab, set_active_tab) = signal("General".to_string());
    let tabs = vec!["General", "Editor", "Whispr", "Appearance", "Files", "Graph"];

    view! {
        <div class="flex h-full bg-gray-900 text-white">
            <div class="w-48 border-r border-gray-700 p-4 space-y-2">
                {tabs.iter().map(|tab| {
                    let tab = tab.to_string();
                    view! {
                        <button 
                            class=move || format!("w-full p-2 text-left rounded {}", if active_tab.get() == tab { "bg-gray-800" } else { "" })
                            on:click=move |_| set_active_tab.set(tab.clone())
                        >
                            {tab}
                        </button>
                    }
                }).collect_view()}
            </div>
            <div class="flex-1 p-6">
                // Tab content would go here
                <h1 class="text-xl font-bold mb-4">{move || active_tab.get()}</h1>
                <p>"Settings content..."</p>
            </div>
        </div>
    }
}
