//! # Calendar — placeholder
//!
//! Phase 0.3 ships a structural placeholder for the date-based navigation
//! workspace.

use crate::components::ui::info::EmptyState;
use crate::components::ui::icons::Icon;
use dioxus::prelude::*;

#[component]
pub fn CalendarPage() -> Element {
    rsx! {
        div { class: "max-w-4xl mx-auto h-full" }
        EmptyState {
            icon: Some(Icon::Calendar),
            title: "Calendar".to_string(),
            description: "Date-based navigation placeholder — implementation arrives in a later phase.".to_string(),
        }
    }
}
