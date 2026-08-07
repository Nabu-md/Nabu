//! # Synchronization Event Bridge Integration Tests
//!
//! Verifies the complete synchronization event pipeline:
//!
//! ```text
//! SyncStatusChanged
//!     │
//!     ▼
//! publish_sync_status_changed()
//!     │  wraps in PipelineEvent::Sync(...) + EventBus::publish by kind
//!     ▼
//! EventBus<PipelineEvent>
//!     │  subscribes "sync.status.changed" kind
//!     ▼
//! SyncSubscriber (validates + forwards to callback)
//!     │  callback = IPC bridge forwarding function
//!     ▼
//! Frontend
//! ```
//!
//! These tests exercise the pipeline as an external consumer would use it
//! (via `nabu_core::sync::...` and `nabu_core::event_bus::...`).
//!
//! They confirm:
//! - `SyncStatusChanged` publishes successfully through the EventBus.
//! - The `SyncSubscriber` receives and validates sync events.
//! - Events traverse the IPC bridge callback path (simulated forwarder).
//! - Invalid events are dropped at both publish and subscribe time.
//! - Concurrent publication from multiple threads is safe.
//! - Duplicate events (same sync_id) are distinguishable by subscribers.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use nabu_core::event_bus::kinds;
use nabu_core::event_bus::{EventBus, PipelineEvent};
use nabu_core::sync::{
    publish_sync_status_changed, SyncStatus, SyncStatusChanged, SyncSubscriber,
    SyncProgress, SyncError,
};

/// Helper: a simple forwarding callback that counts events received.
/// In production, this would be the IPC bridge's `forward_to_tauri` function.
fn counting_forwarder() -> (Arc<AtomicUsize>, Arc<dyn Fn(&SyncStatusChanged) + Send + Sync>) {
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();
    let callback: Arc<dyn Fn(&SyncStatusChanged) + Send + Sync> =
        Arc::new(move |_event: &SyncStatusChanged| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });
    (count, callback)
}

// ---------------------------------------------------------------------------
// 1. Event publication through EventBus
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_publish_delivers_through_event_bus() {
    let bus = EventBus::<PipelineEvent>::new();
    let (count, callback) = counting_forwarder();

    let subscriber = SyncSubscriber::new(callback);
    subscriber.register(&bus).unwrap();

    let event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing)
        .with_previous(SyncStatus::Idle);

    publish_sync_status_changed(&bus, &event);

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn sync_event_bridge_publish_wraps_in_pipeline_event() {
    let bus = EventBus::<PipelineEvent>::new();
    let received = Arc::new(AtomicUsize::new(0));

    let cb = received.clone();
    bus.subscribe(kinds::SYNC_STATUS_CHANGED, move |event: &PipelineEvent| {
        if matches!(event, PipelineEvent::Sync(_)) {
            cb.fetch_add(1, Ordering::SeqCst);
        }
    });

    let event = SyncStatusChanged::new("folder-1", "icloud", SyncStatus::UpToDate);
    publish_sync_status_changed(&bus, &event);

    assert_eq!(received.load(Ordering::SeqCst), 1);
}

#[test]
fn sync_event_bridge_kind_matches_subscriber_kind() {
    let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
    assert_eq!(event.kind(), kinds::SYNC_STATUS_CHANGED);
}

// ---------------------------------------------------------------------------
// 2. Subscriber receives events correctly
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_subscriber_receives_event_fields() {
    let bus = EventBus::<PipelineEvent>::new();
    let received: Arc<std::sync::Mutex<Option<SyncStatusChanged>>> =
        Arc::new(std::sync::Mutex::new(None));

    let received_clone = received.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        let mut guard = received_clone.lock().unwrap();
        *guard = Some(event.clone());
    });
    subscriber.register(&bus).unwrap();

    let original = SyncStatusChanged::new("folder-abc", "syncthing", SyncStatus::Syncing)
        .with_previous(SyncStatus::Idle)
        .with_error("network timeout");

    publish_sync_status_changed(&bus, &original);

    let captured = received.lock().unwrap().clone().unwrap();
    assert_eq!(captured.folder_id, "folder-abc");
    assert_eq!(captured.provider_id, "syncthing");
    assert_eq!(captured.current_status, SyncStatus::Syncing);
    assert_eq!(captured.previous_status, Some(SyncStatus::Idle));
    assert_eq!(captured.error.as_deref(), Some("network timeout"));
    assert_eq!(captured.sync_id, original.sync_id);
}

#[test]
fn sync_event_bridge_subscriber_receives_progress() {
    let bus = EventBus::<PipelineEvent>::new();
    let received: Arc<std::sync::Mutex<Option<SyncStatusChanged>>> =
        Arc::new(std::sync::Mutex::new(None));

    let received_clone = received.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        let mut guard = received_clone.lock().unwrap();
        *guard = Some(event.clone());
    });
    subscriber.register(&bus).unwrap();

    let progress = SyncProgress::new("uploading")
        .with_items(5, Some(10))
        .with_percentage(50.0);

    let event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing)
        .with_progress(progress);

    publish_sync_status_changed(&bus, &event);

    let captured = received.lock().unwrap().clone().unwrap();
    assert!(captured.progress.is_some());
    assert_eq!(captured.progress.as_ref().unwrap().operation, "uploading");
    assert_eq!(captured.progress.as_ref().unwrap().percentage, Some(50.0));
}

#[test]
fn sync_event_bridge_multiple_subscribers() {
    let bus = EventBus::<PipelineEvent>::new();
    let count_a = Arc::new(AtomicUsize::new(0));
    let count_b = Arc::new(AtomicUsize::new(0));

    let cb_a = count_a.clone();
    let sub_a = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        cb_a.fetch_add(1, Ordering::SeqCst);
    });
    sub_a.register(&bus).unwrap();

    let cb_b = count_b.clone();
    let sub_b = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        cb_b.fetch_add(1, Ordering::SeqCst);
    });
    sub_b.register(&bus).unwrap();

    let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
    publish_sync_status_changed(&bus, &event);

    assert_eq!(count_a.load(Ordering::SeqCst), 1);
    assert_eq!(count_b.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// 3. Validation at publish and subscribe time
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_invalid_event_dropped_at_publish() {
    let bus = EventBus::<PipelineEvent>::new();
    let (count, callback) = counting_forwarder();

    let subscriber = SyncSubscriber::new(callback);
    subscriber.register(&bus).unwrap();

    // Invalid: empty folder_id and nil sync_id.
    let bad_event = SyncStatusChanged::default();
    publish_sync_status_changed(&bus, &bad_event);

    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn sync_event_bridge_invalid_event_dropped_at_subscribe() {
    let bus = EventBus::<PipelineEvent>::new();
    let (count, callback) = counting_forwarder();

    let subscriber = SyncSubscriber::new(callback);
    subscriber.register(&bus).unwrap();

    // Bypass publish_sync_status_changed (which validates) and publish
    // directly to the EventBus — the subscriber must still validate.
    let invalid_event = SyncStatusChanged {
        sync_id: uuid::Uuid::nil(),
        folder_id: String::new(),
        provider_id: String::new(),
        current_status: SyncStatus::Syncing,
        ..Default::default()
    };
    bus.publish(
        kinds::SYNC_STATUS_CHANGED,
        &PipelineEvent::Sync(invalid_event),
    );

    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn sync_event_bridge_invalid_progress_dropped_at_publish() {
    let bus = EventBus::<PipelineEvent>::new();
    let (count, callback) = counting_forwarder();

    let subscriber = SyncSubscriber::new(callback);
    subscriber.register(&bus).unwrap();

    let bad_progress = SyncProgress::new("test").with_percentage(150.0);
    let event = SyncStatusChanged::new("f1", "s", SyncStatus::Syncing)
        .with_progress(bad_progress);

    publish_sync_status_changed(&bus, &event);

    assert_eq!(count.load(Ordering::SeqCst), 0);
}

// ---------------------------------------------------------------------------
// 4. IPC bridge forwarding (simulated)
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_serializes_for_ipc() {
    let bus = EventBus::<PipelineEvent>::new();
    let received_payload: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));

    let payload_clone = received_payload.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        // Simulate what forward_to_tauri does: serialize the event.
        let json = serde_json::to_string(event).unwrap();
        let mut guard = payload_clone.lock().unwrap();
        *guard = Some(json);
    });
    subscriber.register(&bus).unwrap();

    let event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing)
        .with_previous(SyncStatus::Idle)
        .with_error("connection lost");

    publish_sync_status_changed(&bus, &event);

    let json = received_payload.lock().unwrap().clone().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["folder_id"], "folder-1");
    assert_eq!(parsed["provider_id"], "syncthing");
    assert_eq!(parsed["current_status"], "syncing");
    assert_eq!(parsed["previous_status"], "idle");
    assert_eq!(parsed["error"], "connection lost");
}

#[test]
fn sync_event_bridge_serializes_through_pipeline_event() {
    let bus = EventBus::<PipelineEvent>::new();
    let received_payload: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));

    let payload_clone = received_payload.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        // Simulate forward_to_tauri: serialize the full PipelineEvent envelope.
        let pipeline = event.to_pipeline_event();
        let json = serde_json::to_string(&pipeline).unwrap();
        let mut guard = payload_clone.lock().unwrap();
        *guard = Some(json);
    });
    subscriber.register(&bus).unwrap();

    let event = SyncStatusChanged::new("f1", "s", SyncStatus::UpToDate);
    publish_sync_status_changed(&bus, &event);

    let json = received_payload.lock().unwrap().clone().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["Sync"]["folder_id"], "f1");
    assert_eq!(parsed["Sync"]["current_status"], "up_to_date");
}

// ---------------------------------------------------------------------------
// 5. Thread safety — concurrent publication
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_concurrent_publication() {
    let bus = EventBus::<PipelineEvent>::new();
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    let subscriber = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    subscriber.register(&bus).unwrap();

    let num_threads = 8;
    let events_per_thread = 50;
    let bus = Arc::new(bus);
    let mut handles = Vec::new();

    for _ in 0..num_threads {
        let bus = bus.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..events_per_thread {
                let event = SyncStatusChanged::new("f1", "syncthing", SyncStatus::Syncing);
                publish_sync_status_changed(&bus, &event);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(count.load(Ordering::SeqCst), num_threads * events_per_thread);
}

#[test]
fn sync_event_bridge_concurrent_multiple_folders() {
    let bus = EventBus::<PipelineEvent>::new();
    let received: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let received_clone = received.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        let mut guard = received_clone.lock().unwrap();
        guard.push(event.folder_id.clone());
    });
    subscriber.register(&bus).unwrap();

    let num_threads = 4;
    let bus = Arc::new(bus);
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let bus = bus.clone();
        let folder_id = format!("folder-{i}");
        handles.push(thread::spawn(move || {
            for j in 0..25 {
                let event = SyncStatusChanged::new(
                    folder_id.clone(),
                    "syncthing",
                    SyncStatus::Syncing,
                );
                publish_sync_status_changed(&bus, &event);
                let _ = j;
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let captured = received.lock().unwrap();
    assert_eq!(captured.len(), num_threads * 25);

    // Each folder should have received events.
    for i in 0..num_threads {
        let folder = format!("folder-{i}");
        assert!(captured.iter().any(|f| f == &folder));
    }
}

// ---------------------------------------------------------------------------
// 6. Idempotency / duplicate prevention
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_distinct_events_each_published() {
    let bus = EventBus::<PipelineEvent>::new();
    let received: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let received_clone = received.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        let mut guard = received_clone.lock().unwrap();
        guard.push(event.sync_id.to_string());
    });
    subscriber.register(&bus).unwrap();

    let event1 = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Idle);
    let event2 = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing);

    publish_sync_status_changed(&bus, &event1);
    publish_sync_status_changed(&bus, &event2);

    let captured = received.lock().unwrap();
    assert_eq!(captured.len(), 2);
    assert_ne!(captured[0], captured[1]);
}

// ---------------------------------------------------------------------------
// 7. Event lifecycle and kind dispatch
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_only_sync_kind_dispatched() {
    let bus = EventBus::<PipelineEvent>::new();
    let sync_count = Arc::new(AtomicUsize::new(0));
    let other_count = Arc::new(AtomicUsize::new(0));

    let sync_cb = sync_count.clone();
    let subscriber = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        sync_cb.fetch_add(1, Ordering::SeqCst);
    });
    subscriber.register(&bus).unwrap();

    // A non-sync event should not trigger the sync subscriber.
    let other_cb = other_count.clone();
    bus.subscribe(kinds::ITEM_STORED, move |event: &PipelineEvent| {
        if matches!(event, PipelineEvent::ItemStored(_)) {
            other_cb.fetch_add(1, Ordering::SeqCst);
        }
    });

    // Publish a sync event.
    let sync_event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
    publish_sync_status_changed(&bus, &sync_event);

    // Publish a non-sync event.
    let item_event = PipelineEvent::ItemStored(nabu_core::event_bus::ItemStoredEvent::new(
        uuid::Uuid::new_v4(),
        "/vault/note.md".to_string(),
        nabu_core::models::ObjectType::Note,
    ));
    bus.publish(kinds::ITEM_STORED, &item_event);

    assert_eq!(sync_count.load(Ordering::SeqCst), 1);
    assert_eq!(other_count.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// 8. Subscription handle
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_subscription_can_unsubscribe() {
    let bus = EventBus::<PipelineEvent>::new();
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    let subscriber = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    let subscription = subscriber.register(&bus).unwrap();

    publish_sync_status_changed(
        &bus,
        &SyncStatusChanged::new("f1", "s", SyncStatus::Idle),
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // Unsubscribe and verify no more events are received.
    subscription.unsubscribe();
    bus.publish(
        kinds::SYNC_STATUS_CHANGED,
        &PipelineEvent::Sync(SyncStatusChanged::new("f2", "s", SyncStatus::Idle)),
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // Re-subscribe and verify events flow again.
    let count2 = Arc::new(AtomicUsize::new(0));
    let count2_clone = count2.clone();
    let sub2 = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
        count2_clone.fetch_add(1, Ordering::SeqCst);
    });
    sub2.register(&bus).unwrap();

    publish_sync_status_changed(
        &bus,
        &SyncStatusChanged::new("f3", "s", SyncStatus::Idle),
    );
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// 9. Serialization round-trip through the full pipeline
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_full_pipeline_round_trip() {
    let bus = EventBus::<PipelineEvent>::new();
    let captured: Arc<std::sync::Mutex<Option<SyncStatusChanged>>> =
        Arc::new(std::sync::Mutex::new(None));

    let captured_clone = captured.clone();
    let subscriber = SyncSubscriber::from(move |event: &SyncStatusChanged| {
        let mut guard = captured_clone.lock().unwrap();
        *guard = Some(event.clone());
    });
    subscriber.register(&bus).unwrap();

    let original = SyncStatusChanged::new("folder-xyz", "syncthing", SyncStatus::Syncing)
        .with_previous(SyncStatus::Idle)
        .with_progress(
            SyncProgress::new("syncing changes")
                .with_items(50, Some(100))
                .with_percentage(50.0),
        )
        .with_error("partial failure");

    // Serialize → deserialize → publish → subscriber receives.
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: SyncStatusChanged = serde_json::from_str(&json).unwrap();

    publish_sync_status_changed(&bus, &deserialized);

    let captured_event = captured.lock().unwrap().clone().unwrap();
    assert_eq!(captured_event, deserialized);
    assert_eq!(captured_event.folder_id, "folder-xyz");
    assert_eq!(captured_event.provider_id, "syncthing");
    assert_eq!(captured_event.current_status, SyncStatus::Syncing);
    assert_eq!(captured_event.previous_status, Some(SyncStatus::Idle));
    assert!(captured_event.progress.is_some());
    assert_eq!(captured_event.error.as_deref(), Some("partial failure"));
}

// ---------------------------------------------------------------------------
// 10. Error type serialization
// ---------------------------------------------------------------------------

#[test]
fn sync_event_bridge_sync_error_serializes() {
    let err = SyncError::invalid_folder("folder-1", "test reason");
    let json = serde_json::to_string(&err).unwrap();
    let back: SyncError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, back);
    assert!(json.contains("folder-1"));
    assert!(json.contains("test reason"));
}
