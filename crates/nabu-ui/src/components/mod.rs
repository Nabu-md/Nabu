//! # Nabu UI components (Dioxus)
//!
//! Phase 0 exposes the migrated modules:
//! - [`app`] — root app shell, routing entry, provider wiring, view switching
//! - [`contexts`] — Dioxus context types and provider components
//! - [`ui`] — shared UI primitives (icons, buttons, inputs, modals, popovers,
//!   dropdowns, accessibility, layout primitives)
//! - [`layout`] — structural layout (ribbon, sidebars, tab bar, workspace)
//! - [`navigation`] — navigation surfaces (navbar, breadcrumbs, command palette,
//!   quick switcher, shortcuts reference, navigation state, command catalog)
//!
//! Phase 1 migrated four primary views:
//! - [`file_tree`] — vault file explorer with DnD, context menu, inline rename
//! - [`note_editor`] — markdown editor with debounced autosave, slash menu, DnD
//! - [`note_view`] — read-only rendered note preview
//! - [`property_editor`] — metadata property editor (text/number/date/select/etc.)
//! - [`editor`] — editor module (slash menu)
//!
//! Phase 0 (continued) migrated additional views from LePtOS → Dioxus:
//! - [`inbox`] — knowledge inbox with split-pane review, metadata editing, DnD
//! - [`dictation_pill`] — floating dictation / scratchpad / file-drop pill
//! - [`statistics`] — vault statistics dashboard (tags, writing streak, storage)
//! - [`streaming`] — real-time streaming session state and live token rendering
//! - [`template_editor`] — template CRUD manager with search and category grouping
//! - [`template_picker`] — searchable template picker
//! - [`settings`] — 15-tab settings panel with IPC persistence
//! - [`recovery`] — crash-recovery banner, save-status indicator, diff view,
//!   version history browser, and snapshot recovery manager

pub mod app;
pub mod activity;
pub mod contexts;
pub mod dictation_pill;
pub mod editor;
pub mod file_tree;
pub mod inbox;
pub mod layout;
pub mod navigation;
pub mod note_editor;
pub mod note_view;
pub mod property_editor;
pub mod recovery;
pub mod settings;
pub mod statistics;
pub mod streaming;
pub mod template_editor;
pub mod template_picker;
pub mod ui;
