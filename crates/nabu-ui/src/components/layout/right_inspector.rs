use crate::components::ui::nav::{TabDef, Tabs};
use leptos::prelude::*;

#[component]
pub fn RightInspector() -> impl IntoView {
    let active_tab = RwSignal::new("🏷️".to_string());
    let tabs = vec![
        TabDef::new("🏷️", "Tags"),
        TabDef::new("🔗", "Backlinks"),
        TabDef::new("➡️", "Outgoing"),
        TabDef::new("📋", "Outline"),
    ];

    view! {
        <div class="w-64 border-l border-gray-700 bg-gray-900 h-screen flex flex-col">
            <div class="flex border-b border-gray-700">
                <Tabs tabs=tabs active=active_tab />
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
