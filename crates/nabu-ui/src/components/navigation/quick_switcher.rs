//! # Quick Switcher
//!
//! Keyboard-first note navigation:
//!
//! - fuzzy matching over note titles, folder paths and note names
//! - recent notes, pinned notes and the full vault index shown when the
//!   query is empty
//! - keyboard-only workflow: ↑/↓/Enter/Escape, ⌘P toggles
//!
//! Focused purely on *navigation* — it opens notes and never executes
//! commands. The Command Palette is the command surface.

use crate::components::contexts::{
    activate_tab, open_tab, use_nav, NavContext, WorkspaceContext, use_workspace,
};
use crate::components::navigation::state::{
    fuzzy_score, record_recent_note, NoteIndexEntry, ViewMode,
};
use crate::components::ui::feedback::set_timeout;
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// One row in the switcher list.
#[derive(Clone, PartialEq)]
enum Row {
    Header(String),
    Note(NoteIndexEntry),
}

/// Builds the ordered rows: Recent → Pinned → All notes (when empty),
/// otherwise fuzzy-filtered notes sorted by score.
fn build_rows(
    notes: &[NoteIndexEntry],
    recent: &[String],
    pinned: &[String],
    query: &str,
) -> Vec<Row> {
    let q = query.trim();
    if q.is_empty() {
        let mut rows = Vec::new();
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let recent_notes: Vec<NoteIndexEntry> = recent
            .iter()
            .filter_map(|p| notes.iter().find(|n| n.path == *p).cloned())
            .collect();
        if !recent_notes.is_empty() {
            rows.push(Row::Header("Recent".to_string()));
            for note in recent_notes {
                seen.insert(&note.path);
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
                seen.insert(&note.path);
                rows.push(Row::Note(note));
            }
        }
        let rest: Vec<NoteIndexEntry> = notes
            .iter()
            .filter(|n| !seen.contains(&*n.path))
            .cloned()
            .collect();
        let mut rest = rest;
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
            scored.push((score, note.clone()));
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

fn note_count(rows: &[Row]) -> usize {
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
fn open_note(mut nav: NavContext, ws: WorkspaceContext, path: String) {
    open_tab(ws, &path);
    record_recent_note(nav, &path);
    nav.switcher_open.set(false);
    nav.view_mode.set(ViewMode::Editor);
}

/// Pre-computed index for each row (Some(idx) for Note rows, None for Header rows).
fn indexed_rows(rows: &[Row]) -> Vec<(Row, Option<usize>)> {
    let mut note_idx = 0usize;
    rows.iter()
        .map(|row| match row {
            Row::Header(_) => (row.clone(), None),
            Row::Note(_) => {
                let idx = note_idx;
                note_idx += 1;
                (row.clone(), Some(idx))
            }
        })
        .collect()
}

/// Builds switcher row VNodes. Kept as a free function (outside rsx!) to avoid
/// `let` inside rsx! loops.
fn build_switcher_rows(
    indexed: &[(Row, Option<usize>)],
    mut active: Signal<usize>,
    mut nav: NavContext,
    mut workspace: WorkspaceContext,
) -> Vec<VNode> {
    indexed
        .iter()
        .map(|(row, note_idx_opt)| match row {
            Row::Header(cat) => rsx! {
                div { class: "palette-category", "{cat}" }
            },
            Row::Note(note) => {
                let this_idx = note_idx_opt.copied().unwrap_or(0);
                let title = note.title.clone();
                let folder = folder_of(note).to_string();
                let path = note.path.clone();
                let is_active = this_idx == *active.read();
                let nav_clone = nav;
                let ws = workspace;
                rsx! {
                    button {
                        r#type: "button",
                        role: "option",
                        "aria-selected": "{is_active}",
                        class: if is_active { "palette-item palette-item-active" } else { "palette-item" },
                        onmouseover: move |_| {
                            active.set(this_idx);
                        },
                        onclick: move |_| {
                            open_note(nav_clone, ws, path.clone());
                        },
                        span { class: "palette-item-icon", "aria-hidden": "true", {render_icon_view(Icon::FileText)} }
                        span { class: "palette-item-body" }
                        span { class: "palette-item-label", "{title}" }
                        span { class: "palette-item-desc", "{folder}" }
                    }
                }
            }
        })
        .collect()
}

/// The Quick Switcher overlay. Rendered once at the app root.
#[component]
pub fn QuickSwitcher() -> Element {
    let mut nav = use_nav();
    let open = nav.switcher_open;
    let workspace = use_workspace();
    let mut query = use_signal(|| String::new());
    let mut active = use_signal(|| 0usize);

    // Focus the input + reset state whenever the switcher opens.
    use_effect(move || {
        if *open.read() {
            query.set(String::new());
            active.set(0);
            set_timeout(
                move || {
                    if *open.read() {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(input) = document.get_element_by_id("quick-switcher-input") {
                                    if let Some(input) =
                                        input.dyn_ref::<web_sys::HtmlInputElement>()
                                    {
                                        let _ = input.focus();
                                    }
                                }
                            }
                        }
                    }
                },
                10,
            );
        }
    });

    let nav_ref = nav;

    // Pinned notes = pinned workspace tabs.
    let pinned_count = workspace.tabs.read().iter().filter(|t| t.pinned).count();

    // Compute rows inline.
    let rows = if *open.read() {
        let notes = nav.notes_index.read();
        let pinned: Vec<String> = if pinned_count > 0 {
            workspace
                .tabs
                .read()
                .iter()
                .filter(|t| t.pinned)
                .map(|t| t.path.clone())
                .collect()
        } else {
            Vec::new()
        };
        build_rows(
            &notes,
            &nav.recent_notes.read(),
            &pinned,
            &query.read(),
        )
    } else {
        Vec::new()
    };
    let count = note_count(&rows);

    // Pre-compute indexed rows and VNodes (avoids `let` inside rsx! loops).
    let indexed = indexed_rows(&rows);
    let switcher_rows: Vec<VNode> = if count > 0 {
        build_switcher_rows(&indexed, active, nav_ref, workspace)
    } else {
        Vec::new()
    };

    rsx! {
        if *open.read() {
            div {
                class: "dialog-overlay palette-overlay",
                onclick: move |_| {
                    nav.switcher_open.set(false);
                    query.set(String::new());
                },
                div {
                    class: "palette panel",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": "Quick switcher",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    input {
                        id: "quick-switcher-input",
                        class: "palette-input",
                        r#type: "text",
                        placeholder: "Jump to a note…",
                        value: "{*query.read()}",
                        onchange: move |ev: FormEvent| {
                            query.set(ev.value());
                            active.set(0);
                        },
                        onkeydown: move |ev: KeyboardEvent| {
                            let key = ev.key();
                            if key == Key::Escape {
                                ev.prevent_default();
                                nav.switcher_open.set(false);
                                query.set(String::new());
                            } else if key == Key::ArrowDown {
                                ev.prevent_default();
                                if count == 0 {
                                    active.set(0);
                                } else {
                                    let a = *active.read();
                                    active.set((a + 1) % count);
                                }
                            } else if key == Key::ArrowUp {
                                ev.prevent_default();
                                if count == 0 {
                                    active.set(0);
                                } else {
                                    let a = *active.read();
                                    let c = count;
                                    active.set(if a == 0 { c - 1 } else { a - 1 });
                                }
                            } else if key == Key::Enter {
                                ev.prevent_default();
                                let mut idx = *active.read();
                                for row in &rows {
                                    if let Row::Note(note) = row {
                                        if idx == 0 {
                                            open_note(nav_ref, workspace, note.path.clone());
                                            break;
                                        }
                                        idx -= 1;
                                    }
                                }
                            }
                        },
                    }
                    kbd { class: "palette-hint", "esc" }
                    div { class: "palette-list" }
                    if count == 0 {
                        div { class: "palette-empty" }
                        if query.read().trim().is_empty() {
                            "No notes in the vault yet"
                        } else {
                            "No notes match"
                        }
                    }
                    for vnode in switcher_rows {
                        {vnode}
                    }
                    div { class: "palette-footer" }
                    span { "↑↓ navigate" }
                    span { "↵ open" }
                    span { "esc close" }
                }
            }
        }
    }
}
