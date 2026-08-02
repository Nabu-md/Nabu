use crate::components::file_tree::FileTree;
use leptos::prelude::*;

/// Left sidebar — the vault file explorer.
///
/// Contains the interactive [`FileTree`] (real vault data, context menus,
/// inline rename, drag-and-drop, multi-select + batch actions) plus a quick
/// "New note" action.
#[component]
pub fn LeftSidebar() -> impl IntoView {
    view! {
        <div class="w-64 border-r border-gray-700 bg-gray-900 h-full flex flex-col min-w-0">
            <FileTree />
        </div>
    }
}
