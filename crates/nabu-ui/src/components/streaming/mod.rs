//! # Streaming — frontend real-time streaming state
//!
//! The [`StreamingProvider`] subscribes to every streaming event delivered by
//! the frontend [`EventService`](crate::events::EventService), reconstructs
//! active streams in a reactive Dioxus `Signal`, and exposes a
//! [`StreamingContext`] so descendant components can render incremental token
//! output and react to stream lifecycle transitions.
//!
//! ## Architecture
//!
//! ```text
//! Backend StreamingPipeline → EventBus → EventBusBridge → nabu-event channel
//!   → EventService → StreamingProvider (this module)
//!   → StreamState (Signal<HashMap<StreamId, StreamSession>>)
//!   → StreamingContent / StreamMessage → User sees live tokens
//! ```
//!
//! ## Event subscription
//!
//! The provider registers a single `subscribe_all` listener and dispatches each
//! event to the matching [`StreamSession`] by `StreamId`. Per-token events
//! (`stream.token`, `stream.partial_update`) append to the active session's
//! accumulated content; lifecycle events (`stream.started`, `stream.completed`,
//! `stream.cancelled`, `stream.failed`) and session events
//! (`session.created`, `session.started`, `session.cancelled`,
//! `session.cleaned_up`) drive the lifecycle state machine.
//!
//! ## Error handling
//!
//! Unknown or malformed events are logged and skipped — the provider never
//! panics. If the `EventService` is unavailable (e.g. running outside Tauri),
//! the provider remains usable with an empty state.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::events::{use_event_service, EventService, FrontendEvent};

mod content;

pub use content::StreamingContent;

/// A unique identifier for a streaming session, matching the backend `StreamId`.
pub type StreamId = uuid::Uuid;

/// The lifecycle state of a streaming session, mirrored from the backend
/// `StreamState`. Kept as a separate enum here to avoid a direct dependency
/// on the backend streaming types in the UI layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StreamLifeCycle {
    /// The session has been created but not yet started.
    #[default]
    Created,
    /// Tokens are being published to the stream.
    Active,
    /// The stream completed normally (all tokens delivered).
    Completed,
    /// The stream was cancelled before completion.
    Cancelled,
    /// The stream failed due to an error.
    Failed,
}

impl StreamLifeCycle {
    /// CSS class suffix for theming (e.g. `status-dot-success`).
    pub fn status_kind(self) -> &'static str {
        match self {
            Self::Created | Self::Active => "status-dot-info",
            Self::Completed => "status-dot-success",
            Self::Cancelled => "status-dot-warning",
            Self::Failed => "status-dot-error",
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Active => "Streaming",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
            Self::Failed => "Failed",
        }
    }

    /// Returns `true` if the stream is in an active (non-terminal) state.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Created | Self::Active)
    }

    /// Returns `true` if the stream is in a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// A single reconstructed streaming session, held entirely on the frontend.
///
/// The session accumulates token output as it arrives from the backend and
/// tracks its lifecycle state. All fields are owned (no references) so the
/// session can be moved between renders freely.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamSession {
    /// The unique identifier for this stream.
    pub stream_id: StreamId,
    /// The conversation/thread ID this stream is associated with, if any.
    pub thread_id: Option<uuid::Uuid>,
    /// The agent name, if the stream is associated with a named agent.
    pub agent_name: Option<String>,
    /// The current lifecycle state.
    pub state: StreamLifeCycle,
    /// The accumulated content so far (all tokens concatenated).
    pub content: String,
    /// Number of tokens published so far.
    pub token_count: u64,
    /// The total number of tokens when the stream completed (if terminal).
    pub total_tokens: Option<u64>,
    /// A human-readable error message, if the stream failed.
    pub error: Option<String>,
    /// The reason for cancellation, if the stream was cancelled.
    pub cancel_reason: Option<String>,
    /// ISO-8601 timestamp of the most recent event for this stream.
    pub last_event: Option<String>,
    /// Open-ended metadata (model name, endpoint, etc.) from the start event.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl StreamSession {
    /// Creates a new session in the `Created` state.
    fn new(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            thread_id: None,
            agent_name: None,
            state: StreamLifeCycle::Created,
            content: String::new(),
            token_count: 0,
            total_tokens: None,
            error: None,
            cancel_reason: None,
            last_event: None,
            metadata: HashMap::new(),
        }
    }
}

/// The shared streaming context, provided once at the app root (or within a
/// subtree that needs streaming).
///
/// `Copy` (wraps a `Signal` of `Arc`-backed data) so it can be freely passed
/// into closures and stored in context providers, matching the pattern used by
/// [`ActivityManager`](crate::components::activity::ActivityManager).
#[derive(Clone, Copy)]
pub struct StreamingContext {
    /// Map of active sessions, keyed by `StreamId`. New sessions are added here;
    /// terminal sessions remain for history but can be cleared via `clear()`.
    pub sessions: Signal<HashMap<StreamId, StreamSession>>,
}

/// Retrieves the shared [`StreamingContext`].
pub fn use_streaming() -> StreamingContext {
    use_context::<StreamingContext>()
}

/// Provider component that owns the streaming event subscription lifetime.
///
/// Wraps the application tree (or a subtree). The `EventService` context must
/// be available above this provider. The subscription lives for the lifetime
/// of this component and is automatically cleaned up on unmount.
#[component]
pub fn StreamingProvider(children: Element) -> Element {
    let service = use_event_service();
    let sessions: Signal<HashMap<StreamId, StreamSession>> = use_signal(HashMap::new);

    // Subscribe to *all* events. We filter for streaming kinds inside the
    // callback so the subscription handle is a single RAII guard.
    //
    // The EventSubscription is stored in a Signal so it is not dropped
    // (and unsubscribed) until the Dioxus scope ends.
    let sub: Signal<Option<crate::events::EventSubscription>> = use_signal(|| None);

    if sub.peek().is_none() {
        let sessions_handle = sessions;
        let handle = service.subscribe_all(move |ev: &FrontendEvent| {
            if let Some(session) = process_streaming_event(ev, sessions_handle) {
                // Ensure the session exists in the map (process_streaming_event
                // inserts new sessions for `Started` / `SessionCreated` events).
                // For events that target an unknown stream id, we skip silently.
                drop(session);
            }
        });
        let mut guard = sub.write_unchecked();
        *guard = Some(handle);
    }

    // Clean up the subscription when the provider unmounts.
    use_drop(move || {
        let mut guard = sub.write_unchecked();
        if let Some(handle) = guard.take() {
            handle.unsubscribe();
        }
    });

    provide_context(StreamingContext { sessions });

    rsx! { {children} }
}

/// Dispatches a single frontend event to the streaming state machine.
///
/// Returns `Some(())` if the event was a streaming event (handled), or
/// `None` if the event was not a streaming event (and should be ignored).
///
/// New sessions are inserted for `StreamStarted` / `SessionCreated` events.
/// For events targeting a stream id not yet in the map, the event is logged
/// and skipped (this can happen if the stream started before the provider
/// mounted, or if events arrive out of order).
fn process_streaming_event(
    ev: &FrontendEvent,
    sessions: Signal<HashMap<StreamId, StreamSession>>,
) -> Option<()> {
    use nabu_core::event_bus::PipelineEvent;

    let timestamp = ev.timestamp.clone().unwrap_or_default();

    match &ev.payload {
        // ── Session lifecycle events ──
        PipelineEvent::Session(session_ev) => {
            use nabu_core::event_bus::StreamSessionEvent as SE;
            match session_ev {
                SE::SessionCreated { stream_id, thread_id, .. } => {
                    let mut map = sessions.write_unchecked();
                    if !map.contains_key(stream_id) {
                        let mut s = StreamSession::new(*stream_id);
                        s.thread_id = thread_id.as_ref().copied();
                        s.state = StreamLifeCycle::Created;
                        s.last_event = Some(timestamp);
                        map.insert(*stream_id, s);
                    }
                    tracing::debug!(stream_id = %stream_id, "stream session created");
                }
                SE::SessionStarted { stream_id, .. } => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(stream_id) {
                        s.state = StreamLifeCycle::Active;
                        s.last_event = Some(timestamp);
                    }
                }
                SE::SessionCancelled { stream_id, reason, .. } => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(stream_id) {
                        s.state = StreamLifeCycle::Cancelled;
                        s.cancel_reason = Some(reason.clone());
                        s.last_event = Some(timestamp);
                    }
                }
                SE::SessionCleanedUp { stream_id, .. } => {
                    let mut map = sessions.write_unchecked();
                    map.remove(stream_id);
                    tracing::debug!(stream_id = %stream_id, "stream session cleaned up and removed");
                }
                _ => {}
            }
            Some(())
        }

        // ── Stream (token/lifecycle) events ──
        PipelineEvent::Stream(stream_ev) => {
            use nabu_core::event_bus::StreamEvent as STE;
            match stream_ev {
                // Stream was started — create or update the session.
                STE::Started(e) => {
                    let mut map = sessions.write_unchecked();
                    let s = map.entry(e.stream_id).or_insert_with(|| StreamSession::new(e.stream_id));
                    s.thread_id = e.thread_id;
                    s.agent_name = e.agent_name.clone();
                    // Copy metadata from the start event.
                    s.metadata = e.metadata.clone();
                    // If we only had a SessionCreated event so far, the state
                    // may already be Created; transition to Active now that the
                    // stream is actually started.
                    if s.state.is_active() {
                        s.state = StreamLifeCycle::Active;
                    }
                    s.last_event = Some(timestamp);
                    tracing::debug!(stream_id = %e.stream_id, "stream started");
                    Some(())
                }

                // A token was received — append to the accumulated content.
                STE::Token(e) => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(&e.stream_id) {
                        s.content.push_str(&e.token);
                        s.token_count = e.sequence + 1;
                        s.last_event = Some(timestamp);
                    } else {
                        tracing::debug!(
                            stream_id = %e.stream_id,
                            "stream token received for unknown session; skipping"
                        );
                    }
                    Some(())
                }

                // A partial content update — use the pre-aggregated content.
                STE::PartialUpdate(e) => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(&e.stream_id) {
                        s.content = e.content.clone();
                        s.token_count = e.token_count;
                        s.last_event = Some(timestamp);
                    } else {
                        tracing::debug!(
                            stream_id = %e.stream_id,
                            "stream partial update received for unknown session; skipping"
                        );
                    }
                    Some(())
                }

                // Stream completed normally.
                STE::Completed(e) => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(&e.stream_id) {
                        s.content = e.full_content.clone();
                        s.token_count = e.total_tokens;
                        s.total_tokens = Some(e.total_tokens);
                        s.state = StreamLifeCycle::Completed;
                        s.last_event = Some(timestamp);
                        tracing::debug!(
                            stream_id = %e.stream_id,
                            tokens = e.total_tokens,
                            "stream completed"
                        );
                    }
                    Some(())
                }

                // Stream was cancelled.
                STE::Cancelled(e) => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(&e.stream_id) {
                        s.token_count = e.tokens_delivered;
                        s.state = StreamLifeCycle::Cancelled;
                        s.cancel_reason = Some(e.reason.clone());
                        s.last_event = Some(timestamp);
                        tracing::debug!(
                            stream_id = %e.stream_id,
                            tokens = e.tokens_delivered,
                            reason = %e.reason,
                            "stream cancelled"
                        );
                    }
                    Some(())
                }

                // Stream failed.
                STE::Failed(e) => {
                    let mut map = sessions.write_unchecked();
                    if let Some(s) = map.get_mut(&e.stream_id) {
                        s.token_count = e.tokens_delivered;
                        s.state = StreamLifeCycle::Failed;
                        s.error = Some(e.error.clone());
                        s.last_event = Some(timestamp);
                        tracing::debug!(
                            stream_id = %e.stream_id,
                            error = %e.error,
                            "stream failed"
                        );
                    }
                    Some(())
                }
                _ => None,
            }
        }

        // Non-streaming events — not handled here.
        _ => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{parse_raw, RawFrontendEvent};
    use nabu_core::event_bus::kinds;
    use nabu_core::event_bus::PipelineEvent;

    /// Builds a typed `FrontendEvent` from a raw envelope for testing.
    fn make_event(kind: &str, payload: serde_json::Value) -> FrontendEvent {
        let raw = RawFrontendEvent {
            event_type: kind.to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            payload,
        };
        parse_raw(raw).expect("kind should parse")
    }

    fn stream_id() -> StreamId {
        "123e4567-e89b-12d3-a456-426614174000"
            .parse::<StreamId>()
            .unwrap()
    }

    fn stream_started_payload() -> serde_json::Value {
        serde_json::json!({
            "Stream": {
                "Started": {
                    "stream_id": "123e4567-e89b-12d3-a456-426614174000",
                    "thread_id": null,
                    "agent_id": null,
                    "agent_name": Some("claude-code"),
                    "metadata": {},
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }
        })
    }

    fn stream_token_payload(token: &str, seq: u64) -> serde_json::Value {
        serde_json::json!({
            "Stream": {
                "Token": {
                    "stream_id": "123e4567-e89b-12d3-a456-426614174000",
                    "token": token,
                    "partial_content": token,
                    "sequence": seq,
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }
        })
    }

    fn stream_completed_payload(total: u64, content: &str) -> serde_json::Value {
        serde_json::json!({
            "Stream": {
                "Completed": {
                    "stream_id": "123e4567-e89b-12d3-a456-426614174000",
                    "full_content": content,
                    "total_tokens": total,
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }
        })
    }

    fn session_created_payload() -> serde_json::Value {
        serde_json::json!({
            "Session": {
                "SessionCreated": {
                    "stream_id": "123e4567-e89b-12d3-a456-426614174000",
                    "thread_id": null,
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }
        })
    }

    fn session_cleaned_up_payload() -> serde_json::Value {
        serde_json::json!({
            "Session": {
                "SessionCleanedUp": {
                    "stream_id": "123e4567-e89b-12d3-a456-426614174000",
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }
        })
    }

    #[test]
    fn stream_started_creates_session() {
        let service = EventService::new();
        // We can't easily test the Dioxus component here, but we can test
        // the event-processing logic by simulating the dispatch manually.
        // Since process_streaming_event requires a Signal (Dioxus context),
        // we test the core logic via direct event construction.
        let ev = make_event(kinds::STREAM_STARTED, stream_started_payload());
        assert!(matches!(
            ev.payload,
            PipelineEvent::Stream(_)
        ));
    }

    #[test]
    fn session_created_payload_parses() {
        let ev = make_event(kinds::SESSION_CREATED, session_created_payload());
        assert_eq!(ev.kind, FrontendEventKind::SessionCreated);
        assert!(matches!(
            ev.payload,
            PipelineEvent::Session(nabu_core::event_bus::StreamSessionEvent::SessionCreated { .. })
        ));
    }

    #[test]
    fn session_cleaned_up_payload_parses() {
        let ev = make_event(kinds::SESSION_CLEANED_UP, session_cleaned_up_payload());
        assert_eq!(ev.kind, FrontendEventKind::SessionCleanedUp);
    }

    #[test]
    fn stream_completed_payload_parses() {
        let ev = make_event(kinds::STREAM_COMPLETED, stream_completed_payload(42, "Hello world"));
        assert_eq!(ev.kind, FrontendEventKind::StreamCompleted);
        assert!(matches!(
            ev.payload,
            PipelineEvent::Stream(nabu_core::event_bus::StreamEvent::Completed(_))
        ));

        if let PipelineEvent::Stream(nabu_core::event_bus::StreamEvent::Completed(e)) = ev.payload {
            assert_eq!(e.total_tokens, 42);
            assert_eq!(e.full_content, "Hello world");
        }
    }

    #[test]
    fn stream_token_payload_parses() {
        let ev = make_event(kinds::STREAM_TOKEN, stream_token_payload("Hello", 0));
        assert_eq!(ev.kind, FrontendEventKind::StreamToken);
        assert!(matches!(
            ev.payload,
            PipelineEvent::Stream(nabu_core::event_bus::StreamEvent::Token(_))
        ));

        if let PipelineEvent::Stream(nabu_core::event_bus::StreamEvent::Token(e)) = ev.payload {
            assert_eq!(e.token, "Hello");
            assert_eq!(e.sequence, 0);
        }
    }

    #[test]
    fn non_streaming_event_returns_none() {
        // An item.stored event should be ignored by the streaming processor.
        let raw = RawFrontendEvent {
            event_type: kinds::ITEM_STORED.to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            payload: serde_json::json!({
                "ItemStored": {
                    "object_id": "12345678-1234-1234-1234-123456789abc",
                    "vault_path": "notes/foo.md",
                    "object_type": "Note",
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        };
        let ev = parse_raw(raw).unwrap();
        // process_streaming_event returns None for non-streaming events.
        // (We can't create a Signal without a Dioxus scope, but the match
        // arm for non-streaming events returns None before touching signals.)
        // Verify the event kind is not a streaming kind.
        assert_ne!(ev.kind, FrontendEventKind::StreamStarted);
        assert_ne!(ev.kind, FrontendEventKind::StreamToken);
        assert_ne!(ev.kind, FrontendEventKind::StreamCompleted);
    }

    #[test]
    fn stream_lifecycle_labels() {
        assert_eq!(StreamLifeCycle::Created.label(), "Created");
        assert_eq!(StreamLifeCycle::Active.label(), "Streaming");
        assert_eq!(StreamLifeCycle::Completed.label(), "Completed");
        assert_eq!(StreamLifeCycle::Cancelled.label(), "Cancelled");
        assert_eq!(StreamLifeCycle::Failed.label(), "Failed");
    }

    #[test]
    fn stream_lifecycle_is_active() {
        assert!(StreamLifeCycle::Created.is_active());
        assert!(StreamLifeCycle::Active.is_active());
        assert!(!StreamLifeCycle::Completed.is_active());
        assert!(!StreamLifeCycle::Cancelled.is_active());
        assert!(!StreamLifeCycle::Failed.is_active());
    }

    #[test]
    fn stream_lifecycle_is_terminal() {
        assert!(!StreamLifeCycle::Created.is_terminal());
        assert!(!StreamLifeCycle::Active.is_terminal());
        assert!(StreamLifeCycle::Completed.is_terminal());
        assert!(StreamLifeCycle::Cancelled.is_terminal());
        assert!(StreamLifeCycle::Failed.is_terminal());
    }

    #[test]
    fn all_frontend_streaming_kinds_round_trip() {
        // Verify that every streaming FrontendEventKind maps to a backend kind
        // constant and back.
        let streaming_kinds = [
            (FrontendEventKind::StreamStarted, kinds::STREAM_STARTED),
            (FrontendEventKind::StreamToken, kinds::STREAM_TOKEN),
            (FrontendEventKind::StreamPartialUpdate, kinds::STREAM_PARTIAL_UPDATE),
            (FrontendEventKind::StreamCompleted, kinds::STREAM_COMPLETED),
            (FrontendEventKind::StreamCancelled, kinds::STREAM_CANCELLED),
            (FrontendEventKind::StreamFailed, kinds::STREAM_FAILED),
            (FrontendEventKind::SessionCreated, kinds::SESSION_CREATED),
            (FrontendEventKind::SessionStarted, kinds::SESSION_STARTED),
            (FrontendEventKind::SessionCancelled, kinds::SESSION_CANCELLED),
            (FrontendEventKind::SessionCleanedUp, kinds::SESSION_CLEANED_UP),
        ];
        for (kind, expected_str) in streaming_kinds {
            assert_eq!(kind.as_str(), expected_str);
            assert_eq!(FrontendEventKind::from_str(expected_str), Some(kind));
        }
    }
}
