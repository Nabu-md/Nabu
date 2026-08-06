//! Layout primitives — panel, section, stack, grid, container.

use dioxus::prelude::*;

#[component]
pub fn Panel(
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "panel{extra}", {children} }
    }
}

#[component]
pub fn Section(
    title: String,
    #[props(optional)] description: Option<String>,
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        section { class: "section{extra}" }
        header {
            h3 { class: "section-title", "{title}" }
            {description.map(|d| rsx! { p { class: "section-desc", "{d}" } })}
        }
        {children}
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StackDirection {
    #[default] Vertical,
    Horizontal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StackGap {
    #[default] Md,
    None,
    Sm,
    Lg,
    Xl,
}

impl StackGap {
    fn class(self) -> &'static str {
        match self {
            StackGap::None => "",
            StackGap::Sm => "stack-gap-2",
            StackGap::Md => "stack-gap-3",
            StackGap::Lg => "stack-gap-4",
            StackGap::Xl => "stack-gap-6",
        }
    }
}

#[component]
pub fn Stack(
    #[props(optional)] direction: StackDirection,
    #[props(optional)] gap: StackGap,
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let row = if direction == StackDirection::Horizontal {
        " stack-row"
    } else {
        ""
    };
    let gap_class = gap.class();
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "stack{row} {gap_class}{extra}", {children} }
    }
}

#[component]
pub fn Grid(
    #[props(optional)] min: Option<&'static str>,
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div {
            class: "grid-auto{extra}",
            style: if let Some(min_w) = min { format!("--grid-min: {min_w};") } else { "" },
            {children}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ContainerWidth {
    #[default] Medium,
    Narrow,
    Wide,
}

impl ContainerWidth {
    fn class(self) -> &'static str {
        match self {
            ContainerWidth::Narrow => "container-narrow",
            ContainerWidth::Medium => "container-medium",
            ContainerWidth::Wide => "container-wide",
        }
    }
}

#[component]
pub fn Container(
    #[props(optional)] width: ContainerWidth,
    #[props(optional)] class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "{width.class()} {extra.trim()}", {children} }
    }
}
