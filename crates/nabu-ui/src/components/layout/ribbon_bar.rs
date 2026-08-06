//! # Ribbon Bar — the left vertical icon bar
//!
//! Preserves the Leptos behaviour: a narrow vertical bar of icon buttons
//! (vault explorer, search, graph, daily note, dictation, canvas, settings).

use crate::components::contexts::ViewMode;
use crate::components::contexts::{use_nav, NavContext};
use crate::components::ui::button::IconButton;
use crate::components::ui::icons::{IconEl, Icon};
use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RibbonAction {
    ToggleSidebar,
    OpenSearch,
    OpenGraph,
    OpenCanvas,
    OpenSettings,
}

/// The ribbon bar — a narrow vertical column of icon buttons.
#[component]
pub fn RibbonBar() -> Element {
    let nav: NavContext = use_nav();

    let open_sidebar = move |_| {
        nav.show_left_sidebar.set(true);
    };

    let open_graph = move |_| {
        nav.view_mode.set(ViewMode::Graph);
    };

    let open_canvas = move |_| {
        nav.view_mode.set(ViewMode::Canvas);
    };

    let toggle_dictation = move |_| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("toggle_dictation_pill", args).await;
        });
    };

    let open_settings = move |_| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let _ = crate::ipc::tauri_invoke("open_settings", args).await;
        });
    };

    rsx! {
        div {
            class: "w-12 h-screen border-r border-gray-700 bg-gray-900 flex flex-col items-center py-4 space-y-4",
        }
        IconButton {
            title: "Vault Explorer",
            on_click: open_sidebar,
            IconEl { icon: Icon::Folder }
        }
        IconButton {
            title: "Global Search",
            on_click: move |_| {
                nav.view_mode.set(ViewMode::Search);
                nav.search_query.set(String::new());
            },
            IconEl { icon: Icon::Search }
        }
        IconButton {
            title: "Graph View",
            on_click: open_graph,
            IconEl { icon: Icon::Network }
        }
        IconButton {
            title: "Daily Note",
            on_click: move |_| {
                // ⌘⇧D opens the daily note — reuse the shared helper.
                let ws = crate::components::contexts::use_workspace();
                crate::components::navigation::commands::open_daily_note(ws, crate::components::ui::feedback::use_toast()).call(());
            },
            IconEl { icon: Icon::Calendar }
        }
        IconButton {
            title: "Dictation",
            on_click: toggle_dictation,
            IconEl { icon: Icon::Mic }
        }
        IconButton {
            title: "Canvas",
            on_click: open_canvas,
            IconEl { icon: Icon::Palette }
        }
        div { class: "flex-grow" }
        IconButton {
            title: "Settings",
            on_click: open_settings,
            IconEl { icon: Icon::Settings }
        }
    }
}
