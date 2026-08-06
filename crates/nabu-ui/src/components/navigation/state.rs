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
//! dedicated `nabu.*` keys via the existing `settings_get` / `settings_set`
//! commands — no new storage architecture.
//!
//! ## Reactivity note
//!
//! [`NavContext`] is `Copy` and must be captured **at render time** by callers
//! (`let nav = use_nav();`). Helpers take the context by value so they are
//! safe to call from async tasks and raw DOM callbacks.

use crate::components::ui::feedback::{use_tasks, use_toast};
use crate::components::ui::icons::Icon;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── ViewMode ─────────────────────────────────────────────────────

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
    /// Infinite visual workspace.
    Canvas,
    /// Distraction-free reading experience.
    Reader,
    /// Side-by-side note / revision comparison.
    Comparison,
    /// Vault-wide metrics and insights.
    Statistics,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Dashboard
    }
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

/// Canonical string key for a view mode.
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
        ViewMode::Calendar => "calendar",
        ViewMode::Archive => "archive",
        ViewMode::SmartFolders => "smart_folders",
        ViewMode::Canvas => "canvas",
        ViewMode::Reader => "reader",
        ViewMode::Comparison => "comparison",
        ViewMode::Statistics => "statistics",
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
        ViewMode::Calendar => "Calendar",
        ViewMode::Archive => "Archive",
        ViewMode::SmartFolders => "Smart Folders",
        ViewMode::Canvas => "Canvas",
        ViewMode::Reader => "Reader",
        ViewMode::Comparison => "Comparison",
        ViewMode::Statistics => "Statistics",
    }
}

/// Icon for a view mode.
pub fn view_mode_icon(mode: ViewMode) -> Icon {
    match mode {
        ViewMode::Dashboard => Icon::Dashboard,
        ViewMode::Editor => Icon::FilePen,
        ViewMode::Graph => Icon::Network,
        ViewMode::Inbox => Icon::Inbox,
        ViewMode::ReadingQueue => Icon::BookOpen,
        ViewMode::Templates => Icon::ClipboardList,
        ViewMode::Settings => Icon::Settings,
        ViewMode::Trash => Icon::Trash2,
        ViewMode::History => Icon::History,
        ViewMode::Recovery => Icon::LifeBuoy,
        ViewMode::Search => Icon::Search,
        ViewMode::Calendar => Icon::Calendar,
        ViewMode::Archive => Icon::Archive,
        ViewMode::SmartFolders => Icon::FolderTree,
        ViewMode::Canvas => Icon::Palette,
        ViewMode::Reader => Icon::BookText,
        ViewMode::Comparison => Icon::Comparison,
        ViewMode::Statistics => Icon::TrendingUp,
    }
}

// ── Supporting types ───────────────────────────────────────────────

/// One note in the vault index (mirrors the backend `NoteIndexEntry`).
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

pub fn dashboard_section_label(id: &str) -> &'static str {
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

// ── NavContext ───────────────────────────────────────────────────

/// Shared navigation context — carries view-mode, sidebar / inspector
/// visibility, overlay state, and persisted discovery data.
#[derive(Clone, Copy)]
pub struct NavContext {
    /// The active top-level screen.
    pub view_mode: Signal<ViewMode>,
    /// Command palette overlay visibility.
    pub palette_open: Signal<bool>,
    /// Quick switcher overlay visibility.
    pub switcher_open: Signal<bool>,
    /// Shortcuts reference overlay visibility.
    pub shortcuts_open: Signal<bool>,
    /// Query used when opening the search page (prefill).
    pub search_query: Signal<String>,
    /// Left sidebar visibility.
    pub show_left_sidebar: Signal<bool>,
    /// Right inspector visibility.
    pub show_right_inspector: Signal<bool>,
    /// Recently opened note paths (most recent first).
    pub recent_notes: Signal<Vec<String>>,
    /// Favourite note paths.
    pub favourites: Signal<Vec<String>>,
    /// Recent search strings (most recent first).
    pub recent_searches: Signal<Vec<String>>,
    /// Saved searches.
    pub saved_searches: Signal<Vec<SavedSearch>>,
    /// Saved smart folders (virtual collections powered by queries).
    pub smart_folders: Signal<Vec<crate::models::organisation::SmartFolder>>,
    /// Recently run command ids (most recent first).
    pub recent_commands: Signal<Vec<String>>,
    /// Favourite command ids.
    pub favourite_commands: Signal<Vec<String>>,
    /// The full vault note index (title + folder + mtime).
    pub notes_index: Signal<Vec<NoteIndexEntry>>,
    /// Enabled dashboard section ids (order preserved).
    pub dashboard_sections: Signal<Vec<String>>,
    /// The current vault's display name.
    pub vault_name: Signal<String>,
}

/// Retrieves the navigation context.
pub fn use_nav() -> NavContext {
    use_context::<NavContext>()
}

/// Provider component for navigation state.
#[component]
pub fn NavProvider(children: Element) -> Element {
    provide_context(NavContext {
        view_mode: use_signal(|| ViewMode::Dashboard),
        palette_open: use_signal(|| false),
        switcher_open: use_signal(|| false),
        shortcuts_open: use_signal(|| false),
        search_query: use_signal(|| String::new()),
        show_left_sidebar: use_signal(|| true),
        show_right_inspector: use_signal(|| true),
        recent_notes: use_signal(Vec::<String>::new),
        favourites: use_signal(Vec::<String>::new),
        recent_searches: use_signal(Vec::<String>::new),
        saved_searches: use_signal(Vec::<SavedSearch>::new),
        smart_folders: use_signal(Vec::<crate::models::organisation::SmartFolder>::new),
        recent_commands: use_signal(Vec::<String>::new),
        favourite_commands: use_signal(Vec::<String>::new),
        notes_index: use_signal(Vec::<NoteIndexEntry>::new),
        dashboard_sections: use_signal(|| {
            DASHBOARD_SECTIONS.iter().map(|s| s.to_string()).collect()
        }),
        vault_name: use_signal(|| String::new()),
    });

    // Persist discovery state once we have a vault name (deferred load).
    let nav = use_nav();
    let initialized = use_signal(|| false);
    if !*initialized.read() {
        initialized.set(true);
        load_all_nav_state(nav);
        load_notes_index(nav);
    }

    rsx! { {children} }
}

// ── Persistence helpers ────────────────────────────────────────────

const K_RECENT_NOTES: &str = "nabu.recent_notes";
const K_FAVOURITES: &str = "nabu.favourites";
const K_RECENT_SEARCHES: &str = "nabu.recent_searches";
const K_SAVED_SEARCHES: &str = "nabu.saved_searches";
const K_SMART_FOLDERS: &str = "nabu.smart_folders";
const K_RECENT_COMMANDS: &str = "nabu.recent_commands";
const K_FAV_COMMANDS: &str = "nabu.favourite_commands";
const K_DASH_SECTIONS: &str = "nabu.dashboard.sections";

const INDEX_FAILURE_TITLE: &str = "Couldn't build the vault index";

fn settings_persist(key: &str, value: serde_json::Value) {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "key": key, "value": value }))
        .unwrap();
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("settings_set", args).await;
    });
}

/// Loads every persisted discovery list into the context (call once after
/// [`NavProvider`] creates the context).
pub fn load_all_nav_state(mut nav: NavContext) {
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
        let smart =
            load_json::<Vec<crate::models::organisation::SmartFolder>>(K_SMART_FOLDERS)
                .await
                .unwrap_or_default();
        nav.smart_folders.set(smart);
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
pub fn record_recent_note(mut nav: NavContext, path: &str) {
    nav.recent_notes.with_mut(|l| push_unique(l, path, 20));
    let snapshot = nav.recent_notes.read().clone();
    settings_persist(K_RECENT_NOTES, serde_json::to_value(snapshot).unwrap());
}

/// Toggles a note in the favourites list.
pub fn toggle_favourite(mut nav: NavContext, path: &str) {
    let has = nav.favourites.read().iter().any(|p| p == path);
    nav.favourites.with_mut(|l| {
        if has {
            l.retain(|p| p != path);
        } else {
            l.push(path.to_string());
        }
    });
    let snapshot = nav.favourites.read().clone();
    settings_persist(K_FAVOURITES, serde_json::to_value(snapshot).unwrap());
}

/// Records a search string in the recent-searches list.
pub fn record_recent_search(mut nav: NavContext, query: &str) {
    if query.trim().is_empty() {
        return;
    }
    nav.recent_searches.with_mut(|l| push_unique(l, query, 10));
    let snapshot = nav.recent_searches.read().clone();
    settings_persist(K_RECENT_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Clears the recent-searches history.
pub fn clear_recent_searches(mut nav: NavContext) {
    nav.recent_searches.set(Vec::new());
    settings_persist(
        K_RECENT_SEARCHES,
        serde_json::to_value(Vec::<String>::new()).unwrap(),
    );
}

/// Adds a saved search (deduped by name).
pub fn save_search(mut nav: NavContext, name: &str, query: &str) {
    let name = if name.trim().is_empty() {
        query.trim().to_string()
    } else {
        name.trim().to_string()
    };
    if name.is_empty() || query.trim().is_empty() {
        return;
    }
    nav.saved_searches.with_mut(|l| {
        l.retain(|s| s.name != name);
        l.push(SavedSearch {
            name,
            query: query.trim().to_string(),
        });
    });
    let snapshot = nav.saved_searches.read().clone();
    settings_persist(K_SAVED_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Removes a saved search by name.
pub fn remove_saved_search(mut nav: NavContext, name: &str) {
    nav.saved_searches.with_mut(|l| l.retain(|s| s.name != name));
    let snapshot = nav.saved_searches.read().clone();
    settings_persist(K_SAVED_SEARCHES, serde_json::to_value(snapshot).unwrap());
}

/// Saves (creates or updates) a smart folder, deduped by id, and persists it.
pub fn save_smart_folder(
    mut nav: NavContext,
    folder: crate::models::organisation::SmartFolder,
) {
    nav.smart_folders.with_mut(|l| {
        if let Some(existing) = l.iter_mut().find(|f| f.id == folder.id) {
            *existing = folder.clone();
        } else {
            l.push(folder);
        }
    });
    let snapshot = nav.smart_folders.read().clone();
    settings_persist(K_SMART_FOLDERS, serde_json::to_value(snapshot).unwrap());
}

/// Removes a smart folder by id and persists.
pub fn remove_smart_folder(mut nav: NavContext, id: &str) {
    nav.smart_folders.with_mut(|l| l.retain(|f| f.id != id));
    let snapshot = nav.smart_folders.read().clone();
    settings_persist(K_SMART_FOLDERS, serde_json::to_value(snapshot).unwrap());
}

/// Records a command id as recently run (deduped, capped at 12).
pub fn record_recent_command(mut nav: NavContext, id: &str) {
    nav.recent_commands.with_mut(|l| push_unique(l, id, 12));
    let snapshot = nav.recent_commands.read().clone();
    settings_persist(K_RECENT_COMMANDS, serde_json::to_value(snapshot).unwrap());
}

/// Toggles a command id in the favourite-commands list.
pub fn toggle_favourite_command(mut nav: NavContext, id: &str) {
    let has = nav.favourite_commands.read().iter().any(|c| c == id);
    nav.favourite_commands.with_mut(|l| {
        if has {
            l.retain(|c| c != id);
        } else {
            l.push(id.to_string());
        }
    });
    let snapshot = nav.favourite_commands.read().clone();
    settings_persist(K_FAV_COMMANDS, serde_json::to_value(snapshot).unwrap());
}

/// Sets the enabled dashboard sections and persists them.
pub fn set_dashboard_sections(mut nav: NavContext, sections: Vec<String>) {
    nav.dashboard_sections.set(sections.clone());
    settings_persist(K_DASH_SECTIONS, serde_json::to_value(sections).unwrap());
}

// ── Vault note index ──────────────────────────────────────────────

/// Loads the vault note index from the backend (`notes_index`).
pub fn load_notes_index(mut nav: NavContext) {
    let toasts = use_toast();
    let tasks = use_tasks();
    let task_id = tasks.start("Indexing vault…");
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("notes_index", empty).await;
        tasks.finish(&task_id);
        if let Ok(index) = serde_wasm_bindgen::from_value::<Vec<NoteIndexEntry>>(result) {
            nav.notes_index.set(index);
            if toasts.has_toast_with_title(INDEX_FAILURE_TITLE) {
                toasts.dismiss_by_title(INDEX_FAILURE_TITLE);
                toasts.success("Index rebuilt", "The vault index is up to date.");
            }
        } else {
            if !toasts.has_toast_with_title(INDEX_FAILURE_TITLE) {
                let retry_nav = nav;
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
                score += 15;
            }
            if ci > 0 && (cand[ci - 1] == ' ' || cand[ci - 1] == '-' || cand[ci - 1] == '/') {
                score += 8;
            }
            if let Some(p) = prev {
                if ci == p + 1 {
                    score += 6;
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
