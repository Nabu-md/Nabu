# Lifecycle Integration Testing

## Overview

The `lifecycle_full.rs` integration test suite validates the end-to-end
application lifecycle for the Nabu Capability Platform. It exercises the real
`ApplicationContext` composition root with production services wired together
— no mocks — across startup, runtime, graceful shutdown, crash recovery, and
session restoration.

## Architecture

### Test Strategy

```text
┌──────────────────────────────────────────────────────────┐
│  Lifecycle Full Lifecycle Integration Tests             │
│  (crates/nabu-core/tests/lifecycle_full.rs)              │
│                                                            │
│  Uses:                                                    │
│    - ApplicationContext  (direct lifecycle driving)      │
│    - Application         (stage transition validation)   │
│    - Real services: StorageManager, ConversationStore,   │
│      WorkerPool, DurableJobQueue, CaptureEngine,         │
│      ProcessingPipeline, PipelineExecutor                │
│    - No mocks — all production service implementations   │
│    - Each test gets a unique tempdir (parallel-safe)     │
└──────────────────────────────────────────────────────────┘
```

### Lifecycle Stage Transitions

The canonical lifecycle follows a one-way, strictly-forward state machine:

```text
Created → Initialized → Running → Shutdown
                    ↘
                      (skip to Shutdown for early exit)
```

Each stage has a single source of truth — the `LifecycleManager` — which uses
an atomic `AtomicU8` for lock-free stage queries. Both `Application` and
`ApplicationContext` maintain independent `LifecycleManager` instances:

- **`ApplicationContext`** — its lifecycle is advanced by `ctx.initialize()`,
  `ctx.start()`, and `ctx.shutdown()`. Health checks and metrics report from
  here.
- **`Application`** — its lifecycle is advanced by `app.initialize()`,
  `app.start()`, and `app.shutdown()`. Used for stage-transition validation
  and service-level dependency injection.

During `ctx.initialize()` and `ctx.start()`, the context drives each service's
lifecycle methods in dependency-safe order:

```text
StorageManager → ConversationStore → WorkerPool → PipelineExecutor →
CaptureEngine → Indexer → VaultGraph → PluginManager
```

During `ctx.shutdown()`, services are shut down in reverse dependency order
(consumers before providers).

### Test Helpers

| Helper | Description |
|--------|-------------|
| `test_knowledge_object()` | Creates a `KnowledgeObject` of type `Note` with Markdown content and two tags |
| `build_minimal_app()` | Builds an `Application` with only the three required services (`capture_engine`, `pipeline`, `storage_manager`) on a temp vault |
| `build_full_app()` / `build_full_app_on()` | **Deprecated** — use `build_full_context()` instead |
| `build_full_context()` | Builds an `ApplicationContext` with all core services on a fresh temp vault |
| `build_full_context_on(vault_path)` | Builds an `ApplicationContext` on a specific vault path; used by crash recovery tests that need two contexts on the same vault |

### Service Wiring (matches Tauri's `build_application_context`)

The test helpers register services in the same order and with the same
configuration as the production Tauri frontend:

| Key | Service | Lifecycle? | Metrics? |
|-----|---------|------------|----------|
| `event_bus` | `EventBus<PipelineEvent>` | No | No |
| `performance_monitor` | `PerformanceMonitor` | No | Yes |
| `storage_manager` | `StorageManager` | Yes | Yes |
| `conversation_store` | `ConversationStore` | Yes | No |
| `pipeline` | `ProcessingPipeline` | No | No |
| `job_queue` | `DurableJobQueue` | No (no Lifecycle impl) | Yes |
| `pipeline_executor` | `PipelineExecutor` | Yes | No |
| `worker_pool` | `WorkerPool` | Yes | Yes |
| `capture_engine` | `CaptureEngine` | Yes | Yes |

## Test Categories

### 1. Startup & Initialization (5 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_startup_transitions_through_all_stages` | Application stage transitions: Created → Initialized → Running → Shutdown |
| `lifecycle_full_health_check_before_startup` | Health check reports `Created` stage, not initialized, overall `Healthy` |
| `lifecycle_full_health_check_healthy_after_start` | Health check reports `Running` stage, all services visible, `running_service_count >= 1` |
| `lifecycle_full_start_requires_initialization` | `Application::start()` panics if `initialize()` was not called first |
| `lifecycle_full_initialize_validates_missing_services` | `initialize()` returns `Err` with list of missing required service keys |
| `lifecycle_full_service_count_grows_with_full_app` | Full context registers more services than a minimal app |

### 2. Runtime Behavior (5 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_storage_save_and_load` | `StorageManager.save()` persists to disk; `load()` returns the same object |
| `lifecycle_full_storage_publishes_item_stored_event` | Save publishes an `ItemStored` event on the EventBus |
| `lifecycle_full_conversation_save_and_load` | `ConversationStore.save()` persists a `Thread`; `load()` returns it with correct title |
| `lifecycle_full_conversation_list_lists_saved_threads` | `ConversationStore.list()` returns all saved threads |
| `lifecycle_full_job_queue_persists_jobs` | `DurableJobQueue.enqueue()` persists jobs to disk; `load_by_status()` retrieves them |

### 3. Graceful Shutdown (5 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_graceful_shutdown_transitions_to_shutdown` | `ctx.shutdown()` transitions to `Shutdown` stage |
| `lifecycle_full_double_shutdown_is_safe` | Second `shutdown()` is a no-op (no error) |
| `lifecycle_full_shutdown_without_start` | Can shut down from `Initialized` stage (skip Running) |
| `lifecycle_full_shutdown_from_created` | Can shut down from `Created` stage (never started) |
| `lifecycle_full_health_check_after_shutdown` | Health check reflects shutdown: not running, still initialized, stage is `Shutdown` |

### 4. Crash Recovery (3 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_storage_survives_restart` | A KnowledgeObject saved before a simulated crash is loadable from a new context on the same vault |
| `lifecycle_full_conversations_survive_restart` | Thread persisted before crash is reloaded during `initialize()` of a new context |
| `lifecycle_full_job_queue_survives_restart` | A job enqueued before crash is reloaded by `DurableJobQueue::new()` on the same queue path |

**Crash simulation**: Tests drop the context without calling `shutdown()`,
simulating a process crash. `StorageManager` is write-through (data is on disk
immediately after `save()`), so no explicit flush is needed.
`ConversationStore` uses atomic writes (write-to-temp + rename), so persisted
threads survive unclean shutdowns. `DurableJobQueue` persists each job as an
individual JSON file, so queued jobs survive.

### 5. Session Restoration (1 test)

| Test | Validates |
|------|-----------|
| `lifecycle_full_session_restoration` | Full lifecycle: save objects + conversations, restart on same vault, verify all data is restored. Also checks health, metrics, and list operations survive restart. |

### 6. Lifecycle Stage Tracking & Metrics (4 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_metrics_collected_during_runtime` | `ctx.metrics()` returns data from registered `MetricsAggregator` services (PerformanceMonitor, StorageManager, WorkerPool) |
| `lifecycle_full_lifecycle_stage_transitions_are_one_way` | Application stages are strictly forward; service lifecycle stages match app stage |
| `lifecycle_full_health_check_stage_reflects_current_phase` | `health_check().lifecycle_stage` matches the context's current stage at each phase |
| `lifecycle_full_health_check_lists_all_managed_services` | `health_check().services` includes at least `capture_engine`; `running_service_count >= 1` when running |
| `lifecycle_full_lifecycle_service_keys_match_registry` | Health report service count is consistent with registered lifecycle services |

### 7. Event Bus Integration (3 tests)

| Test | Validates |
|------|-----------|
| `lifecycle_full_event_bus_subscribes_and_receives_events` | Subscriber receives `ItemStored` events when storage saves objects |
| `lifecycle_full_event_bus_multiple_subscribers_receive_events` | Multiple subscribers each receive the same event |
| `lifecycle_full_event_bus_event_delivery_after_shutdown_stops` | No new events fire after `shutdown()`; context is in `Shutdown` stage |

## Async Runtime Requirement

Tests that call `ctx.start()` must be annotated with
`#[tokio::test(flavor = "multi_thread")]` because `WorkerPool::start()` spawns
tokio tasks that require a running multi-threaded runtime. Tests that only
validate stage transitions on `Application` (which calls `engine.start()` etc.
synchronously) can use plain `#[test]`.

## Running the Tests

```bash
# From the nabu-core crate directory:
cd crates/nabu-core
cargo test lifecycle_full

# With output:
cargo test lifecycle_full -- --nocapture

# Single-threaded (eliminates all parallelism-related flakiness):
cargo test lifecycle_full -- --test-threads=1
```

## Crash Recovery Notes

The `DurableJobQueue` does not implement the `Lifecycle` trait — it has no
`shutdown()` method that stops its internal `running` flag. The `WorkerPool`
owns the queue and its `shutdown()` drains workers and aborts their tokio task
handles. In the `lifecycle_full_job_queue_survives_restart` test, a brief
`100ms` sleep after `ctx.shutdown()` in Phase 1 ensures the runtime's worker
tasks have fully terminated before Phase 2 reopens the queue directory,
preventing file handle races.
