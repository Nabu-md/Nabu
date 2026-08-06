//! # Command Palette
//!
//! A complete command centre overlay:
//!
//! - fuzzy search across label, aliases, category and description
//! - categories as grouped headers
//! - recent commands and favourite commands pinned to the top when the
//!   query is empty
//! - keyboard-only navigation (↑/↓/Enter/Escape, ⌘K toggles)
//! - star a command to favourite it (persisted)
//!
//! Opening the palette from elsewhere should call `nav.palette_open.set(true)`
//! — the overlay is rendered once at the app root and reacts to that signal.

use crate::components::contexts::{use_nav, WorkspaceContext, use_workspace};
use crate::components::navigation::commands::{all_commands, AppCommand, CommandContext};
use crate::components::navigation::state::{
    fuzzy_score, record_recent_command, toggle_favourite_command, NoteIndexEntry,
};
use crate::components::ui::feedback::{set_timeout, use_toast};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// One row in the palette list — a category header or a command entry.
#[derive(Clone)]
enum Row {
    Header(String),
    Command(AppCommand),
}

/// Builds the ordered rows: recent → favourites → categories (when the query
/// is empty), otherwise fuzzy-filtered commands grouped by category.
fn build_rows(
    catalog: &[AppCommand],
    query: &str,
    recent_ids: &[String],
    fav_ids: &[String],
) -> Vec<Row> {
    let q = query.trim();
    if q.is_empty() {
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let recents: Vec<AppCommand> =
            crate::components::navigation::commands::resolve_commands_by_id(catalog, recent_ids);
        if !recents.is_empty() {
            rows.push(Row::Header("Recent".to_string()));
            for cmd in recents {
                seen.insert(*cmd.id);
                rows.push(Row::Command(cmd));
            }
        }

        let favs: Vec<AppCommand> =
            crate::components::navigation::commands::resolve_commands_by_id(catalog, fav_ids)
                .into_iter()
                .filter(|c| !seen.contains(c.id))
                .collect();
        if !favs.is_empty() {
            rows.push(Row::Header("Favourites".to_string()));
            for cmd in favs {
                seen.insert(*cmd.id);
                rows.push(Row::Command(cmd));
            }
        }

        // Remaining commands grouped by category.
        let mut groups: Vec<(&str, Vec<AppCommand>)> = Vec::new();
        for cmd in catalog {
            if seen.contains(cmd.id) {
                continue;
            }
            match groups.iter_mut().find(|(cat, _)| *cat == cmd.category) {
                Some((_, list)) => list.push(cmd.clone()),
                None => groups.push((cmd.category, vec![cmd.clone()])),
            }
        }
        for (category, list) in groups {
            rows.push(Row::Header(category.to_string()));
            for cmd in list {
                rows.push(Row::Command(cmd));
            }
        }
        return rows;
    }

    // Fuzzy filtering: score each command on label + aliases + description.
    let mut scored: Vec<(u32, AppCommand)> = Vec::new();
    for cmd in catalog {
        let mut best: Option<u32> = None;
        for text in std::iter::once(cmd.label)
            .chain(cmd.aliases.iter().copied())
            .chain([cmd.category, cmd.description])
        {
            if let Some(s) = fuzzy_score(q, text) {
                best = Some(best.map_or(s, |b: u32| b.max(s)));
            }
        }
        if let Some(score) = best {
            scored.push((score, cmd.clone()));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let mut rows = Vec::new();
    let mut groups: Vec<(&str, Vec<AppCommand>)> = Vec::new();
    for (_, cmd) in scored {
        match groups.iter_mut().find(|(cat, _)| *cat == cmd.category) {
            Some((_, list)) => list.push(cmd),
            None => groups.push((cmd.category, vec![cmd])),
        }
    }
    for (category, list) in groups {
        rows.push(Row::Header(category.to_string()));
        for cmd in list {
            rows.push(Row::Command(cmd));
        }
    }
    rows
}

/// Counts the command rows (headers excluded).
fn command_count(rows: &[Row]) -> usize {
    rows.iter().filter(|r| matches!(r, Row::Command(_))).count()
}

/// The Command Palette overlay. Rendered once at the app root.
#[component]
pub fn CommandPalette() -> Element {
    let nav = use_nav();
    let open = nav.palette_open;
    let query = use_signal(|| String::new());
    let active = use_signal(|| 0usize);

    // Build the catalog at render time (captures nav/workspace/toasts by value).
    let toasts = use_toast();
    let workspace = use_workspace();
    let ctx = CommandContext {
        nav,
        workspace,
        toasts,
    };
    let catalog = all_commands(ctx);

    // Focus the input whenever the palette opens.
    use_effect(move || {
        if *open.read() {
            query.set(String::new());
            active.set(0);
            set_timeout(
                move || {
                    if *open.read() {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(input) =
                                    document.get_element_by_id("command-palette-input")
                                {
                                    if let Some(input) = input
                                        .dyn_ref::<web_sys::HtmlInputElement>()
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

    // Compute rows inline (cannot use use_memo — Row wraps Callback which is
    // not PartialEq).
    let rows = if *open.read() {
        build_rows(
            &catalog,
            &query.read(),
            &nav.recent_commands.read(),
            &nav.favourite_commands.read(),
        )
    } else {
        Vec::new()
    };
    let count = command_count(&rows);

    rsx! {
        if *open.read() {
            div {
                class: "dialog-overlay palette-overlay",
                onclick: move |_| {
                    nav.palette_open.set(false);
                    query.set(String::new());
                },
                div {
                    class: "palette panel",
                    role: "dialog",
                    "aria-modal": "true",
                    "aria-label": "Command palette",
                    onclick: |ev: MouseEvent| ev.stop_propagation(),
                    div { class: "palette-search-wrap" }
                    span { class: "palette-search-icon", "aria-hidden": "true", {render_icon_view(Icon::Command)} }
                    input {
                        id: "command-palette-input",
                        class: "palette-input",
                        r#type: "text",
                        placeholder: "Type a command or search…",
                        value: "{*query.read()}",
                        onchange: move |ev: FormEvent| {
                            query.set(ev.value());
                            active.set(0);
                        },
                        onkeydown: move |ev: KeyboardEvent| {
                            let key = ev.key();
                            if key == Key::Escape {
                                ev.prevent_default();
                                nav.palette_open.set(false);
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
                                    if let Row::Command(cmd) = row {
                                        if idx == 0 {
                                            record_recent_command(nav_ref, cmd.id);
                                            nav.palette_open.set(false);
                                            query.set(String::new());
                                            cmd.run.call(());
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
                            "No commands yet"
                        } else {
                            "No commands match"
                        }
                    } else {
                        let mut cmd_idx = 0usize;
                        for row in &rows {
                            match row {
                                Row::Header(cat) => {
                                    div { class: "palette-category", "{cat}" }
                                }
                                Row::Command(cmd) => {
                                    let this_idx = cmd_idx;
                                    cmd_idx += 1;
                                    let is_active = this_idx == *active.read();
                                    let cmd_id = cmd.id;
                                    let cmd_icon = cmd.icon;
                                    let cmd_label = cmd.label;
                                    let cmd_desc = cmd.description;
                                    let cmd_shortcut = cmd.shortcut;
                                    let is_fav = nav_ref
                                        .favourite_commands
                                        .read()
                                        .iter()
                                        .any(|c| *c == cmd_id);
                                    let cmd_for_run = cmd.clone();
                                    let nav_clone = nav_ref;
                                    let fav_nav = nav_ref;
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
                                                record_recent_command(nav_clone, cmd_id);
                                                nav_clone.palette_open.set(false);
                                                query.set(String::new());
                                                cmd_for_run.run.call(());
                                            },
                                            span { class: "palette-item-icon", "aria-hidden": "true", {render_icon_view(cmd_icon)} }
                                            span { class: "palette-item-body" }
                                            span { class: "palette-item-label", "{cmd_label}" }
                                            span { class: "palette-item-desc", "{cmd_desc}" }
                                            if let Some(s) = cmd_shortcut {
                                                kbd { class: "palette-shortcut", "{s}" }
                                            }
                                            span {
                                                class: "palette-star",
                                                title: if is_fav { "Remove from favourites" } else { "Add to favourites" },
                                                onclick: move |_| {
                                                    toggle_favourite_command(fav_nav, cmd_id);
                                                },
                                                if is_fav {
                                                    {render_icon_view(Icon::Star)}
                                                } else {
                                                    {render_icon_view(Icon::StarHalf)}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "palette-footer" }
                    span { "↑↓ navigate" }
                    span { "↵ run" }
                    {render_icon_view(Icon::Star)}
                    " favourite"
                    span { "esc close" }
                }
            }
        }
    }
}
