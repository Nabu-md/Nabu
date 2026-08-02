//! # Quick Switcher
//!
//! Keyboard-first note navigation:
//!
//! - fuzzy matching over note titles, folder paths and note names
//! - recent notes, pinned notes and the full vault index shown when the
//!   query is empty
//! - folder-aware search (matches against `folder/title` and the folder path)
//! - keyboard-only workflow: ↑/↓/Enter/Escape, ⌘P toggles
//!
//! Focused purely on *navigation* — it opens notes and never executes
//! commands. The Command Palette is the command surface.

use crate::components::navigation::state::{fuzzy_score, record_recent_note, use_nav, NoteIndexEntry};
use crate::components::workspace::{use_workspace, WorkspaceContext};
use leptos::prelude::*;

/// One row in the switcher list.
#[derive(Clone, PartialEq)]
enum Row {
    Header(String),
    Note(NoteIndexEntry),
}

/// Builds the ordered rows: Recent → Pinned → All notes (when empty),
/// otherwise fuzzy-filtered notes sorted by score.
fn build_rows(
    notes: Vec<NoteIndexEntry>,
    recent: &[String],
    pinned: &[String],
    query: &str,
) -> Vec<Row> {
    let q = query.trim();
    if q.is_empty() {
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let recent_notes: Vec<NoteIndexEntry> = recent
            .iter()
            .filter_map(|p| notes.iter().find(|n| n.path == *p).cloned())
            .collect();
        if !recent_notes.is_empty() {
            rows.push(Row::Header("Recent".to_string()));
            for note in recent_notes {
                seen.insert(note.path.clone());
                rows.push(Row::Note(note));
            }
        }
        let pinned_notes: Vec<NoteIndexEntry> = pinned
            .iter()
            .filter_map(|p| notes.iter().find(|n| n.path == *p).cloned())
            .collect();
        if !pinned_notes.is_empty() {
            rows.push(Row::Header("Pinned".to_string()));
            for note in pinned_notes {
                seen.insert(note.path.clone());
                rows.push(Row::Note(note));
            }
        }
        let mut rest: Vec<NoteIndexEntry> = notes.into_iter().filter(|n| !seen.contains(&n.path)).collect();
        rest.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        if !rest.is_empty() {
            rows.push(Row::Header("All notes".to_string()));
            for note in rest {
                rows.push(Row::Note(note));
            }
        }
        return rows;
    }

    // Fuzzy match on `title`, `folder/title` and the folder path.
    let mut scored: Vec<(u32, NoteIndexEntry)> = Vec::new();
    for note in notes {
        let mut best: Option<u32> = None;
        let consider = |text: &str, best: &mut Option<u32>| {
            if let Some(s) = fuzzy_score(q, text) {
                *best = Some(best.map_or(s, |b: u32| b.max(s)));
            }
        };
        consider(&note.title, &mut best);
        let folder_path = if note.folder.is_empty() {
            note.title.clone()
        } else {
            format!("{}/{}", note.folder, note.title)
        };
        consider(&folder_path, &mut best);
        consider(&note.folder, &mut best);
        consider(&note.path, &mut best);
        if let Some(score) = best {
            scored.push((score, note));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
    let mut rows = Vec::new();
    if !scored.is_empty() {
        rows.push(Row::Header(format!("{} matches", scored.len())));
    }
    for (_, note) in scored {
        rows.push(Row::Note(note));
    }
    rows
}

fn command_count(rows: &[Row]) -> usize {
    rows.iter().filter(|r| matches!(r, Row::Note(_))).count()
}

fn folder_of(note: &NoteIndexEntry) -> &str {
    if note.folder.is_empty() {
        "/"
    } else {
        &note.folder
    }
}

/// Opens a note in the workspace, records it as recent and closes the overlay.
fn open_note(nav: crate::components::navigation::state::NavContext, ws: WorkspaceContext, path: String) {
    crate::components::workspace::open_tab(ws, &path);
    record_recent_note(nav, &path);
    nav.switcher_open.set(false);
    // Make sure we're in the editor view so the note is visible.
    nav.view_mode.set(crate::components::navigation::state::ViewMode::Editor);
}

/// The Quick Switcher overlay. Rendered once at the app root.
#[component]
pub fn QuickSwitcher() -> impl IntoView {
    let nav = use_nav();
    let open = nav.switcher_open;
    let workspace = use_workspace();
    let (query, set_query) = signal(String::new());
    let (active, set_active) = signal(0usize);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if open.get() {
            set_query.set(String::new());
            set_active.set(0);
            set_timeout(
                move || {
                    if let Some(el) = input_ref.get() {
                        let _ = el.focus();
                    }
                },
                std::time::Duration::from_millis(10),
            );
        }
    });

    // Pinned notes = pinned workspace tabs.
    let pinned = Memo::new(move |_| {
        workspace
            .tabs
            .get()
            .into_iter()
            .filter(|t| t.pinned)
            .map(|t| t.path)
            .collect::<Vec<_>>()
    });

    let rows = Memo::new(move |_| {
        build_rows(
            nav.notes_index.get(),
            &nav.recent_notes.get(),
            &pinned.get(),
            &query.get(),
        )
    });

    let close = Callback::new(move |_| {
        open.set(false);
        set_query.set(String::new());
    });

    view! {
        {move || if open.get() {
            let rows_list = rows.get();
            let count = command_count(&rows_list);
            let active_idx = active.get().min(count.saturating_sub(1));

            let on_keydown = move |ev: web_sys::KeyboardEvent| {
                let key = ev.key();
                if key == "Escape" {
                    ev.prevent_default();
                    close.run(());
                } else if key == "ArrowDown" {
                    ev.prevent_default();
                    set_active.update(|i| *i = if count == 0 { 0 } else { (*i + 1) % count });
                } else if key == "ArrowUp" {
                    ev.prevent_default();
                    set_active.update(|i| *i = if count == 0 { 0 } else { (*i + count - 1) % count });
                } else if key == "Enter" {
                    ev.prevent_default();
                    let rows_now = rows.get();
                    let mut idx = active.get();
                    for row in rows_now {
                        if let Row::Note(note) = row {
                            if idx == 0 {
                                open_note(nav, workspace, note.path);
                                break;
                            }
                            idx -= 1;
                        }
                    }
                }
            };

            view! {
                <div class="dialog-overlay palette-overlay" on:click=move |_| close.run(())>
                    <div
                        class="palette panel"
                        role="dialog"
                        aria-modal="true"
                        aria-label="Quick switcher"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="palette-search-wrap">
                            <span class="palette-search-icon" aria-hidden="true">"⚡"</span>
                            <input
                                node_ref=input_ref
                                class="palette-input"
                                type="text"
                                placeholder="Jump to a note…"
                                prop:value=query
                                on:input=move |ev| {
                                    set_query.set(event_target_value(&ev));
                                    set_active.set(0);
                                }
                                on:keydown=on_keydown
                            />
                            <kbd class="palette-hint">"esc"</kbd>
                        </div>
                        <div class="palette-list">
                            {if count == 0 {
                                view! {
                                    <div class="palette-empty">
                                        {if query.get().trim().is_empty() {
                                            "No notes in the vault yet".to_string()
                                        } else {
                                            format!("No notes match “{}”", query.get().trim())
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                let mut note_idx = 0usize;
                                rows_list.into_iter().map(|row| {
                                    match row {
                                        Row::Header(cat) => {
                                            view! { <div class="palette-category">{cat}</div> }.into_any()
                                        }
                                        Row::Note(note) => {
                                            let this_idx = note_idx;
                                            note_idx += 1;
                                            let is_active = this_idx == active_idx;
                                            let title = note.title.clone();
                                            let folder = folder_of(&note).to_string();
                                            let path = note.path.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    role="option"
                                                    aria-selected=is_active
                                                    class=move || format!("palette-item{}", if is_active { " palette-item-active" } else { "" })
                                                    on:mouseenter=move |_| set_active.set(this_idx)
                                                    on:click=move |_| open_note(nav, workspace, path.clone())
                                                >
                                                    <span class="palette-item-icon" aria-hidden="true">"📄"</span>
                                                    <span class="palette-item-body">
                                                        <span class="palette-item-label">{title}</span>
                                                        <span class="palette-item-desc">{folder}</span>
                                                    </span>
                                                </button>
                                            }.into_any()
                                        }
                                    }
                                }).collect_view().into_any()
                            }}
                        </div>
                        <div class="palette-footer">
                            <span>"↑↓" " navigate"</span>
                            <span>"↵" " open"</span>
                            <span>"esc" " close"</span>
                        </div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
