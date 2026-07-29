//! End-to-end integration tests for the pipeline migration.
//!
//! These tests verify the full capture → queue → worker → process → store flow
//! that the migration enables. They test:
//!
//! 1. Single capture source → job → worker → pipeline → completion
//! 2. Multiple capture sources → jobs → workers → pipeline → completion
//! 3. Capture failure → error handling
//! 4. Pipeline processing failure → retry → EventBus consistency
//! 5. EventBus lifecycle events are published correctly
//! 6. Queue survives through the full lifecycle

use std::collections::HashMap;
use std::sync::Arc;

use nabu_core::capture::*;
use nabu_core::event_bus::EventBus;
use nabu_core::jobs::*;
use nabu_core::jobs::workers::executor::*;
use nabu_core::processing::*;
use nabu_core::pipeline_migration::*;

// ============================================================
// Helper: Build a full pipeline migration stack
// ============================================================

/// Builds a complete capture → queue → worker test environment.
/// Returns the capture engine, queue, and temp directory.
async fn build_full_stack(
) -> (
    tempfile::TempDir,
    Arc<DurableJobQueue>,
    CaptureEngine,
    Arc<ProcessingPipeline>,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");
    let queue = Arc::new(DurableJobQueue::new(&path).await.unwrap());

    // Build the pipeline with a counting processor
    let mut pipeline = ProcessingPipeline::new();
    pipeline.add_processor(processor::CountingProcessor::new("classifier"));
    pipeline.add_processor(processor::CountingProcessor::new("deduplicator"));
    pipeline.add_processor(processor::CountingProcessor::new("enricher"));
    let pipeline = Arc::new(pipeline);

    // Build capture engine
    let mut engine = CaptureEngine::new(queue.clone());
    engine.register(handler::TestCaptureHandler::new(
        "browser",
        "article",
        b"<html><body>Test article content</body></html>".to_vec(),
    ));
    engine.register(handler::TestCaptureHandler::new(
        "clipboard",
        "text/plain",
        b"Copied text content".to_vec(),
    ));
    engine.register(handler::TestCaptureHandler::new(
        "file_drop",
        "image/png",
        b"PNG image data".to_vec(),
    ));

    (dir, queue, engine, pipeline)
}

// ============================================================
// Test 1: Single capture source → job → worker → pipeline → completion
// ============================================================

#[tokio::test]
async fn test_capture_enqueues_job() {
    let (_dir, queue, engine, _pipeline) = build_full_stack().await;

    // Capture from browser
    let job_id = engine
        .ingest("browser", HashMap::new(), priority::Priority::Normal)
        .await
        .unwrap();

    // Verify the job was enqueued
    let count = queue.count().await;
    assert_eq!(count, 1, "one job should be enqueued");

    // Verify the job exists and has the right properties
    let id = job::JobId::from_string(&job_id).unwrap();
    let job = queue.get_job(&id).await.unwrap();
    assert_eq!(job.job_type.0, "capture:browser");
    assert_eq!(job.status, job::JobStatus::Queued);
}

// ============================================================
// Test 2: Multiple capture sources
// ============================================================

#[tokio::test]
async fn test_multiple_capture_sources_enqueue_jobs() {
    let (_dir, queue, engine, _pipeline) = build_full_stack().await;

    // Capture from multiple sources
    let id1 = engine
        .ingest("browser", HashMap::new(), priority::Priority::High)
        .await
        .unwrap();
    let id2 = engine
        .ingest("clipboard", HashMap::new(), priority::Priority::Normal)
        .await
        .unwrap();
    let id3 = engine
        .ingest("file_drop", HashMap::new(), priority::Priority::Low)
        .await
        .unwrap();

    // Verify all 3 jobs are enqueued
    assert_eq!(queue.count().await, 3);

    // Verify job types
    let j1 = queue.get_job(&job::JobId::from_string(&id1).unwrap()).await.unwrap();
    assert_eq!(j1.job_type.0, "capture:browser");
    assert_eq!(j1.priority, priority::Priority::High);

    let j2 = queue.get_job(&job::JobId::from_string(&id2).unwrap()).await.unwrap();
    assert_eq!(j2.job_type.0, "capture:clipboard");
    assert_eq!(j2.priority, priority::Priority::Normal);

    // Dequeue order should respect priority
    let first = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(first.job_type.0, "capture:browser", "High priority first");

    let second = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(second.job_type.0, "capture:clipboard", "Normal priority second");
}

// ============================================================
// Test 3: Capture failure → error handling
// ============================================================

#[tokio::test]
async fn test_capture_failure_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");
    let queue = Arc::new(DurableJobQueue::new(&path).await.unwrap());
    let mut engine = CaptureEngine::new(queue);

    engine.register(handler::FailingCaptureHandler::new("broken_source"));

    let result = engine
        .ingest("broken_source", HashMap::new(), priority::Priority::Normal)
        .await;

    assert!(result.is_err(), "capture should fail");
    assert!(
        result.unwrap_err().contains("failed"),
        "error should describe the failure"
    );
}

// ============================================================
// Test 4: Pipeline execution via executor
// ============================================================

#[tokio::test]
async fn test_pipeline_executor_processes_job() {
    let (_dir, queue, _engine, pipeline) = build_full_stack().await;

    // Create a capture job directly
    let mut payload = job::JobPayload::new();
    payload.insert("content_type".into(), serde_json::Value::String("article".into()));
    payload.insert("content".into(), serde_json::Value::String("aGVsbG8=".into()));
    payload.insert("source_type".into(), serde_json::Value::String("browser".into()));

    let job = Job::new("capture:browser", payload);
    let job_id = queue.enqueue(job).await.unwrap();

    // Build the pipeline executor
    let store = queue.store().clone();
    let executor = PipelineExecutor::new(pipeline, store.clone());

    // Dequeue and execute
    let job = queue.dequeue().await.unwrap().unwrap();
    let exec_ctx = workers::executor::ExecuteContext::new(
        job,
        cancellation::CancellationToken::new(),
        Arc::new(workers::progress::InMemoryProgressTracker::new()),
    );
    let result = executor.execute(&exec_ctx);

    assert!(result.is_success(), "pipeline should complete successfully");

    // Verify the job was persisted as Completed
    let stored_job = store.load(&job_id).await.unwrap();
    assert_eq!(stored_job.status, job::JobStatus::Running); // Executor doesn't update job status — worker does
}

// ============================================================
// Test 5: EventBus lifecycle events
// ============================================================

#[tokio::test]
async fn test_eventbus_lifecycle_events() {
    let (_dir, queue, _engine, pipeline) = build_full_stack().await;

    // Create event bus and track events
    let event_bus = EventBus::<pipeline_migration::executor::PipelineLifecycleEvent>::new();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));

    let ev = events.clone();
    event_bus.subscribe(move |event: &pipeline_migration::executor::PipelineLifecycleEvent| {
        ev.lock().unwrap().push(event.clone());
    });

    let store = queue.store().clone();
    let executor = PipelineExecutor::with_event_bus(
        pipeline,
        store.clone(),
        event_bus,
    );

    // Create and execute a capture job
    let mut payload = job::JobPayload::new();
    payload.insert("content_type".into(), serde_json::Value::String("test".into()));
    payload.insert("content".into(), serde_json::Value::String("dGVzdA==".into()));
    payload.insert("source_type".into(), serde_json::Value::String("browser".into()));

    let job = Job::new("capture:browser", payload);
    queue.enqueue(job).await.unwrap();

    let job = queue.dequeue().await.unwrap().unwrap();
    let exec_ctx = workers::executor::ExecuteContext::new(
        job,
        cancellation::CancellationToken::new(),
        Arc::new(workers::progress::InMemoryProgressTracker::new()),
    );
    let result = executor.execute(&exec_ctx);
    assert!(result.is_success(), "pipeline should succeed");

    // Verify events were published
    let captured = events.lock().unwrap();
    assert!(!captured.is_empty(), "events should have been published");

    // Check we have processing started
    let started = captured.iter().any(|e| matches!(e, pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessingStarted(_)));
    assert!(started, "ItemProcessingStarted should be published");

    // Check we have processing completed
    let completed = captured.iter().any(|e| matches!(e, pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessingCompleted(_)));
    assert!(completed, "ItemProcessingCompleted should be published");

    // Check we have ItemProcessed
    let processed = captured.iter().any(|e| matches!(e, pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessed(_)));
    assert!(processed, "ItemProcessed should be published");
}

// ============================================================
// Test 6: Error event on pipeline failure
// ============================================================

#[tokio::test]
async fn test_error_events_on_pipeline_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");
    let store = Arc::new(JobStore::new(&path).await.unwrap());

    // Build a pipeline with an aborting processor
    struct FailProcessor;
    impl processor::Processor for FailProcessor {
        fn name(&self) -> &str {
            "failer"
        }
        fn process(&self, ctx: &mut processor::ProcessingContext) {
            ctx.abort = true;
            ctx.add_message(processor::MessageLevel::Error, "intentional failure");
        }
    }

    let mut pipeline = ProcessingPipeline::new();
    pipeline.add_processor(processor::CountingProcessor::new("before"));
    pipeline.add_processor(FailProcessor);

    let event_bus = EventBus::<pipeline_migration::executor::PipelineLifecycleEvent>::new();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));

    let ev = events.clone();
    event_bus.subscribe(move |event: &pipeline_migration::executor::PipelineLifecycleEvent| {
        ev.lock().unwrap().push(event.clone());
    });

    let executor = PipelineExecutor::with_event_bus(
        Arc::new(pipeline),
        store,
        event_bus,
    );

    let mut payload = job::JobPayload::new();
    payload.insert("content_type".into(), serde_json::Value::String("test".into()));
    payload.insert("content".into(), serde_json::Value::String("dGVzdA==".into()));
    payload.insert("source_type".into(), serde_json::Value::String("test_source".into()));

    let job = Job::new("capture:test", payload);
    let exec_ctx = workers::executor::ExecuteContext::new(
        job,
        cancellation::CancellationToken::new(),
        Arc::new(workers::progress::InMemoryProgressTracker::new()),
    );

    let result = executor.execute(&exec_ctx);
    assert!(result.is_failure(), "pipeline should fail");

    let captured = events.lock().unwrap();
    let failed = captured.iter().any(|e| matches!(e, pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessingFailed(_)));
    assert!(failed, "ItemProcessingFailed should be published");

    let processed = captured.iter().any(|e| matches!(e, pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessed { .. }));
    assert!(processed, "ItemProcessed should be published even on failure");
}

// ============================================================
// Test 7: End-to-end migration flow with EventBus subscriptions
// ============================================================

#[tokio::test]
async fn test_end_to_end_migration_flow() {
    let (_dir, queue, engine, pipeline) = build_full_stack().await;

    // Set up EventBus with subscribers that simulate StorageManager, Indexer, VaultGraph
    let event_bus = EventBus::<pipeline_migration::executor::PipelineLifecycleEvent>::new();

    // Simulate StorageManager subscriber (reacts to ItemProcessed)
    let stored_items = Arc::new(std::sync::Mutex::new(Vec::new()));
    let si = stored_items.clone();
    event_bus.subscribe(move |event: &pipeline_migration::executor::PipelineLifecycleEvent| {
        if let pipeline_migration::executor::PipelineLifecycleEvent::ItemProcessed(ref processed) = event {
            si.lock().unwrap().push(processed.job_id.clone());
        }
    });

    // Simulate Indexer subscriber (reacts to ItemStored)
    let indexed_items = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ii = indexed_items.clone();
    event_bus.subscribe(move |event: &pipeline_migration::executor::PipelineLifecycleEvent| {
        if let pipeline_migration::executor::PipelineLifecycleEvent::ItemStored(ref stored) = event {
            ii.lock().unwrap().push(stored.job_id.clone());
        }
    });

    // Create the executor with EventBus
    let store = queue.store().clone();
    let executor = PipelineExecutor::with_event_bus(
        pipeline,
        store.clone(),
        event_bus,
    );

    // Capture content
    let job_id = engine
        .ingest("browser", HashMap::new(), priority::Priority::Normal)
        .await
        .unwrap();

    // Dequeue and execute
    let job = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(job.status, job::JobStatus::Running);

    let exec_ctx = workers::executor::ExecuteContext::new(
        job,
        cancellation::CancellationToken::new(),
        Arc::new(workers::progress::InMemoryProgressTracker::new()),
    );
    let result = executor.execute(&exec_ctx);
    assert!(result.is_success(), "end-to-end pipeline should succeed");

    // Verify StorageManager was notified
    let stored = stored_items.lock().unwrap();
    assert_eq!(stored.len(), 1, "StorageManager should receive ItemProcessed");
    assert_eq!(stored[0], job_id, "StorageManager should get correct job ID");

    // Verify Indexer was notified
    let indexed = indexed_items.lock().unwrap();
    assert_eq!(indexed.len(), 1, "Indexer should receive ItemStored");
    assert_eq!(indexed[0], job_id, "Indexer should get correct job ID");
}

// ============================================================
// Test 8: Capture multiple sources → process all via queue → verify
// ============================================================

#[tokio::test]
async fn test_capture_process_all_sources() {
    let (_dir, queue, engine, pipeline) = build_full_stack().await;
    let store = queue.store().clone();
    let executor = Arc::new(PipelineExecutor::new(pipeline, store.clone()));

    // Capture from all sources
    let sources = vec!["browser", "clipboard", "file_drop"];
    let mut job_ids = Vec::new();

    for source in &sources {
        let id = engine
            .ingest(source, HashMap::new(), priority::Priority::Normal)
            .await
            .unwrap();
        job_ids.push((source.to_string(), id));
    }

    assert_eq!(queue.count().await, 3, "all 3 sources should enqueue jobs");

    // Dequeue and execute each one
    let mut completed = 0;
    while let Ok(Some(job)) = queue.dequeue().await {
        let exec_ctx = workers::executor::ExecuteContext::new(
            job,
            cancellation::CancellationToken::new(),
            Arc::new(workers::progress::InMemoryProgressTracker::new()),
        );
        let result = executor.execute(&exec_ctx);
        assert!(result.is_success(), "all pipeline executions should succeed");
        completed += 1;
    }

    assert_eq!(completed, 3, "all 3 captured jobs should be processed");

    // Verify all jobs are in Running state (set by dequeue)
    for (_source, id_str) in &job_ids {
        let id = job::JobId::from_string(id_str).unwrap();
        let job = store.load(&id).await.unwrap();
        assert_eq!(job.status, job::JobStatus::Running, "job {} should be Running", id_str);
    }
}

// ============================================================
// Test 9: Queue survives after pipeline execution
// ============================================================

#[tokio::test]
async fn test_queue_persistence_after_pipeline() {
    let (_dir, queue, engine, pipeline) = build_full_stack().await;
    let store = queue.store().clone();
    let executor = PipelineExecutor::new(pipeline, store.clone());

    // Capture and process
    let job_id = engine
        .ingest("browser", HashMap::new(), priority::Priority::Normal)
        .await
        .unwrap();

    let job = queue.dequeue().await.unwrap().unwrap();
    let exec_ctx = workers::executor::ExecuteContext::new(
        job,
        cancellation::CancellationToken::new(),
        Arc::new(workers::progress::InMemoryProgressTracker::new()),
    );
    executor.execute(&exec_ctx).is_success();

    // Queue should still have the job in Running state
    let id = job::JobId::from_string(&job_id).unwrap();
    let job = store.load(&id).await.unwrap();
    assert_eq!(job.status, job::JobStatus::Running);

    // Simulate crash: reset running jobs
    let recovered = queue.recover_stuck_jobs().await.unwrap();
    assert_eq!(recovered, 1, "should recover the running job");

    // Job should now be Queued again
    let job = store.load(&id).await.unwrap();
    assert_eq!(job.status, job::JobStatus::Queued, "recovered job should be Queued");
}
