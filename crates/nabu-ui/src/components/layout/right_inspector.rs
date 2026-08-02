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
                    "🏷️" => view! {
                        <crate::components::ui::info::EmptyState
                            icon="🏷️"
                            title="No tags yet".to_string()
                            description="Tags you add to the active note will appear here.".to_string()
                        ></crate::components::ui::info::EmptyState>
                    }.into_any(),
                    "🔗" => view! {
                        <crate::components::ui::info::EmptyState
                            icon="🔗"
                            title="No backlinks yet".to_string()
                            description="Other notes that link to this one will appear here.".to_string()
                        ></crate::components::ui::info::EmptyState>
                    }.into_any(),
                    "➡️" => view! {
                        <crate::components::ui::info::EmptyState
                            icon="➡️"
                            title="No outgoing links".to_string()
                            description="Links you write in this note will appear here.".to_string()
                        ></crate::components::ui::info::EmptyState>
                    }.into_any(),
                    "📋" => view! {
                        <crate::components::ui::info::EmptyState
                            icon="📋"
                            title="No outline yet".to_string()
                            description="Headings in this note will be listed here for quick navigation.".to_string()
                        ></crate::components::ui::info::EmptyState>
                    }.into_any(),
                    _ => view! {}.into_any(),
                }}
            </div>
        </div>
    }
}
