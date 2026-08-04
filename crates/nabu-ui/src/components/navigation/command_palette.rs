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
//! Runs every application command registered in [`all_commands`]. Opening the
//! palette from elsewhere should call `nav.palette_open.set(true)` — the
//! overlay is rendered once at the app root and reacts to that signal.

use crate::components::navigation::commands::{all_commands, AppCommand, CommandContext};
use crate::components::navigation::state::{
    fuzzy_score, record_recent_command, toggle_favourite_command, use_nav,
};
use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// One row in the palette list — a category header or a command entry.
#[derive(Clone)]
enum Row {
    Header(String),
    Command(AppCommand),
}

/// Builds the ordered rows: recent → favourites → categories (when the query
/// is empty), otherwise fuzzy-filtered commands grouped by category.
fn build_rows(
    catalog: Vec<AppCommand>,
    query: &str,
    recent_ids: &[String],
    fav_ids: &[String],
) -> Vec<Row> {
    let q = query.trim();
    if q.is_empty() {
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let recents =
            crate::components::navigation::commands::resolve_commands_by_id(&catalog, recent_ids);
        if !recents.is_empty() {
            rows.push(Row::Header("Recent".to_string()));
            for cmd in recents {
                seen.insert(cmd.id);
                rows.push(Row::Command(cmd));
            }
        }

        let favs =
            crate::components::navigation::commands::resolve_commands_by_id(&catalog, fav_ids);
        let favs: Vec<AppCommand> = favs.into_iter().filter(|c| !seen.contains(c.id)).collect();
        if !favs.is_empty() {
            rows.push(Row::Header("Favourites".to_string()));
            for cmd in favs {
                seen.insert(cmd.id);
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
        return rows;
    }

    // Fuzzy filtering: score each command on label + aliases + description.
    let mut scored: Vec<(u32, AppCommand)> = Vec::new();
    for cmd in catalog {
        let mut best: Option<u32> = None;
        for text in [cmd.label]
            .into_iter()
            .chain(cmd.aliases.iter().copied())
            .chain([cmd.category, cmd.description])
        {
            if let Some(s) = fuzzy_score(q, text) {
                best = Some(best.map_or(s, |b: u32| b.max(s)));
            }
        }
        if let Some(score) = best {
            scored.push((score, cmd));
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
pub fn CommandPalette() -> impl IntoView {
    let nav = use_nav();
    let open = nav.palette_open;
    let (query, set_query) = signal(String::new());
    let (active, set_active) = signal(0usize);

    // Build the catalog at render time (captures nav/workspace/toasts by value).
    let toasts = crate::components::ui::feedback::use_toast();
    let workspace = crate::components::workspace::use_workspace();
    let ctx = CommandContext {
        nav,
        workspace,
        toasts,
    };
    let catalog = all_commands(ctx);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    // Focus the input + reset state whenever the palette opens.
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

    let run_command = Callback::new(move |cmd: AppCommand| {
        record_recent_command(nav, cmd.id);
        open.set(false);
        cmd.run.run(());
    });

    // Derived rows. `Row` wraps `AppCommand` (contains a `Callback`, which is
    // not `PartialEq`), so use `Signal::derive` rather than `Memo::new`.
    let rows = Signal::derive(move || {
        build_rows(
            catalog.clone(),
            &query.get(),
            &nav.recent_commands.get(),
            &nav.favourite_commands.get(),
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

            // Keyboard navigation on the input.
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
                        if let Row::Command(cmd) = row {
                            if idx == 0 {
                                run_command.run(cmd);
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
                        aria-label="Command palette"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="palette-search-wrap">
                            <span class="palette-search-icon" aria-hidden="true">"⌘"</span>
                            <input
                                node_ref=input_ref
                                class="palette-input"
                                type="text"
                                placeholder="Type a command or search…"
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
                                            "No commands yet".to_string()
                                        } else {
                                            format!("No commands match “{}”", query.get().trim())
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                let mut cmd_idx = 0usize;
                                rows_list.into_iter().map(|row| {
                                    match row {
                                        Row::Header(cat) => {
                                            view! { <div class="palette-category">{cat}</div> }.into_any()
                                        }
                                        Row::Command(cmd) => {
                                            let this_idx = cmd_idx;
                                            cmd_idx += 1;
                                            let is_active = this_idx == active_idx;
                                            let id = cmd.id;
                                            let icon = cmd.icon;
                                            let label = cmd.label;
                                            let description = cmd.description;
                                            let shortcut = cmd.shortcut;
                                            let is_fav = nav.favourite_commands.with(|f| f.iter().any(|c| *c == id));
                                            let cmd_for_run = cmd.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    role="option"
                                                    aria-selected=is_active
                                                    class=move || format!("palette-item{}", if is_active { " palette-item-active" } else { "" })
                                                    on:mouseenter=move |_| set_active.set(this_idx)
                                                    on:click=move |_| run_command.run(cmd_for_run.clone())
                                                >
                                                    <span class="palette-item-icon" aria-hidden="true">{render_icon_view(icon)}</span>
                                                    <span class="palette-item-body">
                                                        <span class="palette-item-label">{label}</span>
                                                        <span class="palette-item-desc">{description}</span>
                                                    </span>
                                                    {shortcut.map(|s| view! { <kbd class="palette-shortcut">{s}</kbd> }.into_any())}
                                                    <span
                                                        class="palette-star"
                                                        title=move || if is_fav { "Remove from favourites" } else { "Add to favourites" }
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            toggle_favourite_command(nav, id);
                                                        }
                                                    >
                                                        {move || if is_fav { render_icon_view(Icon::Star) } else { render_icon_view(Icon::StarHalf) }}
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
                            <span>"↵" " run"</span>
                            <span>{render_icon_view(Icon::Star)} " favourite"</span>
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
