//! # StreamingContent — live token rendering component
//!
//! Renders the active streaming sessions from [`StreamingContext`] with
//! incremental token display, lifecycle badges, and error/cancel messaging.
//! This is the primary UI surface for streaming — it can be embedded in any
//! view (chat area, inline note generation, etc.).

use dioxus::prelude::*;

use super::{StreamLifeCycle, StreamSession, StreamingContext, use_streaming};

/// Renders active streaming sessions as live chat-style message bubbles.
///
/// Subscribes to the shared `StreamingContext` and re-renders whenever a session
/// updates. Active (non-terminal) sessions float to the top. Terminal sessions
/// remain visible for history and can be cleared via [`StreamingContext::clear`].
#[component]
pub fn StreamingContent(
    /// Optional CSS classes for the container.
    #[props(optional)]
    class: Option<String>,
) -> Element {
    let ctx = use_streaming();
    let sessions = ctx.sessions.clone();
    let extra = class.unwrap_or_default();

    // Collect sessions, sort (active first, most recent first), and render.
    // We sort into a Vec here (outside rsx!) to avoid re-sorting on every
    // render key comparison.
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
        }
        if sorted.is_empty() {
            div { class: "streaming-empty text-xs text-gray-500", "No active streams" }
        } else {
            for session in &sorted {
                StreamMessage { session: session.clone() }
            }
        }
    }
}

/// A single streaming session rendered as a chat-style message bubble.
#[component]
fn StreamMessage(
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

    // Pre-compute display strings before rsx! (Dioxus 0.6 doesn't support
    // inline conditionals inside string literals).
    let agent_label = agent_name.as_deref().unwrap_or("agent");
    let state_class = state.status_kind();
    let state_label = state.label();
    let status_badge_class = format!("status-dot {state_class}");

    // Build the subtitle line depending on lifecycle state.
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

    // The content area: for active streams, show accumulated content with a
    // typing indicator; for terminal streams, show the full content.
    let is_active = !state.is_terminal();

    rsx! {
        div {
            class: "stream-message group",
            key: "{stream_id}",
        }
        div {
            class: "stream-message-inner",
        }
        span {
            class: status_badge_class,
            role: "status",
            "aria-label": "{state_label}",
        }
        div {
            class: "stream-message-content",
        }
        div {
            class: "stream-message-text",
            // Use a pre-formatted block so whitespace in token chunks is preserved.
            "white-space": "pre-wrap",
            "{content}",
        }
        {is_active.then_some(rsx! {
            span { class: "stream-typing-indicator", "▊" }
        })}
        div {
            class: "stream-message-meta",
        }
        span {
            class: "stream-agent-name",
            "{agent_label}",
        }
        span {
            class: "stream-subtitle",
            "{subtitle}",
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
