use leptos::prelude::*;

#[component]
pub fn LeftSidebar() -> impl IntoView {
    view! {
        <div class="w-64 border-r border-gray-700 bg-gray-900 h-screen flex">
            <div class="w-12 border-r border-gray-700 flex flex-col items-center py-4 space-y-4">
                // Icons
                <div class="text-gray-400">"New"</div>
                <div class="text-gray-400">"Search"</div>
            </div>
            <div class="flex-1 p-2">
                // File Tree
            </div>
        </div>
    }
}
