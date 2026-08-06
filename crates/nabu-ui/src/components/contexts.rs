//! # Dioxus Context Providers
//!
//! This module defines the context types and provider components that wire
//! shared application state through the component tree. Each provider wraps its
//! children and calls `provide_context` so descendant components can retrieve
//! the context with `use_context::<T>()`.
//!
//! Phase 0 provides minimal stubs for all contexts. Toast and task contexts
//! live in [`crate::components::ui::feedback`] alongside their presentation
//! components. The remaining contexts (theme, history, save status, workspace,
//! navigation) are provided here at the app root.
//!
//! Navigation state ([`ViewMode`], [`NavContext`]) and the full set of
//! navigation-state helpers live in [`crate::components::navigation::state`] and
//! are re-exported from here so existing call sites keep working.

use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

// ── Re-exports from navigation::state ───────────────────────────

// Forward every navigation-state symbol so `use crate::components::contexts::*`
// gives callers everything they need (ViewMode, NavContext, NavProvider, …).
pub use crate::components::navigation::state::{
    dashboard_section_label, fuzzy_score, load_all_nav_state, load_notes_index,
    parse_view_mode, record_recent_note, record_recent_search, remove_saved_search,
    save_search, set_dashboard_sections, toggle_favourite, toggle_favourite_command,
    NoteIndexEntry, NavContext, NavProvider, SavedSearch, ViewMode, DASHBOARD_SECTIONS,
    use_nav,
};

// ── Theme ──────────────────────────────────────────────────────

/// Provider that initialises the theme context (via [`crate::provide_theme`])
/// and renders children.  The theme signal persists across re-renders because
/// `provide_theme` uses `use_signal`.
#[component]
pub fn ThemeProvider(
    children: Element,
    initial_theme: String,
) -> Element {
    crate::provide_theme(initial_theme);
    rsx! { {children} }
}

// ── History (undo / redo) ─────────────────────────────────────────

/// Provider component for undo/redo history.
///
/// Wires up the [`crate::history::HistoryContext`], refreshes the initial
/// backend state on mount, and installs the global undo/redo keyboard
/// shortcuts (Cmd/Ctrl+Z / Shift+Z / Ctrl+Y) exactly once for the lifetime of
/// the app window.
#[component]
pub fn HistoryProvider(children: Element) -> Element {
    let can_undo = use_signal(|| false);
    let can_redo = use_signal(|| false);
    let toasts = crate::components::ui::feedback::use_toast();
    let history = crate::history::HistoryContext {
        can_undo,
        can_redo,
    };
    provide_context(history);

    // One-time mount effects (signal-guarded so re-renders don't re-fire).
    let mut initialized = use_signal(|| false);
    if !*initialized.read() {
        initialized.set(true);
        crate::history::refresh_history_state(history);
        crate::history::install_undo_shortcuts(history, toasts);
    }

    rsx! { {children} }
}

/// Retrieves the history context.
pub fn use_history() -> crate::history::HistoryContext {
    use_context::<crate::history::HistoryContext>()
}

// ── Save Status ──────────────────────────────────────────────────

/// Current save state of the active document.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SaveStatusType {
    Unsaved,
    Saving,
    Saved,
    Error,
}

/// Shared save-status context.
#[derive(Clone, Copy)]
pub struct SaveStatusContext {
    pub status: Signal<SaveStatusType>,
    pub last_saved: Signal<Option<String>>, // RFC 3339 timestamp
}

/// Retrieves the save-status context.
pub fn use_save_status() -> SaveStatusContext {
    use_context::<SaveStatusContext>()
}

/// Provider component for save-status tracking.
#[component]
pub fn SaveStatusProvider(children: Element) -> Element {
    provide_context(SaveStatusContext {
        status: use_signal(|| SaveStatusType::Saved),
        last_saved: use_signal(|| None),
    });
    rsx! { {children} }
}

/// Save-status indicator shown in the navbar.  Reads the shared
/// [`SaveStatusContext`] and renders a coloured dot + label.
#[component]
pub fn SaveStatusIndicator() -> Element {
    let ctx = use_save_status();
    let status = ctx.status;
    let label = move || match *status.read() {
        SaveStatusType::Unsaved => "Unsaved",
        SaveStatusType::Saving => "Saving…",
        SaveStatusType::Saved => "Saved",
        SaveStatusType::Error => "Error",
    };
    let dot_class = move || match *status.read() {
        SaveStatusType::Unsaved => "save-dot-idle",
        SaveStatusType::Saving => "save-dot-saving",
        SaveStatusType::Saved => "save-dot-saved",
        SaveStatusType::Error => "save-dot-failed",
    };
    rsx! {
        span { class: "save-status", role: "status", "aria-label": "Save status: {label()}" }
        span { class: "save-dot {dot_class()}", "aria-hidden": "true" }
        span { class: "save-status-label", "{label()}" }
    }
}

// ── Notification Bell ────────────────────────────────────────────

/// Compact notification bell shown in the navbar.  Reads the shared
/// [`crate::components::ui::feedback::ToastContext`] for outstanding toasts.
#[component]
pub fn NotificationBell() -> Element {
    let toasts = crate::components::ui::feedback::use_toast();
    let count = toasts.toasts.read().len();
    rsx! {
        button {
            r#type: "button",
            class: "navbar-action",
            title: "Notifications",
            "aria-label": "Notifications",
            "aria-haspopup": "true",
        }
        if count > 0 {
            {render_icon_view(Icon::BellRing)}
            span { class: "relative", "aria-hidden": "true" }
            span {
                class: "absolute -top-1 -right-1 flex h-3 w-3",
                span { class: "inline-flex items-center justify-center rounded-full bg-red-500 text-xs text-white" }
                "{count}"
            }
        } else {
            {render_icon_view(Icon::Bell)}
        }
    }
}

// ── Workspace (tabs) ───────────────────────────────────────────────

/// One open note tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenTab {
    /// Vault-relative path of the note.
    pub path: String,
    /// Display title (derived from the file name).
    pub title: String,
    /// Pinned tabs survive "Close Others" / "Close All".
    pub pinned: bool,
}

impl OpenTab {
    pub fn new(path: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            title: title.into(),
            pinned: false,
        }
    }
}

/// Derives a tab title from a vault-relative path (the file name without `.md`).
pub fn title_from_path(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_string()
}

/// Opens a note in a tab: adds the tab if missing and makes it active.
pub fn open_tab(mut ctx: WorkspaceContext, path: &str) {
    let title = title_from_path(path);
    ctx.tabs.with_mut(|tabs| {
        if !tabs.iter().any(|t| t.path == path) {
            tabs.push(OpenTab {
                path: path.to_string(),
                title: title.clone(),
                pinned: false,
            });
        }
    });
    ctx.active_path.set(Some(path.to_string()));
}

/// Makes an already-open tab active without adding it again.
pub fn activate_tab(mut ctx: WorkspaceContext, path: &str) {
    ctx.active_path.set(Some(path.to_string()));
}

/// Closes a tab. When the active tab closes, activates the neighbouring tab.
pub fn close_tab(mut ctx: WorkspaceContext, path: &str) {
    let was_active = ctx.active_path.read().as_deref() == Some(path);
    let (remaining, closed_index) = {
        let tabs = ctx.tabs.read();
        let idx = tabs.iter().position(|t| t.path == path);
        let remaining = tabs
            .iter()
            .filter(|t| t.path != path)
            .cloned()
            .collect::<Vec<_>>();
        (remaining, idx)
    };
    ctx.tabs.set(remaining.clone());
    if was_active {
        let next = closed_index
            .and_then(|i| {
                remaining
                    .get(i)
                    .or_else(|| remaining.get(i.saturating_sub(1)))
                    .map(|t| t.path.clone())
            });
        ctx.active_path.set(next);
    }
}

/// Closes every tab except `keep` (and pinned tabs).
pub fn close_others(mut ctx: WorkspaceContext, keep: &str) {
    ctx.tabs.with_mut(|tabs| {
        tabs.retain(|t| t.path == keep || t.pinned);
    });
    ctx.active_path.set(Some(keep.to_string()));
}

/// Closes all unpinned tabs.
pub fn close_all(mut ctx: WorkspaceContext) {
    ctx.tabs.with_mut(|tabs| {
        tabs.retain(|t| t.pinned);
    });
    ctx.active_path.set(None);
}

/// Toggles the pinned flag on a tab.
pub fn pin_tab(mut ctx: WorkspaceContext, path: &str) {
    ctx.tabs.with_mut(|tabs| {
        if let Some(tab) = tabs.iter_mut().find(|t| t.path == path) {
            tab.pinned = !tab.pinned;
        }
    });
}

/// Moves a tab from one index to another (drag-reorder).
pub fn reorder_tab(mut ctx: WorkspaceContext, from: usize, to: usize) {
    if from == to {
        return;
    }
    ctx.tabs.with_mut(|tabs| {
        if from >= tabs.len() || to >= tabs.len() {
            return;
        }
        let tab = tabs.remove(from);
        tabs.insert(to, tab);
    });
}

/// Rewrites every open tab whose path lives under `old_prefix` (the folder
/// itself, or any descendant) to the corresponding path under `new_prefix`.
/// Used when a folder is renamed or moved so tabs keep tracking the notes
/// inside it.
pub fn rename_tab_prefix(mut ctx: WorkspaceContext, old_prefix: &str, new_prefix: &str) {
    if old_prefix.is_empty() {
        return;
    }
    let old = format!("{old_prefix}/");
    let new = if new_prefix.is_empty() {
        String::new()
    } else {
        format!("{new_prefix}/")
    };
    ctx.tabs.with_mut(|tabs| {
        for tab in tabs.iter_mut() {
            if tab.path == old_prefix {
                tab.path = new_prefix.to_string();
                tab.title = title_from_path(new_prefix);
            } else if let Some(rest) = tab.path.strip_prefix(&old) {
                tab.path = format!("{new}{rest}");
                tab.title = title_from_path(&tab.path);
            }
        }
    });
    let active = ctx.active_path.read().as_ref().cloned();
    if let Some(active) = active {
        let new_active = if active == old_prefix {
            Some(new_prefix.to_string())
        } else if let Some(rest) = active.strip_prefix(&old) {
            Some(format!("{new}{rest}"))
        } else {
            None
        };
        if new_active.is_some() {
            ctx.active_path.set(new_active);
        }
    }
}

/// Requests the file tree to reload (after create / rename / delete / move /
/// duplicate). The tree watches this counter.
pub fn refresh_tree(mut ctx: WorkspaceContext) {
    ctx.refresh_tree.with_mut(|v| *v = v.wrapping_add(1));
}

/// Marks `path`'s on-disk content as changed outside the editor (e.g. a
/// mention converted to a wikilink by the right inspector / graph panel). The
/// editor watches [`WorkspaceContext::content_version`] and re-reads the file
/// so its buffer matches disk instead of overwriting the newer content on the
/// next autosave. The path is carried so only the affected note reloads.
pub fn bump_content_version(mut ctx: WorkspaceContext, path: &str) {
    let path = path.to_string();
    ctx.content_version.with_mut(|(p, v)| {
        *p = path;
        *v = v.wrapping_add(1);
    });
}

/// Shared workspace context — tracks open tabs and the active note path.
#[derive(Clone, Copy)]
pub struct WorkspaceContext {
    /// Open tabs in display order.
    pub tabs: Signal<Vec<OpenTab>>,
    /// The active note (vault-relative path).
    pub active_path: Signal<Option<String>>,
    /// Bumped by the file tree after structural mutations so the tree can
    /// reload itself without a full remount.
    pub refresh_tree: Signal<u32>,
    /// Updated whenever a note's content is changed on disk outside the editor.
    pub content_version: Signal<(String, u32)>,
}

/// Retrieves the workspace context.
pub fn use_workspace() -> WorkspaceContext {
    use_context::<WorkspaceContext>()
}

/// Provider component for workspace tab management.
#[component]
pub fn WorkspaceProvider(children: Element) -> Element {
    provide_context(WorkspaceContext {
        tabs: use_signal(Vec::<OpenTab>::new),
        active_path: use_signal(|| None::<String>),
        refresh_tree: use_signal(|| 0u32),
        content_version: use_signal(|| (String::new(), 0u32)),
    });
    rsx! { {children} }
}
