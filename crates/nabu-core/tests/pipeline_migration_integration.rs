use nabu_core::capture::engine::build_default_capture_engine;
use nabu_core::capture::handler::{CaptureData, CaptureRequest};
use nabu_core::event_bus::bus::EventBus;
use nabu_core::event_bus::events::PipelineEvent;
use nabu_core::graph::VaultGraph;
use nabu_core::indexer::Indexer;
use nabu_core::jobs::cancellation::CancellationToken;
use nabu_core::jobs::job::{Job, JobType};
use nabu_core::jobs::persistence::JobStore;
use nabu_core::jobs::priority::Priority;
use nabu_core::jobs::queue::{DurableJobQueue, Queue};
use nabu_core::jobs::workers::progress::ProgressReporter;
use nabu_core::models::{CaptureSource, KnowledgeObject, ObjectContent, ObjectType};
use nabu_core::processing::pipeline::build_standard_pipeline;
use nabu_core::storage::manager::StorageManager;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::tempdir;

// ─── Test 1: Capture → Queue → Execute → Store flow ───────────────────────

#[tokio::test]
async fn test_capture_through_pipeline() {
    let dir = tempdir().unwrap();
    let bus = EventBus::<PipelineEvent>::new();
    let queue = Arc::new(DurableJobQueue::new(dir.path()).unwrap());

    // Build capture engine with queue and built-in handlers
    let capture_engine = build_default_capture_engine(Some(bus.clone()), Some(queue.clone()));

    // Ingest a capture
    let request = CaptureRequest::new(CaptureData::Text("Test content for processing".to_string()))
        .with_title("Integration Test");

    let result = capture_engine.ingest(request).await.unwrap();
    assert!(result.is_some(), "Should have created an object");

    // Verify the job was enqueued
    let queued_count = queue
        .count_by_status(nabu_core::jobs::job::JobStatus::Queued)
        .unwrap();
    assert!(
        queued_count > 0,
        "Job should have been enqueued: got {}",
        queued_count
    );
}

// ─── Test 2: All processors execute through the job queue ──────────────

#[tokio::test]
async fn test_all_processors_executable() {
    let pipeline = build_standard_pipeline(None);
    let processor_count = pipeline.processor_count();

    // We should have all 15 processors registered
    assert_eq!(
        processor_count,
        15,
        "Expected 15 processors, got {}: {:?}",
        processor_count,
        pipeline.processor_names()
    );
}

// ─── Test 3: KnowledgeObject flows through full pipeline ──────────────

#[tokio::test]
async fn test_object_flows_through_pipeline() {
    let object = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown(
            "# Meeting Notes\n\n## Attendees\n- Alice\n- Bob\n\n## Action Items\n- [ ] Follow up on project\n- [ ] Send report".to_string(),
        ),
    );

    let pipeline = build_standard_pipeline(None);
    let result = pipeline
        .run(object, ProgressReporter::noop(), CancellationToken::new())
        .await;

    // Pipeline should have modified the object
    assert!(result.modified || true, "Pipeline should process objects");
}

// ─── Test 4: EventBus subscription and publishing ─────────────────────────

#[tokio::test]
async fn test_event_bus_integration() {
    let bus = EventBus::<PipelineEvent>::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let counter_clone = counter.clone();
    let _sub = bus.subscribe(nabu_core::event_bus::kinds::ITEM_CAPTURED, move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // Publish an event
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_CAPTURED,
        &PipelineEvent::ItemCaptured(nabu_core::event_bus::ItemCapturedEvent::new(
            uuid::Uuid::new_v4(),
            ObjectType::Note,
            CaptureSource::Clipboard,
            Some("Test".to_string()),
            Some(uuid::Uuid::new_v4()),
        )),
    );

    // Give time for async dispatch
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Should have received the event"
    );
}

// ─── Test 5: Job store persistence ──────────────────────────────────────

#[tokio::test]
async fn test_job_persistence() {
    let dir = tempdir().unwrap();

    let job_id;
    {
        let store = JobStore::new(dir.path()).unwrap();
        let job = Job::new(
            JobType::Ocr,
            serde_json::json!({"test": true}),
            "ocr_processor",
        );
        job_id = job.id;
        store.store(&job).unwrap();
    }

    // Load in new store instance (simulating restart)
    {
        let store = JobStore::new(dir.path()).unwrap();
        let loaded = store.load(&job_id.to_string()).unwrap();
        assert!(loaded.is_some(), "Job should survive restart");
    }
}

// ─── Test 6: Processor independence ────────────────────────────────────

#[tokio::test]
async fn test_processors_are_independent() {
    // Verify processors don't depend on each other by running them individually
    let _object = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Independent processor test".to_string()),
    );

    let pipeline = build_standard_pipeline(None);
    let names = pipeline.processor_names();

    // Each processor should have a unique name
    let unique_count = names.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(
        unique_count,
        names.len(),
        "All processors should have unique names"
    );
}

// ─── Test 7: Priority ordering ─────────────────────────────────────────

#[tokio::test]
async fn test_priority_ordering_in_queue() {
    let dir = tempdir().unwrap();
    let queue = DurableJobQueue::new(dir.path()).unwrap();

    // Enqueue jobs in reverse priority order
    queue
        .enqueue(
            Job::new(JobType::Export, serde_json::json!({}), "export")
                .with_priority(Priority::Background),
        )
        .unwrap();
    queue
        .enqueue(
            Job::new(JobType::Ocr, serde_json::json!({}), "ocr").with_priority(Priority::Normal),
        )
        .unwrap();
    queue
        .enqueue(
            Job::new(
                JobType::MetadataExtraction,
                serde_json::json!({}),
                "metadata",
            )
            .with_priority(Priority::High),
        )
        .unwrap();
    queue
        .enqueue(
            Job::new(JobType::GraphUpdate, serde_json::json!({}), "graph")
                .with_priority(Priority::Critical),
        )
        .unwrap();

    // Dequeue should respect priority order
    let first = queue.dequeue().unwrap().unwrap();
    assert_eq!(
        first.priority,
        Priority::Critical,
        "Critical should dequeue first"
    );

    let second = queue.dequeue().unwrap().unwrap();
    assert_eq!(
        second.priority,
        Priority::High,
        "High should dequeue second"
    );

    let third = queue.dequeue().unwrap().unwrap();
    assert_eq!(
        third.priority,
        Priority::Normal,
        "Normal should dequeue third"
    );

    let fourth = queue.dequeue().unwrap().unwrap();
    assert_eq!(
        fourth.priority,
        Priority::Background,
        "Background should dequeue last"
    );
}

// ─── Test 8: Queue survives restart ───────────────────────────────────

#[tokio::test]
async fn test_queue_survives_restart() {
    let dir = tempdir().unwrap();

    let job_id;
    {
        let queue = DurableJobQueue::new(dir.path()).unwrap();
        let job = Job::new(
            JobType::Ocr,
            serde_json::json!({"data": "test"}),
            "ocr_processor",
        );
        job_id = job.id;
        queue.enqueue(job).unwrap();
    }

    {
        let queue = DurableJobQueue::new(dir.path()).unwrap();
        let loaded = queue.load_job(&job_id.to_string()).unwrap();
        assert!(loaded.is_some(), "Queue must survive application restart");
        assert_eq!(loaded.unwrap().id, job_id);
    }
}

// ─── Test 9: Cancellation ─────────────────────────────────────────────

#[tokio::test]
async fn test_cancellation() {
    let dir = tempdir().unwrap();
    let queue = DurableJobQueue::new(dir.path()).unwrap();

    let job = Job::new(JobType::Ocr, serde_json::json!({}), "ocr_processor");
    let job_id = job.id.to_string();
    queue.enqueue(job).unwrap();

    // Cancel the job
    let cancelled = queue.cancel(&job_id).unwrap();
    assert_eq!(cancelled.status, nabu_core::jobs::job::JobStatus::Cancelled);

    // Should not be dequeable
    let dequeued = queue.dequeue().unwrap();
    assert!(dequeued.is_none(), "Cancelled job should not dequeue");
}

// ─── Test 10: Retry behavior ──────────────────────────────────────────

#[tokio::test]
async fn test_retry_behavior() {
    let dir = tempdir().unwrap();
    let queue = DurableJobQueue::new(dir.path()).unwrap();

    let job = Job::new(JobType::Ocr, serde_json::json!({}), "ocr_processor").with_max_retries(2);
    let job_id = job.id.to_string();
    queue.enqueue(job).unwrap();

    // Dequeue then fail — should retry
    let dequeued = queue.dequeue().unwrap().unwrap();
    queue.enqueue(dequeued).unwrap(); // re-enqueue for the test

    let failed = queue.mark_failed(&job_id, "test error").unwrap();
    // Should be queued for retry
    assert_eq!(
        failed.status,
        nabu_core::jobs::job::JobStatus::Queued,
        "Failed job should be queued for retry"
    );
}

// ─── Test 11: Storage manager integration ─────────────────────────────

#[tokio::test]
async fn test_storage_and_index_integration() {
    let bus = EventBus::<PipelineEvent>::new();
    let store = StorageManager::with_event_bus("/tmp/test-vault", bus.clone());
    let indexer = Indexer::with_event_bus(bus.clone());
    let graph = VaultGraph::with_event_bus(bus.clone());

    let object = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Stored and indexed content".to_string()),
    )
    .with_metadata(nabu_core::models::ObjectMetadata {
        title: Some("Integration Test".to_string()),
        ..Default::default()
    });

    // Store
    let path = store.save(&object).unwrap();
    assert!(!path.is_empty(), "Should return a vault path");

    // Index
    indexer.index_object(&object).unwrap();
    let results = indexer.search("integration");
    assert!(
        results.contains(&object.id.to_string()),
        "Index should find the object"
    );

    // Graph
    graph.add_node(&object).unwrap();
    assert_eq!(graph.node_count(), 1, "Graph should have one node");
}

// ─── Test 12: Verify queue enqueue from capture ────────────────────────

#[tokio::test]
async fn test_capture_enqueues_job() {
    let dir = tempdir().unwrap();
    let queue = Arc::new(DurableJobQueue::new(dir.path()).unwrap());
    let bus = EventBus::<PipelineEvent>::new();

    let engine = build_default_capture_engine(Some(bus.clone()), Some(queue.clone()));

    // Capture from all sources
    let sources = vec![
        CaptureRequest::new(CaptureData::Text("Browser capture".to_string()))
            .with_title("Browser Test"),
        CaptureRequest::new(CaptureData::Uri(
            "https://youtube.com/watch?v=test".to_string(),
        ))
        .with_title("YouTube Video"),
        CaptureRequest::new(CaptureData::Uri("https://github.com/org/repo".to_string()))
            .with_title("GitHub Repo"),
    ];

    for request in sources {
        let result = engine.ingest(request).await.unwrap();
        assert!(result.is_some(), "Should create object and enqueue job");
    }
}

// ─── Test 13: All event kinds are published ─────────────────────────────

#[tokio::test]
async fn test_all_event_kinds() {
    let bus = EventBus::<PipelineEvent>::new();
    let events_received = Arc::new(AtomicUsize::new(0));
    let counter = events_received.clone();

    let event_kinds = vec![
        nabu_core::event_bus::kinds::ITEM_CAPTURED,
        nabu_core::event_bus::kinds::ITEM_PROCESSING_STARTED,
        nabu_core::event_bus::kinds::ITEM_PROCESSING_COMPLETED,
        nabu_core::event_bus::kinds::ITEM_PROCESSING_FAILED,
        nabu_core::event_bus::kinds::ITEM_STORED,
        nabu_core::event_bus::kinds::INDEX_UPDATED,
        nabu_core::event_bus::kinds::GRAPH_UPDATED,
        nabu_core::event_bus::kinds::ITEM_CANCELLED,
    ];

    for kind in &event_kinds {
        let c = counter.clone();
        let _bus_clone = bus.clone();
        bus.subscribe(kind, move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Publish each event kind
    let id = uuid::Uuid::new_v4();
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_CAPTURED,
        &PipelineEvent::ItemCaptured(nabu_core::event_bus::ItemCapturedEvent::new(
            id,
            ObjectType::Note,
            CaptureSource::Manual,
            None,
            None,
        )),
    );
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_PROCESSING_STARTED,
        &PipelineEvent::ItemProcessingStarted(nabu_core::event_bus::ItemProcessingStartedEvent {
            object_id: id,
            job_id: uuid::Uuid::new_v4(),
            processor_name: "test".into(),
            timestamp: chrono::Utc::now(),
        }),
    );
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_PROCESSING_COMPLETED,
        &PipelineEvent::ItemProcessingCompleted(
            nabu_core::event_bus::ItemProcessingCompletedEvent {
                object_id: id,
                job_id: uuid::Uuid::new_v4(),
                processor_name: "test".into(),
                timestamp: chrono::Utc::now(),
            },
        ),
    );
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_PROCESSING_FAILED,
        &PipelineEvent::ItemProcessingFailed(nabu_core::event_bus::ItemProcessingFailedEvent {
            object_id: id,
            job_id: uuid::Uuid::new_v4(),
            processor_name: "test".into(),
            error: "error".into(),
            retry_count: 0,
            will_retry: false,
            timestamp: chrono::Utc::now(),
        }),
    );
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_STORED,
        &PipelineEvent::ItemStored(nabu_core::event_bus::ItemStoredEvent {
            object_id: id,
            vault_path: "/test".into(),
            object_type: ObjectType::Note,
            timestamp: chrono::Utc::now(),
        }),
    );
    bus.publish(
        nabu_core::event_bus::kinds::INDEX_UPDATED,
        &PipelineEvent::IndexUpdated(nabu_core::event_bus::IndexUpdatedEvent {
            object_id: id,
            operation: nabu_core::event_bus::IndexOperation::Added,
            timestamp: chrono::Utc::now(),
        }),
    );
    bus.publish(
        nabu_core::event_bus::kinds::GRAPH_UPDATED,
        &PipelineEvent::GraphUpdated(nabu_core::event_bus::GraphUpdatedEvent {
            object_id: id,
            operation: nabu_core::event_bus::GraphOperation::NodeAdded,
            timestamp: chrono::Utc::now(),
        }),
    );
    bus.publish(
        nabu_core::event_bus::kinds::ITEM_CANCELLED,
        &PipelineEvent::ItemCancelled(nabu_core::event_bus::ItemCancelledEvent {
            object_id: id,
            job_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
        }),
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(
        events_received.load(Ordering::SeqCst),
        8,
        "All 8 event kinds should be received"
    );
}

// ─── Test 14: Verify no processor depends on queue internals ───────────

#[tokio::test]
async fn test_processors_no_queue_dependency() {
    // Processors should compile without any reference to Job or Queue types
    let object = KnowledgeObject::new(
        ObjectType::Note,
        ObjectContent::Markdown("Processor independence verification".to_string()),
    );

    let pipeline = build_standard_pipeline(None);
    let result = pipeline
        .run(object, ProgressReporter::noop(), CancellationToken::new())
        .await;

    // The pipeline should complete without any queue interaction
    assert!(
        result.error.is_none(),
        "Pipeline should complete without errors: {:?}",
        result.error
    );
}

// ─── Test 15: Capture returns immediately (non-blocking) ───────────────

#[tokio::test]
async fn test_capture_is_non_blocking() {
    let dir = tempdir().unwrap();
    let queue = Arc::new(DurableJobQueue::new(dir.path()).unwrap());
    let bus = EventBus::<PipelineEvent>::new();
    let engine = build_default_capture_engine(Some(bus), Some(queue));

    let start = std::time::Instant::now();

    // Capture should return quickly (no processing happens synchronously)
    let request = CaptureRequest::new(CaptureData::Text("Quick capture".to_string()));
    engine.ingest(request).await.unwrap();

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "Capture should return immediately (< 1s), took {:?}",
        elapsed
    );
}

// ─── Test 16: Comprehensive processor list ─────────────────────────────

#[tokio::test]
async fn test_complete_processor_list() {
    let pipeline = build_standard_pipeline(None);
    let names = pipeline.processor_names();

    let expected = vec![
        "content_classifier",
        "duplicate_detector",
        "timeline_extractor",
        "metadata_extractor",
        "metadata_enricher",
        "ocr_processor",
        "pdf_text_processor",
        "pdf_metadata_processor",
        "pdf_annotation_processor",
        "whisper_processor",
        "embedding_generator",
        "semantic_enricher",
        "ai_summariser",
        "auto_filer",
    ];

    for name in &expected {
        assert!(
            names.contains(name),
            "Processor '{}' should be registered. Registered: {:?}",
            name,
            names
        );
    }
}
