//! # Keyboard shortcuts — registry, reference dialog, global listener
//!
//! A single registry of every shortcut the app binds, used both to *install*
//! the global window listener and to render the searchable shortcuts
//! reference dialog (single source of truth).
//!
//! The global listener is installed once per app mount and removed on cleanup.
//! Shortcuts that type inside editors/inputs are deliberately skipped so
//! native editing behaviour is preserved (the editor's own keydown handlers
//! own Cmd+B/I etc.).

use crate::components::contexts::{open_tab, activate_tab, WorkspaceContext, use_workspace};
use crate::components::navigation::commands::{create_new_note, open_daily_note};
use crate::components::navigation::state::{use_nav, ViewMode};
use crate::components::ui::feedback::{set_timeout, use_toast, ToastContext};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;
use wasm_bindgen::prelude::JsCast;

// ── Shortcut catalog ──────────────────────────────────────────────

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
/// dialog. Global bindings are handled in [`install_global_shortcuts`].
pub const SHORTCUTS: &[Shortcut] = &[
    // ── Command palette / navigation ──────────────────────────────
    Shortcut { category: "Navigation", keys: "⌘K", description: "Open the command palette" },
    Shortcut { category: "Navigation", keys: "⌘P", description: "Open the quick switcher" },
    Shortcut { category: "Navigation", keys: "⌘⇧F", description: "Open full-text search" },
    Shortcut { category: "Navigation", keys: "⌘1", description: "Go to Dashboard" },
    Shortcut { category: "Navigation", keys: "⌘2", description: "Go to Editor" },
    Shortcut { category: "Navigation", keys: "⌘3", description: "Go to Graph" },
    Shortcut { category: "Navigation", keys: "⌘,", description: "Open Settings" },
    Shortcut { category: "Navigation", keys: "⌘⇧C", description: "Open Canvas" },
    Shortcut { category: "Navigation", keys: "⌘⇧1", description: "Open Reader Mode" },
    Shortcut { category: "Navigation", keys: "⌘⇧M", description: "Open Comparison View" },
    Shortcut { category: "Navigation", keys: "⌘⇧S", description: "Open Statistics" },
    Shortcut { category: "Navigation", keys: "⌘⇧?", description: "Open shortcuts reference" },
    Shortcut { category: "Navigation", keys: "Esc", description: "Close any overlay / palette" },
    // ── Note management ───────────────────────────────────────────
    Shortcut { category: "Notes", keys: "⌘N", description: "Create a new note" },
    Shortcut { category: "Notes", keys: "⌘⇧D", description: "Open the daily note" },
    // ── Workspace / panels ────────────────────────────────────────
    Shortcut { category: "Workspace", keys: "⌘\\", description: "Toggle the left sidebar" },
    Shortcut { category: "Workspace", keys: "⌘⇧\\", description: "Toggle the right inspector" },
    Shortcut { category: "Workspace", keys: "⌘Z", description: "Undo" },
    Shortcut { category: "Workspace", keys: "⌘⇧Z / Ctrl+Y", description: "Redo" },
    // ── Editor ────────────────────────────────────────────────────
    Shortcut { category: "Editor", keys: "⌘B", description: "Bold selection (markdown ** **)" },
    Shortcut { category: "Editor", keys: "⌘I", description: "Italic selection (markdown * *)" },
    Shortcut { category: "Editor", keys: "/", description: "Open the slash (block) menu" },
];

// ── Helpers ───────────────────────────────────────────────────────

/// Returns `true` when keyboard focus is inside an editable element so
/// global shortcuts never hijack typing.
fn focus_is_editable() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Some(document) = window.document() else { return false };
    let Some(active) = document.active_element() else { return false };
    let tag = active.tag_name().to_ascii_lowercase();
    if tag == "textarea" || tag == "input" || tag == "select" {
        return true;
    }
    active
        .get_attribute("contenteditable")
        .map_or(false, |v| !v.eq_ignore_ascii_case("false"))
}

// ── Global listener installation ──────────────────────────────────

/// Installs the global keyboard shortcuts for the app shell. Called once;
/// a static guard prevents double-registration.
pub fn install_global_shortcuts(
    mut nav: crate::components::contexts::NavContext,
    workspace: WorkspaceContext,
    toasts: ToastContext,
) {
    static INSTALLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    if INSTALLED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    let Some(window) = web_sys::window() else { return };

    let handler = Closure::<dyn Fn(web_sys::KeyboardEvent)>::wrap(Box::new(
        move |ev: web_sys::KeyboardEvent| {
            let meta = ev.meta_key() || ev.ctrl_key();
            let shift = ev.shift_key();
            let key = ev.key();

            // Overlay-open: only palette/quick-switcher toggles still apply.
            if *nav.palette_open.read() || *nav.switcher_open.read() || *nav.shortcuts_open.read() {
                if meta && !shift && key.eq_ignore_ascii_case("k") {
                    ev.prevent_default();
                    nav.palette_open.set(!*nav.palette_open.read());
                    return;
                }
                if meta && !shift && key.eq_ignore_ascii_case("p") {
                    ev.prevent_default();
                    nav.switcher_open.set(!*nav.switcher_open.read());
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
                let cmd = create_new_note(workspace, toasts);
                cmd.call(());
            } else if meta && shift && key.eq_ignore_ascii_case("d") {
                ev.prevent_default();
                let cmd = open_daily_note(workspace, toasts);
                cmd.call(());
            } else if meta && !shift && key == "\\" {
                ev.prevent_default();
                nav.show_left_sidebar.with_mut(|v| *v = !*v);
            } else if meta && shift && key == "\\" {
                ev.prevent_default();
                nav.show_right_inspector.with_mut(|v| *v = !*v);
            } else if meta && !shift && key == "," {
                ev.prevent_default();
                nav.view_mode.set(ViewMode::Settings);
            } else if meta && !shift {
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
        },
    ));

    let _ = window.add_event_listener_with_callback(
        "keydown",
        handler.as_ref().unchecked_ref(),
    );
    std::mem::forget(handler);
}

/// Component that installs global keyboard shortcuts on mount.  Renders no
/// output.  Must be placed inside all context providers.
#[component]
pub fn KeyboardShortcuts() -> Element {
    let nav = use_nav();
    let workspace = use_workspace();
    let toasts = use_toast();

    // One-time mount effect — signal-guarded so re-renders don't re-install.
    let mut installed = use_signal(|| false);
    if !*installed.read() {
        installed.set(true);
        install_global_shortcuts(nav, workspace, toasts);
    }

    rsx! {}
}

// ── Shortcuts reference dialog ────────────────────────────────────

/// The searchable shortcuts reference dialog. Rendered once at the app root.
#[component]
pub fn ShortcutReference() -> Element {
    let mut nav = use_nav();
    let open = nav.shortcuts_open;
    let mut query = use_signal(|| String::new());

    // Focus the input + reset state whenever the dialog opens.
    use_effect(move || {
        if *open.read() {
            query.set(String::new());
            let open_signal = open;
            set_timeout(move || {
                if *open_signal.read() {
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            if let Some(input) = document.get_element_by_id("shortcuts-input") {
                                if let Some(input) =
                                    input.dyn_ref::<web_sys::HtmlInputElement>()
                                {
                                    let _ = input.focus();
                                }
                            }
                        }
                    }
                }
            }, 10);
        }
    });

    // Group shortcuts by category, filtered by the query.
    let groups: Vec<(Vec<&'static Shortcut>, &'static str)> = {
        let q = query.read().trim().to_lowercase();
        let mut buckets: Vec<(&'static str, Vec<&'static Shortcut>)> = Vec::new();
        for shortcut in SHORTCUTS {
            if !q.is_empty()
                && !shortcut.keys.to_lowercase().contains(&q)
                && !shortcut.description.to_lowercase().contains(&q)
                && !shortcut.category.to_lowercase().contains(&q)
            {
                continue;
            }
            match buckets.iter_mut().find(|(cat, _)| *cat == shortcut.category) {
                Some((_, list)) => list.push(shortcut),
                None => buckets.push((shortcut.category, vec![shortcut])),
            }
        }
        buckets.into_iter().map(|(cat, items)| (items, cat)).collect()
    };

    let close_handler = move |_| {
        nav.palette_open.set(false);
        nav.switcher_open.set(false);
        nav.shortcuts_open.set(false);
    };

    rsx! {
        if *open.read() {
            div {
                class: "dialog-overlay",
                onclick: move |_| close_handler(()),
                div {
                    class: "shortcut-dialog panel",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": "Keyboard shortcuts",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    onkeydown: move |ev: KeyboardEvent| {
                        if ev.key() == Key::Escape {
                            close_handler(());
                        }
                    },
                    div { class: "shortcut-dialog-header" }
                    h2 { class: "shortcut-dialog-title" }
                    {render_icon_view(Icon::Keyboard)}
                    " Keyboard Shortcuts"
                    button {
                        r#type: "button",
                        class: "dialog-close",
                        "aria-label": "Close",
                        onclick: move |_| close_handler(()),
                        {render_icon_view(Icon::X)}
                    }
                    div { class: "shortcut-dialog-search" }
                    input {
                        id: "shortcuts-input",
                        class: "input",
                        r#type: "text",
                        placeholder: "Search shortcuts…",
                        value: "{*query.read()}",
                        onchange: move |ev: FormEvent| {
                            query.set(ev.value());
                        },
                        onkeydown: move |ev: KeyboardEvent| {
                            if ev.key() == Key::Escape {
                                close_handler(());
                            }
                        },
                    }
                    div { class: "shortcut-dialog-body" }
                    if groups.is_empty() {
                        div { class: "palette-empty", "No shortcuts match" }
                    } else {
                        for (list, category) in groups {
                            div { class: "shortcut-group" }
                            div { class: "palette-category", "{category}" }
                            for shortcut in list {
                                div { class: "shortcut-row" }
                                span { class: "shortcut-desc", "{shortcut.description}" }
                                kbd { class: "shortcut-keys", "{shortcut.keys}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
