//! Streaming pipeline — the core publish API for token streaming.
//!
//! The [`StreamingPipeline`] is the primary entry point for publishing streamed
//! tokens to the frontend through the EventBus. It manages the creation of
//! streaming sessions and provides a clean API for:
//!
//! - Starting streams (creating sessions)
//! - Publishing tokens
//! - Completing streams
//! - Cancelling streams
//! - Failing streams
//!
//! The pipeline delegates session storage and lifecycle management to the
//! [`StreamManager`](crate::streaming::StreamManager).

use std::sync::Arc;

use crate::event_bus::{EventBus, PipelineEvent, StreamId};
use crate::streaming::errors::StreamResult;
use crate::streaming::session::{StreamingSession, StreamSessionHandle};

/// The streaming pipeline — the core API for token streaming.
///
/// The pipeline wraps a reference to the [`EventBus`] and provides methods
/// for creating and managing streaming sessions. Each stream is a
/// [`StreamingSession`] accessible via a [`StreamSessionHandle`].
///
/// ## Usage
///
/// ```no_run
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::streaming::StreamingPipeline;
/// use std::sync::Arc;
///
/// let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
/// let pipeline = StreamingPipeline::new(bus);
///
/// // Start a new stream
/// let handle = pipeline.start_stream(None, None, None).unwrap();
///
/// // Publish tokens
/// handle.publish_token("Hello").unwrap();
/// handle.publish_token(" world").unwrap();
///
/// // Complete the stream
/// handle.complete().unwrap();
/// ```
pub struct StreamingPipeline {
    event_bus: Arc<EventBus<PipelineEvent>>,
}

impl StreamingPipeline {
    /// Create a new streaming pipeline with the given EventBus.
    ///
    /// The EventBus is the single transport for all streaming events —
    /// no bypass, no direct callbacks.
    pub fn new(event_bus: Arc<EventBus<PipelineEvent>>) -> Self {
        Self { event_bus }
    }

    /// Returns a reference to the EventBus.
    pub fn event_bus(&self) -> &Arc<EventBus<PipelineEvent>> {
        &self.event_bus
    }

    /// Start a new streaming session.
    ///
    /// Creates a new [`StreamingSession`] with the given parameters, assigns
    /// it a fresh `StreamId`, and publishes the "stream started" events
    /// through the EventBus.
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
    pub fn start_stream(
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

        Ok(handle)
    }

    /// Publish a token to an existing stream.
    ///
    /// The handle must refer to an active (non-terminal, non-cancelled) stream.
    /// After publication, the token is published through the EventBus as a
    /// `StreamEvent::Token`.
    pub fn publish_token(
        &self,
        handle: &StreamSessionHandle,
        token: impl Into<String>,
    ) -> StreamResult<()> {
        handle.publish_token(token)
    }

    /// Publish a partial content update for a stream.
    ///
    /// This is a convenience method that publishes the full accumulated
    /// content through the EventBus without adding a new token.
    pub fn publish_partial_update(&self, handle: &StreamSessionHandle) -> StreamResult<()> {
        handle.publish_partial_update()
    }

    /// Complete a stream normally.
    ///
    /// Publishes a `StreamEvent::Completed` terminal event through the EventBus.
    pub fn complete_stream(&self, handle: &StreamSessionHandle) -> StreamResult<()> {
        handle.complete()
    }

    /// Cancel a stream before completion.
    ///
    /// Publishes a `StreamEvent::Cancelled` terminal event through the EventBus.
    /// After cancellation, no further tokens can be published to the stream.
    pub fn cancel_stream(
        &self,
        handle: &StreamSessionHandle,
        reason: impl Into<String>,
    ) -> StreamResult<()> {
        handle.cancel(reason)
    }

    /// Fail a stream with an error.
    ///
    /// Publishes a `StreamEvent::Failed` terminal event through the EventBus.
    pub fn fail_stream(
        &self,
        handle: &StreamSessionHandle,
        error: impl Into<String>,
    ) -> StreamResult<()> {
        handle.fail(error)
    }

    /// Returns `true` if the given stream is still active.
    pub fn is_stream_active(&self, handle: &StreamSessionHandle) -> bool {
        handle.is_active()
    }

    /// Returns the number of tokens published to the given stream.
    pub fn stream_token_count(&self, handle: &StreamSessionHandle) -> u64 {
        handle.token_count()
    }

    /// Returns the accumulated partial content for the given stream.
    pub fn stream_partial_content(&self, handle: &StreamSessionHandle) -> String {
        handle.partial_content()
    }
}

impl std::fmt::Debug for StreamingPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingPipeline")
            .field("event_bus_subscribers", &self.event_bus.event_kind_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::{EventBus, PipelineEvent, StreamEvent, kinds};
    use crate::streaming::errors::StreamManagerError;
    use std::sync::atomic::AtomicUsize;

    fn test_bus() -> Arc<EventBus<PipelineEvent>> {
        Arc::new(EventBus::new())
    }

    #[tokio::test]
    async fn start_stream_creates_active_session() {
        let bus = test_bus();
        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();

        assert_eq!(handle.state(), crate::streaming::StreamState::Active);
        assert!(handle.is_active());
    }

    #[tokio::test]
    async fn publish_token_delivers_through_event_bus() {
        let bus = test_bus();

        let token_count = Arc::new(AtomicUsize::new(0));
        let token_count_clone = token_count.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |_ev: &PipelineEvent| {
            token_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();

        pipeline.publish_token(&handle, "Hello").unwrap();
        pipeline.publish_token(&handle, " world").unwrap();

        assert_eq!(token_count.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(handle.token_count(), 2);
        assert_eq!(handle.partial_content(), "Hello world");
    }

    #[tokio::test]
    async fn complete_stream_publishes_completed_event() {
        let bus = test_bus();

        let completed_count = Arc::new(AtomicUsize::new(0));
        let completed_count_clone = completed_count.clone();
        bus.subscribe(kinds::STREAM_COMPLETED, move |_ev: &PipelineEvent| {
            completed_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();
        pipeline.publish_token(&handle, "test").unwrap();
        pipeline.complete_stream(&handle).unwrap();

        assert_eq!(handle.state(), crate::streaming::StreamState::Completed);
        assert!(completed_count.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cancel_stream_stops_token_delivery() {
        let bus = test_bus();

        let token_count = Arc::new(AtomicUsize::new(0));
        let tc = token_count.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
            if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
                let _ = e;
                tc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();

        pipeline.publish_token(&handle, "before").unwrap();
        pipeline.cancel_stream(&handle, "user requested").unwrap();

        let result = pipeline.publish_token(&handle, "after");
        assert!(matches!(result, Err(StreamManagerError::Cancelled { .. })));

        // Only the "before" token should have been published
        assert_eq!(token_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fail_stream_transitions_to_failed() {
        let bus = test_bus();

        let failed_count = Arc::new(AtomicUsize::new(0));
        let fc = failed_count.clone();
        bus.subscribe(kinds::STREAM_FAILED, move |_ev: &PipelineEvent| {
            fc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();
        pipeline.publish_token(&handle, "partial").unwrap();
        pipeline.fail_stream(&handle, "connection lost").unwrap();

        assert_eq!(handle.state(), crate::streaming::StreamState::Failed);
        assert!(failed_count.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn stream_with_thread_and_agent_metadata() {
        let bus = test_bus();
        let pipeline = StreamingPipeline::new(bus);
        let thread_id = uuid::Uuid::new_v4();
        let handle = pipeline
            .start_stream(Some(thread_id), Some(uuid::Uuid::nil()), Some("test-agent".into()))
            .unwrap();

        assert_eq!(handle.stream_id().to_string().len(), 36); // UUID format
        assert!(handle.is_active());
    }

    #[tokio::test]
    async fn stream_events_carry_correct_stream_id() {
        let bus = test_bus();

        let received_ids = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ids_clone = received_ids.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
            if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
                ids_clone.lock().expect("lock").push(e.stream_id);
            }
        });

        let pipeline = StreamingPipeline::new(bus);
        let handle = pipeline.start_stream(None, None, None).unwrap();
        let stream_id = handle.stream_id();

        pipeline.publish_token(&handle, "a").unwrap();
        pipeline.publish_token(&handle, "b").unwrap();

        let ids = received_ids.lock().expect("lock");
        assert_eq!(*ids, vec![stream_id, stream_id]);
        assert_eq!(ids.len(), 2);
    }
}
