//! # Home screen
//!
//! Shown when no note is selected in the editor view. Provides a welcome,
//! recent activity (recently modified + recently opened) and quick actions:
//! create note, open daily note, open vault, continue working.

use crate::components::navigation::state::{record_recent_note, use_nav, NoteIndexEntry, ViewMode};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::workspace::use_workspace;
use leptos::prelude::*;

fn recent_activity(nav: crate::components::navigation::state::NavContext) -> Vec<NoteIndexEntry> {
    let index = nav.notes_index.get();
    let recent = nav.recent_notes.get();
    let mut merged: Vec<NoteIndexEntry> = recent
        .iter()
        .filter_map(|p| index.iter().find(|n| n.path == *p).cloned())
        .collect();
    // Fill with recently-modified notes not already listed.
    for note in index.iter().take(6) {
        if merged.len() >= 8 {
            break;
        }
        if !merged.iter().any(|m| m.path == note.path) {
            merged.push(note.clone());
        }
    }
    merged
}

/// The home screen — the informative landing when no note is selected.
#[component]
pub fn HomeScreen() -> impl IntoView {
    let nav = use_nav();
    let ws = use_workspace();
    let toasts = use_toast();
    let activity = Memo::new(move |_| recent_activity(nav));

    // Quick actions. Reuse the shared helpers from the command catalog so the
    // create-note / daily-note / open-vault logic lives in exactly one place.
    let create_note = crate::components::navigation::commands::create_new_note(ws, toasts);
    let daily_note = crate::components::navigation::commands::open_daily_note(ws, toasts);
    let open_vault = crate::components::navigation::commands::open_vault_folder();

    let continue_working = Callback::new(move |_| {
        let recent = nav.recent_notes.get();
        if let Some(path) = recent.first() {
            crate::components::workspace::open_tab(ws, path);
            record_recent_note(nav, path);
        } else if let Some(note) = nav.notes_index.get().first() {
            crate::components::workspace::open_tab(ws, &note.path);
        }
    });

    view! {
        <div class="home-screen">
            <div class="home-hero">
                <h1 class="home-title">
                    {move || format!("Welcome to {}", nav.vault_name.get())}
                </h1>
                <p class="home-subtitle">
                    "Your knowledge base is ready. Start a note, search everything, or open today's entry."
                </p>
                <div class="home-actions">
                    <button type="button" class="dash-action" on:click=move |_| create_note.run(())>
                        {render_icon_view(Icon::Plus)} <span>"Create Note"</span>
                    </button>
                    <button type="button" class="dash-action" on:click=move |_| daily_note.run(())>
                        {render_icon_view(Icon::Calendar)} <span>"Open Daily Note"</span>
                    </button>
                    <button type="button" class="dash-action" on:click=move |_| continue_working.run(())>
                        {render_icon_view(Icon::Play)} <span>"Continue Working"</span>
                    </button>
                    <button type="button" class="dash-action" on:click=move |_| open_vault.run(())>
                        {render_icon_view(Icon::FolderOpen)} <span>"Open Vault"</span>
                    </button>
                    <button
                        type="button"
                        class="dash-action"
                        on:click=move |_| {
                            nav.search_query.set(String::new());
                            nav.view_mode.set(ViewMode::Search);
                        }
                    >
                        {render_icon_view(Icon::Search)} <span>"Search"</span>
                    </button>
                </div>
            </div>

            <div class="home-activity">
                <h2 class="home-section-title">"Recent Activity"</h2>
                {move || {
                    let notes = activity.get();
                    if notes.is_empty() {
                        view! { <div class="dash-empty">"No notes yet — press ⌘N to create your first note."</div> }.into_any()
                    } else {
                        notes.into_iter().map(|note| {
                            let path = note.path.clone();
                            let title = note.title.clone();
                            let folder_display = if note.folder.is_empty() {
                                "/".to_string()
                            } else {
                                note.folder.clone()
                            };
                            view! {
                                <div
                                    class="dash-note"
                                    on:click=move |_| {
                                        crate::components::workspace::open_tab(ws, &path);
                                        record_recent_note(nav, &path);
                                    }
                                >
                                    <span class="dash-note-icon" aria-hidden="true">{render_icon_view(Icon::FileText)}</span>
                                    <span class="dash-note-main">
                                        <span class="dash-note-title">{title}</span>
                                        <span class="dash-note-folder">{folder_display}</span>
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
