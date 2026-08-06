//! Navigation primitives — tabs, breadcrumbs, sidebar items, toolbar buttons,
//! navigation groups.

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// One tab definition.
#[derive(Clone)]
pub struct TabDef {
    pub id: String,
    pub label: String,
    pub icon: Option<Icon>,
}

impl PartialEq for TabDef {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.label == other.label && self.icon == other.icon
    }
}

impl TabDef {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
        }
    }

    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// One tab button inside [`Tabs`].
#[component]
fn TabButton(
    tab: TabDef,
    current: String,
    active: Signal<String>,
    on_change: Option<EventHandler<String>>,
) -> Element {
    let id = tab.id.clone();
    let label = tab.label.clone();
    let icon = tab.icon;
    let is_active = id == current;
    let cb = on_change;
    rsx! {
        button {
            key: "{id}",
            r#type: "button",
            role: "tab",
            "aria-selected": "{is_active}",
            class: if is_active { "tab tab-active" } else { "tab" },
            onclick: move |_| {
                let mut s = active;
                s.set(id.clone());
                if let Some(cb) = cb.as_ref() {
                    cb.call(id.clone());
                }
            },
            if let Some(ic) = icon {
                {render_icon_view(ic)}
            }
            "{label}"
        }
    }
}

/// Tab strip. Use with [`TabDef`] and an active-id signal.
#[component]
pub fn Tabs(
    /// Tab definitions.
    tabs: Vec<TabDef>,
    /// Two-way bound active tab id.
    active: Signal<String>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    /// Called when the active tab changes.
    #[props(optional)]
    on_change: Option<EventHandler<String>>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let current = active.read().clone();
    let cb = on_change;
    rsx! {
        div { class: "tabs{extra}", role: "tablist" }
        for tab in tabs {
            TabButton {
                tab: tab,
                current: current.clone(),
                active: active,
                on_change: cb,
            }
        }
    }
}

/// One breadcrumb item.
#[derive(Clone)]
pub struct Breadcrumb {
    pub label: String,
    pub on_click: Option<EventHandler<()>>,
}

impl PartialEq for Breadcrumb {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

/// Breadcrumb trail. The last item is rendered as the current (non-clickable)
/// crumb; all others are links.
#[component]
pub fn Breadcrumbs(
    /// Breadcrumb items, in order from root to current.
    items: Vec<Breadcrumb>,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let count = items.len();
    rsx! {
        nav { class: "breadcrumbs{extra}", "aria-label": "Breadcrumb" }
        for (i, crumb) in items.into_iter().enumerate() {
            {
                let is_last = i == count - 1;
                let label = crumb.label.clone();
                let on_click = crumb.on_click;
                rsx! {
                    span { class: "breadcrumb-sep", "aria-hidden": "true", "/" }
                    if is_last {
                        span { class: "breadcrumb-current", "aria-current": "page", "{label}" }
                    }
                    if !is_last {
                        button {
                            r#type: "button",
                            class: "breadcrumb-link",
                            onclick: move |_| {
                                if let Some(cb) = on_click.as_ref() {
                                    cb.call(());
                                }
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}

/// Sidebar navigation item. Optionally highlights when active and shows a count.
#[component]
pub fn SidebarItem(
    /// Label text.
    label: String,
    /// Whether this item is currently active.
    #[props(optional)]
    active: bool,
    /// Optional count badge on the right.
    #[props(optional)]
    count: Option<usize>,
    /// Optional icon prefix.
    #[props(optional)]
    icon: Option<Icon>,
    /// Optional nested indent level (in multiples of `space-4`).
    #[props(optional)]
    indent: Option<u32>,
    /// Called when the item is clicked.
    #[props(optional)]
    on_click: Option<EventHandler<()>>,
) -> Element {
    let mut base = String::from("sidebar-item");
    if active {
        base.push_str(" sidebar-item-active");
    }
    let indent_class = match indent {
        Some(1) => " pl-4",
        Some(2) => " pl-8",
        Some(3) => " pl-12",
        _ => "",
    };
    let aria_now = if active { "page" } else { "" };
    let on_click_cb = on_click;
    rsx! {
        button {
            r#type: "button",
            class: "{base}{indent_class}",
            "aria-current": aria_now,
            onclick: move |_| {
                if let Some(cb) = on_click_cb.as_ref() {
                    cb.call(());
                }
            },
            if let Some(ic) = icon {
                {render_icon_view(ic)}
            }
            span { class: "flex-1 text-left truncate", "{label}" }
            if let Some(c) = count {
                span { class: "sidebar-item-count", "{c}" }
            }
        }
    }
}

/// Toolbar button — an icon/text button with an active state.
#[component]
pub fn ToolbarButton(
    /// Label text.
    label: String,
    /// Tooltip.
    #[props(optional)]
    title: Option<&'static str>,
    /// Active (pressed) state.
    #[props(optional)]
    active: bool,
    /// Disables the button.
    #[props(optional)]
    disabled: bool,
    /// Called when clicked.
    #[props(optional)]
    on_click: Option<EventHandler<()>>,
) -> Element {
    let on_click_cb = on_click;
    rsx! {
        button {
            r#type: "button",
            class: if active { "toolbar-btn toolbar-btn-active" } else { "toolbar-btn" },
            title: title,
            "aria-pressed": active,
            disabled: disabled,
            onclick: move |_| {
                if let Some(cb) = on_click_cb.as_ref() {
                    cb.call(());
                }
            },
            "{label}"
        }
    }
}

/// Navigation group — a titled column of [`SidebarItem`]s.
#[component]
pub fn NavGroup(
    /// Group title.
    title: String,
    /// Extra utility classes.
    #[props(optional)]
    class: Option<&'static str>,
    children: Element,
) -> Element {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    rsx! {
        div { class: "flex flex-col gap-0.5{extra}" }
        div { class: "sidebar-group-title", "{title}" }
        {children}
    }
}
