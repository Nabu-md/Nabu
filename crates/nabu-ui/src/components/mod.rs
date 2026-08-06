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

pub mod app;
pub mod contexts;
pub mod layout;
pub mod navigation;
pub mod ui;
