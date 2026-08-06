//! Card primitives — standard, outlined, elevated, interactive, collapsible.

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// Visual variants for [`Card`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CardVariant {
    /// Standard card with shadow.
    #[default]
    Standard,
    /// Flat card with border only.
    Outlined,
    /// Card with stronger shadow / hover elevation.
    Elevated,
    /// Interactive card — hover border + pointer cursor.
    Interactive,
}

impl CardVariant {
    fn classes(self) -> &'static str {
        match self {
            CardVariant::Standard => "",
            CardVariant::Outlined => "card-outlined",
            CardVariant::Elevated => "card-elevated",
            CardVariant::Interactive => "card-hover cursor-pointer",
        }
    }
}

/// Card container. Use with [`CardHeader`], [`CardBody`], [`CardFooter`].
#[component]
pub fn Card(
    /// Visual variant.
    #[props(optional)]
    variant: CardVariant,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Optional click handler (turns the card into a button-like target).
    #[props(optional)]
    on_click: Option<EventHandler<()>>,
    children: Element,
) -> Element {
    let mut base = String::from("card");
    let variant_classes = variant.classes();
    if !variant_classes.is_empty() {
        base.push(' ');
        base.push_str(variant_classes);
    }
    if let Some(extra) = class {
        base.push(' ');
        base.push_str(extra);
    }
    let is_clickable = on_click.is_some();
    let on_click_cb = on_click;
    rsx! {
        div {
            class: base,
            role: if is_clickable { Some("button") } else { None },
            tabindex: if is_clickable { Some("0") } else { None },
            onclick: move |_| {
                if let Some(cb) = on_click_cb.as_ref() {
                    cb.call(());
                }
            },
            onkeydown: move |ev: KeyboardEvent| {
                if is_clickable && (ev.key() == Key::Enter || ev.key().to_string() == " ") {
                    ev.prevent_default();
                    if let Some(cb) = on_click_cb.as_ref() {
                        cb.call(());
                    }
                }
            },
            {children}
        }
    }
}

/// Card header — optional title and actions on the right.
#[component]
pub fn CardHeader(
    /// Optional title text.
    #[props(optional)]
    title: Option<String>,
    /// Optional subtitle text.
    #[props(optional)]
    subtitle: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "card-header" }
        div { class: "flex flex-col gap-0.5 min-w-0" }
        {title.map(|t| rsx! { h3 { class: "card-title", "{t}" } })}
        {subtitle.map(|s| rsx! { p { class: "card-subtitle", "{s}" } })}
        div { class: "flex items-center gap-2 ml-auto", {children} }
    }
}

/// Card body — main content area.
#[component]
pub fn CardBody(children: Element) -> Element {
    rsx! {
        div { class: "card-body", {children} }
    }
}

/// Card footer — actions row.
#[component]
pub fn CardFooter(children: Element) -> Element {
    rsx! {
        div { class: "card-footer", {children} }
    }
}

/// Collapsible card — click on the header toggles the body.
#[component]
pub fn CollapsibleCard(
    /// Header title.
    title: String,
    /// Two-way bound open state.
    open: Signal<bool>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let mut open_sig = open;
    rsx! {
        div { class: "card{extra}" }
        button {
            r#type: "button",
            class: "card-header w-full text-left cursor-pointer",
            "aria-expanded": "{*open_sig.read()}",
            onclick: move |_| open_sig.toggle(),
            h3 { class: "card-title", "{title}" }
            span { class: "ml-auto text-gray-500", "aria-hidden": "true" }
            if *open_sig.read() {
                {render_icon_view(Icon::ChevronDown)}
            } else {
                {render_icon_view(Icon::ChevronRight)}
            }
        }
        if *open_sig.read() {
            div { class: "card-body", {children} }
        }
    }
}
