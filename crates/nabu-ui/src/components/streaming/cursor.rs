//! # StreamingCursor — animated cursor indicator
//!
//! Renders a blinking block cursor (caret) at the end of active streaming
//! content, giving the user a clear visual cue that tokens are being appended
//! in real time.
//!
//! The cursor is deliberately lightweight — a single animated element with
//! no Dioxus state — so it adds zero overhead to the token-rendering pipeline.

use dioxus::prelude::*;

/// Height of the cursor relative to the line height (em).
const CURSOR_HEIGHT: &str = "1.1em";

/// A blinking block cursor that indicates active streaming.
///
/// Renders a tall, thin block that pulses via CSS animation. The animation
/// runs on the compositor thread (CSS `@keyframes`) so it does not trigger
/// Dioxus re-renders. The `aria-hidden="true"` flag prevents screen readers
/// from announcing the visual cursor — the `aria-live="polite"` region on the
/// message text itself already announces incremental content.
#[component]
pub fn StreamingCursor(
    /// Extra CSS classes (optional).
    #[props(optional)]
    class: Option<&'static str>,
) -> Element {
    let extra = class.unwrap_or_default();
    rsx! {
        span {
            class: "stream-cursor {extra}",
            "aria-hidden": "true",
            style: "height: {CURSOR_HEIGHT};",
        }
    }
}
