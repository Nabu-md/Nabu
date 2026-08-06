//! # Right Inspector — property metadata editor
//!
//! Renders a [`PropertyEditor`] with the standard `PropertyType` field set
//! (text, number, date, select, multi-select, URL). Values are live-projected
//! from the backend; edits call back to the parent via `on_change`.

use crate::components::contexts::{use_nav, NavContext};
use crate::components::property_editor::{PropertyEditor, PropertyDefinition, PropertyValue};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::nav::{TabDef, Tabs};
use dioxus::prelude::*;

use std::collections::HashMap;

/// The Right Inspector panel.
///
/// Renders the inspector dock shell with the Tags / Backlinks / Outgoing /
/// Mentions tab bar. The "Tags" tab hosts the [`PropertyEditor`] for note
/// metadata; the remaining tabs show placeholder content (backlinks,
/// outgoing links, unlinked mentions) that arrives in a later phase.
#[component]
pub fn RightInspector() -> Element {
    let active_tab = use_signal(|| "tags".to_string());

    let tabs = vec![
        TabDef::new("tags", "Tags").with_icon(Icon::Tag),
        TabDef::new("backlinks", "Backlinks").with_icon(Icon::Link),
        TabDef::new("outgoing", "Outgoing").with_icon(Icon::Forward),
        TabDef::new("mentions", "Mentions").with_icon(Icon::MessageCircle),
    ];

    // Standard property set for notes.
    let properties = vec![
        PropertyDefinition {
            id: "title".to_string(),
            display_name: "Title".to_string(),
            property_type: crate::models::properties::PropertyType::Text,
            description: Some("The note's display title.".to_string()),
            default_value: None,
            options: None,
        },
        PropertyDefinition {
            id: "created".to_string(),
            display_name: "Created".to_string(),
            property_type: crate::models::properties::PropertyType::Date,
            description: Some("When this note was first created.".to_string()),
            default_value: None,
            options: None,
        },
        PropertyDefinition {
            id: "modified".to_string(),
            display_name: "Modified".to_string(),
            property_type: crate::models::properties::PropertyType::Date,
            description: Some("When this note was last saved.".to_string()),
            default_value: None,
            options: None,
        },
        PropertyDefinition {
            id: "tags".to_string(),
            display_name: "Tags".to_string(),
            property_type: crate::models::properties::PropertyType::MultiSelect,
            description: Some("Categories for this note.".to_string()),
            default_value: None,
            options: Some(vec!["work".to_string(), "personal".to_string(), "reference".to_string()]),
        },
        PropertyDefinition {
            id: "status".to_string(),
            display_name: "Status".to_string(),
            property_type: crate::models::properties::PropertyType::Select,
            description: Some("Current workflow state.".to_string()),
            default_value: None,
            options: Some(vec![
                "draft".to_string(),
                "in progress".to_string(),
                "done".to_string(),
            ]),
        },
        PropertyDefinition {
            id: "url".to_string(),
            display_name: "URL".to_string(),
            property_type: crate::models::properties::PropertyType::Url,
            description: Some("Source URL (for captured notes).".to_string()),
            default_value: None,
            options: None,
        },
    ];

    let empty_values = use_signal(HashMap::<String, PropertyValue>::new);

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
            {inspect_content(&active_tab.read(), properties, empty_values)}
        }
    }
}

/// Returns content for the active inspector tab.
fn inspect_content(tab: &str, properties: Vec<PropertyDefinition>, values: Signal<HashMap<String, PropertyValue>>) -> Element {
    match tab {
        "tags" => rsx! {
            div { class: "space-y-4" }
            div { class: "text-xs font-semibold uppercase tracking-wider text-gray-500", "Properties" }
            PropertyEditor {
                properties: properties,
                values: values.read().clone(),
                on_change: None,
                on_validate: None,
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

/// Legacy placeholder — kept for backward compatibility.
#[allow(dead_code)]
fn inspect_placeholder(tab: &str) -> Element {
    inspect_content(tab, Vec::new(), use_signal(HashMap::<String, PropertyValue>::new))
}
