//! # Right Inspector — container
//!
//! Phase 0.3 migrates **only the container** (dock shell, tab bar, layout).
//! The inspector's content (Tags / Backlinks / Outgoing / Mentions driven by
//! the backend `note_links` command) arrives in a later phase.

use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::nav::{TabDef, Tabs};
use dioxus::prelude::*;

/// The Right Inspector panel container.
///
/// Renders the inspector dock shell with the Tags / Backlinks / Outgoing /
/// Mentions tab bar and placeholder content for each tab. Real content is
/// migrated in a later phase.
#[component]
pub fn RightInspector() -> Element {
    let active_tab = use_signal(|| "tags".to_string());

    let tabs = vec![
        TabDef::new("tags", "Tags").with_icon(Icon::Tag),
        TabDef::new("backlinks", "Backlinks").with_icon(Icon::Link),
        TabDef::new("outgoing", "Outgoing").with_icon(Icon::Forward),
        TabDef::new("mentions", "Mentions").with_icon(Icon::MessageCircle),
    ];

    rsx! {
        div {
            class: "right-inspector w-64 border-l border-gray-700 bg-gray-900 h-screen flex flex-col transition-[width] duration-slow ease-standard",
            div { class: "flex border-b border-gray-700" }
            Tabs {
                tabs: tabs,
                active: active_tab,
                on_change: None,
            }
            div { class: "flex-1 overflow-y-auto p-4 text-gray-300 text-sm" }
            {inspect_placeholder(&active_tab.read())}
        }
    }
}

/// Returns placeholder content for the active inspector tab.
fn inspect_placeholder(tab: &str) -> Element {
    match tab {
        "tags" => rsx! {
            div {
                class: "empty-state",
                div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(Icon::Tag)} }
                div { class: "empty-state-title", "Tags" }
                div { class: "empty-state-desc", "Tags in the active note's frontmatter will appear here." }
            }
        },
        "backlinks" => rsx! {
            div {
                class: "empty-state",
                div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(Icon::Link)} }
                div { class: "empty-state-title", "Backlinks" }
                div { class: "empty-state-desc", "Other notes that link to this one will appear here." }
            }
        },
        "outgoing" => rsx! {
            div {
                class: "empty-state",
                div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(Icon::Forward)} }
                div { class: "empty-state-title", "Outgoing" }
                div { class: "empty-state-desc", "Links you write in this note will appear here." }
            }
        },
        "mentions" => rsx! {
            div {
                class: "empty-state",
                div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(Icon::MessageCircle)} }
                div { class: "empty-state-title", "Unlinked mentions" }
                div { class: "empty-state-desc", "Plain-text references to existing notes will appear here." }
            }
        },
        _ => rsx! {},
    }
}
