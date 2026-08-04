//! # Crash recovery banner
//!
//! Shown at the top of the dashboard when the previous run ended unexpectedly.
//! Offers the user a choice — restore the previous session, discard the
//! recovery, or inspect the recovered files — so recoverable work is never
//! silently discarded.

use crate::components::recovery::session::{RecoveryStatus, SessionState};
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::feedback::use_toast;
use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;

/// The recovery banner, driven by an optional pending recovery status.
#[component]
pub fn RecoveryBanner(
    /// The pending recovery status; `None` hides the banner.
    recovery: RwSignal<Option<RecoveryStatus>>,
    /// Called when the user chooses to restore the previous session.
    on_restore: Callback<SessionState>,
    /// Called when the user chooses to inspect recovered files.
    on_inspect: Callback<()>,
) -> impl IntoView {
    let toasts = use_toast();

    let discard = Callback::new(move |_| {
        let toasts_discard = toasts;
        crate::components::recovery::session::recovery_discard();
        crate::components::recovery::session::session_clear();
        toasts_discard.info("Recovery", "The previous session was discarded.");
        recovery.set(None);
    });

    let restore = Callback::new(move |_| {
        if let Some(status) = recovery.get() {
            if let Some(session) = status.session {
                crate::components::recovery::session::recovery_discard();
                on_restore.run(session);
            }
        }
        recovery.set(None);
    });

    view! {
        {move || if let Some(status) = recovery.get() {
            view! {
                <div class="recovery-banner" role="status" aria-live="polite">
                    <div class="recovery-banner-icon" aria-hidden="true">{render_icon_view(Icon::LifeBuoy)}</div>
                    <div class="flex-1 min-w-0">
                        <div class="text-sm font-semibold text-gray-100">"Recover previous session?"</div>
                        <div class="text-xs text-gray-400">
                            {if status.crashed {
                                "Nabu closed unexpectedly last time. You can restore your previous session — nothing is lost."
                            } else {
                                "A saved session is available from a previous run."
                            }}
                            {status.session.as_ref().map(|s| {
                                s.saved_at.as_deref().map(|t| format!(" Saved {}.", t)).unwrap_or_default()
                            }).unwrap_or_default()}
                        </div>
                    </div>
                    <div class="flex items-center gap-2">
                        <Button variant=ButtonVariant::Primary on_click=restore>
                            "Restore session"
                        </Button>
                        <Button variant=ButtonVariant::Outline on_click=Callback::new(move |_| on_inspect.run(()))>
                            "Inspect"
                        </Button>
                        <Button variant=ButtonVariant::Ghost on_click=discard>
                            "Discard"
                        </Button>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
