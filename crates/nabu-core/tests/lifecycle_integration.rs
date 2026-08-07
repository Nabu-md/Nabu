//! Lifecycle integration tests for `WorkerPool`.
//!
//! Verifies that `WorkerPool` correctly implements the shared `Lifecycle`
//! trait and can be managed through the standard lifecycle interface:
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! A multi-threaded tokio runtime is used so that worker tasks (which are
//! async) continue running on runtime worker threads while the synchronous
//! `shutdown()` drain loop blocks the calling thread.

use std::sync::Arc;
use tempfile::tempdir;
use tokio::runtime::Runtime;

use nabu_core::jobs::{DurableJobQueue, ExecutorRegistry, WorkerPool};
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};

/// Helper: create a fresh WorkerPool backed by a temporary queue.
fn make_pool(worker_count: usize) -> (Runtime, WorkerPool, tempfile::TempDir) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = tempdir().unwrap();
    let queue = Arc::new(DurableJobQueue::new(dir.path()).unwrap());
    let executors = Arc::new(ExecutorRegistry::new());
    let pool = WorkerPool::new(worker_count, queue, executors);
    (runtime, pool, dir)
}

// ---------------------------------------------------------------------------
// Full lifecycle flow
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_integration() {
    let (runtime, pool, _dir) = make_pool(2);

    // --- Initial state: Created ---
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Created);
    assert!(!pool.is_initialized());
    assert!(!pool.is_running());
    assert!(!pool.is_shutdown());
    assert!(!pool.is_shutting_down());

    // --- Initialize: Created → Initialized ---
    assert!(pool.initialize().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Initialized);
    assert!(pool.is_initialized());
    assert!(!pool.is_running());

    // --- Start: Initialized → Running ---
    // start() requires a tokio runtime context for worker spawning.
    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Running);
    assert!(pool.is_running());
    assert!(!pool.is_shutdown());
    assert!(!pool.is_shutting_down());

    // Workers should have been spawned
    let health = pool.health();
    assert_eq!(health.worker_count, 2);

    // Give workers a moment to register themselves with the shutdown
    // coordinator (they call register() at the start of run()).
    std::thread::sleep(std::time::Duration::from_millis(100));

    // --- Shutdown: Running → Shutdown ---
    assert!(pool.shutdown().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Shutdown);
    assert!(pool.is_shutdown());
    assert!(pool.is_shutting_down());

    // After shutdown, no workers should remain active
    let health = pool.health();
    assert_eq!(health.active_workers, 0);
}

// ---------------------------------------------------------------------------
// Backward-transition rejection
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_backward_transition_rejected() {
    let (runtime, pool, _dir) = make_pool(1);

    // Created → Initialized → Running → Shutdown
    runtime.block_on(async {
        assert!(pool.initialize().is_ok());
        assert!(pool.start().is_ok());
    });
    assert!(pool.shutdown().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Shutdown);

    // Cannot go backward: Shutdown → Initialized
    assert!(pool.initialize().is_err());

    // Cannot restart: Shutdown → Running
    assert!(pool.start().is_err());
}

// ---------------------------------------------------------------------------
// Start without explicit initialize (auto-advance)
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_start_without_initialize() {
    let (runtime, pool, _dir) = make_pool(1);

    // start() auto-advances Created → Initialized → Running
    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Running);
    assert!(pool.is_running());

    // Cleanup
    assert!(pool.shutdown().is_ok());
}

// ---------------------------------------------------------------------------
// Double start is a no-op (no duplicate workers)
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_double_start_no_duplicates() {
    let (runtime, pool, _dir) = make_pool(2);

    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });

    // Give workers time to register
    std::thread::sleep(std::time::Duration::from_millis(100));
    let health_after_first_start = pool.health();
    assert_eq!(health_after_first_start.active_workers, 2);

    // Second start should be a safe no-op — no duplicate workers spawned
    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });

    std::thread::sleep(std::time::Duration::from_millis(100));
    let health_after_second_start = pool.health();
    assert_eq!(health_after_second_start.active_workers, 2);
    assert_eq!(health_after_second_start.worker_count, 2);

    // Cleanup
    assert!(pool.shutdown().is_ok());
}

// ---------------------------------------------------------------------------
// Double shutdown is a no-op
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_double_shutdown() {
    let (runtime, pool, _dir) = make_pool(2);

    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });

    // First shutdown
    assert!(pool.shutdown().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Shutdown);
    assert!(pool.is_shutdown());

    // Second shutdown — should succeed without error
    assert!(pool.shutdown().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Shutdown);

    // active_workers should be 0
    assert_eq!(pool.health().active_workers, 0);
}

// ---------------------------------------------------------------------------
// Graceful shutdown drains workers
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_shutdown_drains_workers() {
    let (runtime, pool, _dir) = make_pool(2);

    runtime.block_on(async {
        assert!(pool.initialize().is_ok());
        assert!(pool.start().is_ok());
    });

    // Give workers time to register
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Workers should be registered
    assert_eq!(pool.health().active_workers, 2);

    // Shutdown should drain and stop all workers
    assert!(pool.shutdown().is_ok());

    // After shutdown, no workers should remain active
    assert_eq!(pool.health().active_workers, 0);
    assert!(pool.is_shutdown());
}

// ---------------------------------------------------------------------------
// start() fails gracefully without a runtime context
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_start_without_runtime_returns_error() {
    let (_runtime, pool, _dir) = make_pool(1);

    // Calling start() outside a tokio runtime context should return an error,
    // not panic.
    let result = pool.start();
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("tokio runtime") || msg.contains("runtime"),
        "Error should mention runtime: {}",
        msg
    );

    // Pool should still be in Created stage (start failed before spawning)
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Created);
}

// ---------------------------------------------------------------------------
// PoolHealth includes lifecycle stage
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_health_includes_stage() {
    let (runtime, pool, _dir) = make_pool(1);

    // Before start
    assert_eq!(pool.health().lifecycle_stage, LifecycleStage::Created);

    // After initialize
    assert!(pool.initialize().is_ok());
    assert_eq!(pool.health().lifecycle_stage, LifecycleStage::Initialized);

    // After start
    runtime.block_on(async {
        assert!(pool.start().is_ok());
    });
    assert_eq!(pool.health().lifecycle_stage, LifecycleStage::Running);

    // After shutdown
    assert!(pool.shutdown().is_ok());
    assert_eq!(pool.health().lifecycle_stage, LifecycleStage::Shutdown);
}

// ---------------------------------------------------------------------------
// WorkerPool implements Lifecycle trait
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_trait_is_implemented() {
    let (runtime, pool, _dir) = make_pool(1);

    // Verify the trait is implemented by calling through the trait
    let pool_ref: &dyn Lifecycle = &pool;
    assert_eq!(pool_ref.name(), "worker_pool");

    // initialize via trait
    assert!(pool_ref.initialize().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Initialized);

    // start via trait (needs runtime context)
    runtime.block_on(async {
        assert!(pool_ref.start().is_ok());
    });
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Running);

    // shutdown via trait
    assert!(pool_ref.shutdown().is_ok());
    assert_eq!(pool.lifecycle_stage(), LifecycleStage::Shutdown);
}
