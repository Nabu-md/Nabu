//! Informational primitives — tooltip, empty state, callout, help text.

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

#[component]
pub fn Tooltip(
    text: String,
    #[props(optional)] position: Option<&'static str>,
    children: Element,
) -> Element {
    let pos = position.unwrap_or("top");
    let pos_class = match pos {
        "bottom" => "tooltip-bottom",
        "left" => "tooltip-left",
        "right" => "tooltip-right",
        _ => "",
    };
    rsx! {
        span { class: "tooltip-wrap", tabindex: "0" }
        {children}
        span { class: "tooltip-text {pos_class}", role: "tooltip", "{text}" }
    }
}

#[component]
pub fn EmptyState(
    /// Optional icon.
    #[props(optional)]
    icon: Option<Icon>,
    /// Title text.
    title: String,
    /// Optional description.
    #[props(optional)]
    description: Option<String>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "empty-state{extra}" }
        {icon.map(|ic| rsx! {
            div { class: "empty-state-icon", "aria-hidden": "true", {render_icon_view(ic)} }
        })}
        div { class: "empty-state-title", "{title}" }
        {description.map(|d| rsx! { div { class: "empty-state-desc", "{d}" } })}
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CalloutKind {
    #[default] Info,
    Success,
    Warning,
    Error,
}

impl CalloutKind {
    fn class(self) -> &'static str {
        match self {
            CalloutKind::Info => "callout-info",
            CalloutKind::Success => "callout-success",
            CalloutKind::Warning => "callout-warning",
            CalloutKind::Error => "callout-error",
        }
    }
}

#[component]
pub fn Callout(
    kind: CalloutKind,
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "callout {kind.class()}{extra}", {children} }
    }
}

#[component]
pub fn HelpText(text: String) -> Element {
    rsx! {
        span { class: "field-hint", "{text}" }
    }
}
