//! Layout primitives — panel, section, stack, grid, container.

use leptos::prelude::*;

/// Panel — a bordered container with standard padding.
#[component]
pub fn Panel(
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! { <div class=format!("panel{extra}")>{children()}</div> }
}

/// Section — a titled content block with vertical rhythm.
#[component]
pub fn Section(
    /// Section title.
    title: String,
    /// Optional description under the title.
    #[prop(optional)]
    description: Option<String>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <section class=format!("section{extra}")>
            <header>
                <h3 class="section-title">{title}</h3>
                {description.map(|d| view! { <p class="section-desc">{d}</p> }.into_any())}
            </header>
            {children()}
        </section>
    }
}

/// Stack direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StackDirection {
    #[default]
    Vertical,
    Horizontal,
}

/// Stack gap size.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StackGap {
    #[default]
    Md,
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

/// Stack — a flex column (or row) with consistent spacing.
#[component]
pub fn Stack(
    /// Direction.
    #[prop(optional)]
    direction: StackDirection,
    /// Gap size.
    #[prop(optional)]
    gap: StackGap,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let row = if direction == StackDirection::Horizontal {
        " stack-row"
    } else {
        ""
    };
    let gap_class = gap.class();
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("stack{row} {gap_class}{extra}")>{children()}</div>
    }
}

/// Grid — a responsive auto-fill grid with a minimum column width.
#[component]
pub fn Grid(
    /// Minimum column width (CSS value, e.g. "200px").
    #[prop(optional)]
    min: Option<&'static str>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let style = min.map(|m| format!("--grid-min: {m};"));
    view! {
        <div class=format!("grid-auto{extra}") style=style>{children()}</div>
    }
}

/// Container width.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ContainerWidth {
    #[default]
    Medium,
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

/// Container — a centred, max-width wrapper.
#[component]
pub fn Container(
    /// Width variant.
    #[prop(optional)]
    width: ContainerWidth,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("{} {}", width.class(), extra.trim())>{children()}</div>
    }
}
