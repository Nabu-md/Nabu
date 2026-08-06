//! # Archive — placeholder
//!
//! Phase 0.3 ships a structural placeholder for the non-destructive
//! archiving workspace.

use crate::components::ui::info::EmptyState;
use crate::components::ui::icons::Icon;
use dioxus::prelude::*;

#[component]
pub fn ArchivePage() -> Element {
    rsx! {
        div { class: "max-w-4xl mx-auto h-full" }
        EmptyState {
            icon: Some(Icon::Archive),
            title: "Archive".to_string(),
            description: "Archived notes placeholder — implementation arrives in a later phase.".to_string(),
        }
    }
}
