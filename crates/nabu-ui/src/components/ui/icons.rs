//! # Nabu Icon System
//!
//! Centralized re-export wall for every Lucide icon used in the application.
//!
//! **Why a single module instead of ad-hoc imports everywhere?**
//!
//! - One import path for all icons: `use crate::components::ui::icons::*`.
//! - Easy to swap out the icon library in the future if we ever move away
//!   from Lucide.
//! - Every icon is documented inline with its corresponding Lucide component
//!   name so upgrading or auditing the set is straightforward.
//!
//! Icons inherit `currentColor` by default so they adapt to light / dark /
//! system themes automatically. The `class` prop can be used to control
//! sizing via existing design tokens.

// ── Navigation & Layout ─────────────────────────────────────
pub use lucide_leptos::arrow_left;
pub use lucide_leptos::arrow_right;
pub use lucide_leptos::arrow_up;
pub use lucide_leptos::arrow_down;
pub use lucide_leptos::chevron_down;
pub use lucide_leptos::chevron_left;
pub use lucide_leptos::chevron_right;
pub use lucide_leptos::chevron_up;
pub use lucide_leptos::chevrons_left;
pub use lucide_leptos::chevrons_right;
pub use lucide_leptos::home;
pub use lucide_leptos::corner_up_left;   // undo arrow
pub use lucide_leptos::corner_up_right;  // redo arrow
pub use lucide_leptos::external_link;
pub use lucide_leptos::menu;
pub use lucide_leptos::more_horizontal;
pub use lucide_leptos::more_vertical;
pub use lucide_leptos::panel_left;
pub use lucide_leptos::panel_right;

// ── Files & Vault ────────────────────────────────────────────
pub use lucide_leptos::file;
pub use lucide_leptos::file_text;
pub use lucide_leptos::file_pen;
pub use lucide_leptos::file_plus;
pub use lucide_leptos::folder;
pub use lucide_leptos::folder_open;
pub use lucide_leptos::folder_plus;
pub use lucide_leptos::folder_tree;
pub use lucide_leptos::folder_key;
pub use lucide_leptos::folder_archive;
pub use lucide_leptos::sticky_note;   // generic note
pub use lucide_leptos::files;
pub use lucide_leptos::copy;
pub use lucide_leptos::clipboard;
pub use lucide_leptos::clipboard_list;
pub use lucide_leptos::notebook_pen;
pub use lucide_leptos::save;

// ── Search & Find ────────────────────────────────────────────
pub use lucide_leptos::search;
pub use lucide_leptos::search_code;
pub use lucide_leptos::book_open;
pub use lucide_leptos::bookmark;
pub use lucide_leptos::tags;
pub use lucide_leptos::tag;

// ── Views ────────────────────────────────────────────────────
pub use lucide_leptos::list;
pub use lucide_leptos::list_checks;
pub use lucide_leptos::grid;
pub use lucide_leptos::columns;
pub use lucide_leptos::table;
pub use lucide_leptos::table_2;
pub use lucide_leptos::square_kanban;
pub use lucide_leptos::gallery_vertical_end;
pub use lucide_leptos::gallery_vertical_2;
pub use lucide_leptos::palette;
pub use lucide_leptos::layers;
pub use lucide_leptos::library;
pub use lucide_leptos::library_big;

// ── Knowledge Graph ──────────────────────────────────────────
pub use lucide_leptos::network;
pub use lucide_leptos::link;
pub use lucide_leptos::link_2;
pub use lucide_leptos::message_square;
pub use lucide_leptos::message_circle;
pub use lucide_leptos::speech;

// ── Calendar & Time ──────────────────────────────────────────
pub use lucide_leptos::calendar;
pub use lucide_leptos::calendar_days;
pub use lucide_leptos::clock;
pub use lucide_leptos::history;
pub use lucide_leptos::timer;

// ── Actions ─────────────────────────────────────────────────
pub use lucide_leptos::plus;
pub use lucide_leptos::x;
pub use lucide_leptos::circle_check;
pub use lucide_leptos::circle_x;
pub use lucide_leptos::square_check;   // filled check
pub use lucide_leptos::circle_help;     // help / info glyph
pub use lucide_leptos::info;
pub use lucide_leptos::triangle_alert;
pub use lucide_leptos::octagon_alert;
pub use lucide_leptos::check;
pub use lucide_leptos::rotate_cw;
pub use lucide_leptos::rotate_ccw;
pub use lucide_leptos::undo;
pub use lucide_leptos::redo;
pub use lucide_leptos::trash;
pub use lucide_leptos::trash_2;
pub use lucide_leptos::archive;
pub use lucide_leptos::archive_restore;
pub use lucide_leptos::star;
pub use lucide_leptos::zap;
pub use lucide_leptos::pen_line;
pub use lucide_leptos::file_pen_line;
pub use lucide_leptos::square_pen;

// ── Status & Feedback ────────────────────────────────────────
pub use lucide_leptos::bell;
pub use lucide_leptos::bell_ring;
pub use lucide_leptos::eye;
pub use lucide_leptos::eye_off;
pub use lucide_leptos::lightbulb;
pub use lucide_leptos::flame;
pub use lucide_leptos::sparkles;
pub use lucide_leptos::loader;
pub use lucide_leptos::refresh_cw;
pub use lucide_leptos::refresh_ccw;
pub use lucide_leptos::life_buoy;

// ── Communication ────────────────────────────────────────────
pub use lucide_leptos::mail;
pub use lucide_leptos::send;
pub use lucide_leptos::share;
pub use lucide_leptos::share_2;
pub use lucide_leptos::reply;
pub use lucide_leptos::reply_all;

// ── Objects & Tools ──────────────────────────────────────────
pub use lucide_leptos::settings;
pub use lucide_leptos::settings_2;
pub use lucide_leptos::brush;
pub use lucide_leptos::mic;
pub use lucide_leptos::camera;
pub use lucide_leptos::play;
pub use lucide_leptos::package;
pub use lucide_leptos::target;
pub use lucide_leptos::command;
pub use lucide_leptos::monitor;
pub use lucide_leptos::smartphone;
pub use lucide_leptos::tablet;
pub use lucide_leptos::laptop;

// ── Charts & Stats ───────────────────────────────────────────
pub use lucide_leptos::chart_bar;
pub use lucide_leptos::chart_column;
pub use lucide_leptos::chart_line;
pub use lucide_leptos::chart_pie;
pub use lucide_leptos::trending_up;
pub use lucide_leptos::trending_down;

// ── Misc ─────────────────────────────────────────────────────
pub use lucide_leptos::app_window;
pub use lucide_leptos::app_window_mac;
pub use lucide_leptos::dock;
pub use lucide_leptos::inbox;
pub use lucide_leptos::pin;
pub use lucide_leptos::map_pin;
pub use lucide_leptos::scissors;
pub use lucide_leptos::download;
pub use lucide_leptos::upload;
pub use lucide_leptos::import;
pub use lucide_leptos::book_open_check;
pub use lucide_leptos::book_text;
pub use lucide_leptos::book_marked;
pub use lucide_leptos::globe;
pub use lucide_leptos::sliders;
pub use lucide_leptos::sliders_horizontal;
pub use lucide_leptos::toggle_left;
pub use lucide_leptos::toggle_right;
pub use lucide_leptos::sun;
pub use lucide_leptos::moon;
pub use lucide_leptos::wand_2;
pub use lucide_leptos::circle_ellipsis;
pub use lucide_leptos::ellipsis;