//! Navigation primitives — tabs, breadcrumbs, sidebar items, toolbar buttons,
//! navigation groups.

use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// One tab definition.
#[derive(Clone)]
pub struct TabDef {
    pub id: String,
    pub label: String,
    /// Optional icon rendered in the tab header.
    pub icon: Option<Icon>,
}

impl TabDef {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
        }
    }

    /// Attach an icon to the tab.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Tab strip. Use with [`TabDef`] and an active-id signal.
#[component]
pub fn Tabs(
    /// Tab definitions.
    tabs: Vec<TabDef>,
    /// Two-way bound active tab id.
    active: RwSignal<String>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    /// Called when the active tab changes.
    #[prop(optional)]
    on_change: Option<Callback<String>>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let cb = on_change;
    view! {
        <div class=format!("tabs{extra}") role="tablist">
            {tabs.into_iter().map(|tab| {
                let id = tab.id.clone();
                let id_for_active = id.clone();
                let id_for_click = id.clone();
                let label = tab.label.clone();
                let is_active = Memo::new(move |_| active.get() == id_for_active);
                let on_click_cb = cb;
                view! {
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || is_active.get()
                        class=move || format!("tab{}", if is_active.get() { " tab-active" } else { "" })
                        on:click=move |_| {
                            active.set(id_for_click.clone());
                            if let Some(cb) = on_click_cb.as_ref() {
                                cb.run(id_for_click.clone());
                            }
                        }
                    >
                        {tab.icon.map(|ic| render_icon_view(ic))}
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

/// One breadcrumb item.
#[derive(Clone)]
pub struct Breadcrumb {
    pub label: String,
    pub on_click: Option<Callback<()>>,
}

impl Breadcrumb {
    pub fn new(label: impl Into<String>, on_click: Option<Callback<()>>) -> Self {
        Self {
            label: label.into(),
            on_click,
        }
    }
}

/// Breadcrumb trail. The last item is rendered as the current (non-clickable)
/// crumb; all others are links.
#[component]
pub fn Breadcrumbs(
    /// Breadcrumb items, in order from root to current.
    items: Vec<Breadcrumb>,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    let count = items.len();
    view! {
        <nav class=format!("breadcrumbs{extra}") aria-label="Breadcrumb">
            {items.into_iter().enumerate().map(|(i, crumb)| {
                let is_last = i == count - 1;
                let label = crumb.label.clone();
                let on_click = crumb.on_click;
                view! {
                    {if i > 0 {
                        view! { <span class="breadcrumb-sep" aria-hidden="true">"/"</span> }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                    {if is_last {
                        view! { <span class="breadcrumb-current" aria-current="page">{label}</span> }.into_any()
                    } else {
                        view! {
                            <button type="button" class="breadcrumb-link" on:click=move |_| {
                                if let Some(cb) = on_click.as_ref() { cb.run(()); }
                            }>
                                {label}
                            </button>
                        }.into_any()
                    }}
                }
            }).collect_view()}
        </nav>
    }
}

/// Sidebar navigation item. Optionally highlights when active and shows a count.
#[component]
pub fn SidebarItem(
    /// Label text.
    label: String,
    /// Whether this item is currently active.
    #[prop(optional)]
    active: bool,
    /// Optional count badge on the right.
    #[prop(optional)]
    count: Option<usize>,
    /// Optional icon prefix.
    #[prop(optional)]
    icon: Option<Icon>,
    /// Optional nested indent level (in multiples of `space-4`).
    #[prop(optional)]
    indent: Option<u32>,
    /// Called when the item is clicked.
    #[prop(optional)]
    on_click: Option<Callback<()>>,
) -> impl IntoView {
    let mut base = String::from("sidebar-item");
    if active {
        base.push_str(" sidebar-item-active");
    }
    // Static class literals so Tailwind's scanner picks them up (no dynamic
    // class-name construction).
    let indent_class = match indent {
        Some(1) => " pl-4",
        Some(2) => " pl-8",
        Some(3) => " pl-12",
        _ => "",
    };
    view! {
        <button
            type="button"
            class=format!("{base}{indent_class}")
            aria-current=move || if active { Some("page") } else { None }
            on:click=move |_| {
                if let Some(cb) = on_click.as_ref() { cb.run(()); }
            }
        >
            {icon.map(|ic| render_icon_view(ic))}
            <span class="flex-1 text-left truncate">{label}</span>
            {count.map(|c| view! { <span class="sidebar-item-count">{c}</span> }.into_any())}
        </button>
    }
}

/// Toolbar button — an icon/text button with an active state.
#[component]
pub fn ToolbarButton(
    /// Label text.
    label: String,
    /// Tooltip.
    #[prop(optional)]
    title: Option<&'static str>,
    /// Active (pressed) state.
    #[prop(optional)]
    active: bool,
    /// Disables the button.
    #[prop(optional)]
    disabled: bool,
    /// Called when clicked.
    #[prop(optional)]
    on_click: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || format!("toolbar-btn{}", if active { " toolbar-btn-active" } else { "" })
            title=title
            aria-pressed=move || active
            disabled=disabled
            on:click=move |_| {
                if let Some(cb) = on_click.as_ref() { cb.run(()); }
            }
        >
            {label}
        </button>
    }
}

/// Navigation group — a titled column of [`SidebarItem`]s.
#[component]
pub fn NavGroup(
    /// Group title.
    title: String,
    /// Extra utility classes.
    #[prop(optional)]
    class: Option<&'static str>,
    children: ChildrenFn,
) -> impl IntoView {
    let extra = class.map(|c| format!(" {c}")).unwrap_or_default();
    view! {
        <div class=format!("flex flex-col gap-0.5{extra}")>
            <div class="sidebar-group-title">{title}</div>
            {children()}
        </div>
    }
}
