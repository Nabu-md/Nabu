//! # Navigation state — shared signals + persistence
//!
//! The [`NavContext`] is provided once at the app root and carries every
//! signal the navigation surfaces share: the active [`ViewMode`], the open
//! overlays (command palette / quick switcher / shortcuts reference), and the
//! persisted discovery data (recently opened notes, favourites, recent &
//! saved searches, recent & favourite commands, dashboard section config).
//!
//! ## Persistence
//!
//! Discovery data is persisted to the backend settings store (`.json`) under
//! dedicated `nabu.*` keys via the existing `settings_set` / `settings_get`
//! commands — no new storage architecture.
//!
//! ## Reactivity note
//!
//! [`NavContext`] is `Copy` and must be captured **at render time** by callers
//! (`let nav = use_nav();`). Helpers take the context by value so they are
//! safe to call from async tasks and raw DOM callbacks.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Title of the persistent failure notification for the vault note index.
/// Shared between the push and the success-path dismissal so they always
/// agree.
const INDEX_FAILURE_TITLE: &str = "Couldn't build the vault index";

/// The top-level screens of the app.
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
        _ => ViewMode::Editor,
    }
}

/// Canonical string key for a view mode (session persistence, shortcuts).
pub fn view_mode_key(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Dashboard => "dashboard",
        ViewMode::Editor => "editor",
        ViewMode::Graph => "graph",
        ViewMode::Inbox => "inbox",
        ViewMode::ReadingQueue => "reading_queue",
        ViewMode::Templates => "templates",
        ViewMode::Settings => "settings",
        ViewMode::Trash => "trash",
        ViewMode::History => "history",
        ViewMode::Recovery => "recovery",
        ViewMode::Search => "search",
    }
}

/// Human-readable label for a view mode.
pub fn view_mode_label(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Dashboard => "Dashboard",
        ViewMode::Editor => "Editor",
        ViewMode::Graph => "Graph",
        ViewMode::Inbox => "Inbox",
        ViewMode::ReadingQueue => "Reading Queue",
        ViewMode::Templates => "Templates",
        ViewMode::Settings => "Settings",
        ViewMode::Trash => "Trash",
        ViewMode::History => "History",
        ViewMode::Recovery => "Recovery",
        ViewMode::Search => "Search",
    }
}

/// A single note in the vault index (mirrors the backend `NoteIndexEntry`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteIndexEntry {
    pub path: String,
    pub title: String,
    pub folder: String,
    pub modified_at: String,
    #[serde(default)]
    pub pinned: bool,
}

/// A saved search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub name: String,
    pub query: String,
}

/// Every dashboard section id, in display order.
pub const DASHBOARD_SECTIONS: &[&str] = &[
    "quick_actions",
    "recently_modified",
    "favourites",
    "recently_opened",
    "pinned",
    "inbox",
    "recent_searches",
    "summary",
];

fn section_label(id: &str) -> &'static str {
    match id {
        "quick_actions" => "Quick Actions",
        "recently_modified" => "Recently Modified",
        "favourites" => "Favourites",
        "recently_opened" => "Recently Opened",
        "pinned" => "Pinned",
        "inbox" => "Inbox",
        "recent_searches" => "Recent Searches",
        "summary" => "Workspace Summary",
        _ => "Section",
    }
}

/// Shared navigation state.
#[derive(Clone, Copy)]
pub struct NavContext {
    /// The active top-level screen.
    pub view_mode: RwSignal<ViewMode>,
    /// Command palette overlay visibility.
    pub palette_open: RwSignal<bool>,
    /// Quick switcher overlay visibility.
    pub switcher_open: RwSignal<bool>,
    /// Shortcuts reference overlay visibility.
    pub shortcuts_open: RwSignal<bool>,
    /// Query used when opening the search page (prefill).
    pub search_query: RwSignal<String>,
    /// Left sidebar visibility.
    pub show_left_sidebar: RwSignal<bool>,
    /// Right inspector visibility.
    pub show_right_inspector: RwSignal<bool>,
    /// Recently opened note paths (most recent first).
    pub recent_notes: RwSignal<Vec<String>>,
    /// Favourite note paths.
    pub favourites: RwSignal<Vec<String>>,
    /// Recent search strings (most recent first).
    pub recent_searches: RwSignal<Vec<String>>,
    /// Saved searches.
    pub saved_searches: RwSignal<Vec<SavedSearch>>,
    /// Recently run command ids (most recent first).
    pub recent_commands: RwSignal<Vec<String>>,
    /// Favourite command ids.
    pub favourite_commands: RwSignal<Vec<String>>,
    /// The full vault note index (title + folder + mtime).
    pub notes_index: RwSignal<Vec<NoteIndexEntry>>,
    /// Enabled dashboard section ids (order preserved).
    pub dashboard_sections: RwSignal<Vec<String>>,
    /// The current vault's display name.
    pub vault_name: RwSignal<String>,
}

/// Provides the navigation context (call once at the app root).
pub fn provide_navigation() -> NavContext {
    let ctx = NavContext {
        view_mode: RwSignal::new(ViewMode::Dashboard),
        palette_open: RwSignal::new(false),
        switcher_open: RwSignal::new(false),
        shortcuts_open: RwSignal::new(false),
        search_query: RwSignal::new(String::new()),
        show_left_sidebar: RwSignal::new(true),
        show_right_inspector: RwSignal::new(true),
        recent_notes: RwSignal::new(Vec::new()),
        favourites: RwSignal::new(Vec::new()),
        recent_searches: RwSignal::new(Vec::new()),
        saved_searches: RwSignal::new(Vec::new()),
        recent_commands: RwSignal::new(Vec::new()),
        favourite_commands: RwSignal::new(Vec::new()),
        notes_index: RwSignal::new(Vec::new()),
        dashboard_sections: RwSignal::new(DASHBOARD_SECTIONS.iter().map(|s| s.to_string()).collect()),
        vault_name: RwSignal::new(String::new()),
    };
    provide_context(ctx);
    ctx
}

/// Retrieves the navigation context (call inside a [`provide_navigation`]
/// subtree, at render time).
pub fn use_nav() -> NavContext {
    expect_context::<NavContext>()
}

// ── Persistence helpers ────────────────────────────────────────────

const K_RECENT_NOTES: &str = "nabu.recent_notes";
const K_FAVOURITES: &str = "nabu.favourites";
const K_RECENT_SEARCHES: &str = "nabu.recent_searches";
const K_SAVED_SEARCHES: &str = "nabu.saved_searches";
const K_RECENT_COMMANDS: &str = "nabu.recent_commands";
const K_FAV_COMMANDS: &str = "nabu.favourite_commands";
const K_DASH_SECTIONS: &str = "nabu.dashboard.sections";

fn settings_persist(key: &str, value: serde_json::Value) {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "key": key, "value": value }))
        .unwrap();
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("settings_set", args).await;
    });
}

/// Loads every persisted discovery list into the context (call once after
/// [`provide_navigation`], e.g. from the app shell's mount).
pub fn load_all_nav_state(nav: NavContext) {
    spawn_local(async move {
        let recent = load_string_list(K_RECENT_NOTES).await;
        nav.recent_notes.set(recent);
        let favs = load_string_list(K_FAVOURITES).await;
        nav.favourites.set(favs);
        let searches = load_string_list(K_RECENT_SEARCHES).await;
        nav.recent_searches.set(searches);
        let saved = load_json::<Vec<SavedSearch>>(K_SAVED_SEARCHES)
            .await
            .unwrap_or_default();
        nav.saved_searches.set(saved);
        let recent_cmds = load_string_list(K_RECENT_COMMANDS).await;
        nav.recent_commands.set(recent_cmds);
        let fav_cmds = load_string_list(K_FAV_COMMANDS).await;
        nav.favourite_commands.set(fav_cmds);
        let sections = load_string_list(K_DASH_SECTIONS).await;
        if !sections.is_empty() {
            nav.dashboard_sections.set(sections);
        }
    });
}

async fn load_string_list(key: &str) -> Vec<String> {
    load_json::<Vec<String>>(key).await.unwrap_or_default()
}

async fn load_json<T: for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "key": key })).unwrap();
    let result = crate::ipc::tauri_invoke("settings_get", args).await;
    serde_wasm_bindgen::from_value::<T>(result).ok()
}

// ── Record helpers (mutate + persist in one call) ──────────────────

fn push_unique(list: &mut Vec<String>, item: &str, cap: usize) {
    list.retain(|p| p != item);
    list.insert(0, item.to_string());
    list.truncate(cap);
}

/// Records a note as recently opened (deduped, capped at 20).
pub fn record_recent_note(nav: NavContext, path: &str) {
    nav.recent_notes.update(|l| push_unique(l, path, 20));
    let snapshot = nav.recent_notes.get_untracked();
    settings_persist(K_RECENT_NOTES, serde_json::to_value(snapshot).unwrap());
}

/// Toggles a note in the favourites list.
pub fn toggle_favourite(nav: NavContext, path: &str) {
    let added = nav.favourites.with(|l| {
        if l.iter().any(|p| p == path) {
            false
        } else {
            true
        }
    });
    nav.favourites.update(|l| {
        if added {
            l.push(path.to_string());
        } else {
            l.retain(|p| p != path);
        }
    });
    let snapshot = nav.favourites.get_untracked();
    settings_persist(K_FAVOURITES, serde_json::to_value(snapshot).unwrap());
}

/// Records a search string in the recent-searches list.
pub fn record_recent_search(nav: NavContext, query: &str) {
    if query.trim().is_empty() {
        return;
    }
    nav.recent_searches.update(|l| push_unique(l, query, 10));
    let snapshot = nav.recent_searches.get_untracked();
    settings_persist(K_RECENT_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Clears the recent-searches history.
pub fn clear_recent_searches(nav: NavContext) {
    nav.recent_searches.set(Vec::new());
    settings_persist(
        K_RECENT_SEARCHES,
        serde_json::to_value(Vec::<String>::new()).unwrap(),
    );
}

/// Adds a saved search (deduped by name).
pub fn save_search(nav: NavContext, name: &str, query: &str) {
    let name = if name.trim().is_empty() {
        query.trim().to_string()
    } else {
        name.trim().to_string()
    };
    if name.is_empty() || query.trim().is_empty() {
        return;
    }
    nav.saved_searches.update(|l| {
        l.retain(|s| s.name != name);
        l.push(SavedSearch {
            name,
            query: query.trim().to_string(),
        });
    });
    let snapshot = nav.saved_searches.get_untracked();
    settings_persist(K_SAVED_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Removes a saved search by name.
pub fn remove_saved_search(nav: NavContext, name: &str) {
    nav.saved_searches.update(|l| l.retain(|s| s.name != name));
    let snapshot = nav.saved_searches.get_untracked();
    settings_persist(K_SAVED_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Records a command id as recently run (deduped, capped at 12).
pub fn record_recent_command(nav: NavContext, id: &str) {
    nav.recent_commands.update(|l| push_unique(l, id, 12));
    let snapshot = nav.recent_commands.get_untracked();
    settings_persist(K_RECENT_COMMANDS, serde_json::to_value(snapshot).unwrap());
}

/// Toggles a command id in the favourite-commands list.
pub fn toggle_favourite_command(nav: NavContext, id: &str) {
    let has = nav
        .favourite_commands
        .with(|l| l.iter().any(|c| c == id));
    nav.favourite_commands.update(|l| {
        if has {
            l.retain(|c| c != id);
        } else {
            l.push(id.to_string());
        }
    });
    let snapshot = nav.favourite_commands.get_untracked();
    settings_persist(K_FAV_COMMANDS, serde_json::to_value(snapshot).unwrap());
}

/// Sets the enabled dashboard sections and persists them.
pub fn set_dashboard_sections(nav: NavContext, sections: Vec<String>) {
    nav.dashboard_sections.set(sections.clone());
    settings_persist(K_DASH_SECTIONS, serde_json::to_value(sections).unwrap());
}

/// Human label for a dashboard section id.
pub fn dashboard_section_label(id: &str) -> &'static str {
    section_label(id)
}

/// Loads the vault note index from the backend (`notes_index`).
pub fn load_notes_index(nav: NavContext) {
    // Capture the toast + task contexts during render — never
    // `expect_context` inside `spawn_local` (no reactive owner on the failure
    // path). The index load registers an indeterminate background task so the
    // NavBar indicator reflects real long-running work.
    let toasts = crate::components::ui::feedback::use_toast();
    let tasks = crate::components::ui::feedback::use_tasks();
    let task_id = tasks.start("Indexing vault…");
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("notes_index", empty).await;
        tasks.finish(&task_id);
        if let Ok(index) = serde_wasm_bindgen::from_value::<Vec<NoteIndexEntry>>(result) {
            nav.notes_index.set(index);
            // A successful (re)load resolves any previous failure — clear the
            // stale warning so the notification center stays truthful, and
            // confirm a retry worked (silent on a clean first load).
            if toasts.has_toast_with_title(INDEX_FAILURE_TITLE) {
                toasts.dismiss_by_title(INDEX_FAILURE_TITLE);
                toasts.success("Index rebuilt", "The vault index is up to date.");
            }
        } else {
            // Persistent + actionable: stays in the notification center until
            // dismissed, with a Retry action that re-runs the load. Dedupe so
            // repeated failures (each launch / retry) don't flood the center.
            let retry_nav = nav;
            if !toasts.has_toast_with_title(INDEX_FAILURE_TITLE) {
                toasts.push_persistent_with_action(
                    crate::components::ui::feedback::ToastKind::Warning,
                    INDEX_FAILURE_TITLE,
                    "Recently modified, favourites and the quick switcher may be incomplete.",
                    crate::components::ui::feedback::ToastAction::new(
                        "Retry",
                        Callback::new(move |_| load_notes_index(retry_nav)),
                    ),
                );
            }
        }
    });
}

// ── Fuzzy matching ─────────────────────────────────────────────────

/// Scores a fuzzy subsequence match of `query` in `candidate`.
///
/// Returns `Some(score)` when every character of the (lowercased) query
/// appears in order in the candidate; `None` otherwise. Higher scores prefer
/// matches at the start, at word boundaries, and with consecutive runs.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32> {
    let q: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let cand: Vec<char> = candidate.chars().map(|c| c.to_ascii_lowercase()).collect();
    let mut qi = 0usize;
    let mut score = 0u32;
    let mut prev: Option<usize> = None;
    for (ci, &c) in cand.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            score += 10;
            if ci == 0 {
                score += 15; // starts at the beginning
            }
            if ci > 0 && (cand[ci - 1] == ' ' || cand[ci - 1] == '-' || cand[ci - 1] == '/') {
                score += 8; // word boundary
            }
            if let Some(p) = prev {
                if ci == p + 1 {
                    score += 6; // consecutive run
                }
            }
            prev = Some(ci);
            qi += 1;
        }
    }
    if qi == q.len() {
        Some(score)
    } else {
        None
    }
}

