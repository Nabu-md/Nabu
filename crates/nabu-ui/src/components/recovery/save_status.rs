//! # Autosave status — "Saving… / Saved / Save failed / Retrying" (Dioxus)
//!
//! A small shared context that reports the current save state of the active
//! note without interrupting the workflow. The [`SaveStatusIndicator`] renders
//! it in the status bar.
//!
//! Changes from LePtOS: `RwSignal` → `Signal`, `provide_context` wrapped in a
//! provider component, `view!` → `rsx!`, `impl IntoView` → `Element`.

use dioxus::prelude::*;

/// Current save state of the active note.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SaveStatus {
    /// Nothing has been edited since the last save.
    #[default]
    Idle,
    /// A save is in flight.
    Saving,
    /// The last save succeeded.
    Saved,
    /// The last save failed.
    Failed,
    /// A failed save is being retried.
    Retrying,
}

impl SaveStatus {
    fn label(self) -> &'static str {
        match self {
            SaveStatus::Idle => "Saved",
            SaveStatus::Saving => "Saving…",
            SaveStatus::Saved => "Saved",
            SaveStatus::Failed => "Save failed",
            SaveStatus::Retrying => "Retrying…",
        }
    }

    fn dot_class(self) -> &'static str {
        match self {
            SaveStatus::Idle => "save-dot-idle",
            SaveStatus::Saving => "save-dot-saving",
            SaveStatus::Saved => "save-dot-saved",
            SaveStatus::Failed => "save-dot-failed",
            SaveStatus::Retrying => "save-dot-retrying",
        }
    }
}

/// Shared autosave status context.
#[derive(Clone, Copy)]
pub struct SaveStatusContext {
    pub status: Signal<SaveStatus>,
    /// Extra detail shown as a tooltip (e.g. the note path).
    pub detail: Signal<String>,
}

/// Provides the autosave status context (call inside a `#[component]`).
pub fn provide_save_status() {
    provide_context(SaveStatusContext {
        status: use_signal(|| SaveStatus::Idle),
        detail: use_signal(String::new),
    });
}

/// Provider component for save-status tracking.
#[component]
pub fn SaveStatusProvider(children: Element) -> Element {
    provide_save_status();
    rsx! { {children} }
}

/// Retrieves the autosave status context.
pub fn use_save_status() -> SaveStatusContext {
    use_context::<SaveStatusContext>()
}

/// A compact status-bar indicator for the save state.
#[component]
pub fn SaveStatusIndicator() -> Element {
    let ctx = use_save_status();
    rsx! {
        span {
            class: "save-status",
            role: "status",
            "aria-live": "polite",
        }
        span {
            class: { format!("save-dot {}", ctx.status.read().dot_class()) },
            "aria-hidden": "true",
        }
        span { class: "save-status-label", "{ctx.status.read().label()}" }
    }
}
