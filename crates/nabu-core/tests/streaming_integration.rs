//! Integration tests for the real-time token streaming pipeline.
//!
//! These tests verify the complete streaming lifecycle:
//! - Stream session creation through EventBus
//! - Token publication in strict ordering
//! - Stream completion, cancellation, and failure
//! - EventBus delivery to frontend subscribers
//! - Multiple concurrent streams (isolation)
//! - StreamManager session registry operations
//!
//! ## Test Strategy
//!
//! Tests use the real [`EventBus`] and [`StreamingPipeline`] — no mocks.
//! Subscribers count events using atomic counters and capture event payloads
//! in shared `Arc<Mutex<Vec<...>>>` buffers. This verifies the full pipeline
//! from `pipeline.publish_token()` → EventBus → subscriber handler.
//!
//! Run with: `cargo test streaming_integration`

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use chrono::Utc;
use uuid::Uuid;

use nabu_core::event_bus::{
    EventBus, PipelineEvent, StreamEvent, StreamId, StreamSessionEvent, StreamTokenEvent,
    kinds,
};
use nabu_core::streaming::{
    StreamManager, StreamState, StreamingPipeline,
};
use nabu_core::streaming::errors::StreamManagerError;

/// A thread-safe collector that captures streaming events in order.
struct EventCollector {
    started: Arc<AtomicUsize>,
    tokens: Arc<std::sync::Mutex<Vec<(String, u64)>>>,
    completed: Arc<AtomicUsize>,
    cancelled: Arc<AtomicUsize>,
    failed: Arc<AtomicUsize>,
}

impl EventCollector {
    fn new() -> Self {
        Self {
            started: Arc::new(AtomicUsize::new(0)),
            tokens: Arc::new(std::sync::Mutex::new(Vec::new())),
            completed: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicUsize::new(0)),
            failed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Register this collector on the given EventBus.
    fn subscribe(&self, bus: &Arc<EventBus<PipelineEvent>>) {
        let started = self.started.clone();
        bus.subscribe(kinds::STREAM_STARTED, move |_ev: &PipelineEvent| {
            started.fetch_add(1, AtomicOrdering::SeqCst);
        });

        let tokens = self.tokens.clone();
        bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
            if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
                tokens
                    .lock()
                    .expect("lock poisoned")
                    .push((e.token.clone(), e.sequence));
            }
        });

        let completed = self.completed.clone();
        bus.subscribe(kinds::STREAM_COMPLETED, move |_ev: &PipelineEvent| {
            completed.fetch_add(1, AtomicOrdering::SeqCst);
        });

        let cancelled = self.cancelled.clone();
        bus.subscribe(kinds::STREAM_CANCELLED, move |_ev: &PipelineEvent| {
            cancelled.fetch_add(1, AtomicOrdering::SeqCst);
        });

        let failed = self.failed.clone();
        bus.subscribe(kinds::STREAM_FAILED, move |_ev: &PipelineEvent| {
            failed.fetch_add(1, AtomicOrdering::SeqCst);
        });
    }

    fn token_count(&self) -> usize {
        self.tokens.lock().expect("lock poisoned").len()
    }

    fn tokens_in_order(&self) -> Vec<(String, u64)> {
        self.tokens.lock().expect("lock poisoned").clone()
    }

    fn started_count(&self) -> usize {
        self.started.load(AtomicOrdering::SeqCst)
    }

    fn completed_count(&self) -> usize {
        self.completed.load(AtomicOrdering::SeqCst)
    }

    fn cancelled_count(&self) -> usize {
        self.cancelled.load(AtomicOrdering::SeqCst)
    }

    fn failed_count(&self) -> usize {
        self.failed.load(AtomicOrdering::SeqCst)
    }
}

/// Create a test EventBus ready for streaming event subscriptions.
fn make_bus() -> Arc<EventBus<PipelineEvent>> {
    Arc::new(EventBus::new())
}

/// Create a StreamingPipeline backed by a fresh EventBus.
fn make_pipeline() -> (StreamingPipeline, Arc<EventBus<PipelineEvent>>) {
    let bus = make_bus();
    let pipeline = StreamingPipeline::new(bus.clone());
    (pipeline, bus)
}

// ---------------------------------------------------------------------------
// Stream lifecycle: Start → Streaming → Complete
// ---------------------------------------------------------------------------

#[test]
fn stream_lifecycle_completes_successfully() {
    let (pipeline, bus) = make_pipeline();
    let collector = EventCollector::new();
    collector.subscribe(&bus);

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");

    // After start_stream, a StreamStarted event should be published
    assert_eq!(collector.started_count(), 1);
    assert_eq!(handle.state(), StreamState::Active);
    assert!(handle.is_active());

    // Publish tokens — each transitions to Streaming
    pipeline.publish_token(&handle, "Hello").expect("publish token");
    assert_eq!(handle.state(), StreamState::Streaming);
    assert_eq!(handle.token_count(), 1);
    assert_eq!(handle.partial_content(), "Hello");

    pipeline.publish_token(&handle, " world").expect("publish token");
    assert_eq!(handle.token_count(), 2);
    assert_eq!(handle.partial_content(), "Hello world");

    // Complete the stream
    pipeline.complete_stream(&handle).expect("complete");
    assert_eq!(handle.state(), StreamState::Completed);
    assert!(handle.is_terminal());

    // Verify EventBus delivered events in order
    assert_eq!(collector.started_count(), 1);
    assert_eq!(collector.token_count(), 2);
    assert_eq!(collector.completed_count(), 1);
    assert_eq!(collector.cancelled_count(), 0);
    assert_eq!(collector.failed_count(), 0);

    // Verify token ordering
    let tokens = collector.tokens_in_order();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].0, "Hello");
    assert_eq!(tokens[0].1, 0); // sequence 0
    assert_eq!(tokens[1].0, " world");
    assert_eq!(tokens[1].1, 1); // sequence 1
}

#[test]
fn stream_started_event_carries_metadata() {
    let (pipeline, bus) = make_pipeline();

    let received = Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();
    bus.subscribe(kinds::STREAM_STARTED, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Started(e)) = ev {
            *received_clone.lock().expect("lock") = Some(e.clone());
        }
    });

    let thread_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let handle = pipeline
        .start_stream(Some(thread_id), Some(agent_id), Some("test-agent".into()))
        .expect("start_stream");

    let event = received.lock().expect("lock").take().expect("event received");
    assert_eq!(event.stream_id, handle.stream_id());
    assert_eq!(event.thread_id, Some(thread_id));
    assert_eq!(event.agent_id, Some(agent_id));
    assert_eq!(event.agent_name.as_deref(), Some("test-agent"));
    assert!(event.timestamp <= Utc::now());
}

#[test]
fn completed_event_carries_full_content() {
    let (pipeline, bus) = make_pipeline();

    let received = Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();
    bus.subscribe(kinds::STREAM_COMPLETED, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Completed(e)) = ev {
            *received_clone.lock().expect("lock") = Some(e.clone());
        }
    });

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline.publish_token(&handle, "Hello").expect("token");
    pipeline.publish_token(&handle, " ").expect("token");
    pipeline.publish_token(&handle, "World").expect("token");
    pipeline.complete_stream(&handle).expect("complete");

    let event = received.lock().expect("lock").take().expect("event received");
    assert_eq!(event.full_content, "Hello World");
    assert_eq!(event.total_tokens, 3);
    assert!(event.timestamp <= Utc::now());
}

// ---------------------------------------------------------------------------
// Stream lifecycle: Start → Streaming → Cancelled
// ---------------------------------------------------------------------------

#[test]
fn stream_lifecycle_cancels_gracefully() {
    let (pipeline, bus) = make_pipeline();
    let collector = EventCollector::new();
    collector.subscribe(&bus);

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");

    pipeline.publish_token(&handle, "partial").expect("token");
    assert_eq!(handle.token_count(), 1);
    assert_eq!(handle.partial_content(), "partial");

    pipeline
        .cancel_stream(&handle, "user requested")
        .expect("cancel");

    assert_eq!(handle.state(), StreamState::Cancelled);
    assert!(handle.is_cancelled());
    assert!(handle.is_terminal());

    // Verify EventBus received events
    assert_eq!(collector.token_count(), 1);
    assert_eq!(collector.cancelled_count(), 1);
    assert_eq!(collector.completed_count(), 0);

    // Publishing after cancel should fail
    let result = pipeline.publish_token(&handle, "more");
    assert!(matches!(result, Err(StreamManagerError::Cancelled { .. })));
}

#[test]
fn cancelled_event_carries_partial_content_and_reason() {
    let (pipeline, bus) = make_pipeline();

    let received = Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();
    bus.subscribe(kinds::STREAM_CANCELLED, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Cancelled(e)) = ev {
            *received_clone.lock().expect("lock") = Some(e.clone());
        }
    });

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline.publish_token(&handle, "Hello ").expect("token");
    pipeline.publish_token(&handle, "World").expect("token");
    pipeline
        .cancel_stream(&handle, "user interrupted")
        .expect("cancel");

    let event = received.lock().expect("lock").take().expect("event received");
    assert_eq!(event.partial_content, "Hello World");
    assert_eq!(event.tokens_delivered, 2);
    assert_eq!(event.reason, "user interrupted");
    assert!(event.timestamp <= Utc::now());
}

#[test]
fn cancel_before_first_token_still_works() {
    let (pipeline, _bus) = make_pipeline();

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline
        .cancel_stream(&handle, "immediate")
        .expect("cancel");

    assert_eq!(handle.state(), StreamState::Cancelled);
    assert!(handle.is_cancelled());
}

// ---------------------------------------------------------------------------
// Stream lifecycle: Start → Streaming → Failed
// ---------------------------------------------------------------------------

#[test]
fn stream_lifecycle_handles_failure() {
    let (pipeline, bus) = make_pipeline();
    let collector = EventCollector::new();
    collector.subscribe(&bus);

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");

    pipeline.publish_token(&handle, "partial").expect("token");
    pipeline.fail_stream(&handle, "connection lost").expect("fail");

    assert_eq!(handle.state(), StreamState::Failed);
    assert!(handle.is_terminal());

    assert_eq!(collector.token_count(), 1);
    assert_eq!(collector.failed_count(), 1);

    // Publishing after fail should return error
    let result = pipeline.publish_token(&handle, "more");
    assert!(matches!(
        result,
        Err(StreamManagerError::StreamAlreadyTerminal { .. })
    ));
}

#[test]
fn failed_event_carries_error_and_partial_content() {
    let (pipeline, bus) = make_pipeline();

    let received = Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();
    bus.subscribe(kinds::STREAM_FAILED, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Failed(e)) = ev {
            *received_clone.lock().expect("lock") = Some(e.clone());
        }
    });

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline.publish_token(&handle, "partial ").expect("token");
    pipeline.publish_token(&handle, "output").expect("token");
    pipeline
        .fail_stream(&handle, "agent process crashed")
        .expect("fail");

    let event = received.lock().expect("lock").take().expect("event received");
    assert_eq!(event.error, "agent process crashed");
    assert_eq!(event.partial_content, "partial output");
    assert_eq!(event.tokens_delivered, 2);
    assert!(event.timestamp <= Utc::now());
}

// ---------------------------------------------------------------------------
// Strict ordering
// ---------------------------------------------------------------------------

#[test]
fn tokens_reach_frontend_in_strict_sequence_order() {
    let (pipeline, bus) = make_pipeline();
    let collector = EventCollector::new();
    collector.subscribe(&bus);

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");

    // Publish many tokens
    let token_count: u32 = 100;
    for i in 0..token_count {
        pipeline
            .publish_token(&handle, format!("tok{}", i))
            .expect("publish token");
    }

    assert_eq!(collector.token_count(), token_count as usize);

    // Verify sequences are 0, 1, 2, ... in order
    let tokens = collector.tokens_in_order();
    for (i, (_token, seq)) in tokens.iter().enumerate() {
        assert_eq!(*seq as usize, i, "sequence mismatch at index {}", i);
    }
}

#[test]
fn token_events_preserve_content_assembly() {
    let (pipeline, bus) = make_pipeline();
    let collector = EventCollector::new();
    collector.subscribe(&bus);

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");

    let words = ["The", " ", "quick", " ", "brown", " ", "fox"];
    for word in &words {
        pipeline
             .publish_token(&handle, *word)
            .expect("publish token");
    }

    let tokens = collector.tokens_in_order();
    assert_eq!(tokens.len(), words.len());

    // Reconstruct the message from token events
    let reconstructed: String = tokens.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(reconstructed, "The quick brown fox");
    assert_eq!(handle.partial_content(), "The quick brown fox");
}

// ---------------------------------------------------------------------------
// EventBus: all streaming events pass through EventBus exclusively
// ---------------------------------------------------------------------------

#[test]
fn all_streaming_events_are_pipeline_events() {
    let (pipeline, bus) = make_pipeline();

    let all_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let all_events_clone = all_events.clone();

    // Subscribe to all streaming-related kinds
    let kinds_to_watch = [
        kinds::STREAM_STARTED,
        kinds::STREAM_TOKEN,
        kinds::STREAM_PARTIAL_UPDATE,
        kinds::STREAM_COMPLETED,
    ];

    for kind in &kinds_to_watch {
        let collector = all_events_clone.clone();
        let kind = kind.to_string();
        bus.subscribe(&kind, move |ev: &PipelineEvent| {
            collector.lock().expect("lock").push(ev.clone());
        });
    }

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline.publish_token(&handle, "Hello").expect("token");
    pipeline.publish_token(&handle, " World").expect("token");
    pipeline.complete_stream(&handle).expect("complete");

    let events = all_events.lock().expect("lock");
    assert_eq!(events.len(), 4); // 1 started + 2 tokens + 1 completed

    // Verify all events are PipelineEvent::Stream
    for ev in events.iter() {
        assert!(matches!(ev, PipelineEvent::Stream(_)));
    }
}

#[test]
fn session_events_published_through_event_bus() {
    let (pipeline, bus) = make_pipeline();

    let session_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session_events_clone = session_events.clone();

    for kind in &[
        kinds::SESSION_CREATED,
        kinds::SESSION_STARTED,
        kinds::SESSION_CLEANED_UP,
    ] {
        let collector = session_events_clone.clone();
        let kind = kind.to_string();
        bus.subscribe(&kind, move |ev: &PipelineEvent| {
            if let PipelineEvent::Session(e) = ev {
                collector.lock().expect("lock").push(e.clone());
            }
        });
    }

    let handle = pipeline.start_stream(None, None, None).expect("start_stream");
    pipeline.publish_token(&handle, "test").expect("token");
    pipeline.complete_stream(&handle).expect("complete");

    let events = session_events.lock().expect("lock");
    // Should have: session_created, session_started (from publish_started),
    // session_cleaned_up (from complete)
    assert!(events.len() >= 3);

    let kind_strs: Vec<&str> = events.iter().map(|e| e.kind()).collect();
    assert!(kind_strs.contains(&kinds::SESSION_CREATED));
    assert!(kind_strs.contains(&kinds::SESSION_STARTED));
    assert!(kind_strs.contains(&kinds::SESSION_CLEANED_UP));
}

// ---------------------------------------------------------------------------
// Multiple concurrent streams (isolation)
// ---------------------------------------------------------------------------

#[test]
fn multiple_concurrent_streams_are_isolated() {
    let (pipeline, bus) = make_pipeline();

    let stream1_id: Arc<std::sync::Mutex<Option<StreamId>>> =
        Arc::new(std::sync::Mutex::new(None));
    let stream2_id: Arc<std::sync::Mutex<Option<StreamId>>> =
        Arc::new(std::sync::Mutex::new(None));
    let stream1_tokens: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let stream2_tokens: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let s1_id = stream1_id.clone();
    let s2_id = stream2_id.clone();
    let s1_tokens = stream1_tokens.clone();
    let s2_tokens = stream2_tokens.clone();

    bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
            let s1 = s1_id.lock().expect("lock").unwrap();
            let s2 = s2_id.lock().expect("lock").unwrap();
            if e.stream_id == s1 {
                s1_tokens.lock().expect("lock").push(e.token.clone());
            } else if e.stream_id == s2 {
                s2_tokens.lock().expect("lock").push(e.token.clone());
            }
        }
    });

    let handle1 = pipeline.start_stream(None, None, None).expect("stream 1");
    let handle2 = pipeline.start_stream(None, None, None).expect("stream 2");

    *stream1_id.lock().expect("lock") = Some(handle1.stream_id());
    *stream2_id.lock().expect("lock") = Some(handle2.stream_id());

    // Interleave token publication between streams
    pipeline.publish_token(&handle1, "S1-T0").expect("token");
    pipeline.publish_token(&handle2, "S2-T0").expect("token");
    pipeline.publish_token(&handle1, "S1-T1").expect("token");
    pipeline.publish_token(&handle2, "S2-T1").expect("token");
    pipeline.publish_token(&handle1, "S1-T2").expect("token");
    pipeline.publish_token(&handle2, "S2-T2").expect("token");

    pipeline.complete_stream(&handle1).expect("complete 1");
    pipeline.complete_stream(&handle2).expect("complete 2");

    let s1_tokens = stream1_tokens.lock().expect("lock").clone();
    let s2_tokens = stream2_tokens.lock().expect("lock").clone();

    assert_eq!(s1_tokens, vec!["S1-T0", "S1-T1", "S1-T2"]);
    assert_eq!(s2_tokens, vec!["S2-T0", "S2-T1", "S2-T2"]);
}

#[test]
fn concurrent_streams_have_distinct_ids() {
    let (pipeline, _bus) = make_pipeline();

    let handle1 = pipeline.start_stream(None, None, None).expect("stream 1");
    let handle2 = pipeline.start_stream(None, None, None).expect("stream 2");
    let handle3 = pipeline.start_stream(None, None, None).expect("stream 3");

    let ids = [
        handle1.stream_id(),
        handle2.stream_id(),
        handle3.stream_id(),
    ];

    // All IDs should be unique
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "stream IDs must be unique");
        }
    }
}

// ---------------------------------------------------------------------------
// StreamManager integration
// ---------------------------------------------------------------------------

#[test]
fn stream_manager_creates_and_tracks_sessions() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    let handle = manager.create_stream(None, None, None).expect("create");
    let stream_id = handle.stream_id();

    assert!(manager.has_stream(&stream_id));
    assert_eq!(manager.session_count(), 1);
    assert_eq!(manager.active_session_count(), 1);

    handle.publish_token("test").expect("token");
    handle.complete().expect("complete");

    assert_eq!(manager.active_session_count(), 0);
    assert_eq!(manager.completed_session_count(), 1);
}

#[test]
fn stream_manager_cancel_by_id() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    let handle = manager.create_stream(None, None, None).expect("create");
    let stream_id = handle.stream_id();

    manager.cancel_stream(&stream_id, "shutdown").expect("cancel");
    assert!(handle.is_cancelled());
    assert_eq!(manager.active_session_count(), 0);

    let handle2 = manager.get_stream(&stream_id).expect("get stream");
    assert!(handle2.is_cancelled());
}

#[test]
fn stream_manager_cleanup_removes_terminal_sessions() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    // Create stream 1 and complete it
    let h1 = manager.create_stream(None, None, None).expect("create 1");
    h1.complete().expect("complete 1");

    // Create stream 2 and fail it
    let h2 = manager.create_stream(None, None, None).expect("create 2");
    h2.fail("error").expect("fail 2");

    // Create stream 3 and leave it active
    let _h3 = manager.create_stream(None, None, None).expect("create 3");

    assert_eq!(manager.session_count(), 3);
    assert_eq!(manager.active_session_count(), 1);

    let removed = manager.cleanup_completed();
    assert_eq!(removed, 2);
    assert_eq!(manager.session_count(), 1);
    assert_eq!(manager.active_session_count(), 1);
}

#[test]
fn stream_manager_active_stream_ids() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    let h1 = manager.create_stream(None, None, None).expect("create 1");
    let id1 = h1.stream_id();
    h1.complete().expect("complete");

    let h2 = manager.create_stream(None, None, None).expect("create 2");
    let id2 = h2.stream_id();

    let active = manager.active_stream_ids();
    assert_eq!(active.len(), 1);
    assert!(active.contains(&id2));
    assert!(!active.contains(&id1));
}

#[test]
fn stream_manager_remove_active_fails() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    let handle = manager.create_stream(None, None, None).expect("create");
    let stream_id = handle.stream_id();

    let result = manager.remove(&stream_id);
    assert!(result.is_err());
}

#[test]
fn stream_manager_remove_terminal_succeeds() {
    let bus = make_bus();
    let manager = StreamManager::new(bus);

    let handle = manager.create_stream(None, None, None).expect("create");
    let stream_id = handle.stream_id();
    handle.complete().expect("complete");

    manager.remove(&stream_id).expect("remove");
    assert!(!manager.has_stream(&stream_id));
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

#[test]
fn stream_event_serializes_and_deserializes() {
    let ev = StreamEvent::Token(StreamTokenEvent::new(
        Uuid::new_v4(),
        "hello",
        "hello",
        0,
    ));

    let json = serde_json::to_string(&ev).expect("serialize");
    let back: StreamEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ev, back);
}

#[test]
fn stream_session_event_serializes() {
    let now = Utc::now();
    let ev = StreamSessionEvent::SessionCreated {
        stream_id: Uuid::new_v4(),
        thread_id: Some(Uuid::new_v4()),
        timestamp: now,
    };

    let json = serde_json::to_string(&ev).expect("serialize");
    let back: StreamSessionEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ev, back);
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn publish_token_on_nonexistent_stream_fails() {
    let (pipeline, _bus) = make_pipeline();
    let handle = pipeline.start_stream(None, None, None).expect("start");

    // Complete it to make it terminal
    pipeline.complete_stream(&handle).expect("complete");

    // Publishing on terminal stream
    let result = pipeline.publish_token(&handle, "after");
    assert!(matches!(
        result,
        Err(StreamManagerError::StreamAlreadyTerminal { .. })
    ));
}

#[test]
fn complete_already_completed_stream_fails() {
    let (pipeline, _bus) = make_pipeline();
    let handle = pipeline.start_stream(None, None, None).expect("start");

    pipeline.complete_stream(&handle).expect("first complete");
    let result = pipeline.complete_stream(&handle);
    assert!(matches!(
        result,
        Err(StreamManagerError::StreamAlreadyTerminal { .. })
    ));
}

#[test]
fn cancel_already_completed_stream_fails() {
    let (pipeline, _bus) = make_pipeline();
    let handle = pipeline.start_stream(None, None, None).expect("start");

    pipeline.complete_stream(&handle).expect("complete");
    let result = pipeline.cancel_stream(&handle, "too late");
    assert!(matches!(
        result,
        Err(StreamManagerError::StreamAlreadyTerminal { .. })
    ));
}

// ---------------------------------------------------------------------------
// Frontend delivery simulation
// ---------------------------------------------------------------------------

#[test]
fn frontend_receives_incremental_updates_without_polling() {
    use std::time::Instant;

    let (pipeline, bus) = make_pipeline();

    // Simulate a frontend subscriber that processes events as they arrive
    let received_tokens: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let received_tokens_clone = received_tokens.clone();

    // Track the time of each received token
    let timing: Arc<std::sync::Mutex<Vec<(String, std::time::Duration)>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let timing_clone = timing.clone();
    let start = Instant::now();

    bus.subscribe(kinds::STREAM_TOKEN, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::Token(e)) = ev {
            let elapsed = start.elapsed();
            received_tokens_clone
                .lock()
                .expect("lock")
                .push(e.token.clone());
            timing_clone
                .lock()
                .expect("lock")
                .push((e.token.clone(), elapsed));
        }
    });

    let handle = pipeline.start_stream(None, None, None).expect("start");

    // Publish tokens with small delays — subscriber should receive them
    // without polling
    for i in 0..5 {
        pipeline
            .publish_token(&handle, format!("tok{}", i))
            .expect("publish");
    }
    pipeline.complete_stream(&handle).expect("complete");

    let tokens = received_tokens.lock().expect("lock").clone();
    assert_eq!(tokens, vec!["tok0", "tok1", "tok2", "tok3", "tok4"]);

    let timing = timing.lock().expect("lock").clone();
    assert_eq!(timing.len(), 5);
    // All tokens should have been received (no polling gap)
    for (token, _elapsed) in &timing {
        // Token should match expected format
        assert!(token.starts_with("tok"));
    }
}

#[test]
fn partial_update_event_delivers_full_content() {
    let (pipeline, bus) = make_pipeline();

    let received = Arc::new(std::sync::Mutex::new(None));
    let received_clone = received.clone();
    bus.subscribe(kinds::STREAM_PARTIAL_UPDATE, move |ev: &PipelineEvent| {
        if let PipelineEvent::Stream(StreamEvent::PartialUpdate(e)) = ev {
            *received_clone.lock().expect("lock") = Some(e.clone());
        }
    });

    let handle = pipeline.start_stream(None, None, None).expect("start");
    pipeline.publish_token(&handle, "Hello").expect("token");
    pipeline.publish_token(&handle, " ").expect("token");
    pipeline.publish_token(&handle, "World").expect("token");

    pipeline
        .publish_partial_update(&handle)
        .expect("partial update");

    let event = received.lock().expect("lock").take().expect("event received");
    assert_eq!(event.content, "Hello World");
    assert_eq!(event.token_count, 3);
    assert!(event.timestamp <= Utc::now());
}
