//! # Home screen — placeholder
//!
//! Shown when no note is selected in the editor view. Phase 0.3 ships a
//! structural placeholder; real quick actions + recent activity arrive later.

use crate::components::contexts::{use_nav, use_workspace};
use crate::components::navigation::state::ViewMode;
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

#[component]
pub fn HomeScreen() -> Element {
    let nav = use_nav();
    rsx! {
        div { class: "home-screen" }
        div { class: "home-hero" }
        h1 { class: "home-title", "Welcome to {nav.vault_name.read().clone()}" }
        p { class: "home-subtitle",
            "Your knowledge base is ready. Start a note, search everything, or open today's entry."
        }
        div { class: "home-actions" }
        div { class: "text-sm text-gray-400",
            {render_icon_view(Icon::Info)}
            " Home screen placeholder — quick actions arrive in a later phase."
        }
    }
}
