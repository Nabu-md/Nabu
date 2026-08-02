//! Informational primitives — tooltip, empty state, callout, help text.

use leptos::prelude::*;

/// Tooltip — wraps a child and shows text on hover / focus.
#[component]
pub fn Tooltip(
    /// Tooltip text.
    text: String,
    /// Placement (top is the only fully supported position today).
    #[prop(optional)]
    position: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let pos = position.unwrap_or("top");
    let pos_class = match pos {
        "bottom" => "tooltip-bottom",
        "left" => "tooltip-left",
        "right" => "tooltip-right",
        _ => "",
    };
    view! {
        <span class="tooltip-wrap" tabindex="0">
            {children()}
            <span class=format!("tooltip-text {pos_class}") role="tooltip">{text}</span>
        </span>
    }
}

/// Empty state — a placeholder for empty lists / panels.
#[component]
pub fn EmptyState(
    /// Optional icon (emoji or glyph).
    #[prop(optional)]
    icon: Option<&'static str>,
    /// Title text.
    title: String,
    /// Optional description.
    #[prop(optional)]
    description: Option<String>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("empty-state{extra}")>
            {icon.map(|i| view! { <div class="empty-state-icon" aria-hidden="true">{i}</div> }.into_any())}
            <div class="empty-state-title">{title}</div>
            {description.map(|d| view! { <div class="empty-state-desc">{d}</div> }.into_any())}
            {children()}
        </div>
    }
}

/// Callout kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CalloutKind {
    #[default]
    Info,
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

/// Callout — a highlighted inset note.
#[component]
pub fn Callout(
    /// Kind.
    kind: CalloutKind,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("callout {}{extra}", kind.class())>{children()}</div>
    }
}

/// Help text — small muted helper line under a field.
#[component]
pub fn HelpText(
    /// Text to show.
    text: String,
) -> impl IntoView {
    view! { <span class="field-hint">{text}</span> }
}
