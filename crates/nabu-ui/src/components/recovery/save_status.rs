//! # Autosave status — "Saving… / Saved / Save failed / Retrying"
//!
//! A small shared context that reports the current save state of the active
//! note without interrupting the workflow. The [`SaveStatusIndicator`] renders
//! it in the status bar; the note editor drives it as it autosaves.
//!
//! ## Reactivity note
//!
//! The context is `Copy` and must be captured at render time — never via
//! `expect_context` inside a `spawn_local` future or a raw DOM callback.

use leptos::prelude::*;

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
    pub status: RwSignal<SaveStatus>,
    /// Extra detail shown as a tooltip (e.g. the note path).
    pub detail: RwSignal<String>,
}

/// Provides the autosave status context (call at the app root).
pub fn provide_save_status() {
    provide_context(SaveStatusContext {
        status: RwSignal::new(SaveStatus::Idle),
        detail: RwSignal::new(String::new()),
    });
}

/// Retrieves the autosave status context.
pub fn use_save_status() -> SaveStatusContext {
    expect_context::<SaveStatusContext>()
}

/// A compact status-bar indicator for the save state.
#[component]
pub fn SaveStatusIndicator() -> impl IntoView {
    let ctx = use_save_status();
    view! {
        <span
            class="save-status"
            role="status"
            aria-live="polite"
            title={move || ctx.detail.get()}
        >
            <span
                class=move || format!("save-dot {}", ctx.status.get().dot_class())
                aria-hidden="true"
            ></span>
            <span class="save-status-label">
                {move || ctx.status.get().label()}
            </span>
        </span>
    }
}
