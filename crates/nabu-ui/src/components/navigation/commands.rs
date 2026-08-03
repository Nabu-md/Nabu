//! # Command catalog
//!
//! The single source of truth for every command the Command Palette can run.
//! Each entry carries a stable id (used for recent/favourite tracking), a
//! label, an optional alias, a category, a human description and a keyboard
//! hint. The [`CommandContext`] bundles the shared contexts the command
//! closures need; [`all_commands`] builds the full catalog.

use crate::components::navigation::state::{NavContext, ViewMode};
use crate::components::ui::feedback::ToastContext;
use crate::components::workspace::WorkspaceContext;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// One command the palette can execute.
#[derive(Clone)]
pub struct AppCommand {
    /// Stable id — used for recent / favourite command tracking.
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Secondary search terms (e.g. "go to", abbreviations).
    pub aliases: &'static [&'static str],
    /// Grouping category.
    pub category: &'static str,
    /// One-line description shown under the label.
    pub description: &'static str,
    /// Keyboard shortcut hint shown on the right (display only).
    pub shortcut: Option<&'static str>,
    /// Icon / emoji prefix.
    pub icon: &'static str,
    /// Executes the command.
    pub run: Callback<()>,
}

/// Everything a command closure needs, captured at render time.
#[derive(Clone, Copy)]
pub struct CommandContext {
    pub nav: NavContext,
    pub workspace: WorkspaceContext,
    pub toasts: ToastContext,
}

fn set_view(nav: NavContext, mode: ViewMode) -> Callback<()> {
    Callback::new(move |_| nav.view_mode.set(mode))
}

fn toggle_sidebar(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.show_left_sidebar.update(|v| *v = !*v);
    })
}

fn toggle_inspector(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.show_right_inspector.update(|v| *v = !*v);
    })
}

fn open_palette(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.palette_open.set(false);
        nav.switcher_open.set(false);
        nav.shortcuts_open.set(false);
        nav.palette_open.set(true);
    })
}

fn open_switcher(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.palette_open.set(false);
        nav.switcher_open.set(false);
        nav.shortcuts_open.set(false);
        nav.switcher_open.set(true);
    })
}

fn open_shortcuts(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.palette_open.set(false);
        nav.switcher_open.set(false);
        nav.shortcuts_open.set(false);
        nav.shortcuts_open.set(true);
    })
}

fn open_search(nav: NavContext) -> Callback<()> {
    Callback::new(move |_| {
        nav.search_query.set(String::new());
        nav.view_mode.set(ViewMode::Search);
    })
}

/// Shared "create a note in the vault root and open it" action. Used by the
/// Command Palette, the Dashboard / Home quick actions and the ⌘N shortcut so
/// the logic lives in exactly one place.
pub fn create_new_note(workspace: WorkspaceContext, toasts: ToastContext) -> Callback<()> {
    Callback::new(move |_| {
        let ws = workspace;
        let toasts = toasts;
        spawn_local(async move {
            let name = format!("note-{}.md", js_sys::Date::new_0().get_time() as u64);
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": name.clone() }))
                .unwrap();
            let result = crate::ipc::tauri_invoke("note_create_file", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                crate::components::workspace::open_tab(ws, &name);
                crate::components::workspace::refresh_tree(ws);
                toasts.success("Note created", name);
            } else {
                toasts.error("Create note", "Could not create that note");
            }
        });
    })
}

/// Shared "quick capture into the Inbox" action — creates a pending
/// KnowledgeObject without touching the filesystem, then lands on the Inbox
/// screen so the user can review it. Contexts are captured at render time and
/// threaded into the async task as plain values (never `use_nav` inside a
/// `spawn_local` future — no reactive owner on the failure path).
pub fn quick_capture(nav: NavContext, toasts: ToastContext) -> Callback<()> {
    Callback::new(move |_| {
        let nav = nav;
        let toasts = toasts;
        spawn_local(async move {
            let stamp = js_sys::Date::new_0();
            let month = stamp.get_month() + 1;
            let day = stamp.get_date();
            let hours = stamp.get_hours();
            let minutes = stamp.get_minutes();
            let meridiem = if hours < 12 { "AM" } else { "PM" };
            let hour12 = if hours % 12 == 0 { 12 } else { hours % 12 };
            let title = format!(
                "Capture — {} {:02}:{:02} {}",
                month, day, hour12, minutes, meridiem
            );
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "title": title,
                "content": "",
            }))
            .unwrap();
            let result = crate::ipc::tauri_invoke("inbox_quick_capture", args).await;
            if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                nav.view_mode.set(ViewMode::Inbox);
                toasts.success("Captured", "Added to your Inbox for review.");
            } else {
                toasts.error("Quick capture", "Could not capture that note");
            }
        });
    })
}

/// Shared "reveal the vault folder in the OS file manager" action.
pub fn open_vault_folder() -> Callback<()> {
    Callback::new(move |_| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": "" })).unwrap();
            let _ = crate::ipc::tauri_invoke("reveal_in_file_manager", args).await;
        });
    })
}

/// Shared "open (or create) today's dated note" action. See
/// [`create_new_note`] for the deduplication rationale.
pub fn open_daily_note(workspace: WorkspaceContext, toasts: ToastContext) -> Callback<()> {
    Callback::new(move |_| {
        let ws = workspace;
        let toasts = toasts;
        spawn_local(async move {
            let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("note_daily", empty).await;
            if let Ok(path) = serde_wasm_bindgen::from_value::<String>(result) {
                crate::components::workspace::open_tab(ws, &path);
                toasts.success("Daily note", format!("Opened {path}"));
            }
        });
    })
}

/// Builds the full command catalog. Call at render time with the shared
/// contexts so closures capture them by value.
pub fn all_commands(ctx: CommandContext) -> Vec<AppCommand> {
    let nav = ctx.nav;
    let ws = ctx.workspace;
    let toasts = ctx.toasts;

    vec![
        // ── Navigation ─────────────────────────────────────────────
        AppCommand {
            id: "nav.dashboard",
            label: "Go to Dashboard",
            aliases: &["home", "start"],
            category: "Navigation",
            description: "Open the home dashboard",
            shortcut: Some("⌘1"),
            icon: "🏠",
            run: set_view(nav, ViewMode::Dashboard),
        },
        AppCommand {
            id: "nav.editor",
            label: "Go to Editor",
            aliases: &["note", "write"],
            category: "Navigation",
            description: "Open the note editor",
            shortcut: Some("⌘2"),
            icon: "📝",
            run: set_view(nav, ViewMode::Editor),
        },
        AppCommand {
            id: "nav.graph",
            label: "Go to Graph",
            aliases: &["canvas", "links", "network"],
            category: "Navigation",
            description: "Open the knowledge graph view",
            shortcut: Some("⌘3"),
            icon: "🕸️",
            run: set_view(nav, ViewMode::Graph),
        },
        AppCommand {
            id: "nav.inbox",
            label: "Go to Inbox",
            aliases: &["capture"],
            category: "Navigation",
            description: "Review captured knowledge",
            shortcut: None,
            icon: "📥",
            run: set_view(nav, ViewMode::Inbox),
        },
        AppCommand {
            id: "nav.reading_queue",
            label: "Go to Reading Queue",
            aliases: &["read later", "queue"],
            category: "Navigation",
            description: "Open the reading queue",
            shortcut: None,
            icon: "📚",
            run: set_view(nav, ViewMode::ReadingQueue),
        },
        AppCommand {
            id: "nav.templates",
            label: "Go to Templates",
            aliases: &["template manager"],
            category: "Navigation",
            description: "Manage note templates",
            shortcut: None,
            icon: "📋",
            run: set_view(nav, ViewMode::Templates),
        },
        AppCommand {
            id: "nav.trash",
            label: "Go to Trash",
            aliases: &["deleted", "recycle bin"],
            category: "Navigation",
            description: "Restore or permanently delete notes",
            shortcut: None,
            icon: "🗑️",
            run: set_view(nav, ViewMode::Trash),
        },
        AppCommand {
            id: "nav.history",
            label: "Go to Version History",
            aliases: &["versions", "snapshots"],
            category: "Navigation",
            description: "Browse note snapshots",
            shortcut: None,
            icon: "🕘",
            run: set_view(nav, ViewMode::History),
        },
        AppCommand {
            id: "nav.recovery",
            label: "Go to Recovery Manager",
            aliases: &["restore", "session"],
            category: "Navigation",
            description: "Inspect and restore saved sessions",
            shortcut: None,
            icon: "🛟",
            run: set_view(nav, ViewMode::Recovery),
        },
        AppCommand {
            id: "nav.calendar",
            label: "Go to Calendar",
            aliases: &["dates", "journal", "daily notes"],
            category: "Navigation",
            description: "Browse notes by date",
            shortcut: None,
            icon: "📅",
            run: set_view(nav, ViewMode::Calendar),
        },
        AppCommand {
            id: "nav.archive",
            label: "Go to Archive",
            aliases: &["archived", "stored"],
            category: "Navigation",
            description: "Restore archived notes",
            shortcut: None,
            icon: "🗃️",
            run: set_view(nav, ViewMode::Archive),
        },
        AppCommand {
            id: "nav.smart_folders",
            label: "Go to Smart Folders",
            aliases: &["virtual folders", "collections", "queries"],
            category: "Navigation",
            description: "Manage query-powered folders",
            shortcut: None,
            icon: "🗂️",
            run: set_view(nav, ViewMode::SmartFolders),
        },
        AppCommand {
            id: "nav.settings",
            label: "Open Settings",
            aliases: &["preferences", "options"],
            category: "Navigation",
            description: "Configure Nabu",
            shortcut: Some("⌘,"),
            icon: "⚙️",
            run: set_view(nav, ViewMode::Settings),
        },
        AppCommand {
            id: "capture.quick",
            label: "Quick Capture to Inbox",
            aliases: &["capture", "clip", "inbox note"],
            category: "Capture",
            description: "Add a pending note to the Inbox for review",
            shortcut: None,
            icon: "⚡",
            run: quick_capture(ctx.nav, ctx.toasts),
        },
        AppCommand {
            id: "nav.search",
            label: "Search all notes",
            aliases: &["find", "full-text", "search"],
            category: "Navigation",
            description: "Open the full-text search page",
            shortcut: Some("⌘⇧F"),
            icon: "🔍",
            run: open_search(nav),
        },
        AppCommand {
            id: "nav.palette",
            label: "Open Command Palette",
            aliases: &["commands", "⌘k", "palette"],
            category: "Navigation",
            description: "Run any command by name",
            shortcut: Some("⌘K"),
            icon: "⌘",
            run: open_palette(nav),
        },
        AppCommand {
            id: "nav.quick_switcher",
            label: "Open Quick Switcher",
            aliases: &["goto note", "switch", "⌘p"],
            category: "Navigation",
            description: "Jump to any note by name",
            shortcut: Some("⌘P"),
            icon: "⚡",
            run: open_switcher(nav),
        },
        AppCommand {
            id: "nav.shortcuts",
            label: "Keyboard Shortcuts Reference",
            aliases: &["hotkeys", "keybindings", "help"],
            category: "Navigation",
            description: "Browse every keyboard shortcut",
            shortcut: None,
            icon: "⌨️",
            run: open_shortcuts(nav),
        },
        // ── Notes ──────────────────────────────────────────────────
        AppCommand {
            id: "note.new",
            label: "Create New Note",
            aliases: &["add note", "new note"],
            category: "Notes",
            description: "Create a note in the vault root and open it",
            shortcut: Some("⌘N"),
            icon: "➕",
            run: create_new_note(ws, toasts),
        },
        AppCommand {
            id: "note.daily",
            label: "Open Daily Note",
            aliases: &["today", "journal"],
            category: "Notes",
            description: "Open (or create) today's dated note",
            shortcut: None,
            icon: "📅",
            run: open_daily_note(ws, toasts),
        },
        // ── View ───────────────────────────────────────────────────
        AppCommand {
            id: "view.sidebar",
            label: "Toggle Left Sidebar",
            aliases: &["explorer", "file tree", "panel"],
            category: "View",
            description: "Show or hide the file explorer",
            shortcut: Some("⌘\\"),
            icon: "📁",
            run: toggle_sidebar(nav),
        },
        AppCommand {
            id: "view.inspector",
            label: "Toggle Right Inspector",
            aliases: &["properties", "backlinks", "panel"],
            category: "View",
            description: "Show or hide the inspector sidebar",
            shortcut: Some("⌘⇧\\"),
            icon: "📋",
            run: toggle_inspector(nav),
        },
        // ── Command palette utilities (discoverable but hidden from
        //    the default list so the palette stays focused).
    ]
}

/// Subset of commands shown while the palette query is empty — the recent
/// commands + favourites, resolved from the full catalog.
pub fn resolve_commands_by_id<'a>(catalog: &'a [AppCommand], ids: &[String]) -> Vec<AppCommand> {
    ids.iter()
        .filter_map(|id| catalog.iter().find(|c| c.id == id).cloned())
        .collect()
}
