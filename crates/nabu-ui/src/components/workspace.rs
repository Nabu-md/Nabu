//! # Workspace tabs context
//!
//! The app shell owns a set of open note tabs (`OpenTab`), an active note
//! path, and a tree-refresh counter. The tab bar renders the tabs, the file
//! tree opens notes into tabs, and the editor is driven by the active path —
//! all through one shared [`WorkspaceContext`].
//!
//! ## Reactivity note
//!
//! The context is `Copy` and must be captured **at render time** by callers
//! (`let workspace = use_workspace();`). Helper functions take the context by
//! value so they are safe to call from async tasks, which have no reactive
//! owner.

use leptos::prelude::*;

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

/// Shared workspace state provided at the app root.
#[derive(Clone, Copy)]
pub struct WorkspaceContext {
    /// Open tabs in display order.
    pub tabs: RwSignal<Vec<OpenTab>>,
    /// The active note (vault-relative path).
    pub active_path: RwSignal<Option<String>>,
    /// Bumped by the file tree after structural mutations so the tree can
    /// reload itself without a full remount.
    pub refresh_tree: RwSignal<u32>,
}

/// Provides the workspace context (call at the app root, before any child
/// that uses [`use_workspace`]).
pub fn provide_workspace() -> WorkspaceContext {
    let ctx = WorkspaceContext {
        tabs: RwSignal::new(Vec::new()),
        active_path: RwSignal::new(None),
        refresh_tree: RwSignal::new(0),
    };
    provide_context(ctx);
    ctx
}

/// Retrieves the workspace context (call inside a [`provide_workspace`]
/// subtree, at render time).
pub fn use_workspace() -> WorkspaceContext {
    expect_context::<WorkspaceContext>()
}

/// Derives a tab title from a vault-relative path (the file name).
pub fn title_from_path(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_string()
}

/// Opens a note in a tab: adds the tab if missing and makes it active.
pub fn open_tab(ctx: WorkspaceContext, path: &str) {
    ctx.tabs.update(|tabs| {
        if !tabs.iter().any(|t| t.path == path) {
            tabs.push(OpenTab {
                path: path.to_string(),
                title: title_from_path(path),
                pinned: false,
            });
        }
    });
    ctx.active_path.set(Some(path.to_string()));
}

/// Makes an already-open tab active without adding it again.
pub fn activate_tab(ctx: WorkspaceContext, path: &str) {
    ctx.active_path.set(Some(path.to_string()));
}

/// Closes a tab. When the active tab closes, activates the neighbouring tab.
pub fn close_tab(ctx: WorkspaceContext, path: &str) {
    let was_active = ctx.active_path.get().as_deref() == Some(path);
    let (remaining, closed_index) = {
        let tabs = ctx.tabs.get();
        let idx = tabs.iter().position(|t| t.path == path);
        let remaining = tabs
            .into_iter()
            .filter(|t| t.path != path)
            .collect::<Vec<_>>();
        (remaining, idx)
    };
    ctx.tabs.set(remaining.clone());
    if was_active {
        let next = closed_index
            .map(|i| {
                // Prefer the tab that was to the right, else the left.
                remaining
                    .get(i)
                    .or_else(|| remaining.get(i.saturating_sub(1)))
                    .map(|t| t.path.clone())
            })
            .flatten();
        ctx.active_path.set(next);
    }
}

/// Closes every tab except `keep` (and pinned tabs).
pub fn close_others(ctx: WorkspaceContext, keep: &str) {
    ctx.tabs.update(|tabs| {
        tabs.retain(|t| t.path == keep || t.pinned);
    });
    ctx.active_path.set(Some(keep.to_string()));
}

/// Closes all unpinned tabs.
pub fn close_all(ctx: WorkspaceContext) {
    ctx.tabs.update(|tabs| {
        tabs.retain(|t| t.pinned);
    });
    ctx.active_path.set(None);
}

/// Toggles the pinned flag on a tab.
pub fn pin_tab(ctx: WorkspaceContext, path: &str) {
    ctx.tabs.update(|tabs| {
        if let Some(tab) = tabs.iter_mut().find(|t| t.path == path) {
            tab.pinned = !tab.pinned;
        }
    });
}

/// Moves a tab from one index to another (drag-reorder).
pub fn reorder_tab(ctx: WorkspaceContext, from: usize, to: usize) {
    if from == to {
        return;
    }
    ctx.tabs.update(|tabs| {
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
/// inside it — otherwise the editor would autosave and recreate files at the
/// now-vanished old location.
pub fn rename_tab_prefix(ctx: WorkspaceContext, old_prefix: &str, new_prefix: &str) {
    // Tabs can never have empty paths, so an empty prefix is always a
    // no-op — bail before `strip_prefix("")` could match every tab.
    if old_prefix.is_empty() {
        return;
    }
    let old = format!("{old_prefix}/");
    let new = if new_prefix.is_empty() {
        String::new()
    } else {
        format!("{new_prefix}/")
    };
    ctx.tabs.update(|tabs| {
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
    if let Some(active) = ctx.active_path.get() {
        if active == old_prefix {
            ctx.active_path.set(Some(new_prefix.to_string()));
        } else if let Some(rest) = active.strip_prefix(&old) {
            ctx.active_path.set(Some(format!("{new}{rest}")));
        }
    }
}

/// Requests the file tree to reload (after create / rename / delete / move /
/// duplicate). The tree watches this counter.
pub fn refresh_tree(ctx: WorkspaceContext) {
    ctx.refresh_tree.update(|v| *v = v.wrapping_add(1));
}
