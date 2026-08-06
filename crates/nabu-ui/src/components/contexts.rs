//! # Dioxus Context Providers
//!
//! This module defines the context types and provider components that wire
//! shared application state through the component tree. Each provider wraps its
//! children and calls `provide_context` so descendant components can retrieve
//! the context with `use_context::<T>()`.
//!
//! Phase 0 provides minimal stubs for all contexts. The toast provider includes
//! a functional (if basic) ToastRegion for user feedback; the others provide
//! the correct shape so Phase 0.2 can drop in full primitives without
//! restructuring the tree.

use dioxus::prelude::*;
use std::collections::HashMap;

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

// ── Toast ────────────────────────────────────────────────────────

/// Kinds of toast notifications.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A single toast notification.
#[derive(Clone, Debug)]
pub struct ToastEntry {
    pub id: String,
    pub title: String,
    pub message: String,
    pub kind: ToastKind,
    /// Auto-dismiss after this many milliseconds (if set).
    pub timeout_ms: Option<u64>,
    /// Whether the user can manually dismiss this toast.
    pub dismissible: bool,
}

/// Shared toast context — the toast region reads `toasts` and the app
/// dispatches new toasts via `push` / `dismiss`.
#[derive(Clone, Copy)]
pub struct ToastContext {
    pub toasts: Signal<Vec<ToastEntry>>,
}

impl ToastContext {
    /// Push a new toast onto the stack.
    pub fn push(&self, toast: ToastEntry) {
        self.toasts.write_unchecked().push(toast);
    }

    /// Remove a toast by id.
    pub fn dismiss(&self, id: &str) {
        self.toasts.write_unchecked().retain(|x| x.id != id);
    }

    /// Convenience helpers — mirror the LePtOS `use_toast()` ergonomics.
    pub fn info(&self, title: impl Into<String>, message: impl Into<String>) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!("toast-{}", COUNTER.fetch_add(1, Ordering::Relaxed));
        self.push(ToastEntry {
            id,
            title: title.into(),
            message: message.into(),
            kind: ToastKind::Info,
            timeout_ms: Some(4000),
            dismissible: true,
        });
    }

    pub fn success(&self, title: impl Into<String>, message: impl Into<String>) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!("toast-{}", COUNTER.fetch_add(1, Ordering::Relaxed));
        self.push(ToastEntry {
            id,
            title: title.into(),
            message: message.into(),
            kind: ToastKind::Success,
            timeout_ms: Some(4000),
            dismissible: true,
        });
    }

    pub fn error(&self, title: impl Into<String>, message: impl Into<String>) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!("toast-{}", COUNTER.fetch_add(1, Ordering::Relaxed));
        self.push(ToastEntry {
            id,
            title: title.into(),
            message: message.into(),
            kind: ToastKind::Error,
            timeout_ms: None,
            dismissible: true,
        });
    }
}

/// Convenience accessor — call inside a `ToastProvider` subtree.
pub fn use_toast() -> ToastContext {
    use_context::<ToastContext>()
}

/// Provider component that creates a `ToastContext` and renders a
/// [`ToastRegion`] to display active toasts.
#[component]
pub fn ToastProvider(children: Element) -> Element {
    let toasts = use_signal(Vec::<ToastEntry>::new);
    provide_context(ToastContext { toasts });

    rsx! {
        ToastRegion {}
        {children}
    }
}

/// Renders the active toast stack (top-right overlay).
#[component]
fn ToastRegion() -> Element {
    let ctx = use_toast();
    let toasts = ctx.toasts.read();

    if toasts.is_empty() {
        rsx! {}
    } else {
        rsx! {
            div {
                id: "toast-region",
                class: "fixed top-4 right-4 z-[1000] flex flex-col gap-2",
                for toast in toasts.iter() {
                    div {
                        key: "{toast.id}",
                        class: "max-w-sm rounded-lg border px-4 py-3 text-sm shadow-lg {toast_cls(toast.kind)}",
                        div { class: "font-medium", "{toast.title}" }
                        div { class: "mt-1 opacity-90", "{toast.message}" }
                    }
                }
            }
        }
    }
}

/// Returns the Tailwind class string for a given [`ToastKind`].
fn toast_cls(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Info => "border-blue-500/30 bg-blue-500/10 text-blue-300",
        ToastKind::Success => "border-green-500/30 bg-green-500/10 text-green-300",
        ToastKind::Warning => "border-amber-500/30 bg-amber-500/10 text-amber-300",
        ToastKind::Error => "border-red-500/30 bg-red-500/10 text-red-300",
    }
}

// ── Task ─────────────────────────────────────────────────────────

/// A background task that the UI can track (e.g. indexer, graph build).
#[derive(Clone, Debug)]
pub struct TaskEntry {
    pub id: String,
    pub title: String,
    pub progress: f64, // 0.0–1.0, -1.0 means indeterminate
    pub status: TaskStatus,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskStatus {
    Running,
    Completed,
    Error,
}

/// Shared task context.
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub tasks: Signal<HashMap<String, TaskEntry>>,
}

impl TaskContext {
    pub fn add(&self, entry: TaskEntry) {
        self.tasks.write_unchecked().insert(entry.id.clone(), entry);
    }

    pub fn update(&self, id: &str, f: impl FnOnce(&mut TaskEntry)) {
        let mut tasks = self.tasks.write_unchecked();
        if let Some(entry) = tasks.get_mut(id) {
            f(entry);
        }
    }

    pub fn remove(&self, id: &str) {
        self.tasks.write_unchecked().remove(id);
    }
}

/// Convenience accessor.
pub fn use_tasks() -> TaskContext {
    use_context::<TaskContext>()
}

/// Provider component for task tracking.
#[component]
pub fn TaskProvider(children: Element) -> Element {
    let tasks = use_signal(HashMap::<String, TaskEntry>::new);
    provide_context(TaskContext { tasks });
    rsx! { {children} }
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
    let entries = use_signal(Vec::<HistoryEntry>::new);
    let pointer = use_signal(|| 0usize);
    provide_context(HistoryContext { entries, pointer });
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
