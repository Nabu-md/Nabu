//! # Dashboard — placeholder
//!
//! Phase 0.3 ships a structural placeholder. The real dashboard widgets
//! (quick actions, recently modified, favourites, pinned tabs, summary)
//! arrive in a later phase.

use crate::components::navigation::state::view_mode_icon;
use crate::components::ui::icons::render_icon_view;
use crate::components::contexts::use_nav;
use crate::components::contexts::ViewMode;
use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    let nav = use_nav();
    let notes_count = nav.notes_index.read().len();
    rsx! {
        div { class: "dashboard" }
        div { class: "dashboard-header" }
        div {
            h1 { class: "dashboard-title", "Home" }
            p { class: "dashboard-subtitle", "Your vault: {nav.vault_name.read().clone()}" }
        }
        div { class: "dashboard-content p-6" }
        div { class: "text-sm text-gray-400 mb-4" }
        {render_icon_view(view_mode_icon(ViewMode::Dashboard))}
        " Dashboard placeholder — view widgets arrive in a later phase."
        if notes_count > 0 {
            p { class: "mt-2 text-xs text-gray-500", "Indexed {notes_count} notes." }
        }
    }
}
