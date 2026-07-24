use leptos::prelude::*;

#[component]
pub fn TabBar() -> impl IntoView {
    view! {
        <div class="flex border-b border-gray-700 bg-gray-800 p-1">
            // Tab bar content
            <div class="px-3 py-1 bg-gray-700 rounded-t text-sm">"Note 1" <button class="ml-2">"×"</button></div>
            <div class="px-3 py-1 text-gray-400 text-sm">"Note 2"</div>
        </div>
    }
}
