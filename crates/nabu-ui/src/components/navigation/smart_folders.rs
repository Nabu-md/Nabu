//! # Smart Folders — placeholder
//!
//! Phase 0.3 ships a structural placeholder. The query-powered folder UI
//! arrives in a later phase.

use crate::components::ui::info::EmptyState;
use crate::components::ui::icons::Icon;
use dioxus::prelude::*;

#[component]
pub fn SmartFoldersPage() -> Element {
    rsx! {
        div { class: "max-w-4xl mx-auto h-full" }
        EmptyState {
            icon: Some(Icon::FolderTree),
            title: "Smart Folders".to_string(),
            description: "Query-powered virtual collections placeholder — implementation arrives in a later phase.".to_string(),
        }
    }
}
