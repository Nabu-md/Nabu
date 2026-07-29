//! Integration tests for the durable job queue.
//!
//! These tests verify the queue behaves correctly across full lifecycle scenarios,
//! including persistence across restarts, priority ordering, retry policies,
//! scheduling, and concurrent access.

use chrono::Duration;
use nabu_core::jobs::*;
use std::sync::Arc;

/// Helper to create a temporary queue for testing.
async fn create_temp_queue() -> (tempfile::TempDir, DurableJobQueue) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");
    let queue = DurableJobQueue::new(&path).await.unwrap();
    (dir, queue)
}

#[tokio::test]
async fn integration_full_lifecycle() {
    let (_dir, queue) = create_temp_queue().await;

    // 1. Enqueue
    let id = queue.create_job("ocr", JobPayload::new()).await.unwrap();

    // 2. Dequeue
    let mut job = queue.dequeue().await.unwrap().expect("should have a job");
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.job_type.0, "ocr");

    // 3. Complete
    job.mark_completed();
    queue.store().update(&job).await.unwrap();

    let stored = queue.get_job(&id).await.unwrap();
    assert_eq!(stored.status, JobStatus::Completed);
}

#[tokio::test]
async fn integration_failure_and_retry() {
    let (_dir, queue) = create_temp_queue().await;

    // Enqueue with retry policy
    let id = queue
        .enqueue(
            Job::new("whisper", JobPayload::new())
                .with_retries(3, RetryPolicy::exponential(
                    Duration::seconds(1),
                    2.0,
                    Duration::seconds(10),
                )),
        )
        .await
        .unwrap();

    // Dequeue and fail
    let mut job = queue.dequeue().await.unwrap().unwrap();
    let will_retry = job.mark_failed("whisper engine timeout".into());
    assert!(will_retry);
    assert_eq!(job.retry_count, 1);
    queue.store().update(&job).await.unwrap();

    // Verify it's back in the queue
    let stored = queue.get_job(&id).await.unwrap();
    assert_eq!(stored.status, JobStatus::Queued);
    assert_eq!(stored.retry_count, 1);
}

#[tokio::test]
async fn integration_priority_ordering() {
    let (_dir, queue) = create_temp_queue().await;

    // Enqueue in reverse priority order
    queue
        .enqueue(
            Job::new("background", JobPayload::new())
                .with_priority(Priority::Background),
        )
        .await
        .unwrap();

    queue
        .enqueue(
            Job::new("low", JobPayload::new())
                .with_priority(Priority::Low),
        )
        .await
        .unwrap();

    // Need some delay so Background has earlier created_at
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    queue
        .enqueue(
            Job::new("critical", JobPayload::new())
                .with_priority(Priority::Critical),
        )
        .await
        .unwrap();

    queue
        .enqueue(
            Job::new("high", JobPayload::new())
                .with_priority(Priority::High),
        )
        .await
        .unwrap();

    // Dequeue order should be: Critical, High, Background, Low
    // (priority first, then FIFO within same priority)
    let first = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(first.job_type.0, "critical", "Critical should be first");

    let second = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(second.job_type.0, "high", "High should be second");

    let third = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(third.job_type.0, "background", "Background should be third");

    let fourth = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(fourth.job_type.0, "low", "Low should be fourth");
}

#[tokio::test]
async fn integration_scheduled_jobs() {
    let (_dir, queue) = create_temp_queue().await;

    // Enqueue a job scheduled 1 hour in the future
    let future = chrono::Utc::now() + Duration::hours(1);
    let future_id = queue
        .enqueue(Job::scheduled("future_task", JobPayload::new(), future))
        .await
        .unwrap();

    // Enqueue immediate jobs
    queue.create_job("immediate_a", JobPayload::new()).await.unwrap();
    queue.create_job("immediate_b", JobPayload::new()).await.unwrap();

    // Dequeue should only return immediate jobs
    let first = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(first.job_type.0, "immediate_a");

    let second = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(second.job_type.0, "immediate_b");

    // Future job should not be returned
    assert!(queue.dequeue().await.unwrap().is_none());

    // Future job is still Scheduled
    let future_job = queue.get_job(&future_id).await.unwrap();
    assert_eq!(future_job.status, JobStatus::Scheduled);

    // Reschedule to past and verify it becomes available
    let past = chrono::Utc::now() - Duration::minutes(5);
    queue
        .reschedule(&future_id, ScheduleSpec::At(past))
        .await
        .unwrap();

    let now_ready = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(now_ready.job_type.0, "future_task");
}

#[tokio::test]
async fn integration_cancellation_flow() {
    let (_dir, queue) = create_temp_queue().await;

    // Enqueue and cancel before dequeue
    let id = queue.create_job("cancel_me", JobPayload::new()).await.unwrap();
    queue.cancel(&id).await.unwrap();

    let job = queue.get_job(&id).await.unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);

    // Should not appear in dequeue
    assert!(queue.dequeue().await.unwrap().is_none());
}

#[tokio::test]
async fn integration_persistence_across_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");

    // First session
    {
        let queue = DurableJobQueue::new(&path).await.unwrap();

        queue
            .enqueue(
                Job::new("survivor_a", JobPayload::new())
                    .with_priority(Priority::High),
            )
            .await
            .unwrap();
        queue
            .enqueue(
                Job::new("survivor_b", JobPayload::new())
                    .with_priority(Priority::Critical),
            )
            .await
            .unwrap();
        queue
            .enqueue(
                Job::scheduled("future_job", JobPayload::new(), chrono::Utc::now() + Duration::hours(2)),
            )
            .await
            .unwrap();

        assert_eq!(queue.count().await, 3);
    } // queue drops — simulating a crash or clean shutdown

    // Second session — verify all jobs survived
    {
        let queue = DurableJobQueue::new(&path).await.unwrap();
        assert_eq!(queue.count().await, 3);

        // Recover any stuck jobs (none in this case)
        let recovered = queue.recover_stuck_jobs().await.unwrap();
        assert_eq!(recovered, 0);

        // Dequeue should respect priority
        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.job_type.0, "survivor_b"); // Critical priority

        let second = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(second.job_type.0, "survivor_a"); // High priority

        // Future job should still be scheduled
        let ready = queue.dequeue().await.unwrap();
        assert!(ready.is_none());
    }
}

#[tokio::test]
async fn integration_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".nabu").join("jobs");

    // Simulate session that crashed with running jobs
    {
        let queue = DurableJobQueue::new(&path).await.unwrap();

        let id = queue.create_job("crashed_job", JobPayload::new()).await.unwrap();

        // Manually set to Running (simulating crash mid-execution)
        let mut job = queue.get_job(&id).await.unwrap();
        job.mark_running();
        queue.store().update(&job).await.unwrap();
    }

    // Recovery session
    {
        let queue = DurableJobQueue::new(&path).await.unwrap();
        let recovered = queue.recover_stuck_jobs().await.unwrap();
        assert_eq!(recovered, 1, "should recover 1 stuck job");

        // The recovered job should be dequeuable
        let job = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Running); // It was dequeued again
        assert_eq!(job.job_type.0, "crashed_job");
    }
}

#[tokio::test]
async fn integration_retry_exhaustion() {
    let (_dir, queue) = create_temp_queue().await;

    // Job with only 1 retry
    let id = queue
        .enqueue(
            Job::new("doomed", JobPayload::new())
                .with_retries(1, RetryPolicy::constant(Duration::seconds(1))),
        )
        .await
        .unwrap();

    // First attempt — fails
    let mut job = queue.dequeue().await.unwrap().unwrap();
    let will_retry = job.mark_failed("attempt 1 failed".into());
    assert!(will_retry);
    queue.store().update(&job).await.unwrap();

    // Wait for retry delay
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    // Second attempt — fails again (exhausted)
    let mut job = queue.dequeue().await.unwrap().unwrap();
    let will_retry = job.mark_failed("attempt 2 failed".into());
    assert!(!will_retry, "should exhaust retries");
    queue.store().update(&job).await.unwrap();

    let final_job = queue.get_job(&id).await.unwrap();
    assert_eq!(final_job.status, JobStatus::PermanentlyFailed);
    assert_eq!(final_job.retry_count, 2);
}

#[tokio::test]
async fn integration_multiple_jobs_with_various_statuses() {
    let (_dir, queue) = create_temp_queue().await;

    // Create jobs in various states
    let id1 = queue.create_job("active", JobPayload::new()).await.unwrap();
    let id2 = queue.create_job("active", JobPayload::new()).await.unwrap();

    // Complete one
    let mut j2 = queue.get_job(&id2).await.unwrap();
    j2.mark_completed();
    queue.store().update(&j2).await.unwrap();

    // Cancel one
    queue.cancel(&id1).await.unwrap();

    // Add new jobs
    let id3 = queue.create_job("new_active", JobPayload::new()).await.unwrap();
    let id4 = queue.create_job("new_active", JobPayload::new()).await.unwrap();

    // Counts
    let queued = queue.count_by_status(JobStatus::Queued).await.unwrap();
    let completed = queue.count_by_status(JobStatus::Completed).await.unwrap();
    let cancelled = queue.count_by_status(JobStatus::Cancelled).await.unwrap();

    assert_eq!(queued, 2, "two queued jobs (id3, id4)");
    assert_eq!(completed, 1, "one completed job (id2)");
    assert_eq!(cancelled, 1, "one cancelled job (id1)");

    // Total should be 4
    assert_eq!(queue.count().await, 4);

    // Dequeue should only return queued jobs
    let d1 = queue.dequeue().await.unwrap().unwrap();
    let d2 = queue.dequeue().await.unwrap().unwrap();
    assert!(queue.dequeue().await.unwrap().is_none());

    assert_eq!(d1.job_type.0, "new_active");
    assert_eq!(d2.job_type.0, "new_active");
}

#[tokio::test]
async fn integration_concurrent_enqueue_and_dequeue() {
    let (_dir, queue) = create_temp_queue().await;
    let queue = Arc::new(queue);

    // Spawn concurrent enqueuers
    let mut handles = Vec::new();
    for i in 0..10 {
        let q = queue.clone();
        handles.push(tokio::spawn(async move {
            q.create_job(format!("job_{}", i), JobPayload::new())
                .await
                .unwrap();
        }));
    }

    // Wait for all enqueues
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(queue.count().await, 10);

    // Dequeue all 10
    for _ in 0..10 {
        let job = queue.dequeue().await.unwrap();
        assert!(job.is_some(), "should dequeue a job");
    }

    assert!(queue.dequeue().await.unwrap().is_none(), "queue should be empty");
}

#[tokio::test]
async fn integration_reschedule_flow() {
    let (_dir, queue) = create_temp_queue().await;

    // Enqueue a job
    let id = queue.create_job("reschedulable", JobPayload::new()).await.unwrap();

    // Reschedule to 1 hour in the future
    let future = chrono::Utc::now() + Duration::hours(1);
    queue.reschedule(&id, ScheduleSpec::At(future)).await.unwrap();

    // Verify it's Scheduled
    let job = queue.get_job(&id).await.unwrap();
    assert_eq!(job.status, JobStatus::Scheduled);

    // Reschedule to immediate
    queue.reschedule(&id, ScheduleSpec::Immediate).await.unwrap();

    let job = queue.get_job(&id).await.unwrap();
    assert_eq!(job.status, JobStatus::Queued);

    // Now it should be dequeuable
    let job = queue.dequeue().await.unwrap().unwrap();
    assert_eq!(job.job_type.0, "reschedulable");
}
