//! # Keyboard shortcuts — registry, reference dialog, global listener
//!
//! A single registry of every shortcut the app binds, used both to *install*
//! the global window listener and to render the searchable shortcuts
//! reference dialog (single source of truth).
//!
//! The global listener is installed once per app mount from [`install_global_shortcuts`]
//! and removed on cleanup. Shortcuts that type inside editors/inputs are
//! deliberately skipped so native editing behaviour is preserved (the
//! editor's own keydown handlers own Cmd+B/I etc.).

use crate::components::navigation::state::{use_nav, ViewMode};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::workspace::use_workspace;
use leptos::prelude::*;
use wasm_bindgen::prelude::JsCast;

/// One registered shortcut.
#[derive(Clone, Copy, PartialEq)]
pub struct Shortcut {
    /// Category shown in the reference dialog.
    pub category: &'static str,
    /// Display keys (e.g. "⌘K").
    pub keys: &'static str,
    /// Human description.
    pub description: &'static str,
}

/// The complete shortcut catalog — the source of truth for the reference
/// dialog. Global bindings are handled in [`install_global_shortcuts`];
/// editor-level bindings live in the editor component and are listed here
/// for discoverability.
pub const SHORTCUTS: &[Shortcut] = &[
    // ── Command palette / navigation ──────────────────────────────
    Shortcut {
        category: "Navigation",
        keys: "⌘K",
        description: "Open the command palette",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘P",
        description: "Open the quick switcher",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧F",
        description: "Open full-text search",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘1",
        description: "Go to Dashboard",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘2",
        description: "Go to Editor",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘3",
        description: "Go to Graph",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘,",
        description: "Open Settings",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧C",
        description: "Open Canvas",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧1",
        description: "Open Reader Mode",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧M",
        description: "Open Comparison View",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧S",
        description: "Open Statistics",
    },
    Shortcut {
        category: "Navigation",
        keys: "⌘⇧?",
        description: "Open shortcuts reference",
    },
    Shortcut {
        category: "Navigation",
        keys: "Esc",
        description: "Close any overlay / palette",
    },
    // ── Note management ───────────────────────────────────────────
    Shortcut {
        category: "Notes",
        keys: "⌘N",
        description: "Create a new note",
    },
    Shortcut {
        category: "Notes",
        keys: "⌘⇧D",
        description: "Open the daily note",
    },
    Shortcut {
        category: "Notes",
        keys: "⌘⇧R",
        description: "Restore selection (Trash screen)",
    },
    Shortcut {
        category: "Notes",
        keys: "⌘⇧⌫",
        description: "Empty trash (Trash screen)",
    },
    // ── Knowledge Inbox ───────────────────────────────────
    Shortcut {
        category: "Inbox",
        keys: "A",
        description: "Approve selected items",
    },
    Shortcut {
        category: "Inbox",
        keys: "R",
        description: "Reject selected items",
    },
    Shortcut {
        category: "Inbox",
        keys: "D",
        description: "Delete selected items",
    },
    Shortcut {
        category: "Inbox",
        keys: "Space",
        description: "Toggle selection of previewed item",
    },
    Shortcut {
        category: "Inbox",
        keys: "Enter",
        description: "Approve previewed item",
    },
    Shortcut {
        category: "Inbox",
        keys: "⌘A",
        description: "Select / deselect all inbox items",
    },
    Shortcut {
        category: "Inbox",
        keys: "⌘⇧F",
        description: "Open inbox search",
    },
    // ── Workspace / panels ────────────────────────────────────────
    Shortcut {
        category: "Workspace",
        keys: "⌘\\",
        description: "Toggle the left sidebar",
    },
    Shortcut {
        category: "Workspace",
        keys: "⌘⇧\\",
        description: "Toggle the right inspector",
    },
    Shortcut {
        category: "Workspace",
        keys: "⌘Z",
        description: "Undo",
    },
    Shortcut {
        category: "Workspace",
        keys: "⌘⇧Z / Ctrl+Y",
        description: "Redo",
    },
    // ── Editor ────────────────────────────────────────────────────
    Shortcut {
        category: "Editor",
        keys: "⌘B",
        description: "Bold selection (markdown ** **)",
    },
    Shortcut {
        category: "Editor",
        keys: "⌘I",
        description: "Italic selection (markdown * *)",
    },
    Shortcut {
        category: "Editor",
        keys: "/",
        description: "Open the slash (block) menu",
    },
    Shortcut {
        category: "Editor",
        keys: "Cmd/Ctrl + Z",
        description: "Undo typing (native)",
    },
];

/// Returns `true` when keyboard focus is inside an editable element so
/// global shortcuts never hijack typing.
fn focus_is_editable() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(active) = document.active_element() else {
        return false;
    };
    let tag = active.tag_name().to_ascii_lowercase();
    if tag == "textarea" || tag == "input" || tag == "select" {
        return true;
    }
    active
        .get_attribute("contenteditable")
        .map_or(false, |v| !v.eq_ignore_ascii_case("false"))
}

/// Installs the global keyboard shortcuts for the app shell. Call inside a
/// component render (so the contexts exist) and remove the returned handle on
/// cleanup.
pub fn install_global_shortcuts() -> WindowListenerHandle {
    let nav = use_nav();
    let workspace = use_workspace();
    let toasts = use_toast();

    let handle = window_event_listener_untyped("keydown", move |ev: web_sys::Event| {
        let ev = ev.unchecked_ref::<web_sys::KeyboardEvent>();
        let meta = ev.meta_key() || ev.ctrl_key();
        let shift = ev.shift_key();
        let key = ev.key();

        // Palette-style overlays own their own keys while open.
        if nav.palette_open.get() || nav.switcher_open.get() || nav.shortcuts_open.get() {
            // ⌘K / ⌘P still toggle (open/close) from anywhere.
            if meta && !shift && key.eq_ignore_ascii_case("k") {
                ev.prevent_default();
                nav.palette_open.set(!nav.palette_open.get());
                return;
            }
            if meta && !shift && key.eq_ignore_ascii_case("p") {
                ev.prevent_default();
                nav.switcher_open.set(!nav.switcher_open.get());
                return;
            }
            if !meta && key == "Escape" {
                ev.prevent_default();
                nav.palette_open.set(false);
                nav.switcher_open.set(false);
                nav.shortcuts_open.set(false);
            }
            return;
        }

        // When typing in an input/textarea, only overlay toggles apply.
        if focus_is_editable()
            && !(meta && (key.eq_ignore_ascii_case("k") || key.eq_ignore_ascii_case("p")))
        {
            return;
        }

        if meta && shift && key.eq_ignore_ascii_case("?") {
            ev.prevent_default();
            nav.shortcuts_open.set(true);
        } else if meta && shift && key.eq_ignore_ascii_case("f") {
            ev.prevent_default();
            nav.search_query.set(String::new());
            nav.view_mode.set(ViewMode::Search);
        } else if meta && !shift && key.eq_ignore_ascii_case("k") {
            ev.prevent_default();
            nav.palette_open.set(true);
        } else if meta && !shift && key.eq_ignore_ascii_case("p") {
            ev.prevent_default();
            nav.switcher_open.set(true);
        } else if meta && !shift && key.eq_ignore_ascii_case("n") {
            ev.prevent_default();
            // ⌘N creates a note — reuse the shared helper (palette + quick
            // actions use the same one). The focus guard above already let
            // meta shortcuts through, so no extra check needed here.
            crate::components::navigation::commands::create_new_note(workspace, toasts).run(());
        } else if meta && shift && key.eq_ignore_ascii_case("d") {
            ev.prevent_default();
            crate::components::navigation::commands::open_daily_note(workspace, toasts).run(());
        } else if meta && !shift && key == "\\" {
            ev.prevent_default();
            nav.show_left_sidebar.update(|v| *v = !*v);
        } else if meta && shift && key == "\\" {
            ev.prevent_default();
            nav.show_right_inspector.update(|v| *v = !*v);
        } else if meta && !shift && key == "," {
            ev.prevent_default();
            nav.view_mode.set(ViewMode::Settings);
        } else if meta && !shift {
            // ⌘1..9 → views (1=dashboard, 2=editor, 3=graph, 9=settings)
            let mode = match key.as_str() {
                "1" => Some(ViewMode::Dashboard),
                "2" => Some(ViewMode::Editor),
                "3" => Some(ViewMode::Graph),
                "9" => Some(ViewMode::Settings),
                _ => None,
            };
            if let Some(mode) = mode {
                ev.prevent_default();
                nav.view_mode.set(mode);
            }
        } else if meta && shift && key.eq_ignore_ascii_case("c") {
            ev.prevent_default();
            nav.view_mode.set(ViewMode::Canvas);
        } else if meta && shift && key == "1" {
            ev.prevent_default();
            nav.view_mode.set(ViewMode::Reader);
        } else if meta && shift && key.eq_ignore_ascii_case("m") {
            ev.prevent_default();
            nav.view_mode.set(ViewMode::Comparison);
        } else if meta && shift && key.eq_ignore_ascii_case("s") {
            ev.prevent_default();
            nav.view_mode.set(ViewMode::Statistics);
        }
    });
    handle
}

/// The searchable shortcuts reference dialog. Rendered once at the app root.
#[component]
pub fn ShortcutReference() -> impl IntoView {
    let nav = use_nav();
    let open = nav.shortcuts_open;
    let (query, set_query) = signal(String::new());
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if open.get() {
            set_query.set(String::new());
            set_timeout(
                move || {
                    if let Some(el) = input_ref.get() {
                        let _ = el.focus();
                    }
                },
                std::time::Duration::from_millis(10),
            );
        }
    });

    let close = Callback::new(move |_| open.set(false));

    // Group shortcuts by category, filtered by the query.
    let groups = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        let mut groups: Vec<(&'static str, Vec<&'static Shortcut>)> = Vec::new();
        for shortcut in SHORTCUTS {
            if !q.is_empty()
                && !shortcut.keys.to_lowercase().contains(&q)
                && !shortcut.description.to_lowercase().contains(&q)
                && !shortcut.category.to_lowercase().contains(&q)
            {
                continue;
            }
            match groups.iter_mut().find(|(cat, _)| *cat == shortcut.category) {
                Some((_, list)) => list.push(shortcut),
                None => groups.push((shortcut.category, vec![shortcut])),
            }
        }
        groups
    });

    view! {
        {move || if open.get() {
            view! {
                <div class="dialog-overlay" on:click=move |_| close.run(())>
                    <div
                        class="shortcut-dialog panel"
                        role="dialog"
                        aria-modal="true"
                        aria-label="Keyboard shortcuts"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="shortcut-dialog-header">
                            <h2 class="shortcut-dialog-title">{render_icon_view(Icon::Keyboard)} Keyboard Shortcuts</h2>
                            <button
                                type="button"
                                class="dialog-close"
                                aria-label="Close"
                                on:click=move |_| close.run(())
                            >
                                {render_icon_view(Icon::X)}
                            </button>
                        </div>
                        <div class="shortcut-dialog-search">
                            <input
                                node_ref=input_ref
                                class="input"
                                type="text"
                                placeholder="Search shortcuts…"
                                prop:value=query
                                on:input=move |ev| set_query.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Escape" {
                                        close.run(());
                                    }
                                }
                            />
                        </div>
                        <div class="shortcut-dialog-body">
                            {move || {
                                let groups = groups.get();
                                if groups.is_empty() {
                                    view! {
                                        <div class="palette-empty">"No shortcuts match"</div>
                                    }.into_any()
                                } else {
                                    groups.into_iter().map(|(category, list)| {
                                        view! {
                                            <div class="shortcut-group">
                                                <div class="palette-category">{category}</div>
                                                {list.into_iter().map(|s| view! {
                                                    <div class="shortcut-row">
                                                        <span class="shortcut-desc">{s.description}</span>
                                                        <kbd class="shortcut-keys">{s.keys}</kbd>
                                                    </div>
                                                }).collect_view()}
                                            </div>
                                        }
                                    }).collect_view().into_any()
                                }
                            }}
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
