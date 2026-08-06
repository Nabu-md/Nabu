//! # Search page — placeholder
//!
//! Phase 0.3 ships a structural placeholder for the full-text search screen.
//! The real search implementation arrives in a later phase.

use crate::components::ui::info::EmptyState;
use crate::components::ui::icons::Icon;
use dioxus::prelude::*;

#[component]
pub fn SearchPage() -> Element {
    rsx! {
        div { class: "max-w-4xl mx-auto h-full" }
        EmptyState {
            icon: Some(Icon::Search),
            title: "Search".to_string(),
            description: "Full-text search placeholder — implementation arrives in a later phase.".to_string(),
        }
    }
}
