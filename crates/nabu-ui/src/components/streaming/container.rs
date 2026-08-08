//! # StreamingContainer — scrollable container with auto-scroll
//!
//! Wraps streaming content in a scrollable viewport that automatically scrolls
//! to the bottom when new tokens arrive, while preserving user control when
//! they deliberately scroll up to review previous content.
//!
//! ## Scrolling behaviour
//!
//! - **Auto-follow** (default): the container scrolls to the bottom on every
//!   new token **only when** the user is already at (or very near) the bottom.
//! - **User drift**: if the user scrolls up more than
//!   [`DRIFT_THRESHOLD_PX`] from the bottom, auto-follow is suspended until
//!   the next stream starts.
//! - **Stream start**: when a new stream begins, auto-follow is re-enabled
//!   and the container scrolls to the bottom.
//!
//! ## Architecture
//!
//! ```text
//! StreamingContainer
//!   └── div.streaming-container  (overflow-y: auto, onscroll → lock)
//!       └── {children}  (e.g. StreamingContent → StreamMessage[])
//!
//! use_effect(sessions) → reads follow lock → scrollToBottom()
//! ```
//!
//! The `follow` lock is a `Signal<bool>`. The scroll handler updates it on
//! every user scroll; the `use_effect` reads it (not as a dependency) and
//! triggers a scroll only when sessions change. This avoids fighting the
//! user during manual scroll.

use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

/// Distance from the bottom (in CSS pixels) within which auto-follow stays active.
/// Moving further than this from the bottom suspends auto-scroll until the next
/// stream starts.
const DRIFT_THRESHOLD_PX: f64 = 40.0;

/// A scrollable container that auto-follows streaming content.
///
/// Place this around [`StreamingContent`](super::StreamingContent) (or any
/// streaming-rendered children) to get automatic, user-aware scrolling.
///
/// The container renders a `div` with `overflow-y: auto`. The `role="log"`
/// and `aria-live="polite"` attributes make screen readers announce new
/// content as it arrives, without interrupting the user.
#[component]
pub fn StreamingContainer(
    /// Child elements (typically `StreamingContent` or `StreamMessage`s).
    children: Element,
    /// Optional CSS classes for the container.
    #[props(optional)]
    class: Option<String>,
) -> Element {
    let ctx = super::use_streaming();
    let extra = class.unwrap_or_default();

    // Auto-follow lock: true when we should scroll to the bottom on updates.
    // Suspended when the user scrolls up past the drift threshold.
    let mut follow = use_signal(|| true);

    // Track the set of active stream IDs so we only re-enable auto-follow when
    // a *new* stream starts (not on every render). In Dioxus 0.6, use_effect
    // fires on every commit; without this guard the follow flag would be
    // reset on every token update, defeating the drift threshold.
    let active_ids = use_signal(HashMap::<super::StreamId, ()>::new);

    let sessions = ctx.sessions.clone();
    {
        let mut follow = follow;
        let mut active_ids = active_ids;
        use_effect(move || {
            let current = sessions.read();
            let new_active: HashMap<_, ()> = current
                .values()
                .filter(|s| !s.state.is_terminal())
                .map(|s| (s.stream_id, ()))
                .collect();

            // Detect if a new active stream appeared since last render.
            let new_stream_started = new_active.keys().any(|id| !active_ids.read().contains_key(id));
            if new_stream_started {
                follow.set(true);
                scroll_to_bottom();
            }

            *active_ids.write_unchecked() = new_active;
            drop(current);
        });
    }

    // Auto-scroll effect: fires whenever sessions are committed (i.e. on
    // new tokens). Reads `follow` as a plain value (not a dependency) so
    // it does not re-trigger on scroll events, and does not reset `follow`.
    {
        let follow = follow;
        use_effect(move || {
            let _len = sessions.read().len();
            if *follow.read() {
                scroll_to_bottom();
            }
        });
    }

    rsx! {
        div {
            class: "streaming-container {extra}",
            role: "log",
            "aria-label": "Streaming output",
            "aria-live": "polite",
            "aria-relevant": "additions text",
            style: "overflow-y: auto; overflow-x: hidden;",
            onscroll: move |ev: dioxus::prelude::ScrollEvent| {
                let web = ev.data().as_web_event();
                if let Some(target) = web.target() {
                    if let Some(el) = target.dyn_ref::<web_sys::Element>() {
                        let scroll_height = el.scroll_height() as f64;
                        let client_height = el.client_height() as f64;
                        let scroll_top = el.scroll_top() as f64;
                        let bottom = scroll_height - client_height - scroll_top;
                        if bottom > DRIFT_THRESHOLD_PX {
                            follow.set(false);
                        } else {
                            follow.set(true);
                        }
                    }
                }
            },
            {children}
        }
    }
}

/// Scrolls the `.streaming-container` element to its bottom.
///
/// Called from `use_effect` callbacks. Uses `document.querySelector` so it
/// works regardless of how deeply the container is nested in the tree.
fn scroll_to_bottom() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Ok(Some(el)) = document.query_selector(".streaming-container") {
                let _ = el.set_scroll_top(el.scroll_height());
            }
        }
    }
}

/// Hook for components that need to imperatively scroll the streaming
/// container.
///
/// Returns a closure that scrolls to the bottom when called. Useful for
/// components outside the container that still need to trigger a scroll
/// (e.g. a "scroll to bottom" button in a toolbar).
pub fn use_auto_scroll() -> impl Fn() {
    move || scroll_to_bottom()
}
