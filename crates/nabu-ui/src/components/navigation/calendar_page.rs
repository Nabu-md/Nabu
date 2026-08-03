//! # Calendar — date-based navigation workspace
//!
//! Phase 13.2 dedicated calendar for navigating notes by date. Supports month,
//! week, day and year views over the vault's dated notes (frontmatter
//! `date:`/`created:` when present, else file mtime), and clicking a date
//! opens or creates the daily note for that day (`daily_note_for`).
//!
//! ## Reactivity note
//!
//! Toast and workspace contexts are `Copy` and captured at render time, then
//! threaded into async tasks as plain values — never `expect_context` inside a
//! `spawn_local` future (no reactive owner on the failure path).

use crate::components::ui::feedback::use_toast;
use crate::components::ui::selection::{Segmented, SegmentedOption};
use crate::components::workspace::{open_tab, use_workspace};
use crate::models::organisation::CalendarEntry;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Calendar granularity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum CalMode {
    #[default]
    Month,
    Week,
    Day,
    Year,
}

/// Formats a JS date as `YYYY-MM-DD`.
fn fmt_date(d: &js_sys::Date) -> String {
    let y = d.get_full_year();
    let m = d.get_month() + 1;
    let day = d.get_date();
    format!("{y:04}-{m:02}-{day:02}")
}

/// `YYYY-MM-DD` → display `MMM D` / `D` helper (date components from strings).
fn date_components(date: &str) -> (i32, u32, u32) {
    let mut it = date.split('-');
    let y = it.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let d = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m, d)
}

/// Human short month name (0-based month index).
fn month_name(m0: u32) -> &'static str {
    match m0 {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        _ => "Dec",
    }
}

/// The Calendar workspace.
#[component]
pub fn CalendarPage() -> impl IntoView {
    let ws = use_workspace();
    let toasts = use_toast();

    // Cursor date (first of the currently shown month).
    let (cursor, set_cursor) = signal(js_sys::Date::new_0());
    let (mode, set_mode) = signal(CalMode::Month);
    // String mirror of `mode` — the shared `Segmented` control binds a string.
    let mode_str = RwSignal::new("month".to_string());
    let (notes, set_notes) = signal(Vec::<CalendarEntry>::new());
    let (loading, set_loading) = signal(false);

    // Load notes for the cursor's month.
    let load_month = {
        let set_notes = set_notes;
        let set_loading = set_loading;
        let cursor = cursor;
        move || {
            let month = {
                let c = cursor.get_untracked();
                format!("{}-{:02}", c.get_full_year(), c.get_month() + 1)
            };
            set_loading.set(true);
            spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&serde_json::json!({ "month": month })).unwrap();
                let result = crate::ipc::tauri_invoke("calendar_notes", args).await;
                set_loading.set(false);
                if let Ok(hits) = serde_wasm_bindgen::from_value::<Vec<CalendarEntry>>(result) {
                    set_notes.set(hits);
                }
            });
        }
    };

    // Initial load + reload when the month changes.
    Effect::new(move |_| {
        let _ = cursor.get();
        load_month();
    });

    // Navigate months.
    let shift_month = Callback::new(move |delta: i32| {
        let c = cursor.get_untracked();
        let y = c.get_full_year();
        let m = c.get_month() as i32 + delta;
        let (ny, nm) = if m < 0 {
            (y - 1, 11)
        } else if m > 11 {
            (y + 1, 0)
        } else {
            (y, m)
        };
        set_cursor.set(js_sys::Date::new_with_year_month_day(ny as u32, nm as i32, 1));
    });

    // Open (or create) the daily note for a date.
    let open_daily = Callback::new(move |date: String| {
        let toasts = toasts;
        let ws = ws;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "date": date.clone() }))
                .unwrap();
            let result = crate::ipc::tauri_invoke("daily_note_for", args).await;
            if let Ok(path) = serde_wasm_bindgen::from_value::<String>(result) {
                open_tab(ws, &path);
                toasts.success("Daily note", format!("Opened {path}"));
            } else {
                toasts.error("Daily note", "Could not open that day's note.");
            }
        });
    });

    // Build the day cells for the current month.
    let month_cells = move || {
        let c = cursor.get_untracked();
        let y = c.get_full_year();
        let m0 = c.get_month();
        let first = js_sys::Date::new_with_year_month_day(y as u32, m0 as i32, 1);
        let lead = first.get_day() as usize;
        let days_in_month = js_sys::Date::new_with_year_month_day(y as u32, (m0 + 1) as i32, 0).get_date() as usize;
        let mut cells: Vec<(Option<String>, Vec<CalendarEntry>)> = Vec::new();
        for _ in 0..lead {
            cells.push((None, Vec::new()));
        }
        for d in 1..=days_in_month {
            let date = format!("{y:04}-{:02}-{d:02}", m0 + 1);
            let day_notes = notes
                .get()
                .into_iter()
                .filter(|n| n.date == date)
                .collect::<Vec<_>>();
            cells.push((Some(date), day_notes));
        }
        cells
    };

    // Week view: 7 days around the cursor (Monday-start). Uses calendar
    // arithmetic (Date normalises out-of-range day components) so DST
    // transitions can't skip or duplicate a local date.
    let week_cells = move || {
        let c = cursor.get_untracked();
        let dow = c.get_day() as i32;
        let monday_offset = if dow == 0 { -6 } else { 1 - dow };
        let mut cells = Vec::new();
        for i in 0..7 {
            let d = js_sys::Date::new_with_year_month_day(
                c.get_full_year() as u32,
                c.get_month() as i32,
                c.get_date() as i32 + monday_offset + i,
            );
            let date = fmt_date(&d);
            let day_notes = notes
                .get()
                .into_iter()
                .filter(|n| n.date == date)
                .collect::<Vec<_>>();
            cells.push((date, day_notes));
        }
        cells
    };

    // Year view: 12 month summaries.
    let year_cells = move || {
        let c = cursor.get_untracked();
        let y = c.get_full_year();
        let all = notes.get();
        (0..12)
            .map(|m| {
                let prefix = format!("{y:04}-{:02}", m + 1);
                let count = all.iter().filter(|n| n.date.starts_with(&prefix)).count();
                (m, prefix, count)
            })
            .collect::<Vec<_>>()
    };

    let mode_options = vec![
        SegmentedOption { value: "month".to_string(), label: "Month".to_string() },
        SegmentedOption { value: "week".to_string(), label: "Week".to_string() },
        SegmentedOption { value: "day".to_string(), label: "Day".to_string() },
        SegmentedOption { value: "year".to_string(), label: "Year".to_string() },
    ];

    // Keep `mode_str` and `mode` in sync from the segmented control.
    Effect::new(move |_| {
        let v = mode_str.get();
        let next = match v.as_str() {
            "week" => CalMode::Week,
            "day" => CalMode::Day,
            "year" => CalMode::Year,
            _ => CalMode::Month,
        };
        if mode.get_untracked() != next {
            set_mode.set(next);
        }
    });

    let day_notes_for = move |date: String| -> Vec<CalendarEntry> {
        notes
            .get()
            .into_iter()
            .filter(|n| n.date == date)
            .collect::<Vec<_>>()
    };

    view! {
        <div class="space-y-6">
            <header class="flex items-center justify-between">
                <div>
                    <h1 class="text-xl font-semibold text-gray-100">"Calendar"</h1>
                    <p class="text-sm text-gray-400 mt-1">
                        "Browse notes by date. Click any day to open or create its daily note."
                    </p>
                </div>
                <div class="flex items-center gap-3">
                    <Segmented selected=mode_str options=mode_options />
                    <button
                        type="button"
                        class="btn btn-outline btn-sm"
                        aria-label="Previous"
                        on:click=move |_| shift_month.run(-1)
                    >
                        "◀"
                    </button>
                    <div class="text-sm font-medium text-gray-200 min-w-24 text-center">
                        {move || {
                            let c = cursor.get();
                            if mode.get() == CalMode::Year {
                                format!("{}", c.get_full_year())
                            } else {
                                format!("{} {}", month_name(c.get_month()), c.get_full_year())
                            }
                        }}
                    </div>
                    <button
                        type="button"
                        class="btn btn-outline btn-sm"
                        aria-label="Next"
                        on:click=move |_| shift_month.run(1)
                    >
                        "▶"
                    </button>
                    {move || if loading.get() {
                        view! { <span class="text-xs text-gray-400">"…"</span> }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </div>
            </header>

            // Month view
            {move || if mode.get() == CalMode::Month {
                view! {
                    <div>
                        <div class="grid grid-cols-7 gap-1 mb-1">
                            {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().map(|d| {
                                view! { <div class="text-center text-xs text-gray-500 py-1 font-medium">{*d}</div> }
                            }).collect_view()}
                        </div>
                        <div class="grid grid-cols-7 gap-1">
                            {month_cells().into_iter().map(|(date_opt, day_notes)| {
                                match date_opt {
                                    None => view! { <div class="min-h-24 rounded-md border border-gray-800"></div> }.into_any(),
                                    Some(date) => {
                                        let is_today = date == {
                                            let now = js_sys::Date::new_0();
                                            fmt_date(&now)
                                        };
                                        let date_open = date.clone();
                                        let day_notes = day_notes;
                                        view! {
                                            <div
                                                class=format!(
                                                    "min-h-24 rounded-md border p-1.5 text-left align-top transition-colors {}",
                                                    if is_today { "border-blue-600 bg-gray-800" } else { "border-gray-800 hover:border-gray-600 hover:bg-gray-800/60" }
                                                )
                                            >
                                                <button
                                                    type="button"
                                                    class="text-xs text-gray-400 hover:text-gray-200"
                                                    aria-label=format!("Open daily note for {}", date)
                                                    on:click=move |_| open_daily.run(date_open.clone())
                                                >
                                                    {date.split('-').next_back().unwrap_or("")}
                                                </button>
                                                {day_notes.iter().take(3).map(|n| {
                                                    let path = n.path.clone();
                                                    let title = n.title.clone();
                                                    view! {
                                                        <div
                                                            class="text-xs text-blue-400 truncate mt-0.5 cursor-pointer hover:underline"
                                                            title=title.clone()
                                                            on:click=move |_| open_tab(ws, &path)
                                                        >
                                                            {title}
                                                        </div>
                                                    }
                                                }).collect_view()}
                                                {if day_notes.len() > 3 {
                                                    view! { <div class="text-[10px] text-gray-500 mt-0.5">{"+" .to_string() + &(day_notes.len() - 3).to_string() + " more"}</div> }.into_any()
                                                } else {
                                                    view! {}.into_any()
                                                }}
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }.into_any()
            } else if mode.get() == CalMode::Week {
                view! {
                    <div class="grid grid-cols-7 gap-1">
                        {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().map(|d| {
                            view! { <div class="text-center text-xs text-gray-500 py-1 font-medium">{*d}</div> }
                        }).collect_view()}
                        {week_cells().into_iter().map(|(date, day_notes)| {
                            let is_today = date == {
                                let now = js_sys::Date::new_0();
                                fmt_date(&now)
                            };
                            let date_open = date.clone();
                            let day_notes = day_notes;
                            view! {
                                <div
                                    class=format!(
                                        "min-h-40 rounded-md border p-1.5 {}",
                                        if is_today { "border-blue-600 bg-gray-800" } else { "border-gray-800" }
                                    )
                                >
                                    <button
                                        type="button"
                                        class="text-xs text-gray-300 hover:text-gray-100"
                                        on:click=move |_| open_daily.run(date_open.clone())
                                    >
                                        {date.split('-').next_back().unwrap_or("")}
                                    </button>
                                    {day_notes.iter().take(6).map(|n| {
                                        let path = n.path.clone();
                                        let title = n.title.clone();
                                        view! {
                                            <div
                                                class="text-xs text-blue-400 truncate mt-1 cursor-pointer hover:underline"
                                                title=title.clone()
                                                on:click=move |_| open_tab(ws, &path)
                                            >
                                                {title}
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            } else if mode.get() == CalMode::Day {
                let c = cursor.get_untracked();
                let date = fmt_date(&c);
                let day_notes = day_notes_for(date.clone());
                view! {
                    <div class="max-w-2xl">
                        <h2 class="text-lg font-semibold text-gray-100 mb-3">{date}</h2>
                        <button
                            type="button"
                            class="btn btn-primary btn-sm mb-4"
                            on:click=move |_| open_daily.run(date.clone())
                        >
                            "Open / create daily note"
                        </button>
                        {if day_notes.is_empty() {
                            view! {
                                <div class="text-sm text-gray-500">"No notes on this day yet."</div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="space-y-2">
                                    {day_notes.into_iter().map(|n| {
                                        let path = n.path.clone();
                                        let title = n.title.clone();
                                        let folder = n.folder.clone();
                                        view! {
                                            <button
                                                type="button"
                                                class="w-full text-left px-3 py-2 rounded-md border border-gray-800 hover:border-gray-600 hover:bg-gray-800/60"
                                                on:click=move |_| open_tab(ws, &path)
                                            >
                                                <div class="text-sm text-gray-200">"📄 " {title}</div>
                                                <div class="text-xs text-gray-500">{folder}</div>
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }}
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="grid grid-cols-3 gap-3">
                        {year_cells().into_iter().map(|(m, prefix, count)| {
                            let prefix_open = prefix.clone();
                            view! {
                                <button
                                    type="button"
                                    class="rounded-md border border-gray-800 p-4 text-left hover:border-gray-600 hover:bg-gray-800/60"
                                    on:click=move |_| {
                                        let (y, mm, _) = date_components(&prefix_open);
                                        set_cursor.set(js_sys::Date::new_with_year_month_day(y as u32, (mm - 1) as i32, 1));
                                        mode_str.set("month".to_string());
                                    }
                                >
                                    <div class="text-sm font-medium text-gray-200">{month_name(m)}</div>
                                    <div class="text-xs text-gray-500 mt-1">{count} " note(s)"</div>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
