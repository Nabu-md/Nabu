//! Stream Manager — thread-safe registry of active streaming sessions.
//!
//! The [`StreamManager`] provides a thread-safe container for tracking all
//! active streaming sessions. It is the central registry that allows the
//! platform to:
//!
//! - Create new streaming sessions
//! - Look up existing sessions by `StreamId`
//! - Cancel sessions by ID
//! - Clean up terminal sessions
//! - Query the number of active sessions
//!
//! The manager is `Send + Sync` and designed to be shared as
//! `Arc<StreamManager>` across threads.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::event_bus::{EventBus, PipelineEvent, StreamId};
use crate::streaming::errors::{StreamManagerError, StreamResult};
use crate::streaming::session::{StreamingSession, StreamSessionHandle};

/// A thread-safe registry for tracking active streaming sessions.
///
/// The manager stores sessions as `Arc<Mutex<StreamingSession>>` records,
/// allowing both the main thread (for synchronous API calls) and background
/// tasks to access and modify session state safely.
///
/// ## Thread Safety
///
/// The manager uses:
/// - `RwLock<HashMap<StreamId, Arc<Mutex<StreamingSession>>>>` for the session map
///   — read lock for lookups, write lock for insertion/removal.
/// - `Mutex<StreamingSession>` for each session's internal state —
///   fine-grained per-session locking.
///
/// Multiple sessions are fully isolated — concurrent operations on different
/// sessions never block each other.
///
/// ## Usage
///
/// ```no_run
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::streaming::StreamManager;
/// use std::sync::Arc;
///
/// let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
/// let manager = Arc::new(StreamManager::new(bus));
///
/// // Create a new stream
/// let handle = manager.create_stream(None, None, None).unwrap();
/// let stream_id = handle.stream_id();
///
/// // Look up by ID
/// let handle2 = manager.get_stream(&stream_id).unwrap();
/// handle2.publish_token("hello").unwrap();
///
/// // Cancel by ID
/// manager.cancel_stream(&stream_id, "user requested").unwrap();
/// assert!(manager.get_stream(&stream_id).unwrap().is_cancelled());
/// ```
pub struct StreamManager {
    /// All active and terminal sessions, keyed by stream ID.
    ///
    /// Sessions are retained until they are explicitly cleaned up via
    /// [`cleanup_completed`](Self::cleanup_completed) or removed via
    /// [`remove`](Self::remove).
    sessions: RwLock<HashMap<StreamId, Arc<Mutex<StreamingSession>>>>,

    /// The EventBus used to publish streaming events.
    event_bus: Arc<EventBus<PipelineEvent>>,
}

impl StreamManager {
    /// Create a new stream manager with the given EventBus.
    pub fn new(event_bus: Arc<EventBus<PipelineEvent>>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            event_bus,
        }
    }

    /// Returns a reference to the EventBus.
    pub fn event_bus(&self) -> &Arc<EventBus<PipelineEvent>> {
        &self.event_bus
    }

    /// Create a new streaming session.
    ///
    /// The session is created in the `Active` state and the "stream started"
    /// events are published to the EventBus.
    ///
    /// # Arguments
    ///
    /// * `thread_id` — The conversation/thread ID this stream is associated with.
    /// * `agent_id` — The originating agent's process ID, if applicable.
    /// * `agent_name` — The agent name, if applicable.
    ///
    /// # Returns
    ///
    /// A `StreamSessionHandle` that can be used to publish tokens and
    /// manage the stream lifecycle.
    pub fn create_stream(
        &self,
        thread_id: Option<uuid::Uuid>,
        agent_id: Option<crate::event_bus::ProcessId>,
        agent_name: Option<String>,
    ) -> StreamResult<StreamSessionHandle> {
        let stream_id = StreamId::new_v4();

        let session = StreamingSession::new(
            stream_id,
            thread_id,
            agent_id,
            agent_name,
            Some(self.event_bus.clone()),
        );

        let handle = StreamSessionHandle::new(session);
        handle.publish_started()?;

        // Insert into the registry
        {
            let mut sessions = self
                .sessions
                .write()
                .expect("stream manager sessions lock poisoned");
            sessions.insert(stream_id, handle.session.clone());
        }

        Ok(handle)
    }

    /// Returns `true` if a stream with the given ID exists in the registry.
    pub fn has_stream(&self, stream_id: &StreamId) -> bool {
        self.sessions
            .read()
            .expect("stream manager sessions lock poisoned")
            .contains_key(stream_id)
    }

    /// Look up a stream by its ID.
    ///
    /// Returns `None` if no stream with the given ID exists.
    pub fn get_stream(&self, stream_id: &StreamId) -> Option<StreamSessionHandle> {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");

        sessions.get(stream_id).map(|session| StreamSessionHandle {
            session: session.clone(),
        })
    }

    /// Returns the number of registered sessions (active and terminal).
    pub fn session_count(&self) -> usize {
        self.sessions
            .read()
            .expect("stream manager sessions lock poisoned")
            .len()
    }

    /// Returns the number of sessions in active (non-terminal) state.
    pub fn active_session_count(&self) -> usize {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");

        sessions
            .values()
            .filter(|s| s.lock().expect("session lock poisoned").is_active())
            .count()
    }

    /// Returns the number of sessions in terminal state.
    pub fn completed_session_count(&self) -> usize {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");

        sessions
            .values()
            .filter(|s| s.lock().expect("session lock poisoned").is_terminal())
            .count()
    }

    /// Remove a stream from the registry.
    ///
    /// The stream must be in a terminal state. Returns `Err` if the stream
    /// is still active or not found. After removal, the stream's events
    /// will no longer be published through the EventBus.
    ///
    /// # Errors
    ///
    /// - [`StreamManagerError::StreamNotFound`] if no stream with the given ID exists.
    /// - [`StreamManagerError::StreamAlreadyTerminal`] is NOT returned —
    ///   the method succeeds if the stream is terminal. If the stream is
    ///   still active, a different error is returned.
    pub fn remove(&self, stream_id: &StreamId) -> StreamResult<()> {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");

        let session = sessions.get(stream_id).cloned().ok_or_else(|| {
            StreamManagerError::StreamNotFound(*stream_id)
        })?;
        drop(sessions);

        let session_guard = session.lock().expect("session lock poisoned");
        if !session_guard.is_terminal() {
            return Err(StreamManagerError::Internal(format!(
                "stream '{}' is still active (state: {:?}); cannot remove non-terminal stream",
                stream_id,
                session_guard.state()
            )));
        }
        drop(session_guard);

        let mut sessions = self
            .sessions
            .write()
            .expect("stream manager sessions lock poisoned");
        sessions.remove(stream_id);

        Ok(())
    }

    /// Cancel a stream by its ID.
    ///
    /// Delegates to [`StreamingSession::cancel`] on the session. After
    /// cancellation, no further tokens can be published to the stream.
    ///
    /// # Errors
    ///
    /// - [`StreamManagerError::StreamNotFound`] if no stream with the given ID exists.
    pub fn cancel_stream(&self, stream_id: &StreamId, reason: impl Into<String>) -> StreamResult<()> {
        let handle = self.get_stream(stream_id).ok_or_else(|| {
            StreamManagerError::StreamNotFound(*stream_id)
        })?;
        handle.cancel(reason)
    }

    /// Complete a stream by its ID.
    ///
    /// Delegates to [`StreamingSession::complete`] on the session.
    ///
    /// # Errors
    ///
    /// - [`StreamManagerError::StreamNotFound`] if no stream with the given ID exists.
    pub fn complete_stream(&self, stream_id: &StreamId) -> StreamResult<()> {
        let handle = self.get_stream(stream_id).ok_or_else(|| {
            StreamManagerError::StreamNotFound(*stream_id)
        })?;
        handle.complete()
    }

    /// Fail a stream by its ID.
    ///
    /// Delegates to [`StreamingSession::fail`] on the session.
    ///
    /// # Errors
    ///
    /// - [`StreamManagerError::StreamNotFound`] if no stream with the given ID exists.
    pub fn fail_stream(&self, stream_id: &StreamId, error: impl Into<String>) -> StreamResult<()> {
        let handle = self.get_stream(stream_id).ok_or_else(|| {
            StreamManagerError::StreamNotFound(*stream_id)
        })?;
        handle.fail(error)
    }

    /// Returns a list of all active (non-terminal) stream IDs.
    pub fn active_stream_ids(&self) -> Vec<StreamId> {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");

        sessions
            .iter()
            .filter_map(|(id, session)| {
                if session.lock().expect("session lock poisoned").is_active() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clean up all completed/cancelled/failed streams.
    ///
    /// Removes sessions that have entered a terminal state from the registry.
    /// Active streams are never removed.
    ///
    /// Returns the number of sessions that were removed.
    pub fn cleanup_completed(&self) -> usize {
        let mut sessions_write = self
            .sessions
            .write()
            .expect("stream manager sessions lock poisoned");

        let initial_count = sessions_write.len();
        sessions_write.retain(|_id, session| {
            // Keep active sessions, remove terminal ones
            session.lock().expect("session lock poisoned").is_active()
        });
        let removed = initial_count - sessions_write.len();

        if removed > 0 {
            tracing::info!(
                subsystem = "streaming",
                removed = removed,
                remaining = sessions_write.len(),
                "Cleaned up completed streams"
            );
        }

        removed
    }
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sessions = self
            .sessions
            .read()
            .expect("stream manager sessions lock poisoned");
        f.debug_struct("StreamManager")
            .field("total_sessions", &sessions.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::{EventBus, PipelineEvent, kinds};
    use crate::streaming::StreamState;

    fn test_bus() -> Arc<EventBus<PipelineEvent>> {
        Arc::new(EventBus::new())
    }

    fn make_manager() -> StreamManager {
        StreamManager::new(test_bus())
    }

    #[test]
    fn new_manager_has_no_sessions() {
        let mgr = make_manager();
        assert_eq!(mgr.session_count(), 0);
        assert_eq!(mgr.active_session_count(), 0);
    }

    #[test]
    fn create_stream_registers_session() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();

        assert_eq!(mgr.session_count(), 1);
        assert_eq!(mgr.active_session_count(), 1);
        assert_eq!(handle.state(), StreamState::Active);
    }

    #[test]
    fn get_stream_returns_handle() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        assert!(mgr.has_stream(&stream_id));
        let handle2 = mgr.get_stream(&stream_id);
        assert!(handle2.is_some());
        assert_eq!(handle2.unwrap().stream_id(), stream_id);
    }

    #[test]
    fn get_nonexistent_stream_returns_none() {
        let mgr = make_manager();
        let id = StreamId::new_v4();
        assert!(!mgr.has_stream(&id));
        assert!(mgr.get_stream(&id).is_none());
    }

    #[test]
    fn publish_token_via_manager_retrieved_handle() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        let handle2 = mgr.get_stream(&stream_id).unwrap();
        handle2.publish_token("hello").unwrap();

        assert_eq!(handle.token_count(), 1);
        assert_eq!(handle.partial_content(), "hello");
    }

    #[test]
    fn cancel_stream_by_id() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        mgr.cancel_stream(&stream_id, "user requested").unwrap();
        assert!(handle.is_cancelled());
        assert!(handle.is_terminal());
        assert_eq!(mgr.active_session_count(), 0);
        assert_eq!(mgr.completed_session_count(), 1);
    }

    #[test]
    fn complete_stream_by_id() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        handle.publish_token("test").unwrap();
        mgr.complete_stream(&stream_id).unwrap();

        assert_eq!(handle.state(), StreamState::Completed);
        assert_eq!(mgr.active_session_count(), 0);
        assert_eq!(mgr.completed_session_count(), 1);
    }

    #[test]
    fn fail_stream_by_id() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        mgr.fail_stream(&stream_id, "error").unwrap();
        assert_eq!(handle.state(), StreamState::Failed);
        assert_eq!(mgr.active_session_count(), 0);
        assert_eq!(mgr.completed_session_count(), 1);
    }

    #[test]
    fn cancel_nonexistent_stream_fails() {
        let mgr = make_manager();
        let id = StreamId::new_v4();
        let result = mgr.cancel_stream(&id, "test");
        assert!(matches!(result, Err(StreamManagerError::StreamNotFound(_))));
    }

    #[test]
    fn complete_nonexistent_stream_fails() {
        let mgr = make_manager();
        let id = StreamId::new_v4();
        let result = mgr.complete_stream(&id);
        assert!(matches!(result, Err(StreamManagerError::StreamNotFound(_))));
    }

    #[test]
    fn fail_nonexistent_stream_fails() {
        let mgr = make_manager();
        let id = StreamId::new_v4();
        let result = mgr.fail_stream(&id, "test");
        assert!(matches!(result, Err(StreamManagerError::StreamNotFound(_))));
    }

    #[test]
    fn remove_terminal_stream_succeeds() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        handle.publish_token("test").unwrap();
        handle.complete().unwrap();

        mgr.remove(&stream_id).unwrap();
        assert!(!mgr.has_stream(&stream_id));
        assert_eq!(mgr.session_count(), 0);
    }

    #[test]
    fn remove_active_stream_fails() {
        let mgr = make_manager();
        let handle = mgr.create_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        let result = mgr.remove(&stream_id);
        assert!(result.is_err());
    }

    #[test]
    fn cleanup_completed_removes_terminal_sessions() {
        let mgr = make_manager();

        // Create and complete stream 1
        let h1 = mgr.create_stream(None, None, None).unwrap();
        let id1 = h1.stream_id();
        h1.complete().unwrap();

        // Create and cancel stream 2
        let h2 = mgr.create_stream(None, None, None).unwrap();
        let id2 = h2.stream_id();
        h2.cancel("test").unwrap();

        // Create active stream 3
        let h3 = mgr.create_stream(None, None, None).unwrap();
        let _id3 = h3.stream_id();

        assert_eq!(mgr.session_count(), 3);
        assert_eq!(mgr.active_session_count(), 1);

        let removed = mgr.cleanup_completed();
        assert_eq!(removed, 2);
        assert_eq!(mgr.session_count(), 1);
        assert_eq!(mgr.active_session_count(), 1);
        assert!(!mgr.has_stream(&id1));
        assert!(!mgr.has_stream(&id2));
        assert!(mgr.has_stream(&_id3));
    }

    #[test]
    fn active_stream_ids_returns_active_only() {
        let mgr = make_manager();

        let h1 = mgr.create_stream(None, None, None).unwrap();
        let id1 = h1.stream_id();
        h1.complete().unwrap();

        let h2 = mgr.create_stream(None, None, None).unwrap();
        let id2 = h2.stream_id();

        let active = mgr.active_stream_ids();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&id2));
        assert!(!active.contains(&id1));
    }

    #[test]
    fn multiple_streams_are_isolated() {
        let bus = test_bus();
        let mgr = StreamManager::new(bus.clone());

        let h1 = mgr.create_stream(None, None, None).unwrap();
        let h2 = mgr.create_stream(None, None, None).unwrap();

        h1.publish_token("stream1-token").unwrap();
        h2.publish_token("stream2-token").unwrap();

        assert_ne!(h1.stream_id(), h2.stream_id());
        assert_eq!(h1.token_count(), 1);
        assert_eq!(h2.token_count(), 1);
        assert_eq!(h1.partial_content(), "stream1-token");
        assert_eq!(h2.partial_content(), "stream2-token");
    }

    #[test]
    fn token_events_preserve_ordering_per_stream() {
        let bus = test_bus();
        let token_order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = token_order.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
            if let PipelineEvent::Stream(crate::event_bus::StreamEvent::Token(e)) = ev {
                order_clone.lock().expect("lock").push(e.sequence);
            }
        });

        let mgr = StreamManager::new(bus);
        let handle = mgr.create_stream(None, None, None).unwrap();

        for i in 0..10 {
            handle.publish_token(format!("token{}", i)).unwrap();
        }

        let order = token_order.lock().expect("lock");
        assert_eq!(*order, (0..10).collect::<Vec<_>>());
    }
}
