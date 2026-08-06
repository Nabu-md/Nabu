//! # Breadcrumb navigation
//!
//! Shows the user's current location — vault ▸ folder hierarchy ▸ current
//! note — and lets them click any level to navigate:
//!
//! - vault crumb → Dashboard
//! - folder crumb → reveals that folder in the sidebar file tree
//! - note crumb → activates the note's tab

use crate::components::contexts::{use_nav, use_workspace};
use crate::components::navigation::state::{view_mode_label, ViewMode};
use crate::components::ui::nav::{Breadcrumb, Breadcrumbs};
use dioxus::prelude::*;

/// Dispatches a `nabu:reveal-note` window event so the file tree reveals a
/// path (expands its parent folders and selects it).
fn reveal_in_sidebar(path: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(&path));
    let Ok(event) =
        web_sys::CustomEvent::new_with_event_init_dict("nabu:reveal-note", &init)
    else {
        return;
    };
    let _ = window.dispatch_event(&event);
}

/// The breadcrumb bar for the current view.
#[component]
pub fn BreadcrumbBar() -> Element {
    let mut nav = use_nav();
    let workspace = use_workspace();

    let mode = *nav.view_mode.read();
    let vault = if nav.vault_name.read().is_empty() {
        "Vault".to_string()
    } else {
        nav.vault_name.read().clone()
    };

    let mut crumbs = vec![Breadcrumb {
        label: vault,
        on_click: Some(EventHandler::new(move |_| {
            nav.view_mode.set(ViewMode::Dashboard);
        })),
    }];

    // Editor view shows folder hierarchy + current note.
    if mode == ViewMode::Editor {
        let active = workspace.active_path.read().clone();
        if let Some(path) = active {
            let (folder, note_name) = match path.rfind('/') {
                Some(i) => (&path[..i], &path[i + 1..]),
                None => ("", path.as_str()),
            };
            let note_display =
                note_name.trim_end_matches(".md").to_string();
            if folder.is_empty() {
                let path_clone = path.clone();
                let ws = workspace;
                crumbs.push(Breadcrumb {
                    label: note_display,
                    on_click: Some(EventHandler::new(move |_| {
                        crate::components::contexts::activate_tab(ws, &path_clone);
                    })),
                });
            } else {
                // Folder hierarchy (each level clickable → reveal in sidebar).
                let mut acc = String::new();
                for part in folder.split('/') {
                    if acc.is_empty() {
                        acc = part.to_string();
                    } else {
                        acc = format!("{acc}/{part}");
                    }
                    let folder_path = acc.clone();
                    crumbs.push(Breadcrumb {
                        label: part.to_string(),
                        on_click: Some(EventHandler::new(move |_| {
                            reveal_in_sidebar(folder_path.clone());
                        })),
                    });
                }
                let path_clone = path.clone();
                let ws = workspace;
                crumbs.push(Breadcrumb {
                    label: note_display,
                    on_click: Some(EventHandler::new(move |_| {
                        crate::components::contexts::activate_tab(ws, &path_clone);
                    })),
                });
            }
        } else {
            crumbs.push(Breadcrumb {
                label: "Home".to_string(),
                on_click: None,
            });
        }
    } else {
        crumbs.push(Breadcrumb {
            label: view_mode_label(mode).to_string(),
            on_click: None,
        });
    }

    rsx! {
        Breadcrumbs { items: crumbs }
    }
}
