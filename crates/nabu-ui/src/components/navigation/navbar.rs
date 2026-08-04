//! # NavBar — the app's top navigation bar
//!
//! Replaces the ad-hoc view-mode button row in the app shell. Shows:
//!
//! - a **breadcrumb** of the current location
//! - a compact **view switcher** (Dashboard / Editor / Graph / …)
//! - the **search** affordance and the **Command Palette** / **Quick
//!   Switcher** / **Shortcuts** buttons
//! - undo / redo, save status, and sidebar toggles

use crate::components::navigation::breadcrumb::BreadcrumbBar;
use crate::components::navigation::state::{use_nav, ViewMode};
use crate::components::recovery::save_status::SaveStatusIndicator;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::history::{redo, undo, use_history};
use leptos::prelude::*;

/// The top navigation bar (rendered inside the main content column).
#[component]
pub fn NavBar() -> impl IntoView {
    let nav = use_nav();
    let history = use_history();
    let toasts = crate::components::ui::feedback::use_toast();

    view! {
        <div class="navbar">
            <div class="navbar-row">
                <BreadcrumbBar />
                <div class="flex-1"></div>
                <div class="navbar-actions">
                    <button
                        type="button"
                        class="navbar-action"
                        title="Search all notes (⌘⇧F)"
                        aria-label="Search"
                        on:click=move |_| {
                            nav.search_query.set(String::new());
                            nav.view_mode.set(ViewMode::Search);
                        }
                    >
                        {render_icon_view(Icon::Search)}
                    </button>
                    <button
                        type="button"
                        class="navbar-action"
                        title="Command palette (⌘K)"
                        aria-label="Command palette"
                        on:click=move |_| nav.palette_open.set(true)
                    >
                        {render_icon_view(Icon::Command)}
                    </button>
                    <button
                        type="button"
                        class="navbar-action"
                        title="Quick switcher (⌘P)"
                        aria-label="Quick switcher"
                        on:click=move |_| nav.switcher_open.set(true)
                    >
                        {render_icon_view(Icon::Zap)}
                    </button>
                    <button
                        type="button"
                        class="navbar-action"
                        title="Keyboard shortcuts (⌘⇧?)"
                        aria-label="Shortcuts reference"
                        on:click=move |_| nav.shortcuts_open.set(true)
                    >
                        {render_icon_view(Icon::Keyboard)}
                    </button>
                    <span class="navbar-sep"></span>
                    <button
                        type="button"
                        class=move || format!(
                            "navbar-action{}",
                            if history.can_undo.get() { "" } else { " navbar-action-disabled" }
                        )
                        title="Undo (⌘Z)"
                        aria-label="Undo"
                        disabled=move || !history.can_undo.get()
                        on:click=move |_| undo(history, toasts)
                    >
                        {render_icon_view(Icon::Undo)}
                    </button>
                    <button
                        type="button"
                        class=move || format!(
                            "navbar-action{}",
                            if history.can_redo.get() { "" } else { " navbar-action-disabled" }
                        )
                        title="Redo (⌘⇧Z)"
                        aria-label="Redo"
                        disabled=move || !history.can_redo.get()
                        on:click=move |_| redo(history, toasts)
                    >
                        {render_icon_view(Icon::Redo)}
                    </button>
                    <span class="navbar-sep"></span>
                    <crate::components::ui::feedback::TaskIndicator />
                    <crate::components::ui::feedback::NotificationBell />
                    <SaveStatusIndicator />
                    <button
                        type="button"
                        class="navbar-action"
                        title="Toggle left sidebar (⌘\\)"
                        aria-label="Toggle left sidebar"
                        on:click=move |_| nav.show_left_sidebar.update(|v| *v = !*v)
                    >
                        {render_icon_view(Icon::Folder)}
                    </button>
                    <button
                        type="button"
                        class="navbar-action"
                        title="Toggle right inspector (⌘⇧\\)"
                        aria-label="Toggle right inspector"
                        on:click=move |_| nav.show_right_inspector.update(|v| *v = !*v)
                    >
                        {render_icon_view(Icon::ClipboardList)}
                    </button>
                </div>
            </div>
        </div>
    }
}
