//! Card primitives — standard, outlined, elevated, interactive, collapsible.

use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

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
    #[prop(optional)]
    variant: CardVariant,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Optional click handler (turns the card into a button-like target).
    #[prop(optional)]
    on_click: Option<Callback<()>>,
    children: ChildrenFn,
) -> impl IntoView {
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
    view! {
        <div
            class=base
            role=move || if is_clickable { Some("button") } else { None }
            tabindex=move || if is_clickable { Some("0") } else { None }
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
            on:keydown=move |ev| {
                if is_clickable && (ev.key() == "Enter" || ev.key() == " ") {
                    ev.prevent_default();
                    if let Some(cb) = on_click {
                        cb.run(());
                    }
                }
            }
        >
            {children()}
        </div>
    }
}

/// Card header — optional title and actions on the right.
#[component]
pub fn CardHeader(
    /// Optional title text.
    #[prop(optional)]
    title: Option<String>,
    /// Optional subtitle text.
    #[prop(optional)]
    subtitle: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <div class="card-header">
            <div class="flex flex-col gap-0.5 min-w-0">
                {title.map(|t| view! { <h3 class="card-title">{t}</h3> }.into_any())}
                {subtitle.map(|s| view! { <p class="card-subtitle">{s}</p> }.into_any())}
            </div>
            <div class="flex items-center gap-2 ml-auto">{children()}</div>
        </div>
    }
}

/// Card body — main content area.
#[component]
pub fn CardBody(children: ChildrenFn) -> impl IntoView {
    view! { <div class="card-body">{children()}</div> }
}

/// Card footer — actions row.
#[component]
pub fn CardFooter(children: ChildrenFn) -> impl IntoView {
    view! { <div class="card-footer">{children()}</div> }
}

/// Collapsible card — click on the header toggles the body.
#[component]
pub fn CollapsibleCard(
    /// Header title.
    title: String,
    /// Two-way bound open state.
    open: RwSignal<bool>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("card{extra}")>
            <button
                type="button"
                class="card-header w-full text-left cursor-pointer"
                aria-expanded=move || open.get()
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <h3 class="card-title">{title}</h3>
                <span class="ml-auto text-gray-500" aria-hidden="true">
                    {move || if open.get() { render_icon_view(Icon::ChevronDown) } else { render_icon_view(Icon::ChevronRight) }}
                </span>
            </button>
            {move || if open.get() {
                view! { <div class="card-body">{children()}</div> }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}
