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

use dioxus::prelude::*;

// ── Theme ──────────────────────────────────────────────────────

/// Provider that initialises the theme context (via [`crate::provide_theme`])
/// and renders children.  The theme signal persists across re-renders because
/// `provide_theme` uses `use_signal`.
#[component]
pub fn ThemeProvider(children: Element, initial_theme: String) -> Element {
    crate::provide_theme(initial_theme);
    rsx! {
        {children}
    }
}

// ── History (undo / redo) ─────────────────────────────────────────

/// One entry in the undo/redo history stack.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub action: String,
}

/// Shared history context — tracks undo/redo state.
#[derive(Clone, Copy)]
pub struct HistoryContext {
    pub entries: Signal<Vec<HistoryEntry>>,
    pub pointer: Signal<usize>,
}

impl HistoryContext {
    pub fn can_undo(&self) -> bool {
        *self.pointer.read() > 0
    }

    pub fn can_redo(&self) -> bool {
        let ptr = *self.pointer.read();
        let len = self.entries.read().len();
        ptr < len
    }

    pub fn push(&self, entry: HistoryEntry) {
        let ptr = *self.pointer.read();
        let mut entries = self.entries.write_unchecked();
        entries.truncate(ptr);
        entries.push(entry);
        drop(entries);
        *self.pointer.write_unchecked() += 1;
    }

    pub fn undo(&self) {
        if self.can_undo() {
            *self.pointer.write_unchecked() -= 1;
        }
    }

    pub fn redo(&self) {
        if self.can_redo() {
            *self.pointer.write_unchecked() += 1;
        }
    }
}

/// Convenience accessor.
pub fn use_history() -> HistoryContext {
    use_context::<HistoryContext>()
}

/// Provider component for undo/redo history.
#[component]
pub fn HistoryProvider(children: Element) -> Element {
    provide_context(HistoryContext {
        entries: use_signal(Vec::<HistoryEntry>::new),
        pointer: use_signal(|| 0usize),
    });
    rsx! { {children} }
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

/// Convenience accessor.
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

// ── Workspace (tabs) ───────────────────────────────────────────────

/// One open tab in the workspace.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub title: String,
    pub source: Option<String>,
    pub active: bool,
}

impl Tab {
    pub fn new(id: impl Into<String>, title: impl Into<String>, source: Option<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source,
            active: false,
        }
    }
}

/// Shared workspace context — tracks open tabs and the active tab.
#[derive(Clone, Copy)]
pub struct WorkspaceContext {
    pub tabs: Signal<Vec<Tab>>,
    pub active_tab: Signal<Option<String>>,
}

impl WorkspaceContext {
    pub fn open_tab(&self, tab: Tab) {
        {
            let mut tabs = self.tabs.write_unchecked();
            for t in tabs.iter_mut() {
                t.active = false;
            }
            tabs.push(tab.clone());
            if let Some(last) = tabs.last_mut() {
                last.active = true;
            }
        }
        *self.active_tab.write_unchecked() = Some(tab.id.clone());
    }

    pub fn activate_tab(&self, id: &str) {
        let mut tabs = self.tabs.write_unchecked();
        for t in tabs.iter_mut() {
            t.active = t.id == id;
        }
        *self.active_tab.write_unchecked() = Some(id.to_string());
    }

    pub fn close_tab(&self, id: &str) {
        let was_active = self.tabs.read().iter().any(|t| t.id == id && t.active);
        {
            let mut tabs = self.tabs.write_unchecked();
            tabs.retain(|t| t.id != id);
        }
        if was_active {
            let new_active = self.tabs.read().last().map(|t| t.id.clone());
            *self.active_tab.write_unchecked() = new_active;
        }
    }
}

/// Convenience accessor.
pub fn use_workspace() -> WorkspaceContext {
    use_context::<WorkspaceContext>()
}

/// Provider component for workspace tab management.
#[component]
pub fn WorkspaceProvider(children: Element) -> Element {
    provide_context(WorkspaceContext {
        tabs: use_signal(Vec::<Tab>::new),
        active_tab: use_signal(|| None::<String>),
    });
    rsx! { {children} }
}

// ── Navigation ───────────────────────────────────────────────────

/// Top-level view modes — drives the ribbon bar and view switcher.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Dashboard,
    Editor,
    Graph,
    Inbox,
    ReadingQueue,
    Templates,
    Settings,
    Trash,
    History,
    Recovery,
    Search,
    Calendar,
    Archive,
    SmartFolders,
    Canvas,
    Reader,
    Comparison,
    Statistics,
}

/// Maps a persisted view-mode string back to a [`ViewMode`].
pub fn parse_view_mode(mode: &str) -> ViewMode {
    match mode.to_lowercase().as_str() {
        "dashboard" => ViewMode::Dashboard,
        "graph" => ViewMode::Graph,
        "inbox" => ViewMode::Inbox,
        "readingqueue" | "reading_queue" => ViewMode::ReadingQueue,
        "templates" => ViewMode::Templates,
        "settings" => ViewMode::Settings,
        "trash" => ViewMode::Trash,
        "history" => ViewMode::History,
        "recovery" => ViewMode::Recovery,
        "search" => ViewMode::Search,
        "calendar" => ViewMode::Calendar,
        "archive" => ViewMode::Archive,
        "smartfolders" | "smart_folders" => ViewMode::SmartFolders,
        "canvas" => ViewMode::Canvas,
        "reader" => ViewMode::Reader,
        "comparison" => ViewMode::Comparison,
        "statistics" => ViewMode::Statistics,
        _ => ViewMode::Editor,
    }
}

/// Shared navigation context — carries view-mode, sidebar / inspector
/// visibility, and panel widths.
#[derive(Clone, Copy)]
pub struct NavContext {
    pub view_mode: Signal<ViewMode>,
    pub sidebar_visible: Signal<bool>,
    pub inspector_visible: Signal<bool>,
    pub sidebar_width: Signal<f64>,
    pub inspector_width: Signal<f64>,
    pub vault_name: Signal<String>,
}

/// Convenience accessor.
pub fn use_nav() -> NavContext {
    use_context::<NavContext>()
}

/// Provider component for navigation state.
#[component]
pub fn NavProvider(children: Element) -> Element {
    provide_context(NavContext {
        view_mode: use_signal(|| ViewMode::Dashboard),
        sidebar_visible: use_signal(|| true),
        inspector_visible: use_signal(|| true),
        sidebar_width: use_signal(|| 280.0),
        inspector_width: use_signal(|| 320.0),
        vault_name: use_signal(String::new),
    });
    rsx! { {children} }
}
