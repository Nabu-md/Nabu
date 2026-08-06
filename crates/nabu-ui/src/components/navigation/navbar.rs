//! # NavBar — the app's top navigation bar
//!
//! Shows:
//!
//! - a **breadcrumb** of the current location
//! - the **quick-search** affordance and the **Command Palette** / **Quick
//!   Switcher** / **Shortcuts** buttons
//! - undo / redo, save-status, task indicator and the notification bell
//! - sidebar / inspector toggles

use crate::components::contexts::{
    use_history, use_nav, NavContext, SaveStatusIndicator, NotificationBell,
    ViewMode,
};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::feedback::TaskIndicator;
use dioxus::prelude::*;

/// The top navigation bar (rendered inside the main content column).
#[component]
pub fn NavBar() -> Element {
    let mut nav: NavContext = use_nav();
    let history = use_history();
    let toasts = crate::components::ui::feedback::use_toast();

    let can_undo = *history.can_undo.read();
    let can_redo = *history.can_redo.read();

    rsx! {
        div { class: "navbar" }
        div { class: "navbar-row" }
        crate::components::navigation::breadcrumb::BreadcrumbBar {}
        div { class: "flex-1" }
        div { class: "navbar-actions" }
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Search all notes (⌘⇧F)",
            "aria-label": "Search",
            onclick: move |_| {
                nav.search_query.set(String::new());
                nav.view_mode.set(ViewMode::Search);
            },
            {render_icon_view(Icon::Search)}
        }
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Command palette (⌘K)",
            "aria-label": "Command palette",
            onclick: move |_| nav.palette_open.set(true),
            {render_icon_view(Icon::Command)}
        }
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Quick switcher (⌘P)",
            "aria-label": "Quick switcher",
            onclick: move |_| nav.switcher_open.set(true),
            {render_icon_view(Icon::Zap)}
        }
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Keyboard shortcuts (⌘⇧?)",
            "aria-label": "Shortcuts reference",
            onclick: move |_| nav.shortcuts_open.set(true),
            {render_icon_view(Icon::Keyboard)}
        }
        span { class: "navbar-sep" }
        button {
            r#type: "button",
            class: if can_undo { "navbar-action" } else { "navbar-action navbar-action-disabled" },
            title: "Undo (⌘Z)",
            "aria-label": "Undo",
            disabled: !can_undo,
            onclick: move |_| {
                crate::history::undo(history, toasts.clone());
            },
            {render_icon_view(Icon::Undo)}
        }
        button {
            r#type: "button",
            class: if can_redo { "navbar-action" } else { "navbar-action navbar-action-disabled" },
            title: "Redo (⌘⇧Z)",
            "aria-label": "Redo",
            disabled: !can_redo,
            onclick: move |_| {
                crate::history::redo(history, toasts.clone());
            },
            {render_icon_view(Icon::Redo)}
        }
        span { class: "navbar-sep" }
        TaskIndicator {}
        NotificationBell {}
        SaveStatusIndicator {}
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Toggle left sidebar (⌘\\)",
            "aria-label": "Toggle left sidebar",
            onclick: move |_| {
                nav.show_left_sidebar.with_mut(|v| *v = !*v);
            },
            {render_icon_view(Icon::Folder)}
        }
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Toggle right inspector (⌘⇧\\)",
            "aria-label": "Toggle right inspector",
            onclick: move |_| {
                nav.show_right_inspector.with_mut(|v| *v = !*v);
            },
            {render_icon_view(Icon::ClipboardList)}
        }
    }
}
