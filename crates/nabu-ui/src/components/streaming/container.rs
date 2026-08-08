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
//! - **User drift**: if the user scrolls up more than `DRIFT_THRESHOLD_PX` from
//!   the bottom, auto-follow is suspended until the next stream starts.
//! - **Stream start**: when a new stream begins, auto-follow is re-enabled
//!   and the container scrolls to the bottom.
//!
//! The scroll lock uses a dedicated `Signal<bool>` so that the
//! `use_effect` that performs the scroll does not create a render loop — the
//! effect fires on scroll events and updates the lock, which in turn
//! controls whether the next token commit triggers a scroll.

use dioxus::prelude::*;

use super::StreamingContext;

/// Distance from the bottom (in CSS pixels) within which auto-follow stays active.
/// Moving further than this from the bottom suspends auto-scroll until the next
/// stream starts.
const DRIFT_THRESHOLD_PX: f64 = 40.0;

/// Props for [`StreamingContainer`].
pub struct StreamingContainerProps {
    /// Optional CSS classes for the outer element.
    pub class: Option<String>,
}

impl Default for StreamingContainerProps {
    fn default() -> Self {
        Self { class: None }
    }
}

/// A scrollable container that auto-follows streaming content.
///
/// Place this around [`StreamingContent`](super::StreamingContent) (or any
/// streaming-rendered children) to get automatic, user-aware scrolling.
///
/// The container renders a div with `overflow-y: auto` and attaches
/// `onscroll` / `use_effect` hooks that manage the auto-follow lock.
#[component]
pub fn StreamingContainer(
    /// Child elements (typically `StreamingContent` or `StreamMessage`s).
    children: Element,
    /// Optional CSS classes.
    #[props(optional)]
    class: Option<String>,
) -> Element {
    let ctx = super::use_streaming();
    let extra = class.unwrap_or_default();

    // Auto-follow lock: true when we should scroll to the bottom on updates.
    // Suspended when the user scrolls up past the drift threshold.
    let follow = use_signal(|| true);

    // Re-enable auto-follow + scroll to bottom whenever a new stream starts.
    let sessions = ctx.sessions.clone();
    {
        let mut follow = follow;
        use_effect(move || {
            let current = sessions.read();
            let has_active = current.values().any(|s| !s.state.is_terminal());
            if has_active {
                follow.set(true);
            }
            drop(current);
            || {};
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
            onscroll: move |ev: dioxus::prelude::MouseEvent| {
                let scroll_el = ev.as_web_event();
                let target = scroll_el.target();
                if let Some(el) = target.dyn_ref::<web_sys::Element>() {
                    if let Some(scroll_el) = el.dyn_ref::<web_sys::Element>() {
                        let bottom = scroll_el.scroll_height() - scroll_el.client_height() - scroll_el.scroll_top();
                        if bottom > DRIFT_THRESHOLD_PX {
                            follow.set(false);
                        } else if bottom < DRIFT_THRESHOLD_PX {
                            follow.set(true);
                        }
                    }
                }
            },
            {children}
        }
    }

    // Auto-scroll effect: fires whenever sessions change AND follow is true.
    // We read `follow` inside the effect body (not as a dependency), so the
    // scroll happens on the *next* render commit after a token arrives —
    // not during the scroll handler itself (which would fight the user).
    {
        let follow = follow.clone();
        let sessions = sessions.clone();
        use_effect(move || {
            if *follow.read() {
                // The container div is the element with class "streaming-container".
                // We scroll it to the bottom via the global document.
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Some(container) = document.query_selector(".streaming-container").ok().flatten() {
                            if let Some(el) = container.dyn_ref::<web_sys::Element>() {
                                let _ = el.set_scroll_top(el.scroll_height());
                            }
                        }
                    }
                }
            }
            // Read sessions to create a render dependency — this effect
            // re-runs whenever sessions are committed (i.e. on new tokens).
            let _ = sessions.read().len();
            || {}
        });
    }
}
