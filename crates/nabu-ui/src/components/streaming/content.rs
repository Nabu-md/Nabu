//! # StreamingContent — live token rendering component
//!
//! Renders the active streaming sessions from [`StreamingContext`] with
//! incremental token display, lifecycle badges, and error/cancel messaging.
//! This is the primary UI surface for streaming — it can be embedded in any
//! view (chat area, inline note generation, etc.).
//!
//! ## Architecture
//!
//! ```text
//! StreamingProvider (EventBus listener)
//!     ↓  writes to Signal<HashMap<StreamId, StreamSession>>
//! StreamingContent (reads Signal, re-renders on every commit)
//!     ↓  clones a sorted Vec<StreamSession> per render
//! StreamMessage (per-session bubble)
//!     ↓  binds content string + cursor/indicator
//! Incremental UI (DOM text node updated by Dioxus diffing)
//! ```
//!
//! ## Key design decisions
//!
//! - The `Signal<HashMap<...>>` is read once per render to produce a sorted
//!   `Vec<StreamSession>`. Sorting into a `Vec` (outside `rsx!`) avoids
//!   re-sorting during key comparisons on every token.
//! - `StreamMessage` keys on `stream_id` so Dioxus reuses the DOM node across
//!   token updates — only the text content changes, not the element identity.
//!   This is what gives us incremental rendering without flickering.
//! - The typing cursor (`StreamingCursor`) is a child element that only renders
//!   while the stream is active, so it disappears cleanly on terminal states.

use dioxus::prelude::*;

use super::{StreamLifeCycle, StreamSession, StreamingContext, StreamingCursor, StreamingIndicator};

/// Renders all streaming sessions as live message bubbles.
///
/// Subscribes to the shared [`StreamingContext`] and re-renders whenever a
/// session updates. Active (non-terminal) sessions float to the top; terminal
/// sessions remain visible for history and can be cleared via
/// [`StreamingContext::clear`].
#[component]
pub fn StreamingContent(
    /// Optional CSS classes for the container.
    #[props(optional)]
    class: Option<String>,
) -> Element {
    let ctx = super::use_streaming();
    let sessions = ctx.sessions.clone();
    let extra = class.unwrap_or_default();

    let sorted: Vec<StreamSession> = {
        let mut list: Vec<_> = sessions.read().values().cloned().collect();
        list.sort_by_key(|s| {
            let terminal = s.state.is_terminal() as u8;
            let seq = s.token_count;
            (terminal, std::cmp::Reverse(seq))
        });
        list
    };

    rsx! {
        div {
            class: "streaming-content {extra}",
            role: "region",
            "aria-label": "Streaming responses",
            if sorted.is_empty() {
                div { class: "streaming-empty text-xs text-gray-500", "No active streams" }
            } else {
                for session in &sorted {
                    StreamMessage { session: session.clone() }
                }
            }
        }
    }
}

/// A single streaming session rendered as a chat-style message bubble.
///
/// The `key` on the outer element ensures Dioxus reuses the DOM node across
/// token updates — only the text changes, not the element identity. This is
/// the mechanism that gives us incremental rendering without flickering.
#[component]
pub fn StreamMessage(
    /// The session to render.
    session: StreamSession,
) -> Element {
    let StreamSession {
        stream_id,
        agent_name,
        state,
        content,
        token_count,
        total_tokens,
        error,
        cancel_reason,
        ..
    } = session;

    let agent_label = agent_name.as_deref().unwrap_or("agent");
    let state_class = state.status_kind();
    let state_label = state.label();
    let status_badge_class = format!("status-dot {state_class}");

    let subtitle = match state {
        StreamLifeCycle::Active => {
            if token_count > 0 {
                format!("{} token{}", token_count, if token_count == 1 { "" } else { "s" })
            } else {
                "Starting…".to_string()
            }
        }
        StreamLifeCycle::Completed => {
            let total = total_tokens.unwrap_or(token_count);
            format!("Completed · {} token{}", total, if total == 1 { "" } else { "s" })
        }
        StreamLifeCycle::Cancelled => {
            format!("Cancelled: {}", cancel_reason.as_deref().unwrap_or("unknown"))
        }
        StreamLifeCycle::Failed => {
            format!("Failed: {}", error.as_deref().unwrap_or("unknown error"))
        }
        StreamLifeCycle::Created => {
            "Created…".to_string()
        }
    };

    let is_active_stream = !state.is_terminal();
    let has_content = !content.is_empty();

    rsx! {
        div {
            class: "stream-message group",
            key: "{stream_id}",
            div {
                class: "stream-message-inner",
                span {
                    class: status_badge_class,
                    role: "status",
                    "aria-label": "{state_label}",
                }
                div {
                    class: "stream-message-content",
                    div {
                        class: "stream-message-text",
                        "white-space": "pre-wrap",
                        "aria-live": if is_active_stream { "polite" } else { "off" },
                        "aria-label": if is_active_stream { "Streaming response" } else { "Complete response" },
                        "{content}",
                    }
                    {is_active_stream.then_some(rsx! {
                        StreamingCursor {}
                    })}
                    {is_active_stream.then_some(rsx! {
                        StreamingIndicator {
                            token_count: Some(token_count),
                        }
                    })}
                }
            }
            div {
                class: "stream-message-meta",
                div {
                    class: "stream-message-header",
                    span {
                        class: "stream-agent-name",
                        "aria-hidden": "true",
                        "{agent_label}",
                    }
                    span {
                        class: "stream-subtitle",
                        "aria-label": "Stream status: {state_label}",
                        "{subtitle}",
                    }
                }
            }
            {error.as_ref().map(|e| rsx! {
                div {
                    class: "stream-error",
                    role: "alert",
                    "aria-label": "Stream error",
                    "{e}",
                }
            })}
            {is_active_stream.then_some(rsx! {
                button {
                    class: "stream-cancel-btn",
                    "aria-label": "Cancel stream",
                    onclick: move |_| {
                        let id = stream_id;
                        wasm_bindgen_futures::spawn_local(async move {
                            super::cancel_stream(id, "user requested").await;
                        });
                    },
                    "Stop",
                }
            })}
            {if !has_content && is_active_stream {
                rsx! {
                    span { class: "stream-placeholder", "aria-hidden": "true", " " }
                }
            } else {
                rsx! {}
            }}
        }
    }
}

/// Extension methods on [`StreamingContext`] for session management.
///
/// These are defined as a trait so they can be used from any component that
/// has a `StreamingContext` in scope.
impl StreamingContext {
    /// Removes all terminal (completed, cancelled, failed) sessions from the
    /// active view. Active sessions are preserved.
    pub fn prune_terminal(self) {
        self.sessions.write_unchecked().retain(|_, s| !s.state.is_terminal());
    }

    /// Clears all terminal sessions, preserving active ones.
    ///
    /// This is the user-facing "dismiss completed streams" operation.
    pub fn clear_terminal(self) {
        self.sessions
            .write_unchecked()
            .retain(|_, s| !s.state.is_terminal());
    }

    /// Clears all sessions — both active and terminal.
    ///
    /// Use with caution: this will lose all accumulated stream content.
    pub fn clear(self) {
        self.sessions.write_unchecked().clear();
    }

    /// Returns the number of sessions in the stream (active + terminal).
    pub fn len(self) -> usize {
        self.sessions.read().len()
    }

    /// Returns `true` if there are no sessions at all.
    pub fn is_empty(self) -> bool {
        self.sessions.read().is_empty()
    }

    /// Returns the number of currently-active (non-terminal) sessions.
    pub fn active_count(self) -> usize {
        self.sessions
            .read()
            .values()
            .filter(|s| !s.state.is_terminal())
            .count()
    }

    /// Returns `true` if any session is in an active (non-terminal) state.
    pub fn has_active(self) -> bool {
        self.active_count() > 0
    }
}
