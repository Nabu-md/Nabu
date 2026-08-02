//! # Dashboard — the configurable home
//!
//! A dedicated home experience showing: Quick Actions, Recently Modified,
//! Favourites, Recently Opened, Pinned, Inbox, Recent Searches and a Workspace
//! Summary. Sections can be toggled via the Customize menu (persisted under
//! `nabu.dashboard.sections`).

use crate::components::navigation::state::{
    clear_recent_searches, record_recent_note, record_recent_search, set_dashboard_sections,
    toggle_favourite, use_nav, NoteIndexEntry, DASHBOARD_SECTIONS,
};
use crate::components::ui::feedback::use_toast;
use crate::components::workspace::{use_workspace, WorkspaceContext};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// A small list-section card with a header row and rows of notes.
#[component]
fn NoteListSection(
    title: String,
    icon: String,
    empty: &'static str,
    notes: Vec<NoteIndexEntry>,
    highlight_paths: Vec<String>,
) -> impl IntoView {
    let nav = use_nav();
    let ws = use_workspace();
    let notes = notes;
    let highlight = highlight_paths;
    view! {
        <div class="dash-card">
            <div class="dash-card-header">
                <span class="dash-card-title">{icon} {title}</span>
            </div>
            <div class="dash-card-body">
                {move || if notes.is_empty() {
                    view! { <div class="dash-empty">{empty}</div> }.into_any()
                } else {
                    notes.clone().into_iter().map(|note| {
                        let path = note.path.clone();
                        let path_for_star = path.clone();
                        let title = note.title.clone();
                        let folder_display = if note.folder.is_empty() {
                            "/".to_string()
                        } else {
                            note.folder.clone()
                        };
                        let is_fav = highlight.contains(&path);
                        let ws = ws;
                        let nav = nav;
                        view! {
                            <div
                                class="dash-note"
                                on:click=move |_| {
                                    crate::components::workspace::open_tab(ws, &path);
                                    record_recent_note(nav, &path);
                                }
                            >
                                <span class="dash-note-icon" aria-hidden="true">"📄"</span>
                                <span class="dash-note-main">
                                    <span class="dash-note-title">{title}</span>
                                    <span class="dash-note-folder">{folder_display}</span>
                                </span>
                                <span
                                    class="dash-note-star"
                                    title=move || if is_fav { "Remove from favourites" } else { "Add to favourites" }
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        toggle_favourite(nav, &path_for_star);
                                    }
                                >
                                    {move || if is_fav { "★" } else { "☆" }}
                                </span>
                            </div>
                        }
                    }).collect_view().into_any()
                }}
            </div>
        </div>
    }
}

/// Pinned tabs section (from the workspace tabs).
#[component]
fn PinnedSection() -> impl IntoView {
    let ws = use_workspace();
    let nav = use_nav();
    let tabs = Memo::new(move |_| ws.tabs.get().into_iter().filter(|t| t.pinned).collect::<Vec<_>>());
    view! {
        <div class="dash-card">
            <div class="dash-card-header">
                <span class="dash-card-title">"📌 Pinned"</span>
            </div>
            <div class="dash-card-body">
                {move || {
                    let tabs = tabs.get();
                    if tabs.is_empty() {
                        view! { <div class="dash-empty">"No pinned tabs — right-click a tab and choose Pin."</div> }.into_any()
                    } else {
                        tabs.into_iter().map(|tab| {
                            let path = tab.path.clone();
                            let title = tab.title.clone();
                            view! {
                                <div
                                    class="dash-note"
                                    on:click=move |_| {
                                        crate::components::workspace::activate_tab(ws, &path);
                                        record_recent_note(nav, &path);
                                    }
                                >
                                    <span class="dash-note-icon" aria-hidden="true">"📌"</span>
                                    <span class="dash-note-main">
                                        <span class="dash-note-title">{title}</span>
                                        <span class="dash-note-folder">"pinned tab"</span>
                                    </span>
                                </div>
                            }
                        }).collect_view().into_any()
                    }
                }}
            </div>
        </div>
    }
}

/// Recent searches chips.
#[component]
fn RecentSearchesSection() -> impl IntoView {
    let nav = use_nav();
    view! {
        <div class="dash-card">
            <div class="dash-card-header">
                <span class="dash-card-title">"🔎 Recent Searches"</span>
                <button
                    type="button"
                    class="btn btn-sm btn-ghost"
                    on:click=move |_| clear_recent_searches(nav)
                >
                    "Clear"
                </button>
            </div>
            <div class="dash-card-body">
                {move || {
                    let searches = nav.recent_searches.get();
                    if searches.is_empty() {
                        view! { <div class="dash-empty">"Search from the navbar or press ⌘⇧F."</div> }.into_any()
                    } else {
                        view! {
                            <div class="dash-chips">
                                {searches.into_iter().map(|s| {
                                    let q = s.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="dash-chip"
                                            on:click=move |_| {
                                                record_recent_search(nav, &q);
                                                nav.search_query.set(q.clone());
                                                nav.view_mode.set(crate::components::navigation::state::ViewMode::Search);
                                            }
                                        >
                                            "🔍 " {s}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

/// Inbox summary: pending / processing counts.
#[component]
fn InboxSection() -> impl IntoView {
    let nav = use_nav();
    let (pending, set_pending) = signal(0usize);
    let (processing, set_processing) = signal(0usize);

    Effect::new(move |_| {
        // Fetch the queue once on mount (untracked so the effect doesn't
        // re-fire on every note-index change).
        let _ = nav.notes_index.get_untracked().len();
        spawn_local(async move {
            let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("inbox_get_queue", empty).await;
            if let Ok(items) = serde_wasm_bindgen::from_value::<Vec<crate::components::inbox::InboxItem>>(result) {
                let pending_count = items.iter().filter(|i| i.status == crate::components::inbox::InboxStatus::Pending).count();
                let processing_count = items.iter().filter(|i| i.status == crate::components::inbox::InboxStatus::Processing).count();
                set_pending.set(pending_count);
                set_processing.set(processing_count);
            }
        });
    });

    view! {
        <div class="dash-card">
            <div class="dash-card-header">
                <span class="dash-card-title">"📥 Inbox"</span>
            </div>
            <div class="dash-card-body">
                <div class="dash-inbox-row">
                    <div class="dash-inbox-stat">
                        <span class="dash-inbox-number">{move || pending.get()}</span>
                        <span class="dash-inbox-label">"pending"</span>
                    </div>
                    <div class="dash-inbox-stat">
                        <span class="dash-inbox-number">{move || processing.get()}</span>
                        <span class="dash-inbox-label">"processing"</span>
                    </div>
                </div>
                <button
                    type="button"
                    class="btn btn-sm mt-2"
                    on:click=move |_| nav.view_mode.set(crate::components::navigation::state::ViewMode::Inbox)
                >
                    "Open Inbox →"
                </button>
            </div>
        </div>
    }
}

/// Workspace summary card.
#[component]
fn SummarySection() -> impl IntoView {
    let nav = use_nav();
    let ws = use_workspace();
    let summary = Memo::new(move |_| {
        let index = nav.notes_index.get();
        let mut folders = std::collections::HashSet::new();
        for note in &index {
            if !note.folder.is_empty() {
                folders.insert(note.folder.clone());
            }
        }
        let tabs = ws.tabs.get();
        let pinned = tabs.iter().filter(|t| t.pinned).count();
        (
            index.len(),
            folders.len(),
            tabs.len(),
            pinned,
        )
    });
    view! {
        <div class="dash-card">
            <div class="dash-card-header">
                <span class="dash-card-title">"🗂️ Workspace Summary"</span>
            </div>
            <div class="dash-card-body">
                {move || {
                    let (notes, folders, tabs, pinned) = summary.get();
                    view! {
                        <div class="dash-summary-grid">
                            <div class="dash-inbox-stat">
                                <span class="dash-inbox-number">{notes}</span>
                                <span class="dash-inbox-label">"notes"</span>
                            </div>
                            <div class="dash-inbox-stat">
                                <span class="dash-inbox-number">{folders}</span>
                                <span class="dash-inbox-label">"folders"</span>
                            </div>
                            <div class="dash-inbox-stat">
                                <span class="dash-inbox-number">{tabs}</span>
                                <span class="dash-inbox-label">"open tabs"</span>
                            </div>
                            <div class="dash-inbox-stat">
                                <span class="dash-inbox-number">{pinned}</span>
                                <span class="dash-inbox-label">"pinned"</span>
                            </div>
                        </div>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

/// The Dashboard view.
#[component]
pub fn Dashboard() -> impl IntoView {
    let nav = use_nav();
    let ws: WorkspaceContext = use_workspace();
    let toasts = use_toast();
    let (show_customize, set_show_customize) = signal(false);

    // Refresh the note index on mount.
    Effect::new(move |_| {
        let _ = ws.refresh_tree.get();
        crate::components::navigation::state::load_notes_index(nav);
    });

    // Derived lists from the index.
    let recently_modified = Memo::new(move |_| {
        let mut notes = nav.notes_index.get();
        notes.truncate(8);
        notes
    });
    let favourites = Memo::new(move |_| {
        let favs = nav.favourites.get();
        let index = nav.notes_index.get();
        favs.iter()
            .filter_map(|p| index.iter().find(|n| n.path == *p).cloned())
            .collect::<Vec<_>>()
    });
    let recently_opened = Memo::new(move |_| {
        let recent = nav.recent_notes.get();
        let index = nav.notes_index.get();
        recent
            .iter()
            .filter_map(|p| index.iter().find(|n| n.path == *p).cloned())
            .take(8)
            .collect::<Vec<_>>()
    });

    // Which sections are enabled (persisted).
    let enabled = Memo::new(move |_| nav.dashboard_sections.get());
    let is_enabled = move |id: &str| enabled.get().iter().any(|s| s == id);

    // Toggle a section.
    let toggle_section = move |id: &str| {
        let mut sections = nav.dashboard_sections.get();
        if sections.iter().any(|s| s == id) {
            sections.retain(|s| s != id);
        } else {
            sections.push(id.to_string());
        }
        set_dashboard_sections(nav, sections);
    };

    // Quick actions. Reuse the shared note-creation / daily-note helpers so
    // this logic lives in exactly one place (also used by the palette, home
    // screen and the ⌘N / ⌘⇧D shortcuts).
    let new_note = crate::components::navigation::commands::create_new_note(ws, toasts);
    let daily_note = crate::components::navigation::commands::open_daily_note(ws, toasts);

    // Quick action: open the vault folder in the OS file manager.
    let open_vault = crate::components::navigation::commands::open_vault_folder();

    // Quick action: continue working → open the most recent note.
    let continue_working = Callback::new(move |_| {
        let recent = nav.recent_notes.get();
        if let Some(path) = recent.first() {
            crate::components::workspace::open_tab(ws, path);
            record_recent_note(nav, path);
        } else {
            // Fall back to the most recently modified note.
            let index = nav.notes_index.get();
            if let Some(note) = index.first() {
                crate::components::workspace::open_tab(ws, &note.path);
            }
        }
    });

    view! {
        <div class="dashboard">
            <div class="dashboard-header">
                <div>
                    <h1 class="dashboard-title">"Home"</h1>
                    <p class="dashboard-subtitle">
                        {move || format!("Your vault: {}", nav.vault_name.get())}
                    </p>
                </div>
                <div class="relative">
                    <button
                        type="button"
                        class="btn btn-sm btn-ghost"
                        on:click=move |_| set_show_customize.update(|v| *v = !*v)
                    >
                        "⚙ Customize"
                    </button>
                    {move || if show_customize.get() {
                        view! {
                            <div class="dash-customize panel" on:click=move |ev| ev.stop_propagation()>
                                <div class="dash-customize-title">"Dashboard sections"</div>
                                {DASHBOARD_SECTIONS.iter().map(|id| {
                                    let id = *id;
                                    let label = crate::components::navigation::state::dashboard_section_label(id);
                                    let checked = is_enabled(id);
                                    view! {
                                        <label class="dash-customize-item">
                                            <input
                                                type="checkbox"
                                                checked=checked
                                                on:change=move |_| toggle_section(id)
                                            />
                                            <span>{label}</span>
                                        </label>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </div>
            </div>

            {move || if is_enabled("quick_actions") {
                view! {
                    <div class="dash-actions">
                        <button type="button" class="dash-action" on:click=move |_| new_note.run(())> "➕" <span>"New Note"</span></button>
                        <button type="button" class="dash-action" on:click=move |_| daily_note.run(())> "📅" <span>"Daily Note"</span></button>
                        <button type="button" class="dash-action" on:click=move |_| continue_working.run(())> "▶️" <span>"Continue Working"</span></button>
                        <button type="button" class="dash-action" on:click=move |_| open_vault.run(())> "📂" <span>"Open Vault"</span></button>
                        <button
                            type="button"
                            class="dash-action"
                            on:click=move |_| {
                                nav.search_query.set(String::new());
                                nav.view_mode.set(crate::components::navigation::state::ViewMode::Search);
                            }
                        >
                            "🔍" <span>"Search"</span>
                        </button>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            <div class="dash-grid">
                {move || if is_enabled("recently_modified") {
                    view! {
                        <NoteListSection
                            title="Recently Modified".to_string()
                            icon="🕒".to_string()
                            empty="No notes yet — create one with ⌘N."
                            notes=recently_modified.get()
                            highlight_paths=nav.favourites.get()
                        />
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("favourites") {
                    view! {
                        <NoteListSection
                            title="Favourites".to_string()
                            icon="⭐".to_string()
                            empty="Star notes to pin them here."
                            notes=favourites.get()
                            highlight_paths=nav.favourites.get()
                        />
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("recently_opened") {
                    view! {
                        <NoteListSection
                            title="Recently Opened".to_string()
                            icon="📂".to_string()
                            empty="Notes you open will show up here."
                            notes=recently_opened.get()
                            highlight_paths=Vec::new()
                        />
                    }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("pinned") {
                    view! { <PinnedSection /> }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("inbox") {
                    view! { <InboxSection /> }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("recent_searches") {
                    view! { <RecentSearchesSection /> }.into_any()
                } else {
                    view! {}.into_any()
                }}
                {move || if is_enabled("summary") {
                    view! { <SummarySection /> }.into_any()
                } else {
                    view! {}.into_any()
                }}
            </div>
        </div>
    }
}
