//! Integration tests for conversation thread persistence and recovery.
//!
//! These tests verify the full persistence lifecycle:
//!
//! ```text
//! Application Starts → Load Persisted Threads → Restore In-Memory State
//!   → Save Updates → Graceful Shutdown → Flush Pending Writes
//!   → Restart → Restore Threads
//! ```
//!
//! Tests cover:
//! - Full lifecycle: initialize → start → shutdown → reinitialize
//! - Thread recovery across store restarts (new process simulation)
//! - Message and turn preservation across restarts
//! - Identifier stability (thread_id, message_id, turn_id all preserved)
//! - Metadata preservation
//! - Corrupted file handling during load
//! - Event publishing on save/update/delete

use std::sync::Arc;

use nabu_core::conversations::{ConversationStore, PersistenceError};
use nabu_core::event_bus::{ConversationEvent, EventBus, PipelineEvent};
use nabu_core::models::conversation::{Message, Participant, Role, Thread, Turn, TurnContent};
use nabu_core::registry::context::ApplicationContext;
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};
use nabu_core::registry::Application;
use tempfile::tempdir;

/// Helper: build a thread with participants, messages, and turns.
fn sample_thread_with_participants(title: &str) -> Thread {
    let alice_id = uuid::Uuid::new_v4();
    let bob_id = uuid::Uuid::new_v4();

    let mut thread = Thread::new()
        .with_title(title)
        .with_participant(
            Participant::new(alice_id, Role::User).with_name("Alice"),
        )
        .with_participant(
            Participant::new(bob_id, Role::Assistant).with_name("Bob"),
        )
        .with_metadata("model", serde_json::json!("gpt-4o"))
        .with_metadata("temperature", serde_json::json!(0.7));

    let system_msg = Message::new(uuid::Uuid::new_v4(), thread.id)
        .with_role(Role::System)
        .with_participant(alice_id)
        .with_turn(Turn::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            TurnContent::text("You are a helpful assistant."),
        ));
    thread = thread.with_message(system_msg);

    let user_msg = Message::new(uuid::Uuid::new_v4(), thread.id)
        .with_role(Role::User)
        .with_participant(alice_id)
        .with_turn(Turn::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            TurnContent::text("Hello! How are you?"),
        ))
        .with_metadata("token_count", serde_json::json!(12));
    thread = thread.with_message(user_msg);

    let assistant_msg = Message::new(uuid::Uuid::new_v4(), thread.id)
        .with_role(Role::Assistant)
        .with_participant(bob_id)
        .with_turn(Turn::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            TurnContent::text("I'm doing well, thank you!"),
        ))
        .with_turn(Turn::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            TurnContent::Unknown(serde_json::json!({
                "tool_call": {"name": "get_weather", "args": {"location": "NYC"}}
            })),
        ));
    thread = thread.with_message(assistant_msg);

    thread
}

// ---------------------------------------------------------------------------
// Full lifecycle through ApplicationContext
// ---------------------------------------------------------------------------

#[test]
fn context_manages_conversation_store_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let ctx = ApplicationContext::builder()
        .build();

    ctx.register("conversation_store", store.clone());

    // Initial state: Created
    assert_eq!(store.lifecycle_stage(), LifecycleStage::Created);
    assert!(!store.is_initialized());
    assert!(!store.is_running());

    // Initialize: loads persisted threads (none on fresh dir)
    assert!(store.initialize().is_ok());
    assert!(store.is_initialized());
    assert!(!store.is_running());

    // Start
    assert!(store.start().is_ok());
    assert!(store.is_running());

    // Shutdown: flushes manifest
    assert!(store.shutdown().is_ok());
    assert!(store.is_shutdown());

    // Operations after shutdown fail
    let thread = sample_thread_with_participants("Post-shutdown");
    assert!(store.save(&thread).is_err());
}

// ---------------------------------------------------------------------------
// Thread recovery across store restarts
// ---------------------------------------------------------------------------

#[test]
fn restart_recovery_preserves_thread_identity() {
    let dir = tempdir().unwrap();
    let thread = sample_thread_with_participants("Persistent Conversation");

    // Phase 1: Save thread, then shut down
    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.start().unwrap();
        store.save(&thread).unwrap();
        store.shutdown().unwrap();
    }

    // Phase 2: New store instance — should reconstruct from disk
    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let loaded = store.load(thread.id).unwrap();

        // Thread identifier preserved
        assert_eq!(loaded.id, thread.id);

        // Thread metadata preserved
        assert_eq!(loaded.title, thread.title);
        assert_eq!(loaded.created_at, thread.created_at);
        assert_eq!(loaded.updated_at, thread.updated_at);
        assert_eq!(loaded.participants.len(), thread.participants.len());
        assert_eq!(
            loaded.metadata.get("model"),
            Some(&serde_json::json!("gpt-4o"))
        );
        assert_eq!(
            loaded.metadata.get("temperature"),
            Some(&serde_json::json!(0.7))
        );

        store.shutdown().unwrap();
    }
}

#[test]
fn restart_recovery_preserves_message_ordering() {
    let dir = tempdir().unwrap();
    let thread = sample_thread_with_participants("Message Ordering Test");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.save(&thread).unwrap();
        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let loaded = store.load(thread.id).unwrap();
        assert_eq!(loaded.messages.len(), thread.messages.len());

        // Message ordering preserved
        for (i, (orig, restored)) in
            thread.messages.iter().zip(loaded.messages.iter()).enumerate()
        {
            assert_eq!(orig.id, restored.id, "message id mismatch at index {}", i);
            assert_eq!(orig.thread_id, restored.thread_id);
            assert_eq!(orig.role, restored.role);
            assert_eq!(orig.created_at, restored.created_at);
            assert_eq!(orig.updated_at, restored.updated_at);
            assert_eq!(orig.turns.len(), restored.turns.len());
        }

        store.shutdown().unwrap();
    }
}

#[test]
fn restart_recovery_preserves_turn_ordering() {
    let dir = tempdir().unwrap();
    let thread = sample_thread_with_participants("Turn Ordering Test");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.save(&thread).unwrap();
        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let loaded = store.load(thread.id).unwrap();

        // The 3rd message has 2 turns (text + tool call)
        let assistant_msg = &loaded.messages[2];
        assert_eq!(assistant_msg.turns.len(), 2);

        // Turn IDs and content preserved
        assert_eq!(
            assistant_msg.turns[0].content.as_text(),
            Some("I'm doing well, thank you!")
        );
        assert_eq!(
            assistant_msg.turns[0].id,
            thread.messages[2].turns[0].id
        );
        assert_eq!(
            assistant_msg.turns[1].id,
            thread.messages[2].turns[1].id
        );
        assert_eq!(
            assistant_msg.turns[1].message_id,
            assistant_msg.id
        );

        store.shutdown().unwrap();
    }
}

#[test]
fn restart_recovery_preserves_parent_child_relationships() {
    let dir = tempdir().unwrap();
    let thread = sample_thread_with_participants("Parent-Child Test");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.save(&thread).unwrap();
        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let loaded = store.load(thread.id).unwrap();

        // Verify parent-child relationships after reload
        assert_eq!(loaded.messages[0].thread_id, loaded.id);
        assert_eq!(loaded.messages[1].thread_id, loaded.id);
        assert_eq!(loaded.messages[2].thread_id, loaded.id);

        for msg in &loaded.messages {
            for turn in &msg.turns {
                assert_eq!(turn.message_id, msg.id);
            }
        }

        // Verify participants
        assert_eq!(loaded.participants.len(), 2);
        assert_eq!(loaded.participants[0].id, loaded.participants[0].id);
        assert_eq!(loaded.participants[0].role, Role::User);
        assert_eq!(loaded.participants[1].role, Role::Assistant);

        store.shutdown().unwrap();
    }
}

// ---------------------------------------------------------------------------
// Multiple threads recovery
// ---------------------------------------------------------------------------

#[test]
fn restart_recovery_loads_all_threads() {
    let dir = tempdir().unwrap();

    let thread_a = sample_thread_with_participants("Thread A");
    let thread_b = sample_thread_with_participants("Thread B");
    let thread_c = sample_thread_with_participants("Thread C");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.start().unwrap();
        store.save(&thread_a).unwrap();
        store.save(&thread_b).unwrap();
        store.save(&thread_c).unwrap();
        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let threads = store.list();
        assert_eq!(threads.len(), 3);

        let ids: std::collections::HashSet<uuid::Uuid> =
            threads.iter().map(|t| t.id).collect();
        assert!(ids.contains(&thread_a.id));
        assert!(ids.contains(&thread_b.id));
        assert!(ids.contains(&thread_c.id));

        // Verify titles preserved
        let titles: std::collections::HashSet<&str> =
            threads.iter().filter_map(|t| t.title.as_deref()).collect();
        assert!(titles.contains("Thread A"));
        assert!(titles.contains("Thread B"));
        assert!(titles.contains("Thread C"));

        store.shutdown().unwrap();
    }
}

// ---------------------------------------------------------------------------
// Update across restart
// ---------------------------------------------------------------------------

#[test]
fn restart_recovery_after_update_preserves_changes() {
    let dir = tempdir().unwrap();
    let mut thread = sample_thread_with_participants("Update Test");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.save(&thread).unwrap();
        store.shutdown().unwrap();
    }

    // Phase 2: Reload, update, shutdown
    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        // Load and add a message
        let mut loaded = store.load(thread.id).unwrap();
        let new_msg = Message::new(uuid::Uuid::new_v4(), loaded.id)
            .with_role(Role::User)
            .with_turn(Turn::new_anonymous(TurnContent::text("New message after restart")));
        loaded = loaded.with_message(new_msg);

        store.update(&mut loaded).unwrap();
        thread = loaded.clone();
        store.shutdown().unwrap();
    }

    // Phase 3: Reload and verify the update persisted
    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let recovered = store.load(thread.id).unwrap();
        assert_eq!(recovered.messages.len(), 4); // original 3 + new 1
        assert_eq!(
            recovered.messages[3].turns[0].content.as_text(),
            Some("New message after restart")
        );

        store.shutdown().unwrap();
    }
}

// ---------------------------------------------------------------------------
// Corrupted file handling
// ---------------------------------------------------------------------------

#[test]
fn restart_recovery_skips_corrupted_threads() {
    let dir = tempdir().unwrap();
    let good_thread = sample_thread_with_participants("Good Thread");

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.save(&good_thread).unwrap();

        // Corrupt one thread file
        let bad_thread = sample_thread_with_participants("Bad Thread");
        let bad_path = store.thread_file_path(bad_thread.id);
        store.save(&bad_thread).unwrap();
        std::fs::write(&bad_path, "{invalid json}").unwrap();

        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let threads = store.list();
        // Only the good thread should be loaded
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, good_thread.id);

        store.shutdown().unwrap();
    }
}

// ---------------------------------------------------------------------------
// Empty store recovery
// ---------------------------------------------------------------------------

#[test]
fn restart_recovery_on_empty_store() {
    let dir = tempdir().unwrap();

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        store.shutdown().unwrap();
    }

    {
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        assert_eq!(store.count(), 0);
        assert!(store.list().is_empty());

        store.shutdown().unwrap();
    }
}

// ---------------------------------------------------------------------------
// Manifest consistency
// ---------------------------------------------------------------------------

#[test]
fn manifest_is_consistent_after_save_and_delete() {
    let dir = tempdir().unwrap();
    let store = ConversationStore::new(dir.path());
    store.initialize().unwrap();

    let thread_a = sample_thread_with_participants("Thread A");
    let thread_b = sample_thread_with_participants("Thread B");

    store.save(&thread_a).unwrap();
    store.save(&thread_b).unwrap();

    // Manifest should exist and contain both threads
    let manifest_path = store.manifest_path().to_path_buf();
    let manifest_json = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(manifest_json.contains(&thread_a.id.to_string()));
    assert!(manifest_json.contains(&thread_b.id.to_string()));

    // Delete one thread
    store.delete(thread_a.id).unwrap();

    // Manifest should be updated
    let manifest_json = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(!manifest_json.contains(&thread_a.id.to_string()));
    assert!(manifest_json.contains(&thread_b.id.to_string()));

    store.shutdown().unwrap();
}

// ---------------------------------------------------------------------------
// Event publishing
// ---------------------------------------------------------------------------

#[test]
fn save_publishes_thread_saved_event() {
    let dir = tempdir().unwrap();
    let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
    let received: Arc<std::sync::Mutex<Vec<ConversationEvent>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let received_clone = received.clone();
    event_bus.subscribe("thread.saved", move |event: &PipelineEvent| {
        if let PipelineEvent::Conversation(conv) = event {
            received_clone.lock().unwrap().push(conv.clone());
        }
    });

    let store = Arc::new(ConversationStore::with_event_bus(
        dir.path(),
        (*event_bus).clone(),
    ));
    store.initialize().unwrap();

    let thread = sample_thread_with_participants("Event Test");
    store.save(&thread).unwrap();

    let events = received.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ConversationEvent::ThreadSaved { thread_id, .. } => {
            assert_eq!(*thread_id, thread.id);
        }
        _ => panic!("expected ThreadSaved event"),
    }
}

#[test]
fn delete_publishes_thread_deleted_event() {
    let dir = tempdir().unwrap();
    let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
    let received: Arc<std::sync::Mutex<Vec<ConversationEvent>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let received_clone = received.clone();
    event_bus.subscribe("thread.deleted", move |event: &PipelineEvent| {
        if let PipelineEvent::Conversation(conv) = event {
            received_clone.lock().unwrap().push(conv.clone());
        }
    });

    let store = Arc::new(ConversationStore::with_event_bus(
        dir.path(),
        (*event_bus).clone(),
    ));
    store.initialize().unwrap();

    let thread = sample_thread_with_participants("Delete Event Test");
    store.save(&thread).unwrap();
    store.delete(thread.id).unwrap();

    let events = received.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ConversationEvent::ThreadDeleted { thread_id, .. } => {
            assert_eq!(*thread_id, thread.id);
        }
        _ => panic!("expected ThreadDeleted event"),
    }
}

#[test]
fn update_publishes_thread_updated_event() {
    let dir = tempdir().unwrap();
    let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
    let received: Arc<std::sync::Mutex<Vec<ConversationEvent>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let received_clone = received.clone();
    event_bus.subscribe("thread.updated", move |event: &PipelineEvent| {
        if let PipelineEvent::Conversation(conv) = event {
            received_clone.lock().unwrap().push(conv.clone());
        }
    });

    let store = Arc::new(ConversationStore::with_event_bus(
        dir.path(),
        (*event_bus).clone(),
    ));
    store.initialize().unwrap();

    let thread = sample_thread_with_participants("Update Event Test");
    store.save(&thread).unwrap();

    let mut loaded = store.load(thread.id).unwrap();
    let new_msg = Message::new(uuid::Uuid::new_v4(), loaded.id)
        .with_role(Role::User)
        .with_turn(Turn::new_anonymous(TurnContent::text("Updated")));
    loaded = loaded.with_message(new_msg);
    store.update(&mut loaded).unwrap();

    let events = received.lock().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ConversationEvent::ThreadUpdated { thread_id, .. } => {
            assert_eq!(*thread_id, thread.id);
        }
        _ => panic!("expected ThreadUpdated event"),
    }
}

// ---------------------------------------------------------------------------
// No events without event bus
// ---------------------------------------------------------------------------

#[test]
fn operations_succeed_without_event_bus() {
    let dir = tempdir().unwrap();
    let store = ConversationStore::new(dir.path());
    store.initialize().unwrap();

    let thread = sample_thread_with_participants("No Event Bus");
    // Should not panic even though event_bus is None
    store.save(&thread).unwrap();
    let mut loaded = store.load(thread.id).unwrap();
    let new_msg = Message::new(uuid::Uuid::new_v4(), loaded.id)
        .with_role(Role::User)
        .with_turn(Turn::new_anonymous(TurnContent::text("Updated")));
    loaded = loaded.with_message(new_msg);
    store.update(&mut loaded).unwrap();

    store.shutdown().unwrap();
}

// ---------------------------------------------------------------------------
// ApplicationContext integration
// ---------------------------------------------------------------------------

#[test]
fn context_resolves_conversation_store() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let ctx = ApplicationContext::builder().build();
    ctx.register("conversation_store", store.clone());

    let resolved = ctx.conversation_store();
    assert!(resolved.is_some());
    assert_eq!(store.lifecycle_stage(), LifecycleStage::Created);
}

#[test]
fn context_lifecycle_manages_conversation_store() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let ctx = ApplicationContext::builder().build();
    ctx.register("conversation_store", store.clone());

    // Register required services for ApplicationContext initialization
    ctx.register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    ctx.register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    ctx.register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(dir.path())),
    );

    // Initialize
    assert!(ctx.initialize().is_ok());
    assert!(store.is_initialized());

    // Start
    assert!(ctx.start().is_ok());
    assert!(store.is_running());

    // Shutdown
    assert!(ctx.shutdown().is_ok());
    assert!(store.is_shutdown());
}

#[test]
fn context_health_check_includes_conversation_store() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let ctx = ApplicationContext::builder().build();
    ctx.register("conversation_store", store.clone());
    ctx.register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    ctx.register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    ctx.register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(dir.path())),
    );

    ctx.initialize().unwrap();
    ctx.start().unwrap();

    let health = ctx.health_check();
    let conv_entry = health.services.iter().find(|s| s.name == "conversation_store");
    assert!(conv_entry.is_some(), "conversation_store should appear in health report");
    assert!(conv_entry.unwrap().healthy);

    ctx.shutdown().unwrap();
}

#[test]
fn context_validates_conversation_store_as_optional() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let ctx = ApplicationContext::builder().build();
    ctx.register("conversation_store", store.clone());

    // Register required services so validation passes
    ctx.register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    ctx.register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    ctx.register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(dir.path())),
    );

    let report = ctx.validate_core_services();
    assert!(report.optional_services.contains(&"conversation_store"));
    assert!(report.is_valid(), "all required services present");
}

// ---------------------------------------------------------------------------
// Application lifecycle integration
// ---------------------------------------------------------------------------

#[test]
fn application_manages_conversation_store_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    let app = Application::builder()
        .with_conversation_store(store.clone())
        .build();

    // Register required services
    app.context().register(
        "capture_engine",
        Arc::new(nabu_core::capture::CaptureEngine::new()),
    );
    app.context().register(
        "pipeline",
        Arc::new(nabu_core::processing::ProcessingPipeline::new()),
    );
    app.context().register(
        "storage_manager",
        Arc::new(nabu_core::storage::StorageManager::new(dir.path())),
    );

    assert_eq!(store.lifecycle_stage(), LifecycleStage::Created);

    assert!(app.initialize().is_ok());
    assert!(store.is_initialized());

    app.start();
    assert!(store.is_running());

    assert!(app.shutdown().is_ok());
    assert!(store.is_shutdown());
}

// ---------------------------------------------------------------------------
// Concurrency / thread safety
// ---------------------------------------------------------------------------

#[test]
fn concurrent_save_and_load_is_thread_safe() {
    let dir = tempdir().unwrap();
    let store = Arc::new(ConversationStore::new(dir.path()));
    store.initialize().unwrap();
    store.start().unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let store_clone = store.clone();
            std::thread::spawn(move || {
                let thread = sample_thread_with_participants(&format!("Concurrent {}", i));
                store_clone.save(&thread).expect("save should succeed");
                store_clone.load(thread.id).expect("load should succeed")
            })
        })
        .collect();

    let results: Vec<Thread> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.len(), 10);

    // All IDs unique
    let ids: std::collections::HashSet<uuid::Uuid> = results.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), 10);

    // All persisted to store
    assert_eq!(store.count(), 10);

    store.shutdown().unwrap();
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn load_nonexistent_returns_thread_not_found() {
    let dir = tempdir().unwrap();
    let store = ConversationStore::new(dir.path());
    store.initialize().unwrap();

    let fake_id = uuid::Uuid::new_v4();
    let result = store.load(fake_id);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(PersistenceError::ThreadNotFound { thread_id })
        if thread_id == fake_id
    ));
}

#[test]
fn delete_nonexistent_returns_thread_not_found() {
    let dir = tempdir().unwrap();
    let store = ConversationStore::new(dir.path());
    store.initialize().unwrap();

    let fake_id = uuid::Uuid::new_v4();
    let result = store.delete(fake_id);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(PersistenceError::ThreadNotFound { thread_id })
        if thread_id == fake_id
    ));
}

#[test]
fn save_replaces_existing_without_duplicates() {
    let dir = tempdir().unwrap();
    let store = ConversationStore::new(dir.path());
    store.initialize().unwrap();

    let thread = sample_thread_with_participants("Replace Test");
    store.save(&thread).unwrap();
    assert_eq!(store.count(), 1);

    // Save again with an added message
    let mut updated = thread.clone();
    let new_msg = Message::new(uuid::Uuid::new_v4(), updated.id)
        .with_role(Role::User)
        .with_turn(Turn::new_anonymous(TurnContent::text("Added message")));
    updated = updated.with_message(new_msg);
    store.save(&updated).unwrap();

    assert_eq!(store.count(), 1); // No duplicate
    let loaded = store.load(thread.id).unwrap();
    assert_eq!(loaded.messages.len(), 4);
}
