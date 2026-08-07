//! # Crash recovery banner (Dioxus migration)
//!
//! Shown at the top of the dashboard when the previous run ended unexpectedly.
//!
//! Changes: `RwSignal` → `Signal`, `Callback::run` → `Callback::call`,
//! `view!` → `rsx!`, `impl IntoView` → `Element`, `.into_any()` removed,
//! `prop:value=` → `value:` + `onchange:`, `class=move ||` → `class: {}`.

use crate::components::recovery::session::{RecoveryStatus, SessionState};
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;

/// The recovery banner, driven by an optional pending recovery status.
#[component]
pub fn RecoveryBanner(
    /// The pending recovery status; `None` hides the banner.
    recovery: Signal<Option<RecoveryStatus>>,
    /// Called when the user chooses to restore the previous session.
    on_restore: Callback<SessionState>,
    /// Called when the user chooses to inspect recovered files.
    on_inspect: Callback<()>,
) -> Element {
    let toasts = crate::components::ui::feedback::use_toast();
    let mut recovery_sig = recovery;

    // discard handler
    let on_discard = move |_: MouseEvent| {
        let mut r = recovery_sig;
        crate::components::recovery::session::recovery_discard();
        crate::components::recovery::session::session_clear();
        r.set(None);
        toasts.info("Recovery", "The previous session was discarded.");
    };

    rsx! {
        if let Some(status) = recovery_sig.read().as_ref() {
            div {
                class: "recovery-banner",
                role: "status",
                "aria-live": "polite",
            }
            div { class: "recovery-banner-icon", "aria-hidden": "true", {render_icon_view(Icon::LifeBuay)} }
            div { class: "flex-1 min-w-0" }
            div { class: "text-sm font-semibold text-gray-100", "Recover previous session?" }
            div { class: "text-xs text-gray-400" }
            {if status.crashed {
                rsx! { "Nabu closed unexpectedly last time. You can restore your previous session — nothing is lost." }
            } else {
                rsx! { "A saved session is available from a previous run." }
            }}
            {status.session.as_ref().and_then(|s| s.saved_at.as_deref()).map(|t| rsx! { " Saved {t}." })}

            div { class: "flex items-center gap-2" }
            Button {
                variant: ButtonVariant::Primary,
                on_click: move |_: MouseEvent| {
                    if let Some(s) = recovery_sig.read().as_ref().and_then(|s| s.session.clone()) {
                        crate::components::recovery::session::recovery_discard();
                        on_restore.call(s);
                    }
                    recovery_sig.set(None);
                },
                {"Restore session"}
            }
            Button {
                variant: ButtonVariant::Outline,
                on_click: move |_: MouseEvent| {
                    let _ = on_inspect.call(());
                },
                {"Inspect"}
            }
            Button {
                variant: ButtonVariant::Ghost,
                on_click: on_discard,
                {"Discard"}
            }
        }
    }
}
