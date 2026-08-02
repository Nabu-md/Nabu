//! # Breadcrumb navigation
//!
//! Shows the user's current location — vault ▸ folder hierarchy ▸ current
//! note — and lets them click any level to navigate:
//!
//! - vault crumb → Dashboard
//! - folder crumb → reveals that folder in the sidebar file tree
//! - note crumb → activates the note's tab

use crate::components::navigation::state::{use_nav, view_mode_label};
use crate::components::ui::nav::{Breadcrumb, Breadcrumbs};
use crate::components::workspace::use_workspace;
use leptos::prelude::*;

/// Dispatches a `nabu:reveal-note` window event so the file tree reveals a
/// path (expands its parent folders and selects it).
fn reveal_in_sidebar(path: String) {
    let Some(window) = web_sys::window() else { return };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(&path));
    let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("nabu:reveal-note", &init)
    else {
        return;
    };
    let _ = window.dispatch_event(&event);
}

/// The breadcrumb bar for the current view.
#[component]
pub fn BreadcrumbBar() -> impl IntoView {
    let nav = use_nav();
    let workspace = use_workspace();

    // `Breadcrumb` wraps a `Callback`, which is not `PartialEq`, so use
    // `Signal::derive` rather than `Memo::new`.
    let crumbs = Signal::derive(move || {
        let mode = nav.view_mode.get();
        let vault = if nav.vault_name.get().is_empty() {
            "Vault".to_string()
        } else {
            nav.vault_name.get()
        };
        let mut items = vec![Breadcrumb::new(
            vault,
            Some(Callback::new(move |_| {
                nav.view_mode
                    .set(crate::components::navigation::state::ViewMode::Dashboard);
            })),
        )];

        // Editor view shows folder hierarchy + current note.
        if mode == crate::components::navigation::state::ViewMode::Editor {
            let active = workspace.active_path.get();
            if let Some(path) = active {
                let (folder, note_name) = match path.rfind('/') {
                    Some(i) => (&path[..i], &path[i + 1..]),
                    None => ("", path.as_str()),
                };
                let note_display = note_name.trim_end_matches(".md").to_string();
                if folder.is_empty() {
                    items.push(Breadcrumb::new(
                        note_display,
                        Some(Callback::new(move |_| {
                            crate::components::workspace::activate_tab(workspace, &path);
                        })),
                    ));
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
                        items.push(Breadcrumb::new(
                            part.to_string(),
                            Some(Callback::new(move |_| reveal_in_sidebar(folder_path.clone()))),
                        ));
                    }
                    items.push(Breadcrumb::new(
                        note_display,
                        Some(Callback::new(move |_| {
                            crate::components::workspace::activate_tab(workspace, &path);
                        })),
                    ));
                }
            } else {
                items.push(Breadcrumb::new("Home".to_string(), None));
            }
        } else {
            items.push(Breadcrumb::new(view_mode_label(mode).to_string(), None));
        }
        items
    });

    view! {
        <Breadcrumbs items=crumbs.get() />
    }
}
