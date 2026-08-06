//! # Nabu Shared Component Library
//!
//! Re-exports the Dioxus-converted icon system. Other UI primitives
//! (button, card, dialog, feedback, etc.) are intentionally excluded from
//! Phase 0 and will be migrated in P0.2.

pub mod icons;

pub use icons::{render_icon, render_icon_view, Icon, IconEl};
