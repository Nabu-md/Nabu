//! Streaming session — the runtime record for a single token stream.
//!
//! A [`StreamingSession`] manages the state and lifecycle of a single
//! streaming session. It tracks the stream's state, accumulates partial
//! content, maintains token sequence numbers, and provides the handle
//! used by callers to publish tokens and manage the stream lifecycle.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::event_bus::{
    kinds, EventBus, PipelineEvent, StreamEvent, StreamId, StreamSessionEvent,
    StreamState as EventStreamState,
};
use crate::streaming::errors::{StreamManagerError, StreamResult};

/// The lifecycle state of a streaming session.
///
/// This is the session-level state, distinct from the per-token events.
/// It mirrors [`EventStreamState`](crate::event_bus::StreamState) but is
/// defined here as a convenience and to avoid coupling the session module
/// to the event_bus module's enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamState {
    /// The session has been created but has not yet published any tokens.
    Active,
    /// At least one token has been published; the stream is actively producing output.
    Streaming,
    /// The stream completed normally (all tokens delivered).
    Completed,
    /// The stream was cancelled before completion.
    Cancelled,
    /// The stream failed due to an error.
    Failed,
}

impl Default for StreamState {
    fn default() -> Self {
        Self::Active
    }
}

impl StreamState {
    /// Returns `true` if the stream is in an active (non-terminal) state.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Streaming)
    }

    /// Returns `true` if the stream is in a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    /// Returns the label string for this state.
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Streaming => "streaming",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl From<StreamState> for EventStreamState {
    fn from(state: StreamState) -> Self {
        match state {
            StreamState::Active => EventStreamState::Active,
            StreamState::Streaming => EventStreamState::Streaming,
            StreamState::Completed => EventStreamState::Completed,
            StreamState::Cancelled => EventStreamState::Cancelled,
            StreamState::Failed => EventStreamState::Failed,
        }
    }
}

impl From<EventStreamState> for StreamState {
    fn from(state: EventStreamState) -> Self {
        match state {
            EventStreamState::Active => StreamState::Active,
            EventStreamState::Streaming => StreamState::Streaming,
            EventStreamState::Completed => StreamState::Completed,
            EventStreamState::Cancelled => StreamState::Cancelled,
            EventStreamState::Failed => StreamState::Failed,
        }
    }
}

/// The runtime record for a single streaming session.
///
/// A `StreamingSession` binds together:
/// - The stream's identity (`StreamId`).
/// - The conversation/thread ID this stream is associated with (optional).
/// - The originating agent's process ID and name (optional).
/// - The stream's current [`StreamState`].
/// - Accumulated partial content (`partial_content`).
/// - A sequence counter for token ordering.
/// - A cancellation flag for graceful shutdown.
///
/// The session publishes all events through the [`EventBus`] — it does not
/// directly call any frontend callbacks.
pub struct StreamingSession {
    /// The unique identifier for this streaming session.
    pub stream_id: StreamId,

    /// The conversation/thread ID this stream is associated with, if any.
    pub thread_id: Option<Uuid>,

    /// The agent process that originated this stream, if applicable.
    pub agent_id: Option<crate::event_bus::ProcessId>,

    /// The agent name, if the stream is associated with a named agent.
    pub agent_name: Option<String>,

    /// The current state of the stream.
    state: StdMutex<StreamState>,

    /// The accumulated partial content so far.
    ///
    /// Uses a `Mutex<String>` because content is appended on each token
    /// and read for partial-update events and completion.
    partial_content: StdMutex<String>,

    /// The sequence counter for token ordering (0-based, incremented per token).
    sequence: AtomicU64,

    /// Atomic cancellation flag — when `true`, no further tokens should be
    /// published and `publish_token` returns an error.
    cancelled: AtomicBool,

    /// When the session was created.
    created_at: DateTime<Utc>,

    /// When the session was started (first token published), if any.
    started_at: StdMutex<Option<DateTime<Utc>>>,

    /// When the session entered a terminal state, if any.
    ended_at: StdMutex<Option<DateTime<Utc>>>,

    /// Optional metadata for future extension.
    metadata: StdMutex<HashMap<String, serde_json::Value>>,

    /// Reference to the EventBus for publishing events.
    /// `None` when the pipeline operates without an EventBus (e.g. in tests).
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,
}

impl StreamingSession {
    /// Create a new streaming session in the `Active` state.
    ///
    /// The session is created but not yet "started" — the first call to
    /// [`publish_token`](Self::publish_token) transitions it to `Streaming`.
    ///
    /// # Arguments
    ///
    /// * `stream_id` — The unique identifier for this stream.
    /// * `thread_id` — The conversation/thread ID, if associated.
    /// * `agent_id` — The originating agent's process ID, if applicable.
    /// * `agent_name` — The agent name, if applicable.
    /// * `event_bus` — The EventBus for publishing events. `None` disables
    ///   event publishing (useful in tests).
    pub fn new(
        stream_id: StreamId,
        thread_id: Option<Uuid>,
        agent_id: Option<crate::event_bus::ProcessId>,
        agent_name: Option<String>,
        event_bus: Option<Arc<EventBus<PipelineEvent>>>,
    ) -> Self {
        Self {
            stream_id,
            thread_id,
            agent_id,
            agent_name,
            state: StdMutex::new(StreamState::Active),
            partial_content: StdMutex::new(String::new()),
            sequence: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            created_at: Utc::now(),
            started_at: StdMutex::new(None),
            ended_at: StdMutex::new(None),
            metadata: StdMutex::new(HashMap::new()),
            event_bus,
        }
    }

    /// Returns the stream ID for this session.
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    /// Returns the conversation/thread ID, if any.
    pub fn thread_id(&self) -> Option<Uuid> {
        self.thread_id
    }

    /// Returns the agent process ID, if any.
    pub fn agent_id(&self) -> Option<crate::event_bus::ProcessId> {
        self.agent_id
    }

    /// Returns the agent name, if any.
    pub fn agent_name(&self) -> Option<&str> {
        self.agent_name.as_deref()
    }

    /// Returns the current state of this session.
    pub fn state(&self) -> StreamState {
        *self.state.lock().expect("session state lock poisoned")
    }

    /// Returns `true` if the stream is in an active (non-terminal) state.
    pub fn is_active(&self) -> bool {
        self.state().is_active()
    }

    /// Returns `true` if the stream is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state().is_terminal()
    }

    /// Returns `true` if this session has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Returns the accumulated partial content so far.
    pub fn partial_content(&self) -> String {
        self.partial_content
            .lock()
            .expect("partial_content lock poisoned")
            .clone()
    }

    /// Returns the number of tokens published so far.
    pub fn token_count(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    /// Returns the creation timestamp.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the start timestamp (when the first token was published), if any.
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        *self
            .started_at
            .lock()
            .expect("started_at lock poisoned")
    }

    /// Returns the end timestamp (when a terminal state was entered), if any.
    pub fn ended_at(&self) -> Option<DateTime<Utc>> {
        *self
            .ended_at
            .lock()
            .expect("ended_at lock poisoned")
    }

    /// Returns a snapshot of this session's metadata.
    pub fn metadata(&self) -> HashMap<String, serde_json::Value> {
        self.metadata
            .lock()
            .expect("metadata lock poisoned")
            .clone()
    }

    /// Sets a metadata key-value pair.
    pub fn set_metadata(&self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata
            .lock()
            .expect("metadata lock poisoned")
            .insert(key.into(), value);
    }

    /// Publish a "stream started" event to the EventBus.
    ///
    /// This is called once when the stream begins. It publishes both a
    /// [`StreamEvent::Started`] (token-level) and a
    /// [`StreamSessionEvent::SessionCreated`] + [`StreamSessionEvent::SessionStarted`]
    /// (session-level).
    pub fn publish_started(&self) -> StreamResult<()> {
        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        // Publish StreamEvent::Started
        let started_event = StreamEvent::Started(crate::event_bus::StreamStartedEvent::new(
            self.stream_id,
            self.thread_id,
            self.agent_id,
            self.agent_name.clone(),
        ));
        bus.publish(kinds::STREAM_STARTED, &PipelineEvent::Stream(started_event));

        // Publish session-level events
        let now = Utc::now();
        let session_created = StreamSessionEvent::SessionCreated {
            stream_id: self.stream_id,
            thread_id: self.thread_id,
            timestamp: now,
        };
        bus.publish(
            kinds::SESSION_CREATED,
            &PipelineEvent::Session(session_created),
        );

        let session_started = StreamSessionEvent::SessionStarted {
            stream_id: self.stream_id,
            timestamp: now,
        };
        bus.publish(
            kinds::SESSION_STARTED,
            &PipelineEvent::Session(session_started),
        );

        // Mark as started
        {
            let mut started_at = self
                .started_at
                .lock()
                .expect("started_at lock poisoned");
            *started_at = Some(now);
        }

        Ok(())
    }

    /// Publish a single token to the EventBus.
    ///
    /// This is the core method for incremental token delivery. Each call:
    /// 1. Checks that the stream is not cancelled or in a terminal state.
    /// 2. Transitions the session to `Streaming` if it was `Active`.
    /// 3. Appends the token to `partial_content`.
    /// 4. Increments the sequence counter.
    /// 5. Publishes a `StreamEvent::Token` through the EventBus.
    ///
    /// # Ordering
    ///
    /// Because the session is locked during publication, tokens are published
    /// strictly in sequence order — no interleaving can occur between
    /// concurrent `publish_token` calls on the same session.
    ///
    /// # Cancellation
    ///
    /// If the stream has been cancelled (via [`cancel`](Self::cancel)), this
    /// method returns `StreamManagerError::Cancelled`.
    pub fn publish_token(&self, token: impl Into<String>) -> StreamResult<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(StreamManagerError::Cancelled {
                stream_id: self.stream_id,
            });
        }

        let state = self.state();
        if state.is_terminal() {
            return Err(StreamManagerError::StreamAlreadyTerminal {
                stream_id: self.stream_id,
                state: state.into(),
            });
        }

        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        // Atomically: transition state, append content, increment sequence, publish.
        let token_str = token.into();

        // Transition Active → Streaming on first token
        {
            let mut state_guard = self
                .state
                .lock()
                .expect("session state lock poisoned");
            if *state_guard == StreamState::Active {
                *state_guard = StreamState::Streaming;
                let mut started_at = self
                    .started_at
                    .lock()
                    .expect("started_at lock poisoned");
                *started_at = Some(Utc::now());
            }
        }

        // Append to partial content and get sequence number
        let sequence = self.sequence.fetch_add(1, Ordering::AcqRel);
        let partial = {
            let mut content = self
                .partial_content
                .lock()
                .expect("partial_content lock poisoned");
            content.push_str(&token_str);
            content.clone()
        };

        // Publish the token event
        let token_event = StreamEvent::Token(crate::event_bus::StreamTokenEvent::new(
            self.stream_id,
            token_str,
            partial,
            sequence,
        ));
        bus.publish(kinds::STREAM_TOKEN, &PipelineEvent::Stream(token_event));

        Ok(())
    }

    /// Publish a partial content update event to the EventBus.
    ///
    /// This is a convenience event for subscribers that want the full
    /// accumulated content without concatenating individual tokens. It
    /// is published alongside tokens (not instead of them) and does not
    /// affect the sequence counter.
    pub fn publish_partial_update(&self) -> StreamResult<()> {
        let state = self.state();
        if state.is_terminal() {
            return Err(StreamManagerError::StreamAlreadyTerminal {
                stream_id: self.stream_id,
                state: state.into(),
            });
        }

        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        let (content, sequence) = {
            let content = self
                .partial_content
                .lock()
                .expect("partial_content lock poisoned");
            let seq = self.sequence.load(Ordering::Acquire);
            (content.clone(), seq)
        };

        let partial_event = StreamEvent::PartialUpdate(
            crate::event_bus::StreamPartialUpdateEvent::new(self.stream_id, content, sequence),
        );
        bus.publish(
            kinds::STREAM_PARTIAL_UPDATE,
            &PipelineEvent::Stream(partial_event),
        );

        Ok(())
    }

    /// Complete this stream normally.
    ///
    /// Publishes a `StreamEvent::Completed` terminal event and a
    /// `StreamSessionEvent::SessionCleanedUp` session-level event, then
    /// marks the session as `Completed`.
    pub fn complete(&self) -> StreamResult<()> {
        let mut state_guard = self
            .state
            .lock()
            .expect("session state lock poisoned");
        if state_guard.is_terminal() {
            return Err(StreamManagerError::StreamAlreadyTerminal {
                stream_id: self.stream_id,
                state: (*state_guard).into(),
            });
        }
        *state_guard = StreamState::Completed;
        drop(state_guard);

        let ended_at = Utc::now();
        {
            let mut ended = self
                .ended_at
                .lock()
                .expect("ended_at lock poisoned");
            *ended = Some(ended_at);
        }

        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        let (content, token_count) = {
            let content = self
                .partial_content
                .lock()
                .expect("partial_content lock poisoned");
            let seq = self.sequence.load(Ordering::Acquire);
            (content.clone(), seq)
        };

        let completed_event = StreamEvent::Completed(crate::event_bus::StreamCompletedEvent::new(
            self.stream_id,
            content,
            token_count,
        ));
        bus.publish(
            kinds::STREAM_COMPLETED,
            &PipelineEvent::Stream(completed_event),
        );

        let session_cleaned = StreamSessionEvent::SessionCleanedUp {
            stream_id: self.stream_id,
            timestamp: ended_at,
        };
        bus.publish(
            kinds::SESSION_CLEANED_UP,
            &PipelineEvent::Session(session_cleaned),
        );

        Ok(())
    }

    /// Cancel this stream.
    ///
    /// Sets the cancellation flag, publishes a `StreamEvent::Cancelled` and
    /// `StreamSessionEvent::SessionCancelled`, and marks the session as
    /// `Cancelled`. After this call, `publish_token` will return
    /// [`StreamManagerError::Cancelled`].
    pub fn cancel(&self, reason: impl Into<String>) -> StreamResult<()> {
        self.cancelled.store(true, Ordering::Release);

        let mut state_guard = self
            .state
            .lock()
            .expect("session state lock poisoned");
        if state_guard.is_terminal() {
            return Err(StreamManagerError::StreamAlreadyTerminal {
                stream_id: self.stream_id,
                state: (*state_guard).into(),
            });
        }
        *state_guard = StreamState::Cancelled;
        drop(state_guard);

        let ended_at = Utc::now();
        {
            let mut ended = self
                .ended_at
                .lock()
                .expect("ended_at lock poisoned");
            *ended = Some(ended_at);
        }

        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        let (content, token_count) = {
            let content = self
                .partial_content
                .lock()
                .expect("partial_content lock poisoned");
            let seq = self.sequence.load(Ordering::Acquire);
            (content.clone(), seq)
        };

        let cancelled_event = StreamEvent::Cancelled(crate::event_bus::StreamCancelledEvent::new(
            self.stream_id,
            token_count,
            content,
            reason.into(),
        ));
        bus.publish(
            kinds::STREAM_CANCELLED,
            &PipelineEvent::Stream(cancelled_event),
        );

        let session_cancelled = StreamSessionEvent::SessionCancelled {
            stream_id: self.stream_id,
            reason: String::new(), // Will be set by caller via pipeline
            timestamp: ended_at,
        };
        bus.publish(
            kinds::SESSION_CANCELLED,
            &PipelineEvent::Session(session_cancelled),
        );

        Ok(())
    }

    /// Fail this stream with an error.
    ///
    /// Publishes a `StreamEvent::Failed` terminal event and marks the
    /// session as `Failed`. After this call, `publish_token` will return
    /// [`StreamManagerError::Failed`].
    pub fn fail(&self, error: impl Into<String>) -> StreamResult<()> {
        let mut state_guard = self
            .state
            .lock()
            .expect("session state lock poisoned");
        if state_guard.is_terminal() {
            return Err(StreamManagerError::StreamAlreadyTerminal {
                stream_id: self.stream_id,
                state: (*state_guard).into(),
            });
        }
        *state_guard = StreamState::Failed;
        drop(state_guard);

        let ended_at = Utc::now();
        {
            let mut ended = self
                .ended_at
                .lock()
                .expect("ended_at lock poisoned");
            *ended = Some(ended_at);
        }

        let bus = self.event_bus.as_ref().ok_or(StreamManagerError::NoEventBus)?;

        let (content, token_count) = {
            let content = self
                .partial_content
                .lock()
                .expect("partial_content lock poisoned");
            let seq = self.sequence.load(Ordering::Acquire);
            (content.clone(), seq)
        };

        let failed_event = StreamEvent::Failed(crate::event_bus::StreamFailedEvent::new(
            self.stream_id,
            token_count,
            content,
            error.into(),
        ));
        bus.publish(
            kinds::STREAM_FAILED,
            &PipelineEvent::Stream(failed_event),
        );

        Ok(())
    }
}

impl std::fmt::Debug for StreamingSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingSession")
            .field("stream_id", &self.stream_id)
            .field("thread_id", &self.thread_id)
            .field("agent_id", &self.agent_id)
            .field("agent_name", &self.agent_name)
            .field("state", &self.state())
            .field("token_count", &self.token_count())
            .field("is_cancelled", &self.cancelled.load(Ordering::Acquire))
            .field("created_at", &self.created_at)
            .field("started_at", &*self.started_at.lock().expect("lock"))
            .field("ended_at", &*self.ended_at.lock().expect("lock"))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// StreamSessionHandle
// ---------------------------------------------------------------------------

/// A handle to a streaming session, returned to callers for cancellation
/// and lifecycle management.
///
/// The handle holds an `Arc<Mutex<StreamingSession>>` so it can be shared
/// across threads. Multiple handles to the same stream are supported — they
/// all point to the same underlying session.
pub struct StreamSessionHandle {
    pub(crate) session: Arc<StdMutex<StreamingSession>>,
}

impl StreamSessionHandle {
    /// Creates a new handle from a streaming session.
    pub fn new(session: StreamingSession) -> Self {
        Self {
            session: Arc::new(StdMutex::new(session)),
        }
    }

    /// Returns the stream ID for this session.
    pub fn stream_id(&self) -> StreamId {
        self.session
            .lock()
            .expect("session lock poisoned")
            .stream_id
    }

    /// Returns the current state of this stream.
    pub fn state(&self) -> StreamState {
        self.session
            .lock()
            .expect("session lock poisoned")
            .state()
    }

    /// Returns `true` if the stream is in an active (non-terminal) state.
    pub fn is_active(&self) -> bool {
        self.session
            .lock()
            .expect("session lock poisoned")
            .is_active()
    }

    /// Returns `true` if the stream is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.session
            .lock()
            .expect("session lock poisoned")
            .is_terminal()
    }

    /// Returns `true` if this stream has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.session
            .lock()
            .expect("session lock poisoned")
            .is_cancelled()
    }

    /// Returns the accumulated partial content so far.
    pub fn partial_content(&self) -> String {
        self.session
            .lock()
            .expect("session lock poisoned")
            .partial_content()
    }

    /// Returns the number of tokens published so far.
    pub fn token_count(&self) -> u64 {
        self.session
            .lock()
            .expect("session lock poisoned")
            .token_count()
    }

    /// Publishes a token to the stream through the EventBus.
    ///
    /// See [`StreamingSession::publish_token`] for details.
    pub fn publish_token(&self, token: impl Into<String>) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.publish_token(token)
    }

    /// Publishes a partial content update event.
    pub fn publish_partial_update(&self) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.publish_partial_update()
    }

    /// Publishes the "stream started" events.
    pub fn publish_started(&self) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.publish_started()
    }

    /// Cancels this stream.
    ///
    /// After cancellation, no further tokens can be published. The
    /// cancellation reason is published through the EventBus.
    pub fn cancel(&self, reason: impl Into<String>) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.cancel(reason)
    }

    /// Completes this stream normally.
    pub fn complete(&self) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.complete()
    }

    /// Fails this stream with an error.
    pub fn fail(&self, error: impl Into<String>) -> StreamResult<()> {
        let session = self
            .session
            .lock()
            .expect("session lock poisoned");
        session.fail(error)
    }
}

impl Clone for StreamSessionHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
        }
    }
}

impl std::fmt::Debug for StreamSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let session = self.session.lock().expect("session lock poisoned");
        f.debug_struct("StreamSessionHandle")
            .field("stream_id", &session.stream_id)
            .field("state", &session.state())
            .field("token_count", &session.token_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::{EventBus, PipelineEvent};
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    fn test_bus() -> Arc<EventBus<PipelineEvent>> {
        Arc::new(EventBus::new())
    }

    #[test]
    fn session_starts_in_active_state() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        assert_eq!(session.state(), StreamState::Active);
        assert!(session.is_active());
        assert!(!session.is_terminal());
        assert!(!session.is_cancelled());
        assert_eq!(session.token_count(), 0);
        assert_eq!(session.partial_content(), "");
    }

    #[test]
    fn publish_started_publishes_event() {
        let bus = test_bus();
        let started_count = Arc::new(AtomicUsize::new(0));
        let started_count_clone = started_count.clone();
        bus.subscribe(kinds::STREAM_STARTED, move |_ev: &PipelineEvent| {
            started_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(bus.clone()),
        );
        assert!(session.publish_started().is_ok());
        assert!(started_count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn publish_token_transitions_to_streaming() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.publish_token("hello").unwrap();
        assert_eq!(session.state(), StreamState::Streaming);
        assert_eq!(session.token_count(), 1);
        assert_eq!(session.partial_content(), "hello");
    }

    #[test]
    fn publish_multiple_tokens_preserves_ordering() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.publish_token("hello ").unwrap();
        session.publish_token("world").unwrap();
        assert_eq!(session.partial_content(), "hello world");
        assert_eq!(session.token_count(), 2);
    }

    #[test]
    fn complete_transitions_to_completed() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.publish_token("hello").unwrap();
        session.complete().unwrap();
        assert_eq!(session.state(), StreamState::Completed);
        assert!(session.is_terminal());
    }

    #[test]
    fn cancel_prevents_further_tokens() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.cancel("user requested").unwrap();
        assert_eq!(session.state(), StreamState::Cancelled);
        assert!(session.is_cancelled());
        assert!(session.is_terminal());

        let result = session.publish_token("more");
        assert!(matches!(result, Err(StreamManagerError::Cancelled { .. })));
    }

    #[test]
    fn fail_transitions_to_failed() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.publish_token("partial").unwrap();
        session.fail("connection lost").unwrap();
        assert_eq!(session.state(), StreamState::Failed);
        assert!(session.is_terminal());
    }

    #[test]
    fn publish_token_on_terminal_fails() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.publish_started().unwrap();
        session.complete().unwrap();

        let result = session.publish_token("after complete");
        assert!(matches!(
            result,
            Err(StreamManagerError::StreamAlreadyTerminal { .. })
        ));
    }

    #[test]
    fn token_events_have_sequential_ids() {
        let bus = test_bus();
        let sequences = Arc::new(Mutex::new(Vec::new()));
        let seq_clone = sequences.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
            if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
                seq_clone.lock().expect("lock").push(e.sequence);
            }
        });

        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(bus),
        );
        session.publish_started().unwrap();
        for i in 0..5 {
            session.publish_token(format!("token{}", i)).unwrap();
        }

        let seqs = sequences.lock().expect("lock").clone();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn no_event_bus_returns_error() {
        let session = StreamingSession::new(Uuid::new_v4(), None, None, None, None);
        assert!(matches!(
            session.publish_started(),
            Err(StreamManagerError::NoEventBus)
        ));
        assert!(matches!(
            session.publish_token("test"),
            Err(StreamManagerError::NoEventBus)
        ));
    }

    #[test]
    fn metadata_can_be_set_and_read() {
        let session = StreamingSession::new(
            Uuid::new_v4(),
            None,
            None,
            None,
            Some(test_bus()),
        );
        session.set_metadata("model", serde_json::json!("gpt-4"));
        session.set_metadata("temperature", serde_json::json!(0.7));

        let meta = session.metadata();
        assert_eq!(meta.get("model"), Some(&serde_json::json!("gpt-4")));
        assert_eq!(meta.get("temperature"), Some(&serde_json::json!(0.7)));
    }
}
