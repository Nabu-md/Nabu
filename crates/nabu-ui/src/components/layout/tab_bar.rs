use leptos::prelude::*;

#[component]
pub fn TabBar() -> impl IntoView {
    view! {
        <div class="flex border-b border-gray-700 bg-gray-900 h-9 items-center px-1">
            <div class="group flex items-center px-3 py-1 bg-gray-800 border-t-2 border-primary text-sm text-white rounded-t h-full">
                "Note 1" 
                <button class="ml-2 opacity-0 group-hover:opacity-100 transition-opacity">"×"</button>
            </div>
            <div class="group flex items-center px-3 py-1 text-sm text-gray-400 h-full hover:bg-gray-800 rounded-t">
                "Note 2"
                <button class="ml-2 opacity-0 group-hover:opacity-100 transition-opacity">"×"</button>
            </div>
            <button class="ml-2 text-gray-400 hover:text-white">"+"</button>
        </div>
    }
}
