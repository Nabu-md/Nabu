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

pub mod app;
pub mod contexts;
pub mod editor;
pub mod file_tree;
pub mod layout;
pub mod navigation;
pub mod note_editor;
pub mod note_view;
pub mod property_editor;
pub mod ui;
