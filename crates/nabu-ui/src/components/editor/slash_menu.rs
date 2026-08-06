//! Slash menu — block-type picker invoked with `/` inside the editor.
//!
//! Dioxus port of the LePtOS `slash_menu.rs`. Renders a small floating list
//! of block types (headings, Kanban board, Vision OCR scan, code/sandbox,
//! callout box). Selecting an item calls `on_select` and closes the menu.

use crate::components::ui::menu::MenuItem;
use dioxus::prelude::*;

/// The slash menu popup.
///
/// `on_select` receives the chosen label (e.g. `"# Heading 1"`) so the editor
/// can insert the corresponding markdown at the cursor.
#[component]
pub fn SlashMenu(on_select: EventHandler<String>) -> Element {
    // Phase 0.4a: same item set as the LePtOS implementation; no redesign.
    let items: Vec<String> = vec![
        "# Heading 1".to_string(),
        "## Heading 2".to_string(),
        "### Heading 3".to_string(),
        "📋 Kanban Board".to_string(),
        "📷 Vision OCR Scan".to_string(),
        "📦 Code Block / Sandbox".to_string(),
        "💡 Callout Box".to_string(),
    ];

    let cb = on_select;

    rsx! {
        div {
            class: "absolute bg-gray-800 border border-gray-700 rounded shadow-lg z-10 w-48 py-1",
            role: "menu",
            "aria-label": "Slash menu",
        }
        for item in items {
            {
                let label = item.clone();
                rsx! {
                    MenuItem {
                        label: label.clone(),
                        on_select: move |_| cb.call(label.clone()),
                    }
                }
            }
        }
    }
}
