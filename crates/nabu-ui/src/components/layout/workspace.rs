//! # Workspace Layout — the app's structural skeleton
//!
//! Composes the ribbon bar, left sidebar, main content area (with tab bar and
//! navbar), right inspector, and the overlay surfaces (command palette, quick
//! switcher, shortcuts reference) into a single layout.
//!
//! View switching is driven by [`NavContext::view_mode`]; actual view content
//! is rendered by [`crate::components::app::ViewContent`] which delegates to
//! placeholder components for each view.

use crate::components::contexts::{use_nav, NavContext};
use crate::components::layout::{LeftSidebar, RightInspector, RibbonBar, TabBar};
use crate::components::navigation::{
    BreadcrumbBar, CommandPalette, NavBar, QuickSwitcher, ShortcutReference,
};
use dioxus::prelude::*;

/// The full workspace layout — ribbon, sidebars, content, and overlays.
///
/// Rendered once the vault is configured (inside [`crate::components::app::AppRouter`]).
#[component]
pub fn WorkspaceLayout() -> Element {
    let nav: NavContext = use_nav();

    rsx! {
        div {
            class: "app flex h-screen w-screen bg-gray-950 text-gray-100 overflow-hidden font-sans select-none transition-colors duration-slow ease-standard",
            // ── Left Ribbon Bar ──
            div { class: "flex-none" }
            RibbonBar {}

            // ── Left Sidebar (vault file explorer) ──
            if *nav.show_left_sidebar.read() {
                div { class: "flex-none" }
                LeftSidebar {}
            }

            // ── Main Content Area ──
            div { class: "flex-1 flex flex-col h-screen overflow-hidden bg-gray-900" }
            div { class: "flex-none" }
            TabBar {}
            div { class: "flex-none" }
            NavBar {}
            div { class: "flex-1 overflow-auto p-4" }
            crate::components::app::ViewContent {}

            // ── Right Inspector Sidebar ──
            if *nav.show_right_inspector.read() {
                div { class: "flex-none" }
                RightInspector {}
            }

            // ── Overlay surfaces ──
            CommandPalette {}
            QuickSwitcher {}
            ShortcutReference {}
        }
    }
}
