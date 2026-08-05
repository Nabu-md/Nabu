# Nabu v0.3 Codebase Audit: Capability Platform Readiness

## Executive Summary

The Nabu codebase has undergone a significant architectural transformation since AUDIT_0.1.
It now contains a **fully realized platform foundation** with:

- A **composition root** (`Application` + `ApplicationBuilder`) with explicit dependency injection
- A **ServiceRegistry** with type-erased singleton/factory registration and category-based discovery
- A **CapabilityRegistry** with built-in capabilities including `nabu:sync`, `nabu:plugin`, `nabu:ai`, `nabu:embedding`
- A **PluginManager** (foundation) with manifest validation, dependency graph resolution, and lifecycle stages
- A **FeatureRegistry** with staged feature flags (`plugin.wasm`, `plugin.external`, etc.)
- A formalized **LifecycleManager** with one-way stage transitions (Created → Initialized → Running → Shutdown)
- A single **EventBus<PipelineEvent>** that is the only pub/sub system
- A **WorkerPool** + **DurableJobQueue** for async processing
- A **PipelineExecutor** that bridges workers to the processing pipeline

The three Capability Platform modules (Syncthing P2P sync, Harper grammar checking, ACP client)
each map onto existing architectural abstractions:

| Capability | Best-fit reuse | Readiness |
|---|---|---|
| Syncthing (P2P sync) | Process management (missing) + EventBus + Settings + CapabilityRegistry | **NOT READY** (process mgmt) |
| Harper (grammar/Lint) | ProcessingPipeline + JobQueue + Processor trait + Settings | **READY** (minor extension) |
| ACP Client (JSON-RPC agents) | Process management (missing) + JobQueue + JobExecutor trait + EventBus + Settings | **NOT READY** (process mgmt) |

The **single largest gap** is the absence of a general-purpose process management abstraction.
The codebase has `std::process::Command` calls scattered in `commands.rs` (fire-and-forget,
no lifecycle tracking) but no `tokio::process` integration, no Tauri sidecar pattern, no
process pool, and no `JobExecutor`-like trait for external subprocesses. A modest
`SubprocessCapability` built on `tokio::process::Command` and `JobExecutor` would unblock
both Syncthing and ACP Client with high reuse of existing infrastructure.

---

## Capability Platform Readiness Matrix

| Investigation Area | Assessment | Evidence |
|---|---|---|
| Runtime Lifecycle | **READY** | `Application::builder()` / `ApplicationBuilder` with explicit DI, `LifecycleManager` with one-way stages, `ctx.initialize()` + `ctx.start()` in `lib.rs:339-342` |
| Background Services | **READY** | `WorkerPool` (4 workers, `ShutdownCoordinator` with 30s drain), `DurableJobQueue` (file-backed), `ProcessingPipeline` (14 processors), `CaptureEngine`, `Indexer`, `VaultGraph` — all registered in `ApplicationContext` |
| Service Registration | **READY** | `ServiceRegistry` with singleton/factory modes, categories, type-erased resolution. `ApplicationContext::register()` / `resolve()` / `resolve_category()`. Typed accessors for all known services. |
| Configuration System | **READY** | `AppSettings` (80+ fields), `SettingsStore` (Mutex + JSON persistence), `update()` for atomic save+persist, `extra_settings` HashMap, `set_feature_toggle()` for feature flags |
| IPC Architecture | **EXTENDABLE** | 80+ `#[tauri::command]` functions registered in `invoke_handler` (lib.rs:201-311). Flat namespace — no per-capability grouping. Could be modularized via command routing. |
| Event System | **READY** | Single `EventBus<PipelineEvent>`, 9 event variants with kind constants. `StorageManager.save()` → `ITEM_STORED` → `Indexer` + `VaultGraph` subscription wired in `lib.rs:162-177` |
| Process Management | **NOT READY** | Only fire-and-forget `std::process::Command` in `commands.rs` (lines 378, 395, 420, 733, 802, 847, 853, 858). No `tokio::process`, no Tauri sidecar, no process lifecycle tracking |
| Async Architecture | **READY** | Tokio with `sync, time, rt, macros` features (nabu-core Cargo.toml:10). `tokio::spawn` for workers/socket server, `tokio::task::spawn_blocking` for CPU-bound PDF/OCR/Whisper. `JoinHandle` + `JoinSet` pattern in `WorkerPool` |
| Trait Architecture | **READY** | `Processor` trait (`processing/processor.rs`), `CaptureHandler` trait (`capture/handler.rs`), `JobExecutor` trait (`jobs/workers/executor.rs`), `Queue` trait (`jobs/queue.rs`), `Lifecycle` trait (`registry/lifecycle.rs`), `LifecycleTrait` for plugins |
| Processing Pipelines | **READY** | `ProcessingPipeline` (ordered, async, cancellable), `CaptureEngine` (handler routing), `PipelineExecutor` (worker → pipeline → storage bridge). Modular design with `register()`/`register_at()` |
| Dependency Direction | **READY** | Clean crate boundary: `src-tauri` → `nabu-core`. All platform abstractions live in `nabu-core` (no Tauri coupling inside core). Core crates are pure Rust with tokio. Future capability crates can depend on `nabu-core` inward. |
| Reuse for Syncthing | **NOT READY** | No process management. EventBus, SettingsStore, CapabilityRegistry (`nabu:sync`), DurableJobQueue all available for reuse. |
| Reuse for Harper | **READY** | `Processor` trait, `ProcessingPipeline`, `JobExecutor`, `ProgressReporter`, `CancellationToken`, `spawn_blocking` for native code. `nabu:embedding` + `nabu:ai` capabilities suggest AI provider abstraction. |
| Reuse for ACP Client | **NOT READY** | No async process management. `JobExecutor` trait, `JobQueue`, `EventBus`, `SettingsStore` available. `CancellationToken` in `jobs/cancellation.rs` provides cooperative cancellation model. |

---

## Existing Reusable Infrastructure

### Ready to Use Immediately

| System | Module | Reusability for Each Capability |
|---|---|---|
| **ServiceRegistry** | `registry/mod.rs` | Syncthing: register `syncthing_manager` service. Harper: register `harper_engine` service. ACP: register `acp_client` service. |
| **CapabilityRegistry** | `plugin/capability.rs` | Built-in capabilities include `nabu:sync`, `nabu:plugin`, `nabu:ai`, `nabu:embedding`. New capabilities can be registered with `capability_registry.register()`. |
| **FeatureRegistry** | `plugin/features.rs` | Flags: `plugin.wasm`, `plugin.lua`, `plugin.external`, `plugin.dev_mode`. New flags can be added. `is_enabled()` / `enable()` / `disable()`. |
| **EventBus<PipelineEvent>** | `event_bus/bus.rs` | Single pub/sub backbone. Subscribers use synchronous closures. `ITEM_STORED`, `INDEX_UPDATED`, `GRAPH_UPDATED` already in use. |
| **DurableJobQueue** | `jobs/queue.rs` | File-backed, persistent queue. `enqueue()`, `dequeue()`, `mark_completed/failed`, `report_progress`. Jobs survive restarts. |
| **WorkerPool** | `jobs/workers/pool.rs` | 4 workers, pulls from queue, dispatches to `ExecutorRegistry`. `ShutdownCoordinator` with 30s drain timeout, `BackpressureController`. |
| **JobExecutor trait** | `jobs/workers/executor.rs` | `execute(&self, job, progress, cancellation) -> JobResult<Job>`. Already implemented by `PipelineExecutor`. External process executors can implement this. |
| **Job model** | `jobs/job.rs` | Rich model: UUID, `JobType`, priority, status, retry policy, progress, cancellation token, tags, metadata. `JobType` enum has `Ocr`, `Whisper`, `PdfTextExtraction`, `MetadataExtraction`, `Processing`, `Sync`, `Embedding`, `Custom`. |
| **SettingsStore** | `settings.rs` | Thread-safe, JSON-persistent. `update()` for atomic changes, `extra_settings` for plugin config, `set_feature_toggle()` for runtime flags. |
| **PipelineExecutor** | `pipeline_migration/executor.rs` | Implements `JobExecutor`. Bridges WorkerPool → ProcessingPipeline → StorageManager. Already registered under `ocr_processor`, `whisper_processor`, `pdf_text_extraction_processor`, `metadata_extraction_processor` in `lib.rs:108-117`. |
| **CaptureHandler trait** | `capture/handler.rs` | `name()`, `source()`, `async capture()`. 14 handlers registered. New handlers can extend this. |
| **Processor trait** | `processing/processor.rs` | `name()`, `process()`, `supports()`. 14 processors registered. New processors can be added to pipeline. |
| **ApplicationContext** | `registry/context.rs` | Typed accessors: `capture_engine()`, `processing_pipeline()`, `job_queue()`, `worker_pool()`, `vault_graph()`, `indexer()`, `storage_manager()`, `history_manager()`, `performance_monitor()`. `register()` for late registration, `resolve<T>()` for type-safe lookup. |
| **LifecycleManager** | `registry/lifecycle.rs` | One-way stages: Created → Initialized → Running → Shutdown. `Lifecycle` trait for services: `initialize()`, `start()`, `shutdown()`. `validate_core_services()` checks required services. |
| **ApplicationBuilder** | `registry/application.rs` | Builder pattern with `.with_event_bus()`, `.with_processing_pipeline()`, `.with_capture_engine()`, `.with_performance_monitor()`, `.with_registry()`. `test_builder()` for testing with mocks. |

### Partially Available (Needs Minor Extension)

| System | Gap | Extension Needed |
|---|---|---|
| **FeatureRegistry** | Not wired to `SettingsStore` | Sync `FeatureRegistry` flags to `AppSettings.extra_settings` |
| **Lifecycle trait** | Only `ApplicationContext` implements it; no service implements `Lifecycle` | Have `WorkerPool`, `StorageManager`, etc. implement `Lifecycle` for proper startup/shutdown |
| **Shutdown handling** | Tauri Exit event only calls `mark_clean_exit()`. Does NOT call `WorkerPool.shutdown()` or `ApplicationContext.shutdown()`. | Add shutdown hook in Tauri's `run()` event handler |
| **EventBus** | Handlers are synchronous (`Fn(&Events)`), no async event processing | Add async event bus variant or `tokio::task::spawn` inside handlers for long-running event subscribers |

---

## Lifecycle & Runtime Analysis

### Startup Sequence

Call path: `run()` (`src-tauri/src/lib.rs:183`) → `tauri::Builder::default()` → `.manage(settings_store)` → `.invoke_handler(...)` → `.setup(|app|)` → `build_application_context(vault_path)` → `ctx.initialize()` → `ctx.start()`

#### `build_application_context(vault_path)` — `lib.rs:55-180`

Construction order (hardcoded, no topological sort):

1. **`EventBus<PipelineEvent>`** — `EventBus::new()` → registered as `"event_bus"` in `ServiceRegistry` (`lib.rs:69`)
2. **`ServiceRegistry`** — `ServiceRegistry::new()` wrapped in `Arc<RwLock<>>`
3. **`CapabilityRegistry`** — `nabu_core::plugin::CapabilityRegistry::new()` → `.register_builtin()` (`lib.rs:60-61`) registers 14 built-in capabilities including `nabu:sync`, `nabu:plugin`, `nabu:ai`, `nabu:embedding`
4. **`ApplicationContext`** — `ApplicationContext::new(registry, event_bus, capability_registry)`. **NOT** `ApplicationBuilder` — the lib.rs setup uses a function that manually constructs the context, bypassing the builder pattern.
5. **`StorageManager`** — `StorageManager::with_event_bus(vault_path, event_bus)` → registered as `"storage_manager"` (`lib.rs:77`). Publishes `ITEM_STORED` on save.
6. **`ProcessingPipeline`** — `build_standard_pipeline(Some(event_bus))` → registered as `"pipeline"` (`lib.rs:81`). 14 processors in `processing/processors/mod.rs`. Processors also registered in `CATEGORY_PROCESSORS` category (`lib.rs:84-89`).
7. **`DurableJobQueue`** — `DurableJobQueue::new(vault_path/.nabu/queue)` → registered as `"job_queue"` (`lib.rs:100`)
8. **`PipelineExecutor`** — `PipelineExecutor::with_event_bus(pipeline, event_bus).with_storage(storage)` → registered in `ExecutorRegistry` under keys: `"ocr_processor"`, `"whisper_processor"`, `"pdf_text_extraction_processor"`, `"metadata_extraction_processor"` (`lib.rs:108-117`)
9. **`WorkerPool`** — `WorkerPool::new(4, queue, executors)` → registered as `"worker_pool"` (`lib.rs:120-121`)
10. **`CaptureEngine`** — `build_default_capture_engine(Some(event_bus), Some(queue))` → registered as `"capture_engine"` (`lib.rs:124-128`). 14 handlers registered in `CATEGORY_CAPTURE_HANDLERS` (`lib.rs:131-136`)
11. **`Indexer`** — `Indexer::with_event_bus(event_bus)` → registered as `"indexer"` wrapped in `Arc<Mutex<>>` (`lib.rs:139`). Subscribes to `ITEM_STORED` (`lib.rs:162-177`).
12. **`VaultGraph`** — `VaultGraph::with_persistence(Some(event_bus), vault_path)` → registered as `"vault_graph"` wrapped in `Arc<RwLock<>>` (`lib.rs:143-147`). Also subscribes to `ITEM_STORED` directly (`lib.rs:162-177`), calling `add_node()` — **bypasses** the `GraphEventBridge`/`IncrementalUpdateEngine` (see Key Finding #4).
13. **`HistoryManager`** — `HistoryManager::new()` → registered as `"history_manager"` wrapped in `Arc<RwLock<>>` (`lib.rs:150-153`)

#### `setup(|app|)` — `lib.rs:312-387`

- Resolves `vault_path` from `SettingsStore.last_vault_path` (defaults to cwd if empty)
- `mark_running(&vault_path)` — writes `.running` marker file for crash recovery
- `build_application_context(vault_path)` — constructs all services
- `ctx.initialize()` — validates core services (`["event_bus", "capture_engine", "pipeline", "storage_manager"]` required, `["job_queue", "worker_pool", "vault_graph", "indexer"]` optional). Logs warning if incomplete.
- `ctx.start()` — transitions to `Running` stage
- `app.manage(ctx)` — stores `ApplicationContext` in Tauri managed state for command access
- `tauri::async_runtime::spawn(pool.start())` — starts worker pool on Tauri's async runtime
- `tauri::async_runtime::spawn(start_socket_server(...))` — starts native messaging socket server
- Safety net: forces window visible after 8s timeout if `on_page_load` never fires (`lib.rs:376-384`)

#### Startup Ordering

**Yes, ordering exists** — hardcoded in `build_application_context()` function, not driven by a topological sort. The order is: EventBus → Storage → Pipeline → Queue → Executor → Workers → Capture → Indexer → Graph → History. This matches the data flow: `CaptureEngine → Queue → Workers → Pipeline → Storage → ITEM_STORED → Indexer + VaultGraph`.

However, this construction is in a free function (`build_application_context`), not in `ApplicationBuilder::build()`. The `ApplicationBuilder` (in `registry/application.rs`) only knows about `event_bus`, `pipeline`, `capture_engine`, and `performance_monitor` — it does NOT register storage, queue, workers, indexer, graph, or history. **The real wiring happens in `build_application_context()` in `lib.rs`, which is a Tauri-specific function, not in the reusable `ApplicationBuilder`.**

### Shutdown Sequence

**Incomplete.** The Tauri `run()` event handler only handles `RunEvent::Exit`:

```rust
// lib.rs:402-412
.run(|app_handle, event| {
    if let tauri::RunEvent::Exit = event {
        let settings = app_handle.state::<SettingsStore>().get();
        let path = settings.last_vault_path.trim().to_string();
        if !path.is_empty() {
            crate::recovery::mark_clean_exit(&PathBuf::from(path));
        }
    }
})
```

**Missing**:
- `WorkerPool.shutdown()` is never called — workers are orphaned (tokio tasks are just dropped on exit)
- `ApplicationContext::shutdown()` is never called — no `Lifecycle` transition to `Shutdown`
- No `Job` drain — in-progress jobs may be lost
- No `Indexer.persist()` call — in-memory index changes since last save may be lost
- No `VaultGraph.persist()` call — graph changes may be lost

The `WorkerPool.shutdown()` method exists (`pool.rs:97-128`) with proper drain + abort logic, but is never invoked from the Tauri shutdown path.

### Crash Recovery

`mark_running(&vault_path)` writes a `.running` file. `mark_clean_exit()` removes it. On startup, `recovery_check` detects a stale `.running` file and offers to restore the last session. This is **implemented and functional**.

---

## Background Services Analysis

| Service | Owner | Startup Location | Shutdown Location | Supervision |
|---|---|---|---|---|
| **EventBus** | `lib.rs:57` | `build_application_context()` | None | N/A (pub/sub, no thread) |
| **StorageManager** | `lib.rs:73` | `build_application_context()` | None | N/A (sync I/O) |
| **ProcessingPipeline** | `lib.rs:80` | `build_application_context()` | None | N/A (stateless) |
| **DurableJobQueue** | `lib.rs:93` | `build_application_context()` | None | N/A (file-backed) |
| **PipelineExecutor** | `lib.rs:104` | `build_application_context()` | None | N/A (stateless) |
| **WorkerPool** | `lib.rs:120` | `tauri::async_runtime::spawn(pool.start())` (`lib.rs:349`) | **None** | 4 workers, `ShutdownCoordinator` with 30s drain |
| **CaptureEngine** | `lib.rs:124` | `build_application_context()` | None | N/A (routes to queue) |
| **Indexer** | `lib.rs:139` | `build_application_context()` | None | Subscribes to `ITEM_STORED` (sync closure) |
| **VaultGraph** | `lib.rs:143` | `build_application_context()` + `with_persistence()` loads from disk | None | Subscribes to `ITEM_STORED` (sync closure), auto-persists on `add_node` |
| **HistoryManager** | `lib.rs:150` | `build_application_context()` | None | N/A (in-memory stacks) |
| **NativeMessagingSocket** | `lib.rs:357` | `tauri::async_runtime::spawn(start_socket_server())` (`lib.rs:360`) | None (uses `tokio::sync::Notify`) | `SocketServerHandle` has `shutdown()` method, never called |

### Supervisor Model

**None**. Background services are started with `tokio::spawn` and their `JoinHandle`s are
dropped. There is no supervisor that restarts crashed workers or monitors service health.
The `WorkerPool` has a `shutdown()` method with drain logic but it is never called on app
exit.

### Background Services as Capability Lifecycle

The existing services already resemble a capability lifecycle:
- **Startup**: `ApplicationContext` registers services → `ctx.initialize()` validates → `ctx.start()` transitions to Running → `tauri::async_runtime::spawn` starts workers
- **Runtime**: EventBus subscribers (Indexer, VaultGraph) react to events
- **Shutdown**: **Missing** — no graceful shutdown hook

To support optional capabilities (Syncthing daemon, ACP agent subprocess), a new
`SubprocessCapability` would need to be:
1. Registered in `ApplicationContext` via `ctx.register("syncthing", Arc::new(...))`
2. Started in the Tauri `setup` closure alongside the WorkerPool
3. Subscribed to EventBus for configuration changes
4. Shut down in the Tauri `Exit` event handler

The existing pattern for starting background services (`tauri::async_runtime::spawn(...)`)
is **directly reusable** for any new capability that needs a background task.

---

## Service Registration Analysis

### ServiceRegistry (`registry/mod.rs:41-189`)

```
ServiceRegistry
├── singletons: HashMap<String, Arc<dyn Any + Send + Sync>>  — type-erased
├── factories: HashMap<String, Box<dyn Fn() -> Arc<dyn Any + Send + Sync>>>
├── categories: HashMap<String, Vec<String>>
└── operations: register(), register_factory(), resolve<T>(), unregister()
```

- **Thread safety**: `Arc<RwLock<ServiceRegistry>>` — shared across all threads
- **Resolution**: Type-erased via `Arc<dyn Any>` with `downcast::<T>()` — panics on type mismatch (documented as programmer error)
- **Categories**: `register_in_category(category, key)` + `resolve_category::<T>(category)` for batch resolution
- **Standard categories**: `CATEGORY_PROCESSORS`, `CATEGORY_CAPTURE_HANDLERS`, `CATEGORY_AI_PROVIDERS`, `CATEGORY_OCR_PROVIDERS`, `CATEGORY_EMBEDDING_PROVIDERS`, `CATEGORY_EXPORTERS`, `CATEGORY_STORAGE_PROVIDERS`, `CATEGORY_CONTENT_PROVIDERS`

### ApplicationContext (`registry/context.rs:141-421`)

Wraps `ServiceRegistry` + `EventBus` + `CapabilityRegistry` + `LifecycleManager`.

**Typed accessors** (hardcoded for known services):
- `capture_engine()` → `resolve("capture_engine")`
- `processing_pipeline()` → `resolve("pipeline")`
- `job_queue()` → `resolve("job_queue")`
- `worker_pool()` → `resolve("worker_pool")`
- `vault_graph()` → `resolve("vault_graph")` (returns `Arc<RwLock<VaultGraph>>`)
- `indexer()` → `resolve("indexer")` (returns `Arc<Mutex<Indexer>>`)
- `storage_manager()` → `resolve("storage_manager")`
- `history_manager()` → `resolve("history_manager")` (returns `Arc<RwLock<HistoryManager>>`)
- `performance_monitor()` → `resolve("performance_monitor")`

**Registration API**: `ctx.register(key, Arc<T>)` and `ctx.register_in_category(category, key)`.

### CapabilityRegistry (`plugin/capability.rs:90-205`)

```
CapabilityRegistry
├── capabilities: HashMap<String, Capability>
├── providers: HashMap<String, String>  (cap_id → provider)
├── enabled: HashSet<String>
├── methods: register(), register_builtin(), enable(), disable(), is_enabled(), list()
└── standard capabilities: nabu:event_bus, nabu:storage, nabu:capture, nabu:processor,
    nabu:graph, nabu:export, nabu:search, nabu:ocr, nabu:ai, nabu:embedding,
    nabu:template, nabu:sync, nabu:watch, nabu:plugin
```

Already declares `nabu:sync` (Cap 1), `nabu:plugin` (Cap 3), `nabu:ai` (Cap 2). Not yet wired
to actual implementations for some.

### Readiness for Capability Registry Evolution

**Existing `ServiceRegistry` IS the capability registry.** It already supports:
- Key-based service registration and resolution
- Category-based grouping (processors, capture_handlers, etc.)
- Factory-based transient resolution

**Extension needed**: A `Capability` abstraction that binds a service key to a lifecycle
(start/stop methods, health checks). The `CapabilityRegistry` (in `plugin/capability.rs`)
is metadata-only (capability names, descriptions, enable/disable flags) — it does NOT
manage service lifecycles. A `CapabilityRuntime` that bridges `ServiceRegistry` ↔
`CapabilityRegistry` would provide the full capability lifecycle (enable → register
service → start → subscribe to events → disable → shutdown → unregister).

---

## IPC & Event Architecture Assessment

### IPC Layer

**Command registration** (`lib.rs:201-311`):

```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        crate::commands::check_vault_exists,
        crate::commands::get_current_vault,
        ...
        crate::history::note_rename,
        crate::history::note_delete,
        ...
        crate::recovery::note_save,
        crate::recovery::note_read,
        ...
    ])
```

Total: ~80 commands across `commands.rs`, `history.rs`, `recovery.rs`, and `settings.rs`.

**Access pattern**: Every command receives `State<ApplicationContext>` or
`State<SettingsStore>` and resolves services via `ctx.resolve("key")`.

**No command routing layer**: Each command function directly accesses the services it needs.
There is no central command dispatcher, no middleware, no per-capability command namespace.

### Readiness for Future Capability IPC

**Current approach**: Add new `#[tauri::command]` functions to the `invoke_handler` list.
This works but creates a flat namespace — no scoping by capability.

**Opportunity**: The `ServiceRegistry` + `ApplicationContext` pattern means capability
commands can resolve their own services via `ctx.resolve("capability_name")` with zero
changes to the Tauri invocation pattern. Only the `invoke_handler` list needs new entries.

**Recommendation**: Add a simple command router that dispatches `invoke(handler, method, args)`
to capability-registered handlers, eliminating the need to edit `lib.rs` for every new command.
The existing `ExecutorRegistry` pattern (register by name, resolve by name) is a direct
precedent.

### EventBus Architecture

```
EventBus<PipelineEvent>  (event_bus/bus.rs:10-128)
├── subscribe(event_kind: &str, handler: Fn(&PipelineEvent)) → Subscription
├── publish(event_kind: &str, event: &PipelineEvent)
├── unsubscribe(subscription: &Subscription)
└── subscriber_count(event_kind) → usize
```

**Key constraint**: Handlers are **synchronous** (`Fn(&PipelineEvent)`, not `async Fn`).
Long-running subscribers must spawn their own tokio tasks inside the handler.

**Event flow** (wired in `lib.rs:155-177`):

```
StorageManager.save(object)
  → publishes ITEM_STORED
  → Indexer.index_object(object)   [in-process, synchronous]
  → VaultGraph.add_node(object)    [in-process, synchronous, auto-persists]
```

**GraphEventBridge** (`graph/incremental/event_wiring.rs`) exists to route `ITEM_STORED`
→ `IncrementalUpdateEngine` → `VaultGraph` with change tracking, region analysis, and
batch updates. **But it is NOT wired in production** — only in tests. The production
path in `lib.rs:162-177` calls `Indexer.index_object()` and `VaultGraph.add_node()`
directly, bypassing the incremental engine entirely.

### Event Streaming to Frontend

**Missing entirely.** The `EventBus` events are internal-only. There is no mechanism to
forward events to the Leptos frontend. The frontend must poll for changes (e.g., calling
`queue_get_all()` to check job progress, `inbox_get_queue()` for new captures).

For capabilities that need to push real-time updates to the UI (ACP agent output,
Syncthing status, Harper analysis progress), the existing `EventBus` would need an
**outgoing bridge** to Tauri events (`window.emit_all()`) or an event-source/WebSocket
bridge.

---

## Configuration & Settings Assessment

### AppSettings (`settings.rs:13-117`)

80+ fields across 14 categories:

| Category | Fields | Examples |
|---|---|---|
| Appearance | 8 | theme, font_size, sidebar_width, high_contrast |
| Editor | 9 | tab_size, word_wrap, auto_save_interval_secs, spell_check |
| Markdown | 5 | markdown_gfm, math rendering, diagram rendering |
| Search | 5 | search_index_on_startup, fuzzy_matching, max_results |
| Graph | 6 | include_folders_in_graph, node_physics, show_tags_as_badges |
| Files & Vaults | 8 | last_vault_path, recent_vaults, trash_retention_policy |
| Import & Export | 4 | default_export_format, import_duplicate_strategy |
| OCR | 3 | ocr_language, confidence_threshold, auto_process_scanned_pdfs |
| Accessibility | 3 | screen_reader_support, keyboard_navigation, focus_ring_visible |
| Performance | 4 | max_undo_history, worker_pool_size, index_on_startup, background_processing |
| Privacy | 5 | launch_at_startup, analytics_enabled, crash_reporting_enabled, auto_lock |
| Keyboard Shortcuts | 3 | voice_hotkey, quick_capture_hotkey, toggle_sidebar_hotkey |
| Advanced | 4 | force_sandbox, debug_mode, developer_tools, experimental_features |
| Experimental | 3 | whisper_model, enable_ai_summarization, enable_semantic_search |
| Extras | 1 | extra_settings: HashMap<String, serde_json::Value> |

### SettingsStore (`settings.rs:254-390`)

```rust
pub struct SettingsStore {
    path: PathBuf,
    inner: Mutex<AppSettings>,
}
```

- **Persistence**: JSON to `app_data_dir/settings.json`, atomic write via `std::fs::write`
- **Thread-safe**: `Mutex<AppSettings>` — all access through `get()`, `set()`, `update()`, `save()`
- **Atomic updates**: `update(updater_fn)` applies changes in-place, clones, persists, then sets
- **Generic key-value**: `extra_settings: HashMap<String, serde_json::Value>` with `get_value()` / `set_value()`
- **Feature toggles**: `set_feature_toggle(id, enabled)` stores in `extra_settings["featureToggles"]`
- **Export/Import**: `export_settings()` → `SettingsExport { version, exported_at, platform, settings }`, `import_settings(payload)` validates version prefix

### Readiness for Capability Configuration

**Existing settings infrastructure is fully reusable.** Each capability can:
1. Store its config in `extra_settings` via `set_value("capability_name", json_value)`
2. Enable/disable via `set_feature_toggle("capability_name", bool)`
3. Read settings via `State<SettingsStore>` in Tauri commands

**Gap**: No automatic propagation of settings changes to running services. When a user
changes a setting in the UI, the corresponding service must be explicitly notified.
Currently, settings are read at startup in `build_application_context()` from the
`SettingsStore` passed via `app.manage()`. There is no `SettingsChanged` event on
the `EventBus`.

**`FeatureRegistry` is disconnected**: `plugin/features.rs` defines `FeatureFlag` with
`FeatureStage` (Stable, Beta, Alpha, Experimental) and an `enable/disable` API, but the
`FeatureRegistry` is only used by `PluginManager` — which is **not instantiated** in the
current Tauri app. The `AppSettings.experimental_features` boolean is the de facto feature
toggle, not the `FeatureRegistry`.

---

## Process Management Assessment

### Current State

**No general-purpose process management infrastructure exists.**

The only process execution in the codebase is via `std::process::Command`:

| File | Line | Command | Purpose |
|---|---|---|---|
| `commands.rs` | 378 | `open` (macOS) | Reveal file in Finder |
| `commands.rs` | 395 | `explorer` (Windows) | Reveal in Explorer |
| `commands.rs` | 420 | `xdg-open` (Linux) | Open in file manager |
| `commands.rs` | 733 | `terminal-notifier` | Show macOS notification |
| `commands.rs` | 802 | `notify-send` | Show Linux notification |
| `commands.rs` | 847 | `open` | Open vault in Finder |
| `commands.rs` | 853 | `explorer` | Reveal in Explorer |
| `commands.rs` | 858 | `xdg-open` | Open in file manager |
| `native/pdfkit.rs` (via `Command`) | 46 | `screencapture` | Screenshot capture |
| `bin/native_messaging_host.rs` | 24 | `std::process::exit` | Exit handler |

**Properties of all current usage:**
- Fire-and-forget: `Command::new(...).spawn()` or `.status()` — no `Child` handle stored
- No lifecycle tracking — process may be orphaned on app exit
- No output capture — stdout/stderr not read
- No cancellation — no `CancellationToken` integration with spawned processes
- No async — all use `std::process::Command` (blocking) or sync I/O

### What's Missing

| Feature | Required For | Current Status |
|---|---|---|
| `tokio::process::Command` integration | ACP client, Syncthing | **None** |
| `tokio::io::AsyncRead/AsyncWrite` for subprocess stdin/stdout/stderr | ACP JSON-RPC, Syncthing IPC | **None** (only sync `std::io::Read/Write` in native_messaging.rs) |
| Subprocess lifecycle management (spawn, monitor, restart, graceful shutdown) | Syncthing daemon, ACP agent | **None** |
| Process exit status tracking | Syncthing, ACP | **None** |
| Subprocess stderr/stdout capture and event forwarding | ACP agent output, Syncthing logs | **None** |
| Process pool abstraction | Managing multiple ACP agents | **None** |
| Tauri sidecar integration (`tauri::tauri.conf.json` sidecar entries) | Bundled Syncthing binary | **None** |

### Native Messaging as Precedent

The `native_messaging_socket.rs` module provides a **partial precedent** for subprocess
communication. The `native_messaging_host.rs` binary communicates via stdin/stdout
(length-prefixed JSON) and relays messages to the Tauri app via a Unix domain socket
(`/tmp/nabu-native-messaging.sock`).

However, this is a **separate binary** launched externally by the OS, not managed by
the Nabu process. It uses `std::io::Read/Write` (blocking), not async I/O. It does not
use `tokio::process::Command` to spawn or manage subprocesses.

### Readiness for Each Capability

| Capability | Process Management Needs | Readiness |
|---|---|---|
| **Syncthing** | Spawn bundled syncthing binary, monitor process, pass config via CLI args, read stdout/stderr, graceful shutdown on app exit | **NOT READY** — no process management abstraction |
| **ACP Client** | Spawn external agent process, write JSON-RPC requests to stdin, read JSON-RPC responses from stdout, track lifecycle, cancel on demand | **NOT READY** — no process management abstraction |
| **Harper** | Link as Rust library (libharper), call via FFI / `spawn_blocking` | **READY** — uses `Processor` trait + `spawn_blocking` pattern, no subprocess needed |

### Where Process Management Should Integrate

Following the existing architectural pattern, a new process management layer should:

1. **Live in `nabu-core`** (not `src-tauri`) — following the pattern where all
   platform abstractions are in the core crate
2. **Implement `JobExecutor`** — so process work flows through the existing
   `WorkerPool` → `ExecutorRegistry` → `Worker` pipeline
3. **Use `tokio::process::Command`** — for async stdin/stdout/stderr and graceful shutdown
4. **Subscribe to `EventBus`** — for configuration changes and shutdown signals
5. **Register in `ServiceRegistry`** — via `ctx.register("syncthing", Arc::new(...))`
6. **Implement `Lifecycle`** — for proper `initialize()` / `start()` / `shutdown()` hooks

The `JobExecutor` trait (`jobs/workers/executor.rs:15-23`) is the **ideal integration
point**:

```rust
#[async_trait]
pub trait JobExecutor: Send + Sync {
    async fn execute(
        &self,
        job: &Job,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> JobResult<Job>;
}
```

Existing `JobType` variants include `Sync`, `Embedding`, `Processing` — all suggesting
the job system was designed to accommodate these capability types. A
`SubprocessExecutor` implementing `JobExecutor` and using `tokio::process::Command`
would slot into the existing `ExecutorRegistry` without any changes to the
worker pool, queue, or pipeline infrastructure.

---

## Async Runtime Assessment

### Tokio Configuration

- **`nabu-core` Cargo.toml:10**: `tokio = { version = "1", features = ["sync", "time", "rt", "macros"] }`
- **`src-tauri` Cargo.toml:34**: `tokio = { version = "1.43.0", features = ["full"] }` (feature superset — "full" includes sync, time, rt, macros, plus net, io, process, signal, fs)
- **Runtime**: Tauri's built-in multi-threaded Tokio runtime (via `tauri::async_runtime`)

### Async Patterns in Use

| Pattern | Location | Usage |
|---|---|---|
| `tokio::spawn` | `lib.rs:349` | Start WorkerPool |
| `tokio::spawn` | `lib.rs:360` | Start socket server |
| `tokio::spawn` | `lib.rs:377` | Start 8s timeout safety net |
| `tokio::spawn` | `native_messaging_socket.rs:227,237` | Socket server accept loop, per-connection handler |
| `tokio::spawn` | `jobs/worker_channel.rs:47` | Event relay task |
| `tokio::spawn` | `jobs/workers/pool.rs:75` | Worker task per worker |
| `tokio::task::spawn_blocking` | `ocr_processor.rs:50` | OCR via Objective-C FFI |
| `tokio::task::spawn_blocking` | `pdf_text_processor.rs:41` | PDF text extraction via PDFKit |
| `tokio::task::spawn_blocking` | `pdf_metadata_processor.rs:40` | PDF metadata via PDFKit |
| `tokio::task::spawn_blocking` | `pdf_annotation_processor.rs:44` | PDF annotation extraction |
| `tokio::task::spawn_blocking` | `whisper_processor.rs:57` | Audio transcription via native whisper |
| `tokio::time::sleep` | `worker.rs:71` | Poll interval when queue empty (100ms) |
| `tokio::time::sleep` | `lib.rs:378` | 8s startup timeout |
| `tokio::sync::Notify` | `native_messaging_socket.rs:222` | Socket server shutdown signal |
| `tokio::sync::Mutex` | `pool.rs:26` | WorkerPool join handles |
| `tokio::net::UnixListener` | `native_messaging_socket.rs:207` | Socket server |
| `tokio::io::AsyncReadExt/AsyncWriteExt` | `native_messaging_socket.rs:16,262-263,347-349` | Socket I/O |
| `tokio::select!` | `native_messaging_socket.rs:229` | Accept + shutdown select |
| `std::sync::Mutex`/`RwLock` | Throughout | Synchronous locking (EventBus, ServiceRegistry, Indexer, VaultGraph) |

### Async Architecture Assessment

**Centralized**: All async work uses `tauri::async_runtime::spawn` or `tokio::spawn`.
There is a single Tokio runtime (Tauri's). No duplicate runtimes.

**Cancellation**: `CancellationToken` (`jobs/cancellation.rs`) provides cooperative
cancellation for the ProcessingPipeline and JobExecutor. Not connected to
`tokio::task::JoinHandle::abort()`.

**Graceful shutdown**: `WorkerPool.shutdown()` (`pool.rs:97-128`) uses
`ShutdownCoordinator` with a 30s drain timeout, then aborts handles. **Not called**
from the Tauri Exit event.

**Backpressure**: `BackpressureController` (`jobs/workers/backpressure.rs`) tracks
pending/running job counts, provides throttling and full-queue detection.

### Readiness for Capability Async Needs

| Capability | Async Needs | Readiness |
|---|---|---|
| **Syncthing** | Async process I/O (stdout/stderr), long-running daemon, graceful shutdown | **NOT READY** — no `tokio::process` usage. `src-tauri` has `"full"` features so tokio::process would compile, but no abstraction exists. |
| **Harper** | CPU-bound analysis | **READY** — `spawn_blocking` pattern is established. Can implement as `Processor` or `JobExecutor`. |
| **ACP Client** | Async stdin/stdout JSON-RPC, subprocess lifecycle, cancellation | **NOT READY** — no `tokio::process` or async I/O for subprocess pipes. `tokio::select!` and `tokio::sync::Notify` patterns exist but would need new subprocess integration. |

---

## Trait Architecture Analysis

### Existing Extensibility Traits

| Trait | File | Purpose | Implementors |
|---|---|---|---|
| `Processor` | `processing/processor.rs:84` | Content processing step in pipeline | `ContentClassifier`, `DuplicateDetector`, `MetadataExtractor`, `MetadataEnricher`, `EmbeddingGenerator`, `SemanticEnricher`, `TimelineExtractor`, `OcrProcessor`, `PdfTextProcessor`, `PdfMetadataProcessor`, `PdfAnnotationProcessor`, `AiSummariser`, `WhisperProcessor`, `AutoFiler` (14 total, in `processing/processors/mod.rs`) |
| `CaptureHandler` | `capture/handler.rs:37` | Routes capture source to KnowledgeObject creation | `BrowserCaptureHandler`, `ClipboardHandler`, `ScreenshotHandler`, `FileDropHandler`, `WatchFolderHandler`, `SafariReaderHandler`, `YouTubeCaptureHandler`, `GitHubRepositoryHandler`, `EmailCaptureHandler`, `BookmarkCaptureHandler`, `ArticleCaptureHandler` (11 handlers in `build_default_capture_engine`) |
| `JobExecutor` | `jobs/workers/executor.rs:15` | Bridges WorkerPool to processing logic | `PipelineExecutor` (production), `NoopExecutor` (test), `FallbackExecutor` (test) |
| `Queue` | `jobs/queue.rs:16` | Storage backend for job queue | `DurableJobQueue` (production) |
| `Lifecycle` | `registry/lifecycle.rs:202` | Service initialization/startup/shutdown hooks | No implementors — used only as documentation of the interface |
| `PluginLifecycleObserver` | `plugin/lifecycle.rs:79` | React to plugin lifecycle events | No implementors |

### Trait Analysis

**`Processor` trait**: Clean interface — `name()`, `process(context, progress, cancellation)`,
`supports(object_type)`. Async. No dependencies on infrastructure beyond the three
parameters. **Ideal for Harper** — a `HarperProcessor` can be added to the pipeline
by registering `Arc::new(HarperProcessor)` in `build_standard_pipeline()`.

**`CaptureHandler` trait**: Clean interface — `name()`, `source()`, `async capture()`.
Returns `Option<CaptureResult>` (None = handler can't handle this request). **Ideal for
new capture sources** — a `SyncthingCaptureHandler` or `AcpCaptureHandler` could be added
to `build_default_capture_engine()`.

**`JobExecutor` trait**: **The most important trait for capability integration.** It
bridges `WorkerPool` (infrastructure layer) to arbitrary processing logic (business layer).
A `SyncthingExecutor` or `AcpExecutor` implementing `JobExecutor` would be registered in
`ExecutorRegistry` and automatically dispatched by workers.

**`Lifecycle` trait**: Defined but **unused** — no service implements it. Services are
started via `tauri::async_runtime::spawn` directly in `lib.rs:setup()`, not through
`initialize()` / `start()` hooks. **Major gap**: the lifecycle abstraction exists but is
not enforced.

**`Queue` trait**: Well-designed — `enqueue()`, `dequeue()`, `cancel()`, `retry()`,
`reschedule()`, `report_progress()`. `DurableJobQueue` is the only implementation. A
`SubprocessQueue` or `CapabilityQueue` could implement this for capability-specific job
types.

### Readiness for Each Capability

| Capability | Reuseable Traits | Readiness |
|---|---|---|
| **Syncthing** | `JobExecutor` (for sync jobs), `Queue` (for task management), `Lifecycle` (for daemon lifecycle) | **EXTENDABLE** — need `JobExecutor` impl + `tokio::process` integration |
| **Harper** | `Processor` (add to pipeline), `JobExecutor` (for standalone analysis) | **READY** — `HarperProcessor` can be added to `build_standard_pipeline()` |
| **ACP Client** | `JobExecutor` (for agent execution), `Queue` (for job scheduling), `Lifecycle` (for process lifecycle) | **EXTENDABLE** — need `JobExecutor` impl + async process integration |

---

## Processing Pipelines Analysis

### Three Pipeline Layers

1. **Capture Pipeline** (`CaptureEngine` + `CaptureHandler`):
   - `CaptureEngine::ingest(request)` → routes to `CaptureHandler::capture()` → enqueues `Job` → publishes `ItemCaptured` event
   - 11 handlers in `build_default_capture_engine()` (`crates/nabu-core/src/capture/engine.rs:155-181`)
   - Each handler: `BrowserCaptureHandler`, `ClipboardHandler`, `ScreenshotHandler`, `FileDropHandler`, `WatchFolderHandler`, `SafariReaderHandler`, `YouTubeCaptureHandler`, `GitHubRepositoryHandler`, `EmailCaptureHandler`, `BookmarkCaptureHandler`, `ArticleCaptureHandler`
   - Result: `KnowledgeObject` → enqueued as `Job` with `JobType` determined by `object_type_to_job_type()`

2. **Processing Pipeline** (`ProcessingPipeline` + `Processor`):
   - `pipeline.run(object, progress, cancellation)` — runs all 14 processors in order
   - Each processor: `ContextClassifier` → `DuplicateDetector` → `TimelineExtractor` → `MetadataExtractor` → `MetadataEnricher` → `OcrProcessor` → `PdfTextProcessor` → `PdfMetadataProcessor` → `PdfAnnotationProcessor` → `WhisperProcessor` → `EmbeddingGenerator` → `SemanticEnricher` → `AiSummariser` → `AutoFiler`
   - Ordering constants defined in `processing/pipeline.rs:274-289`
   - `register()` / `register_at()` for dynamic insertion

3. **Job Pipeline** (`DurableJobQueue` + `WorkerPool` + `ExecutorRegistry` + `PipelineExecutor`):
   - `DurableJobQueue` (file-backed, priority-ordered) ← `CaptureEngine` enqueues jobs
   - `WorkerPool` (4 workers) ← `pool.start()` spawns workers that pull from queue
   - `ExecutorRegistry` ← `PipelineExecutor` registered under 4 processor names (`lib.rs:108-117`)
   - `PipelineExecutor` ← implements `JobExecutor`, runs `ProcessingPipeline`, saves to `StorageManager`

### Pipeline Flow

```
CaptureEngine::ingest(request)
  → CaptureHandler::capture() → KnowledgeObject + JobType
  → Job::new(job_type, payload, processor_name)
  → DurableJobQueue::enqueue(job)
  → Worker (tokio::spawn, lib.rs:349)
  → ExecutorRegistry::get(processor_name) → PipelineExecutor
  → PipelineExecutor::execute(job)
    → ProcessingPipeline::run(object, progress, cancellation)
    → StorageManager::save(result.object)
    → publishes ITEM_STORED
      → Indexer::index_object(object)
      → VaultGraph::add_node(object)
```

### Capability Integration Points

| Capability | Pipeline Integration |
|---|---|
| **Syncthing** | Could integrate at Job pipeline level: `SyncthingExecutor` implements `JobExecutor`, registered in `ExecutorRegistry` under a new `JobType::Sync`. Workers would automatically dispatch sync jobs. Syncthing daemon lifecycle managed separately via `Lifecycle`. |
| **Harper** | Could integrate at Processing pipeline level: `HarperProcessor` implements `Processor`, registered in `ProcessingPipeline::register()`. Would run as step 15 in the pipeline chain. |
| **ACP Client** | Could integrate at Job pipeline level: `AcpExecutor` implements `JobExecutor`, registered under a new `JobType::Custom("acp")`. ACP agent lifecycle (spawn/monitor/shutdown) managed via a new `Lifecycle`-implementing service. |

### Pipeline Modularity

**Assessment: Highly modular.** The three-layer architecture (Capture → Queue → Processing)
with trait-based interfaces (`CaptureHandler`, `Processor`, `JobExecutor`, `Queue`)
means new capabilities can plug into the existing flow without modifying core infrastructure.
The only missing piece is the **lifecycle management** — services are started ad-hoc in
`lib.rs:setup()` rather than through the `Lifecycle` trait's `start()` method.

---

## Dependency Direction

### Crate Structure

```
root workspace (Cargo.toml)
├── crates/nabu-core/     (standalone, path dependency only)
│   ├── Cargo.toml (no Tauri, no UI deps)
│   └── src/
│       ├── capture/      (CaptureEngine, CaptureHandler)
│       ├── diagnostics/    (tracing init, performance monitor)
│       ├── event_bus/    (EventBus<PipelineEvent>)
│       ├── graph/        (VaultGraph, incremental, persistence, recovery, serializer)
│       ├── history/      (HistoryManager — universal undo/redo)
│       ├── indexer/      (inverted index, persist/load)
│       ├── jobs/         (DurableJobQueue, WorkerPool, JobExecutor, Cancellation)
│       ├── models/       (KnowledgeObject, ObjectType, ObjectContent, etc.)
│       ├── native/       (macOS: vision, pdfkit, whisper, screenshot)
│       ├── pipeline_migration/  (PipelineExecutor)
│       ├── plugin/       (CapabilityRegistry, PluginManager, FeatureRegistry, etc.)
│       ├── processing/   (ProcessingPipeline, Processor trait, 14 processors)
│       ├── registry/     (ApplicationContext, ServiceRegistry, LifecycleManager)
│       └── storage/      (StorageManager)
│
└── src-tauri/           (Tauri app, depends on nabu-core)
    ├── Cargo.toml (tauri 2.11.5, rfd, tokio full)
    ├── src/
    │   ├── lib.rs     (build_application_context, run, invoke_handler)
    │   ├── commands.rs (Tauri IPC commands)
    │   ├── history.rs  (undo/redo, trash, note/file ops)
    │   ├── recovery.rs (session, versions, crash recovery)
    │   ├── settings.rs (SettingsStore, AppSettings)
    │   ├── native_messaging.rs (Message type, NativeMessagingHost)
    │   ├── native_messaging_socket.rs (Unix socket server)
    │   └── bin/native_messaging_host.rs (separate process binary)
    └── tauri.conf.json
```

### Key Observations

1. **`nabu-core` has zero Tauri dependency** — all platform abstractions are pure Rust with tokio. Future capability crates can depend on `nabu-core` inward without pulling in Tauri.

2. **`src-tauri` depends on `nabu-core`** as a path dependency (`nabu-core = { path = "../crates/nabu-core" }`). This is the only cross-crate dependency in the project.

3. **`crates/nabu-ui`** is a **standalone workspace** (separate `Cargo.toml` with `[workspace]` table). It does NOT depend on `nabu-core` or `src-tauri`. It's a Leptos CSR frontend compiled to wasm.

4. **`ApplicationBuilder`** in `registry/application.rs` imports `CaptureEngine`, `ProcessingPipeline`, `PerformanceMonitor` directly — creating tight coupling. A capability crate adding new services to the builder would require modifying `ApplicationBuilder`.

5. **`lib.rs`** (`src-tauri/src/lib.rs`) is the **actual composition root** — `build_application_context()` in `lib.rs` constructs the context, not `ApplicationBuilder::build()`. This function hardcodes all 12 service registrations.

6. **No `tokio::process` dependency** in `nabu-core/Cargo.toml` — the `tokio` features are limited to `["sync", "time", "rt", "macros"]`. Process management would require adding the `"process"` feature. `src-tauri/Cargo.toml` uses `"full"`, so it has access to `tokio::process`, but `nabu-core` does not.

---

## Reuse Opportunities by Future Capability

### Capability 1: Syncthing (P2P Sync via bundled sidecar)

#### Existing Infrastructure Available

| System | Symbol | How It's Used |
|---|---|---|
| **CapabilityRegistry** | `plugin/capability.rs:62-79` — `Capability::new("nabu", "sync", "Vault synchronization")` | Already declares `nabu:sync` as a built-in capability. Can be enabled/disabled via `capability_registry.enable("nabu:sync")`. |
| **FeatureRegistry** | `plugin/features.rs:148-174` — does NOT have a sync-specific flag, but has `register()` API | A `sync.enabled` flag can be added. `is_enabled()` controls whether the syncthing daemon starts. |
| **SettingsStore** | `settings.rs:254-390` — `AppSettings` has no syncthing fields, but `extra_settings` HashMap exists | `set_value("syncthing.config", json)` stores config. `get_value("syncthing.config")` reads it. |
| **ServiceRegistry** | `registry/mod.rs:66-107` — `register(key, Arc<T>)` + `resolve<T>(key)` | Register `SyncthingDaemon` as `"syncthing"` service, resolve it from commands. |
| **ExecutionContext** | `registry/context.rs:207-211` — `ctx.register(key, service)` + typed accessors | `ctx.register("syncthing", Arc::new(daemon))` during setup; add `ctx.syncthing()` accessor. |
| **JobQueue** | `jobs/queue.rs:63-603` — `enqueue()`, `dequeue()`, `mark_completed/failed` | Sync tasks (initial sync, delta sync, conflict resolution) can be enqueued as `JobType::Sync`. |
| **WorkerPool** | `jobs/workers/pool.rs:14-191` — 4 workers, `start()`, `shutdown()` | Workers automatically pull and execute sync jobs if a `SyncthingExecutor` is registered. |
| **JobExecutor** | `jobs/workers/executor.rs:15-23` — `execute(job, progress, cancellation)` | `SyncthingExecutor` implements this, registered in `ExecutorRegistry` under `"syncthing_processor"`. |
| **ExecutorRegistry** | `jobs/workers/executor.rs:29-64` — `register(name, executor)` | Register the executor: `executors.register("syncthing_processor", Arc::new(syncthing_executor))`. |
| **EventBus** | `event_bus/bus.rs:10-128` — `subscribe()`, `publish()` | Subscribe to `ITEM_STORED` for incremental sync triggers. Publish `SyncCompleted` / `SyncConflict` events. |
| **LifecycleManager** | `registry/lifecycle.rs:94-190` — `transition_to(stage)` | `SyncthingDaemon` can implement `Lifecycle` trait for `initialize()` (validate config) / `start()` (spawn daemon) / `shutdown()` (graceful stop). |
| **ShutdownCoordinator** | `jobs/workers/shutdown.rs` — 30s drain timeout | Same pattern can guard syncthing daemon shutdown. |

#### What's Missing

| What's Needed | Why |
|---|---|
| **`tokio::process::Command`** | `nabu-core` Cargo.toml:10 only has `tokio` with `["sync", "time", "rt", "macros"]` — no `"process"` feature. Need to add it for async subprocess management. |
| **Subprocess lifecycle manager** | No abstraction for spawning, monitoring, and gracefully shutting down an external process. `std::process::Command` is used fire-and-forget in `commands.rs:378-858`. |
| **Tauri sidecar/bundler integration** | No `tauri.bundle.binaries` or `tauri.bundle.active` configuration in `tauri.conf.json` to bundle the syncthing binary. No Tauri sidecar sidecar resource path resolution. |
| **Subprocess I/O bridging** | No `tokio::io::AsyncRead/AsyncWrite` usage for subprocess stdin/stdout/stderr. Only sync I/O in `native_messaging.rs:12-9`. |
| **Graceful shutdown wiring** | The Tauri `Exit` event (`lib.rs:402-412`) only calls `mark_clean_exit()`. Does not call `ctx.shutdown()` or `WorkerPool.shutdown()`. A `SyncthingDaemon` would be orphaned on exit. |

#### Recommended Integration Path

1. Add a `SubprocessCapability` to `nabu-core` (new module: `core/src/capabilities/process.rs`)
   - Implements `JobExecutor` for sync tasks
   - Uses `tokio::process::Command` with `Stdio::piped()` for async I/O
   - Implements `Lifecycle` trait for start/stop
   - Subscribes to `EventBus` for config changes

2. Register in `build_application_context()`:
   ```rust
   let syncthing = Arc::new(SyncthingCapability::new(vault_path.clone()));
   ctx.register("syncthing", syncthing.clone());
   executors.register("syncthing_processor", syncthing.clone());
   ```

3. Add shutdown to Tauri `Exit` event:
   ```rust
   if let tauri::RunEvent::Exit = event {
       if let Some(ctx) = app_handle.try_state::<ApplicationContext>() {
           ctx.shutdown();
           if let Some(pool) = ctx.worker_pool() { pool.shutdown().await; }
       }
   }
   ```

4. Add syncthing binary to `tauri.conf.json` bundle configuration.

#### Status: **NOT READY** — requires process management abstraction (moderate effort)

### Capability 2: Harper (Grammar & Writing Analysis)

#### Existing Infrastructure Available

| System | Symbol | How It's Used |
|---|---|---|
| **Processor trait** | `processing/processor.rs:84-102` — `name()`, `process(context, progress, cancellation)`, `supports(object_type)` | Harper can be `HarperProcessor` implementing this trait. Add to pipeline via `build_standard_pipeline()`. |
| **ProcessingPipeline** | `processing/pipeline.rs:48-90` — `register()`, `register_at()`, `run()` | Add `pipeline.register(Arc::new(HarperProcessor))` in `build_standard_pipeline()` (after `MetadataEnricher`, before `OcrProcessor`). |
| **Processor ordering constants** | `processing/pipeline.rs:274-289` — `CONTENT_CLASSIFIER`, `METADATA_EXTRACTOR`, etc. | Add `HARPER_PROCESSOR: usize = <appropriate position>`. |
| **JobExecutor trait** | `jobs/workers/executor.rs:15-23` | If Harper analysis is heavy and should be async, implement as `JobExecutor` instead (or in addition). |
| **ProgressReporter** | `jobs/workers/progress.rs` — `set_progress()`, `set_message()` | Harper can report grammar-check progress (e.g., "analyzing paragraph 3/12"). |
| **CancellationToken** | `jobs/cancellation.rs` — `is_cancelled()` | Harper processing can check cancellation between paragraphs. |
| **tokio::task::spawn_blocking** | Used in `pdf_text_processor.rs:41`, `ocr_processor.rs:50`, `whisper_processor.rs:57` | If Harper is linked as a native library (not FFI), wrap its blocking call in `spawn_blocking`. |
| **SettingsStore** | `settings.rs:297-308` — `update()` / `get()` | `AppSettings` has `spell_check: bool` (line 34). Add `harper_enabled`, `harper_language` via `extra_settings` or new `AppSettings` fields. |
| **EventBus** | `event_bus/bus.rs` — `publish()` / `subscribe()` | Publish `HarperAnalysisCompleted` events. Subscribe to `INDEX_UPDATED` for post-index grammar checks. |
| **StorageManager** | `storage/manager.rs:142-193` — `save()` | After Harper adds suggestions/annotations to a `KnowledgeObject`, save updated object (triggers `ITEM_STORED` → indexer + graph update). |
| **FeatureRegistry** | `plugin/features.rs:75-97` — `is_enabled()` / `enable()` | Create a `writing.analysis` feature flag, gate Harper processing behind it. |
| **PerformanceMonitor** | `diagnostics/performance.rs` — metrics | Track Harper analysis duration. |

#### What's Missing

| What's Needed | Why |
|---|---|
| **Harper integration** | Harper needs to be linked as a Rust crate or called via FFI. No `harper` dependency in `Cargo.toml`. Would need to add to `crates/nabu-core/Cargo.toml` or create a wrapper. |
| **`LanguageDiagnostics` event type** | The `PipelineEvent` enum (`event_bus/events.rs:8-29`) has no variant for diagnostics/lint results. Need to add `ItemAnalyzed { object_id, diagnostics }` or similar. |
| **Frontend IPC command** | Need a `#[tauri::command]` to fetch Harper diagnostics for a note. No existing command does this. |

#### Recommended Integration Path

1. Add `HarperProcessor` implementing `Processor` trait:
   ```rust
   pub struct HarperProcessor;
   impl Processor for HarperProcessor {
       fn name(&self) -> &'static str { "harper_processor" }
       async fn process(&self, ctx: &ProcessingContext, progress: ProgressReporter, cancellation: CancellationToken) -> ProcessingResult {
           // Run Harper analysis, add diagnostics to object's custom_properties
       }
   }
   ```

2. Register in `build_standard_pipeline()`:
   ```rust
   pipeline.register(Arc::new(HarperProcessor));
   ```

3. Add `HarperAnalysis` event variant to `PipelineEvent` if real-time UI updates are needed.

4. Add settings: `harper_enabled: bool` to `AppSettings`, `harper_language: String` to `extra_settings`.

#### Status: **READY** — only needs Harper linking and a `Processor` impl (low effort)

### Capability 3: ACP Client (Agent Client Protocol)

#### Existing Infrastructure Available

| System | Symbol | How It's Used |
|---|---|---|
| **JobExecutor trait** | `jobs/workers/executor.rs:15-23` — `execute(job, progress, cancellation) -> JobResult<Job>` | `AcpExecutor` implements this. Registered in `ExecutorRegistry` under `"acp_executor"`. Workers automatically dispatch ACP jobs. |
| **ExecutorRegistry** | `jobs/workers/executor.rs:29-64` — `register(name, executor)` | `executors.register("acp_executor", Arc::new(acp_executor))` in `lib.rs:108-117` pattern. |
| **Job model** | `jobs/job.rs:11-65` — UUID, JobType, payload, progress, cancellation token, tags | `JobType::Custom("acp")` with `payload: { "prompt": "...", "agent_id": "..." }`. `Job::with_object_id()` links to KnowledgeObject. |
| **JobQueue** | `jobs/queue.rs:14-61` — `enqueue()`, `dequeue()`, `cancel()` | ACP agent tasks enqueued as jobs. `cancel()` for stopping agents. |
| **CancellationToken** | `jobs/cancellation.rs` — `is_cancelled()`, `cancel()` | Cooperative cancellation for long-running agent conversations. |
| **EventBus** | `event_bus/bus.rs` — `publish()` / `subscribe()` | Publish `ItemProcessingStarted` / `ItemProcessingProgress` / `ItemProcessingCompleted` events from ACP executor. Subscribe to `ITEM_STORED` for context injection. |
| **PipelineEvent** | `event_bus/events.rs:8-29` — 9 event variants | Reuse `ItemProcessingProgress` for streaming agent output to UI (via event→Tauri event bridge). |
| **SettingsStore** | `settings.rs:254-390` — `extra_settings`, `update()` | Store ACP connection strings, agent configs: `set_value("acp.agents", json)`. |
| **FeatureRegistry** | `plugin/features.rs:75-97` | `register("acp.client", ...)` to gate feature. |
| **PluginCapability** | `plugin/capability.rs:14-48` — `Capability { namespace, name, description, required }` | Register `com.nabu:acp` capability. |
| **WorkerPool** | `jobs/workers/pool.rs:14-191` — `start()`, `shutdown()`, `health()` | Workers pull ACP jobs from queue automatically once executor is registered. |
| **ProgressReporter** | `jobs/workers/progress.rs` — `set_progress()`, `set_message()` | Report agent execution progress (e.g., "step 3/5"). |
| **BackpressureController** | `jobs/workers/backpressure.rs` — throttling | Prevent too many concurrent ACP agents. |
| **tokio async runtime** | `src-tauri/Cargo.toml:34` — `tokio` with `["full"]` | Full async I/O for JSON-RPC over stdin/stdout. `tokio::io::AsyncReadExt` / `AsyncWriteExt` / `select!` available in Tauri layer. |
| **JobQueue IPC commands** | `commands.rs:277-282` — `queue_get_all`, `queue_set_status`, `queue_set_priority`, `queue_set_progress`, `queue_batch_set_status`, `queue_archive_completed` | Frontend can display ACP agent job status, progress, and control (cancel, reprioritize). |

#### What's Missing

| What's Needed | Why |
|---|---|
| **`tokio::process::Command`** in `nabu-core` | For spawning ACP agent subprocesses. `nabu-core/Cargo.toml:10` only has tokio with `["sync", "time", "rt", "macros"]` — no `"process"` or `"io-util"` features. |
| **Async pipe I/O for subprocess** | For JSON-RPC over stdin/stdout. `tokio::io::AsyncReadExt` / `AsyncWriteExt` / `AsyncBufReadExt` needed. Only used in `native_messaging_socket.rs` (Tauri layer, not core). |
| **JSON-RPC infrastructure** | No JSON-RPC client library or abstraction. Would need to add (e.g., `jsonrpc-derive` or manual JSON serialization). |
| **Subprocess lifecycle management** | No abstraction for spawning/managing external processes. Same gap as Syncthing. |
| **Event streaming to frontend** | `EventBus` is internal-only. No bridge to forward events to Leptos UI via Tauri events (`window.emit_all()`). The frontend polls via IPC commands, not events. |
| **Capability → command routing** | No per-capability command namespace. New ACP commands would need to be added to the flat `invoke_handler` list in `lib.rs:201-311`. |
| **`JobType::Acp` variant** | `jobs/job.rs:152-159` has `Ocr`, `Whisper`, `PdfTextExtraction`, `MetadataExtraction`, `Processing`, `Sync`, `Embedding`, `Custom(String)`. Could use `JobType::Custom("acp")` but a dedicated variant would be cleaner. |

#### Recommended Integration Path

1. Add a `SubprocessCapability` to `nabu-core` (shared with Syncthing):
   ```rust
   // crates/nabu-core/src/capabilities/process.rs
   pub struct SubprocessManager { ... }
   impl SubprocessManager {
       async fn spawn(&self, cmd: CommandSpec) -> SubprocessHandle
       async fn write_stdin(&self, handle: &SubprocessHandle, data: &[u8]) -> Result<()>
       async fn read_stdout(&self, handle: &SubprocessHandle) -> Option<String>
   }
   ```

2. Implement `AcpExecutor` as `JobExecutor`:
   - On `execute()`: spawn ACP agent subprocess via `SubprocessManager`, write JSON-RPC request to stdin
   - Report progress via `ProgressReporter` as agent sends partial results
   - Read JSON-RPC responses from stdout via `tokio::io::AsyncReadExt`
   - Check `CancellationToken` periodically for cancellation
   - Return `JobResult<Job>` with completion metadata

3. Register in `build_application_context()`:
   ```rust
   let acp_executor = Arc::new(AcpExecutor::new(event_bus.clone()));
   executors.register("acp_executor", acp_executor);
   ctx.register("acp_executor", acp_executor);
   ```

4. Add ACP IPC commands to `invoke_handler`:
   ```rust
   crate::commands::acp_list_agents,
   crate::commands::acp_execute_prompt,
   crate::commands::acp_cancel_agent,
   ```

5. Add an event bridge: `EventBus` → `window.emit_all("nabu-event", ...)` so the
   Leptos frontend can subscribe to streaming agent output without polling.

#### Status: **NOT READY** — requires subprocess management + event streaming bridge (moderate effort)

---

## Missing Infrastructure

### 1. Subprocess Management (Critical)

**What exists**: `std::process::Command` used fire-and-forget in `commands.rs:378-858`.
**What's needed**: `tokio::process::Command` with async I/O, lifecycle management,
graceful shutdown integration.

**Recommended location**: New module `crates/nabu-core/src/capabilities/process.rs`,
implementing `JobExecutor` + `Lifecycle` traits.

### 2. Graceful Shutdown Hook (High)

**What exists**: `WorkerPool.shutdown()` with 30s drain (`pool.rs:97-128`).
`ApplicationContext.shutdown()`. Neither is called from Tauri's exit path.

**What's needed**: Wire `RunEvent::Exit` to call shutdown on all services.

### 3. Event Streaming to Frontend (High)

**What exists**: `EventBus<PipelineEvent>` with synchronous handlers.
**What's needed**: Bridge `EventBus` → `window.emit_all()` or WebSocket/Tauri event
forwarding for real-time frontend updates.

### 4. Capability Lifecycle Integration (Medium)

**What exists**: `PluginManager` (foundation only, no code loading),
`CapabilityRegistry` (metadata only), `Lifecycle` trait (unused).
**What's needed**: A `CapabilityRuntime` that binds `ServiceRegistry` entries to
`CapabilityRegistry` entries with start/stop lifecycle hooks.

### 5. Feature Flag Propagation (Low)

**What exists**: `FeatureRegistry` in `plugin/features.rs` with `is_enabled()`/`enable()`.
**Gap**: Not connected to `AppSettings` or `extra_settings` persistence. `AppSettings.experimental_features` is the only feature toggle in use.
**What's needed**: Sync `FeatureRegistry` state to `extra_settings["featureToggles"]` on changes.

### 6. JobType Extension (Trivial)

**What exists**: `JobType` enum with variants `Ocr`, `Whisper`, `PdfTextExtraction`,
`MetadataExtraction`, `Processing`, `Sync`, `Embedding`, `Custom(String)`.
**What's needed**: Add `Acp` variant (or use `Custom("acp")`). Register executor
under appropriate name in `ExecutorRegistry`.

---

## Architectural Risks

### Risk 1: `ApplicationBuilder` divergence from `build_application_context()`

`ApplicationBuilder::build()` (`registry/application.rs:336-385`) only knows about
`event_bus`, `pipeline`, `capture_engine`, and `performance_monitor`. The actual
service wiring in `build_application_context()` (`lib.rs:55-180`) manually registers
12 services including `storage_manager`, `job_queue`, `worker_pool`, `vault_graph`,
`indexer`, `history_manager` — none of which are handled by `ApplicationBuilder`.

**Impact**: If `ApplicationBuilder` is used as the canonical construction path (e.g.,
by tests or future capability crates), services will be missing. The builder needs
to be extended with `.with_storage_manager()`, `.with_job_queue()`, `.with_worker_pool()`,
`.with_vault_graph()`, `.with_indexer()`, `.with_history_manager()` methods.

### Risk 2: `GraphEventBridge` is dead code in production

`GraphEventBridge` (`graph/incremental/event_wiring.rs`) provides incremental graph
updates via `ITEM_STORED` → `IncrementalUpdateEngine` → `VaultGraph`. It is fully
implemented with tests. But in `lib.rs:162-177`, the `ITEM_STORED` subscription
calls `indexer.index_object()` and `graph.add_node()` directly — bypassing the
incremental engine, change log, region analysis, and batch processing.

**Impact**: The incremental update infrastructure (10+ modules in
`graph/incremental/`) is compiled but unused in production. If capability crates
try to use it, they'll find it's not wired.

### Risk 3: FeatureRegistry is disconnected from the runtime

`FeatureRegistry` (`plugin/features.rs`) is only used by `PluginManager`
(`plugin/manager.rs:59-70`), which is **never instantiated** in the Tauri app.
The `AppSettings.experimental_features: bool` (settings.rs:107) is the de facto
feature gate. The `FeatureRegistry`'s staged flags (`plugin.wasm`, `plugin.external`,
etc.) have no runtime effect.

**Impact**: Capability toggles can't be managed through the existing feature flag
system. Each capability would need its own settings field.

### Risk 4: Synchronous EventBus limits event subscribers

`EventBus::subscribe()` takes `Fn(&Events) + Send + Sync + 'static` — synchronous
closures. Subscribers that need to perform async work (e.g., network I/O for Syncthing
status reporting) must spawn their own `tokio::spawn` inside the closure. This
can lead to thread pool exhaustion if many subscribers spawn tasks.

**Impact**: Capability event handlers will need careful async task management to
avoid blocking the event bus.

### Risk 5: No event bridge to frontend

The `EventBus` is internal-only. The frontend (Leptos) communicates with the backend
exclusively through Tauri `#[tauri::command]` IPC calls. There is no mechanism to
push events from the backend to the frontend in real-time.

**Impact**: Capabilities that produce streaming output (ACP agent responses,
Syncthing sync progress) can only be consumed via polling. A WebSocket or Tauri
event bridge would be needed for real-time updates.

### Risk 6: `Lifecycle` trait is unused

The `Lifecycle` trait (`registry/lifecycle.rs:202-230`) with `initialize()`, `start()`,
`shutdown()` is defined but **no service implements it**. Services are started via
ad-hoc `tokio::spawn` calls in `lib.rs:setup()`. Shutdown is not handled at all.

**Impact**: New capabilities implementing `Lifecycle` will work, but their lifecycle
hooks won't be called unless the `ApplicationBuilder` or `build_application_context()`
is extended to invoke them.

---

## Recommended Preparation Work

### Phase 1: Capability Framework Foundation (Required before any capability implementation)

**1. Add `tokio::process` and `tokio::io` features to `nabu-core`**
- File: `crates/nabu-core/Cargo.toml:10`
- Change: `tokio = { version = "1", features = ["sync", "time", "rt", "macros"] }` → add `"process"`, `"io-util"`, `"io-std"`, `"fs"`

**2. Create `SubprocessCapability` in `nabu-core`**
- New file: `crates/nabu-core/src/capabilities/process.rs`
- Implement `JobExecutor` trait → integrates with existing WorkerPool/ExecutorRegistry
- Use `tokio::process::Command` for async subprocess spawning
- Implement `Lifecycle` trait for start/stop management
- Support `Stdio::piped()` for stdin/stdout/stderr streaming
- Provide `SubprocessHandle` for process tracking + cancellation

**3. Wire graceful shutdown in Tauri exit handler**
- File: `src-tauri/src/lib.rs:402-412`
- Add: `ctx.shutdown()`, `pool.shutdown().await`, subprocess cleanup in `RunEvent::Exit`

**4. Create `CapabilityRuntime` in `nabu-core`**
- New file: `crates/nabu-core/src/capabilities/runtime.rs`
- Bridges `ServiceRegistry` ↔ `CapabilityRegistry`
- Manages service lifecycles (initialize/start/stop) based on capability enable state
- Can be registered as a service itself in `ApplicationContext`

### Phase 2: Event Streaming to Frontend (Required for real-time capability UI)

**5. Add EventBus → Tauri event bridge**
- New: Bridge `EventBus<PipelineEvent>` to `window.emit_all("nabu-event", event_json)`
- File: `src-tauri/src/lib.rs:setup()` closure
- Enables real-time updates for ACP agent output, Syncthing progress, etc.

### Phase 3: Builder Alignment (Cleanup)

**6. Extend `ApplicationBuilder` to match `build_application_context()`**
- File: `crates/nabu-core/src/registry/application.rs:256-385`
- Add `.with_storage_manager()`, `.with_job_queue()`, `.with_worker_pool()`,
  `.with_vault_graph()`, `.with_indexer()`, `.with_history_manager()` methods
- Allows future capability crates to use the builder pattern without modifying `lib.rs`

**7. Wire `GraphEventBridge` in production**
- File: `src-tauri/src/lib.rs:155-177`
- Replace direct `graph.add_node()` + `indexer.index_object()` calls with
  `GraphEventBridge::wire_incremental_graph_updates()` from `graph/incremental/event_wiring.rs`

### Phase 4: Capability-Specific Extensions

**8. For Harper**: Add `HarperProcessor` implementing `Processor` trait → register in `build_standard_pipeline()`. (Trivial — <20 lines)

**9. For Syncthing**: Extend `SubprocessCapability` for daemon mode. Add `SyncCompleted`/`SyncConflict` event variants to `PipelineEvent`. Add syncthing binary to `tauri.conf.json` bundle.

**10. For ACP Client**: Implement `AcpExecutor` as `JobExecutor` + `SubprocessCapability`. Add ACP IPC commands to `invoke_handler`. Add `acp_agents` config to `AppSettings.extras`.

---

## Conclusion: If Capability Platform Development Started Tomorrow

The **smallest set of architectural improvements** required before implementing Phase 1
(Capability Framework Foundation) is:

1. **Add `tokio` process/io features to `nabu-core`** (Cargo.toml change — 1 line)
2. **Create `SubprocessCapability`** implementing `JobExecutor` + `Lifecycle` in `nabu-core` (new module — ~200 lines)
3. **Wire graceful shutdown** in the Tauri exit handler (`lib.rs:402-412` — ~10 lines)
4. **Create `CapabilityRuntime`** bridging `ServiceRegistry` ↔ `CapabilityRegistry` (new module — ~150 lines)

These four changes would provide:
- Process spawning, monitoring, and lifecycle management for Syncthing and ACP agents
- Integration with the existing WorkerPool/JobQueue via `JobExecutor`
- Proper startup/shutdown hooks via `Lifecycle`
- Capability enable/disable driving service lifecycle via `CapabilityRuntime`

**The existing systems that should become the foundation** are:

| System | Role in Capability Platform |
|---|---|
| `ServiceRegistry` | The service container — all capabilities register here |
| `CapabilityRegistry` | The capability catalog — what the system can do, with enable/disable |
| `JobExecutor` trait | The contract for capability job execution — integrates with WorkerPool |
| `JobQueue` + `WorkerPool` | The async execution backbone — capabilities get jobs dequeued by workers |
| `EventBus<PipelineEvent>` | The communication backbone — capabilities publish/subscribe to events |
| `ApplicationContext` | The composition root — ties services together with lifecycle management |
| `Lifecycle` trait | The startup/shutdown contract — capabilities implement this for proper lifecycle |
| `SettingsStore` | The configuration store — capabilities read/write settings |

With these four preparation steps, all three representative capabilities can be
implemented by extending existing abstractions rather than introducing new ones:

- **Harper**: Implement `Processor` trait, register in `ProcessingPipeline` (2 lines of wiring)
- **Syncthing**: Implement `SubprocessCapability` + `JobExecutor`, register in `ApplicationContext`
- **ACP Client**: Implement `SubprocessCapability` + `JobExecutor`, register in `ApplicationContext`

---

*AUDIT_0.2 covered 15 feature areas with command registration verification. This audit (0.3)
focuses on capability platform readiness, analyzing 12 investigation areas against the three
future capability modules (Syncthing, Harper, ACP Client). The codebase is more sophisticated
than AUDIT_0.1 suggested — it has a full plugin foundation, service registry, and lifecycle
management system. The primary gap is process management, which blocks two of three capabilities.*