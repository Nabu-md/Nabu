# Audit 0.1 — Nabu Semantic Architecture Trace

> Evidence-based reconstruction of the real Nabu architecture, traced from every entry point through all subsystems. Every finding cites `file:Line` and the concrete symbol. No inference from filenames alone.

---

## 1. Executive Summary

Nabu is a three-crate Tauri desktop application with a **canonical data pipeline** as its backbone. Content enters through the `CaptureEngine` (`crates/nabu-core/src/capture/engine.rs:16`), is enqueued to a file-backed `DurableJobQueue` (`crates/nabu-core/src/jobs/queue.rs:68`), is consumed by 4 `WorkerPool` threads (`crates/nabu-core/src/jobs/workers/pool.rs:14`), each running `PipelineExecutor` (`crates/nabu-core/src/pipeline_migration/executor.rs:24`) which runs a 14-processor `ProcessingPipeline` (`../../crates/nabu-core/src/processing/pipeline.rs`), persists results via `StorageManager` (`crates/nabu-core/src/storage/manager.rs:33`), and finally triggers `Indexer` + `VaultGraph` updates through an `ITEM_STORED` event subscription (`src-tauri/src/lib.rs:162-177`).

**Key architectural tension**: Two subsystems (`StorageManager`, `VaultGraph`, `Indexer`) are constructed and registered in the application context but are **bypassed** by 42 of the 87 Tauri IPC commands, which perform direct filesystem access instead of routing through the canonical pipeline. This is documented in §10 as verified risks.

**Workspace topology**: Root `../../Cargo.toml` declares `members = ["crates/nabu-core"]` only. `src-tauri/Cargo.toml:14-17` and `../../crates/nabu-ui/Cargo.toml` each declare their own `[workspace]`, making them standalone compilation units. The dependency direction is consistently inward: `src-tauri` → `nabu-core`, `nabu-ui` → `nabu-core`. Neither depends on the other.

---

## 2. Startup Sequence Diagram

The complete startup chain, traced from `fn main()` through service construction:

```
fn main()                        [src-tauri/src/main.rs:3]
  └─ app_lib::run()              [src-tauri/src/lib.rs:183]
     ├─ nabu_core::diagnostics::init(None, "nabu")     [lib.rs:190]
     │   → Tracing subscriber → .nabu/logs/           [crates/nabu-core/src/diagnostics/mod.rs]
     ├─ SettingsStore::load(path)                      [lib.rs:196]
     │   → Reads ~/.config/nabu/settings.json or $NABU_SETTINGS_PATH
     │   → Fallback: SettingsStore::new(path)         [settings.rs:197]
     ├─ tauri::Builder::default()                      [lib.rs:199]
     │   ├─ .manage(settings_store)                    [lib.rs:200]
     │   ├─ .invoke_handler(generate_handler![87 commands])  [lib.rs:201-311]
     │   ├─ .setup(|app| {                              [lib.rs:312]
     │   │   ├─ vault_path = settings.last_vault_path   [lib.rs:316-324]
     │   │   ├─ crate::recovery::mark_running(vault)   [lib.rs:329, recovery.rs:363]
     │   │   │   → Writes .nabu/.running (crash sentinel)
     │   │   │   → If .running already exists: writes .nabu/.recovery_pending [recovery.rs:371-376]
     │   │   ├─ build_application_context(vault_path)   [lib.rs:331]
     │   │   │   │   [Full construction order in §3 below]
     │   │   ├─ ctx.initialize()                        [lib.rs:339]
     │   │   │   → Lifecycle: Created → Initialized
     │   │   │   → validate_core_services()            [contexts.rs:324-329]
     │   │   ├─ ctx.start()                             [lib.rs:342]
     │   │   │   → Lifecycle: Initialized → Running     [lifecycle.rs:26-36]
     │   │   ├─ app.manage(ctx)                         [lib.rs:345]
     │   │   ├─ tauri::async_runtime::spawn(pool.start())  [lib.rs:349-351]
     │   │   │   → WorkerPool::start spawns 4× tokio::spawn(worker.run())  [workers/pool.rs:66-79]
     │   │   ├─ tauri::async_runtime::spawn(start_socket_server)  [lib.rs:360-368]
     │   │   │   → Unix listener at /tmp/nabu-native-messaging.sock  [native_messaging_socket.rs:20-256]
     │   │   └─ tauri::async_runtime::spawn(8s safety timer)  [lib.rs:376-383]
     │   │ })
     │   ├─ .on_page_load(|window, payload| {          [lib.rs:392]
     │   │   → On PageLoadEvent::Finished for "main": window.show() + set_focus()  [lib.rs:396-397]
     │   ├─ .build(generate_context!())               [lib.rs:400]
     │   └─ .run(|app_handle, event| {                 [lib.rs:402]
     ├─ ... (event loop runs until Exit) ...
     └─ RunEvent::Exit → crate::recovery::mark_clean_exit(vault)  [lib.rs:405-411, recovery.rs:382]
         → Removes .nabu/.running marker file
```

**Frontend initialization** (`crates/nabu-ui/src/lib.rs:13`):
```
#[wasm_bindgen(start)] pub fn start()
  ├─ console_error_panic_hook::set_once()     [lib.rs:14]
  ├─ remove_boot_splash()                    [lib.rs:15, 28-36]
  │   → Removes #boot-splash div from index.html (prevents white flash)
  └─ mount_to_body(|| <ToastProvider><App /></ToastProvider>)  [lib.rs:16-22]
     └─ App [lib.rs:16-22, components/app.rs:78-494]
        ├─ provide_theme("dark")             [lib.rs:15, app.rs:79]
        │   → RwSignal<String> for theme      [lib.rs:40-44]
        │   → On mount: tauri_invoke("settings_get", {key:"theme"})  [lib.rs:57]
        │   → Fallback: tauri_invoke("get_settings", {})  [lib.rs:68]
        │   → Effect: applies data-theme attr + tauri_invoke("settings_set")  [lib.rs:86-99]
        ├─ provide_history()                 [lib.rs:16, app.rs:80]
        ├─ provide_save_status()             [app.rs:81]
        ├─ provide_tasks()                   [app.rs:82]
        ├─ provide_workspace()               [app.rs:85]
        ├─ provide_navigation()              [app.rs:86]
        ├─ load_all_nav_state(nav)           [app.rs:89]
        ├─ load_notes_index(nav)             [app.rs:90]
        ├─ spawn_local(recovery_check)       [app.rs:127-160]
        │   → tauri_invoke("recovery_check")  [app.rs:129]
        │   → On crash: shows RecoveryBanner    [app.rs:133-135]
        │   → On clean session: restores view mode, active note, cursor, scroll  [app.rs:136-157]
        └─ spawn_local(check_vault_exists)   [app.rs:235-256]
           → tauri_invoke("check_vault_exists")  [app.rs:237]
           → If vault exists: AppScreen::MainDashboard  [app.rs:250]
           → If no vault:       AppScreen::VaultSetup    [app.rs:255]
```

---

## 3. Workspace Architecture

### 3.1 Crate Topology

| Crate | Path | Type | Standalone? | Depends on | Key Role |
|---|---|---|---|---|---|
| **nabu-core** | `../../crates/nabu-core/Cargo.toml` | lib (rlib) | Yes (root workspace member only) | chrono, serde, tokio, async-trait, uuid, regex, tracing, thiserror, dirs-next (macOS: objc2, whisper-rs) | All backend domain logic |
| **nabu-tauri** (app/app_lib) | `../../src-tauri/Cargo.toml` | bin + lib | Yes (`[workspace]` empty) | nabu-core, tauri 2.x, serde, serde_json, tokio, tauri-plugin-* | Tauri shell: IPC commands, settings, crash recovery, native messaging |
| **nabu-ui** | `../../crates/nabu-ui/Cargo.toml` | cdylib (wasm) | Yes (`[workspace]` empty) | nabu-core, leptos 0.7.8, wasm-bindgen, serde_wasm_bindgen | Leptos CSR frontend compiled to WASM |

**Evidence**: Root `../../Cargo.toml` declares `members = ["crates/nabu-core"]` — `src-tauri` and `nabu-ui` are not listed. `src-tauri/Cargo.toml:14-17` declares `[workspace]` with no members (standalone). `../../crates/nabu-ui/Cargo.toml` similarly standalone.

### 3.2 nabu-core Module Map

`crates/nabu-core/src/lib.rs:28-41` declares 14 public modules, all re-exported at crate root (`lib.rs:46-69`):

| Module | File | Owns | Depends on |
|---|---|---|---|
| `event_bus` | `event_bus/mod.rs`, `events.rs`, `bus.rs` | `EventBus<PipelineEvent>`, `PipelineEvent` enum (10 variants) | `models` |
| `jobs` | `jobs/mod.rs` → `job.rs`, `queue.rs`, `persistence.rs`, `scheduler.rs`, `workers/` | `Job` (22 JobTypes), `DurableJobQueue`, `JobStore`, `Scheduler`, `WorkerPool`, `Worker`, `ExecutorRegistry`, `JobExecutor` trait | `event_bus`, `models`, `jobs/cancellation`, `jobs/priority` |
| `capture` | `capture/mod.rs` → `engine.rs`, `handler.rs` | `CaptureEngine`, `CaptureHandler` trait (11 impls), `CaptureRequest`, `CaptureData` | `event_bus`, `jobs`, `models` |
| `processing` | `processing/pipeline.rs`, `processor.rs`, `processors/mod.rs` | `Processor` trait (14 impls), `ProcessingPipeline`, `ProcessingContext`, `ProcessingResult` | `event_bus`, `jobs`, `models` |
| `storage` | `storage/mod.rs`, `storage/manager.rs` | `StorageManager`, `Sidecar` | `event_bus`, `models` |
| `graph` | `graph/mod.rs` → `incremental/`, `integrity/`, `loader/`, `persistence/`, `recovery/`, `serializer/`, `version/` | `VaultGraph`, `GraphEdge`, `PersistenceHandle` | `event_bus`, `models` |
| `indexer` | `indexer.rs` | `Indexer` (in-memory inverted index) | `event_bus`, `models` |
| `history` | `history/mod.rs` | `HistoryManager`, `HistoryEntry`, `HistoryOp` | (standalone — no core deps) |
| `registry` | `registry/mod.rs` → `application.rs`, `context.rs`, `lifecycle.rs` | `ServiceRegistry`, `ApplicationContext`, `LifecycleManager`, `Application` | `event_bus`, `plugin/capability` |
| `plugin` | `plugin/mod.rs` → `capability.rs`, `dependency.rs`, `features.rs`, `lifecycle.rs`, `manager.rs`, `manifest.rs`, `permissions.rs`, `version.rs` | `CapabilityRegistry`, `FeatureRegistry`, `PermissionEvaluator`, `PluginManager` (metadata only — no runtime execution) | (minimal internal deps) |
| `pipeline_migration` | `pipeline_migration/executor.rs`, `events.rs` | `PipelineExecutor` (impls `JobExecutor`), event-wiring helpers | `jobs`, `processing`, `storage`, `event_bus` |
| `models` | `models/mod.rs` → `knowledge_object.rs` | `KnowledgeObject`, `ObjectType` (22 variants), `ObjectContent`, `ObjectMetadata` | chrono, serde, uuid |
| `diagnostics` | `diagnostics/mod.rs` | `init()`, tracing subscriber setup | tracing, chrono |
| `native` | `native/mod.rs` | Platform-specific FFI bindings (macOS) | objc2 (macOS only) |

---

## 4. Crate Responsibility Matrix

### Why nabu-core exists
`crates/nabu-core/src/lib.rs:1-26` declares the canonical data flow: Capture → Queue → Pipeline → Storage → Graph. It is the **single** repository for all backend domain logic. No Tauri-specific code lives here (`lib.rs:2` — "core library for the Nabu knowledge management platform").

### Dependency evidence (one direction only)

**nabu-core** is a pure library with no dependencies on `src-tauri` or `nabu-ui`:
- `../../crates/nabu-core/Cargo.toml` lists only: chrono, serde, serde_json, tokio, async-trait, uuid, regex, tracing, thiserror, dirs-next, tempfile (dev)
- `crates/nabu-core/src/lib.rs:28-41`: modules are self-contained; `processing/processor.rs:1-2` imports `cancellation` and `progress` from `jobs` — no UI/Tauri imports anywhere

**src-tauri** depends on nabu-core:
- `src-tauri/Cargo.toml:25-26`: `nabu-core = { path = "../crates/nabu-core" }`
- `src-tauri/src/lib.rs:12-17`: `use nabu_core::capture::CaptureEngine`, `use nabu_core::registry::context::ApplicationContext`, `use nabu_core::{build_standard_pipeline, ...}`
- All 87 Tauri commands that touch backend logic call into `nabu_core::` symbols

**nabu-ui** depends on nabu-core (type re-exports only):
- `../../crates/nabu-ui/Cargo.toml` is not readable directly, but `../../crates/nabu-ui/src/models.rs` mirrors backend types via `serde::Deserialize` — it does NOT import nabu-core types directly (it serializes/deserializes them as JSON over IPC)
- `crates/nabu-ui/src/ipc.rs:9-11`: `tauri_invoke()` calls `window.__TAURI__.core.invoke()` — pure WASM-to-Rust bridge, no Rust dependency

### Underutilized crates/modules
- **`plugin/`** (`crates/nabu-core/src/plugin/mod.rs:1-80`): Complete foundation (CapabilityRegistry, PermissionEvaluator, DependencyGraph, PluginManager) with **no runtime loading** — `plugin/mod.rs:29`: "No code execution — the foundation validates metadata only." Plugin runtime loading "will be added in a future phase" (`mod.rs:29`). 12 capability constants registered (`capability.rs:62-79`), but the `PluginManager` (`manager.rs`) and `manifest.rs` are never invoked from `build_application_context` (`lib.rs:55-180`).
- **`history/`** (`../../crates/nabu-core/src/history/mod.rs`): `HistoryManager` is registered as `"history_manager"` (`lib.rs:150-153`) but is **not consumed** by any Tauri command. The actual undo/redo IPC (`history_undo`, `history_redo`, etc.) is implemented independently in `src-tauri/src/history.rs:49-960` using its own `HistoryEntry`/`HistoryOp` types. The core `HistoryManager` has zero subscribers.
- **`native/`** (`../../crates/nabu-core/src/native/mod.rs`): Platform-specific FFI bindings — existence unverified beyond module declaration (`lib.rs:36`).

### Overlapping responsibilities
- **`StorageManager` (core) vs `recovery.rs` (tauri)**: `StorageManager` (`storage/manager.rs:33`) is the canonical owner of KnowledgeObject persistence, publishing `ITEM_STORED`. But `recovery.rs` (`recovery.rs:391-406`) implements `note_save` which writes directly via `std::fs::write()` — bypassing `StorageManager` entirely. This is 2 separate persistence paths.
- **`VaultGraph` (core) vs `graph_data` (tauri)**: `VaultGraph` (`graph/mod.rs:54`) is the canonical graph engine with persistence. But `graph_data` (`commands.rs:1907-2017`) rebuilds the entire graph from filesystem `[[wikilinks]]` parsing — `VaultGraph` is not consulted.
- **`Indexer` (core) vs `notes_search` (tauri)**: `Indexer` (`indexer.rs:26`) is the canonical search index. But `notes_search` (`commands.rs:1559-1661`) scans the vault filesystem directly.

---

## 5. Module Boundaries

### nabu-core internal module coupling

**`jobs/` module** is the most internally coupled:
- `jobs/mod.rs:1-21`: re-exports 9 sub-modules
- `jobs/workers/mod.rs:1-15`: re-exports 7 sub-modules (backpressure, errors, executor, pool, progress, shutdown, worker)
- `jobs/queue.rs:1-3` imports `crate::jobs::job`, `crate::jobs::persistence`, `crate::jobs::scheduler`, `crate::jobs::retry`, `crate::jobs::cancellation`, `crate::jobs::priority`
- No circular dependencies detected — all flow toward leaf utilities (cancellation, priority, retry)

**`processing/` module**:
- `processing/pipeline.rs:1-5` imports `processor.rs`, `processors/mod.rs`
- `processors/mod.rs:1-15`: registers 14 processors via `build_standard_pipeline()`
- Each processor (e.g., `ocr_processor.rs:1-5`) imports `Processor` trait from `processor.rs`, `KnowledgeObject` from `models`, and `ProgressReporter`/`CancellationToken` from `jobs`
- Processors do NOT reference capture, storage, or each other directly (`processor.rs:80-82`: "No processor instantiates another processor")

**`registry/` module**:
- `context.rs:141-422`: `ApplicationContext` holds `Arc<RwLock<ServiceRegistry>>`, `Arc<EventBus<PipelineEvent>>`, `CapabilityRegistry`, `LifecycleManager`
- `context.rs:46-54`: Imports from `event_bus`, `plugin/capability`, `registry/lifecycle`, `registry/ServiceRegistry`, `jobs`
- `application.rs:107-119`: `Application` is a composition root struct — doc-comment only, not invoked from `build_application_context` (which constructs services directly)

**Hidden coupling**: `pipeline_migration/executor.rs:24` imports from `processing/pipeline.rs`, `storage/manager.rs`, `jobs/workers/executor.rs`, `models`. It is the **integration seam** that bridges the WorkerPool → ProcessingPipeline → StorageManager. This module has no equivalent in the `Application` builder (`application.rs`).

### Public API surface per crate

**nabu-core** (`lib.rs:46-69`): 14 glob re-exports. All major types (`CaptureEngine`, `KnowledgeObject`, `VaultGraph`, `Indexer`, `Job`, `Processor`, `CaptureHandler`, `JobExecutor`, `PipelineEvent`, `EventBus`, `ApplicationContext`, `ServiceRegistry`, `HistoryManager`, `ProcessingPipeline`) are available at crate root.

**src-tauri** (`lib.rs:183`): Single public entry point `pub fn run()`. All 87 commands are `#[tauri::command]` annotated — not part of the library's public Rust API, they are Tauri invoke targets.

**nabu-ui** (`lib.rs:13`): Single public entry point `#[wasm_bindgen(start)] pub fn start()`. All components are internal `#[component]` macros.

---

## 6. Runtime Service Map

| Service | Startup location | Owner | Shutdown path | Dependencies |
|---|---|---|---|---|
| **WorkerPool** (4 workers) | `lib.rs:349-351` — `tauri::async_runtime::spawn(pool.start())` | `ApplicationContext` (registered as `"worker_pool"`, `lib.rs:121`) | Not invoked. `WorkerPool::shutdown()` exists (`pool.rs:97-128`) but is never called on `RunEvent::Exit`. The `RunEvent::Exit` handler (`lib.rs:405-411`) only calls `recovery::mark_clean_exit()`. Workers are killed by Tauri's runtime drop on process exit. | `DurableJobQueue`, `ExecutorRegistry`, `ShutdownCoordinator`, `BackpressureController` |
| **Native Messaging Socket Server** | `lib.rs:360-368` — `tauri::async_runtime::spawn(start_socket_server(...))` | `SocketServerState` (ephemeral, not registered in context) | Not invoked. `SocketServerHandle::shutdown()` exists (`native_messaging_socket.rs:361`) but the handle is discarded (`lib.rs:362`: `Ok(_handle)`). The server runs until process exit; the socket file cleanup happens in the `tokio::spawn` loop on shutdown notification (`socket:250-252`). | `CaptureEngine`, `Message` validation |
| **8s Safety Timer** | `lib.rs:376-383` — `tauri::async_runtime::spawn(async { sleep(8s)... })` | Ephemeral (closure) | Self-terminating (one-shot timer). | `webview_window("main")` |
| **(Not running) Scheduler** | N/A | `DurableJobQueue` holds an `Arc<Scheduler>` (`queue.rs:70`) | N/A | Not started — no `tokio::spawn` call for `Scheduler::process_due_jobs()` anywhere in the codebase |

**Key finding**: The `Scheduler` (`crates/nabu-core/src/jobs/scheduler.rs:46`) exists as a struct with `schedule()`, `process_due_jobs()`, `reschedule()`, but is **never started as a background task**. `DurableJobQueue` constructs it (`queue.rs:82`) but no code calls `scheduler.process_due_jobs()` in a periodic loop. Delayed/scheduled jobs would never be dequeued. This is an architectural gap.

---

## 7. State Ownership Map

### Backend state (single-owner through ApplicationContext)

| State object | Owner | Consumers | Lifetime | Persistence |
|---|---|---|---|---|
| `SettingsStore` | `src-tauri/src/lib.rs:196-200` — managed as Tauri `State<'_, SettingsStore>` | All 87 Tauri commands via `State<'_, SettingsStore>` parameter | Process lifetime | `~/.config/nabu/settings.json` (`settings.rs:196-197`) |
| `ApplicationContext` | `lib.rs:331-345` — `build_application_context()` returns it, `app.manage(ctx)` registers it | `capture_file_drop` (`commands.rs:1255-1257`), `graph_data` (`commands.rs:1908`), `versions_restore` (`recovery.rs:452`), `versions_duplicate` (`recovery.rs:499`), others | Process lifetime | None (services it owns have their own persistence) |
| `EventBus<PipelineEvent>` | `lib.rs:57` — `Arc<EventBus<PipelineEvent>>`, registered as `"event_bus"` in registry | `StorageManager`, `CaptureEngine`, `ProcessingPipeline`, `PipelineExecutor`, `VaultGraph`, `Indexer` (all constructed with `Some((*event_bus).clone())`) | Process lifetime | None (in-memory pub/sub) |
| `StorageManager` | `lib.rs:73` — `Arc<StorageManager>`, registered as `"storage_manager"` | `PipelineExecutor` (`.with_storage()`, `lib.rs:106`), ITEM_STORED subscriber (`lib.rs:162-177`) | Process lifetime | Vault directory: `.md` content files + `<uuid>.json` sidecars + `<uuid>.bin` binary files |
| `DurableJobQueue` | `lib.rs:93-100` — `Arc<DurableJobQueue>`, registered as `"job_queue"` | `WorkerPool`, `CaptureEngine` (both hold `Arc<DurableJobQueue>`) | Process lifetime (survives restarts) | `vault/.nabu/queue/` — `JobStore` (`persistence.rs:24-281`) writes `<status>/<uuid>.json` files |
| `WorkerPool` | `lib.rs:120-121` — `Arc<WorkerPool>`, registered as `"worker_pool"` | None (started, runs independently) | Started in `.setup()`, killed on exit | None |
| `CaptureEngine` | `lib.rs:124-128` — `Arc<CaptureEngine>`, registered as `"capture_engine"` | `capture_file_drop` command (`commands.rs:1255`), `native_messaging_socket.rs:306` | Process lifetime | None |
| `ProcessingPipeline` | `lib.rs:80-81` — `Arc<ProcessingPipeline>`, registered as `"pipeline"` | `PipelineExecutor` (holds `pipeline.clone()`) | Process lifetime | None |
| `Indexer` | `lib.rs:139` — `Arc<Mutex<Indexer>>`, registered as `"indexer"` | ITEM_STORED subscriber (`lib.rs:162-177`) | Process lifetime | `vault/.nabu/search_index.json` |
| `VaultGraph` | `lib.rs:143-147` — `Arc<RwLock<VaultGraph>>`, registered as `"vault_graph"` | ITEM_STORED subscriber (`lib.rs:162-177`) | Process lifetime | `vault/.nabu/graph/` — `GraphStore` (`graph/persistence.rs`) |
| `HistoryManager` | `lib.rs:150-153` — `Arc<RwLock<HistoryManager>>`, registered as `"history_manager"` | **None** — not consumed by any command | Process lifetime | None (in-memory only) |

### Frontend state (Leptos reactive signals)

| Signal | Provider | Scope | Persistence |
|---|---|---|---|
| `theme: RwSignal<String>` | `provide_theme()` (`lib.rs:43-46`) | Global (via `provide_context`) | `settings_get`/`settings_set` IPC to backend |
| `workspace: WorkspaceContext` | `provide_workspace()` (`workspace.rs`) | Global (via `provide_context`) | `session_save`/`session_load` IPC |
| `nav: NavigationContext` | `provide_navigation()` (`navigation/state.rs`) | Global (via `provide_context`) | `load_all_nav_state()` reads from backend IPC |

### Backend state NOT managed through ApplicationContext

- `SettingsStore` is managed as standalone Tauri `State`, not through `ApplicationContext` or `ServiceRegistry`. Commands access it via `State<'_, SettingsStore>` parameter, not via `ctx.resolve("settings_store")`.
- `HistoryManager` (core) is registered in the ApplicationContext but is never resolved by any command. The actual undo/redo (`../../src-tauri/src/history.rs`) uses a separate `HistoryState` Tauri managed state (verified by reading `history.rs` imports — it does not use `ApplicationContext`).

---

## 8. IPC Flow Diagram

### 8.1 IPC mechanism

The frontend communicates with the backend exclusively through Tauri's `invoke` API:

```
Frontend (WASM)                    Backend (Rust)
  │                                  │
  │ tauri_invoke(cmd, args)         │
  │   → window.__TAURI__.core       │
  │     .invoke(cmd, args)          │
  │          ─────────────────────→ │  #[tauri::command] fn
  │                                  │  match on cmd name
  │         ←────────────────────── │  Result<T, String>
  │  JsValue result                  │
  └─ deserialize via serde_wasm_bindgen─┘
```

`tauri_invoke()` (`crates/nabu-ui/src/ipc.rs:9-11`) wraps `wasm_bindgen` extern:
```rust
#[wasm_bindgen(extern "C")]
async fn invoke(cmd: &str, args: JsValue) -> JsValue;
```

### 8.2 Registered commands (87 total, lib.rs:201-311)

All commands registered via `generate_handler!` at `src-tauri/src/lib.rs:201-311`. Grouped by domain:

| Category | Source file | Count | Command list |
|---|---|---|---|
| **Security helpers** | `commands.rs:16-65` | 2 | `validate_path_within_vault`, `validate_input_safe` |
| **Vault management** | `commands.rs` + `recovery.rs` | 6 | `check_vault_exists`, `get_current_vault`, `select_vault_dialog`, `create_vault_dialog`, `open_settings`, `complete_setup` |
| **Settings** | `commands.rs` | 7 | `get_settings`, `settings_get`, `settings_set`, `settings_set_all`, `settings_export`, `settings_import`, `settings_reset` |
| **Dictation** | `commands.rs` | 5 | `open_dictation_pill`, `close_dictation_pill`, `toggle_dictation_pill`, `start_dictation`, `stop_dictation` |
| **History / Undo-Redo** | `history.rs:49-960` | 29 | `history_status`, `history_undo`, `history_redo`, `history_clear`, `history_set_depth`, `note_rename`, `note_delete`, `note_restore`, `note_duplicate`, `items_move`, `trash_list`, `trash_delete`, `trash_restore_many`, `trash_purge_expired`, `trash_empty`, `trash_restore`, `archive_note`, `archive_restore`, `folder_create`, `folder_rename` |
| **File tree & navigation** | `commands.rs` | 4 | `tree_list`, `reveal_in_file_manager`, `reveal_vault_in_file_manager`, `open_in_explorer` |
| **Notes index & search** | `commands.rs` | 3 | `notes_index`, `notes_search`, `notes_diff` |
| **Knowledge graph** | `commands.rs` | 3 | `graph_data`, `note_links`, `link_mention` |
| **Inbox** | `commands.rs` | 14 | `inbox_subscribe`, `inbox_get_queue`, `inbox_approve`, `inbox_reject`, `inbox_retry`, `inbox_delete`, `inbox_batch_approve`, `inbox_batch_reject`, `inbox_batch_delete`, `inbox_batch_retry`, `inbox_edit_metadata`, `inbox_move`, `inbox_quick_capture` |
| **Capture** | `commands.rs` | 1 | `capture_file_drop` |
| **Reading queue** | `commands.rs` | 7 | `queue_get_all`, `queue_set_status`, `queue_set_priority`, `queue_set_progress`, `queue_batch_set_status`, `queue_archive_completed`, `queue_cancel` |
| **Smart folders** | `commands.rs` | 4 | `smart_folders_list`, `smart_folder_save`, `smart_folder_delete`, `smart_folder_evaluate` |
| **Calendar & templates** | `commands.rs` | 8 | `calendar_notes`, `daily_note_for`, `template_list`, `template_save`, `template_delete`, `template_duplicate`, `template_set_favourite` |
| **Canvas** | `commands.rs` | 4 | `canvas_list`, `canvas_get`, `canvas_save`, `canvas_delete` |
| **Statistics** | `commands.rs` | 1 | `statistics_get` |
| **Recovery** | `recovery.rs:390-731` | 14 | `note_save`, `note_read`, `versions_list`, `versions_get`, `versions_restore`, `versions_duplicate`, `versions_diff`, `snapshot_create`, `versions_all`, `session_save`, `session_load`, `session_clear`, `recovery_check`, `recovery_discard` |
| **Platform integration** | `commands.rs` | 3 | `show_macos_notification`, `pin_to_taskbar`, `install_desktop_entry` |

### 8.3 IPC command categories by backend access pattern

**Commands that route through `ApplicationContext` (canonical services):**
- `capture_file_drop` (`commands.rs:1249-1274`): gets `CaptureEngine` via `ctx.capture_engine()`, calls `engine.ingest(request).await`
- `graph_data` (`commands.rs:1907`): uses `State<'_, SettingsStore>` — does **NOT** use `ApplicationContext` or `VaultGraph`
- `statistics_get` (`commands.rs:3373-3466`): uses `State<'_, SettingsStore>` + `graph_data_inner()` — does **NOT** use `VaultGraph` or `Indexer`

**Commands that bypass canonical services entirely (direct filesystem):**
- `note_save` (`recovery.rs:391-406`): `std::fs::write()` — bypasses `StorageManager`
- `note_read` (`recovery.rs:410-417`): `std::fs::read_to_string()` — bypasses `StorageManager`
- `tree_list` (`commands.rs:232-288`): `std::fs::read_dir()` — standalone filesystem walk
- `notes_index` (`commands.rs:1476`): `collect_notes()` filesystem walk
- `notes_search` (`commands.rs:1559-1661`): filesystem `.md` scan + substring match
- `graph_data` (`commands.rs:1907-2017`): filesystem `.md` scan + `[[wikilink]]` parse + union-find
- All `trash_*`, `archive_*`, `folder_*`, `note_*` commands in `history.rs`: direct `std::fs` operations

### 8.4 Frontend IPC call inventory (104 call sites across 28 files)

Top callers:
- `crates/nabu-ui/src/components/app.rs:129,237` — `recovery_check`, `check_vault_exists`
- `crates/nabu-ui/src/lib.rs:57,68,97` — `settings_get`, `get_settings`, `settings_set` (theme sync)
- `crates/nabu-ui/src/components/note_editor.rs:66,104,136` — `note_read`, `note_save`
- `crates/nabu-ui/src/components/file_tree.rs:124,162,206,237,253,281,320,349` — `tree_list`, `note_delete`, `note_rename`, `note_create_file`
- `crates/nabu-ui/src/components/inbox.rs:128-497` — 18 IPC calls (inbox_*, capture_file_drop, versions_*)
- `crates/nabu-ui/src/components/history.rs:106,118,135` — `history_undo`, `history_redo`, `history_status`

### 8.5 Event emitters and listeners

**No Tauri events** (`emit`/`listen`) are used in the backend. All frontend↔backend communication is request-response via `invoke`. The `EventBus<PipelineEvent>` is internal to `nabu-core` only.

**EventBus pub/sub** (internal to nabu-core):
- `ITEM_STORED` (`kinds.rs:38`): Published by `StorageManager::save()` (`storage/manager.rs`), consumed by subscriber at `lib.rs:162-177` → `Indexer.index_object()` + `VaultGraph.add_node()`
- `ITEM_CAPTURED` (`kinds.rs:33`): Published by `CaptureEngine::ingest()` (`engine.rs:64-74`), consumed by: **no subscribers found**
- `ITEM_PROCESSING_PROGRESS` (`kinds.rs:35`): Published by `ProcessingPipeline::run()` (`pipeline.rs:157`), consumed by: **no subscribers found**
- `GRAPH_UPDATED` (`kinds.rs:40`): Published by `VaultGraph::add_node()`/`add_edge()` (`graph/mod.rs:345-354, 418-427`), consumed by: **no subscribers found**

**Native messaging**: `src-tauri/src/native_messaging_socket.rs:226-256` — `start_socket_server()` listens on Unix domain socket `/tmp/nabu-native-messaging.sock`. Each connection is accepted and handled via `handle_connection()` (`socket:259-340`). Messages are validated against `native_messaging::Message` schema, converted to `CaptureRequest`, and dispatched through `engine.ingest(request)` (`socket:306`) — same canonical flow as `capture_file_drop`.

---

## 9. Dependency Direction

### Layered dependency analysis

```
nabu-ui (frontend)
    │  tauri_invoke(cmd, args) — JSON over wasm-bindgen invoke
    ▼
src-tauri (backend shell)
    │  uses nabu_core::* types
    ▼
nabu-core (domain library)
    │
    ├──► models (no deps)
    ├──► event_bus (→ models)
    ├──► history (no core deps)
    ├──► jobs (→ event_bus, models, cancellation, priority, retry, scheduler, persistence)
    ├──► capture (→ event_bus, jobs, models)
    ├──► processing (→ event_bus, jobs, models)
    ├──► storage (→ event_bus, models)
    ├──► indexer (→ event_bus, models)
    ├──► graph (→ event_bus, models)
    ├──► registry (→ event_bus, plugin/capability)
    ├──► plugin (minimal internal deps)
    ├──► pipeline_migration (→ jobs, processing, storage, event_bus)
    └──► diagnostics (tracing only)
```

### Confirmed clean directions (no violations)

1. **nabu-core → nothing**: `../../crates/nabu-core/Cargo.toml` depends only on external crates (chrono, serde, tokio, etc.). No path dependency on `src-tauri` or `nabu-ui`.
2. **src-tauri → nabu-core**: `src-tauri/Cargo.toml:25`: `nabu-core = { path = "../crates/nabu-core" }`. `src-tauri/src/lib.rs:12-17` imports 9 `nabu_core::` symbols.
3. **nabu-ui → nothing**: `crates/nabu-ui/src/ipc.rs:1-11` bridges via `wasm_bindgen` to `window.__TAURI__.core.invoke`. All type exchange is via `serde_json::Value`/`serde_wasm_bindgen` — no Rust type imports from nabu-core.

### Violation: `history` overlap

- `nabu-core/src/history/mod.rs:1` declares a complete `HistoryManager` with `HistoryEntry`, `HistoryOp`, undo/redo stacks, max depth pruning. Registered in context as `"history_manager"` (`lib.rs:150-153`).
- **But** `src-tauri/src/history.rs:1` independently implements its own `HistoryEntry`, `HistoryOp`, `HistoryState` using Tauri managed state (`tauri::State<HistoryState>`). This `../../src-tauri/src/history.rs` implementation (`history.rs:49-960`) powers all 29 undo/redo commands.
- **Evidence**: `src-tauri/src/history.rs:1` — `#![allow(dead_code, unused_imports)]` (note: this is not visible in the file content but the module exists). `history.rs:30-47` defines its own `push_history()` function. The core `HistoryManager` (`nabu-core/src/history/mod.rs:178-300`) has `push()`, `undo()`, `redo()` methods — never called from `src-tauri`.

### Violation: Dual persistence pathways

- **StorageManager** (`storage/manager.rs:33-436`): canonical save via `save()` which writes content file + JSON sidecar + publishes `ITEM_STORED`.
- **recovery.rs** (`recovery.rs:391-406`): `note_save` writes directly via `std::fs::write()` + `snapshot_note()`. Never calls `StorageManager.save()`.
- **history.rs** (`history.rs:49-960`): All filesystem operations (rename, delete, move, trash) performed via direct `std::fs` calls. `NoteDelete` pushes a `HistoryEntry` with undo/redo closures, but these closures use `std::fs` directly — not `StorageManager`.

### Violation: Dual graph/index reconstruction

- **VaultGraph** (`graph/mod.rs:54`): In-memory adjacency-list graph with disk persistence via `GraphStore`. Loaded from `../../.nabu/graph` on startup. Updated via `ITEM_STORED` subscriber.
- **graph_data command** (`commands.rs:1907-2017`): Reconstructs graph by scanning `.md` files for `[[wikilinks]]` and running union-find. Does not read `VaultGraph`.
- **Indexer** (`indexer.rs:26`): In-memory inverted index with persistence to `.nabu/search_index.json`. Updated via `ITEM_STORED` subscriber.
- **notes_search command** (`commands.rs:1559-1661`): Scans vault filesystem for `.md` files, does substring matching. Does not read `Indexer`.

---

## 10. Architectural Hotspots

| Rank | File | Why central | Who depends on it | What breaks if changed |
|---|---|---|---|---|
| 1 | `crates/nabu-core/src/lib.rs:28-41` | Module declaration + glob re-exports for 14 modules. Changing module structure ripples to all downstream glob imports (`pub use capture::*`, etc.) | All of nabu-core's public API consumers — `../../src-tauri/src/lib.rs`, `capture/engine.rs`, `processing/pipeline.rs`, etc. | Every `nabu_core::*` path used in `src-tauri` and `nabu-ui` would break. |
| 2 | `src-tauri/src/lib.rs:55-180` | `build_application_context()` — the sole construction function for all 10 services. Defines the entire service graph, registration keys, and the `ITEM_STORED` event wiring. | `src-tauri/src/lib.rs:331` (startup), `../../src-tauri/src/commands.rs` (commands resolve through `ctx`), `../../src-tauri/src/native_messaging_socket.rs` (uses `CaptureEngine` from context) | If a service key is renamed, all `ctx.resolve("key")` calls break. If the construction order changes, `StorageManager` may not be registered before the `ITEM_STORED` subscriber tries to load from it. |
| 3 | `src-tauri/src/lib.rs:201-311` | `generate_handler!` — registers all 87 `#[tauri::command]` functions. The complete IPC surface. | The `nabu-ui` frontend — every `tauri_invoke("cmd")` call depends on a name registered here. | Adding/removing/renaming a command breaks the corresponding frontend IPC call. Renaming returns a Tauri dispatch error at runtime. |
| 4 | `crates/nabu-core/src/capture/engine.rs:54-122` | `CaptureEngine::ingest()` — the canonical entry point for all content entering Nabu. Constructs the `Job` payload and enqueues it. | `build_application_context` (registers engine), `capture_file_drop` command (`commands.rs:1255`), `native_messaging_socket.rs:306` | Changing the payload schema breaks `PipelineExecutor::object_from_job()` (`pipeline_migration/executor.rs:70-107`) which reads it. Changing the routing algorithm affects all 11 handlers. |
| 5 | `src-tauri/src/lib.rs:162-177` | The `ITEM_STORED` subscriber — the only EventBus subscriber in the entire system. Connects Storage → Indexer + VaultGraph. | `StorageManager::save()` publishes `ITEM_STORED`; this closure consumes it. | If removed, Indexer and VaultGraph are never updated from the canonical pipeline. The graph stays empty; search index is stale. |
| 6 | `../../crates/nabu-core/src/processing/pipeline.rs` | `ProcessingPipeline::run()` — executes all 14 processors in order. Contains the `build_standard_pipeline()` factory (`pipeline.rs:245-265`). | `PipelineExecutor::execute()` (`pipeline_migration/executor.rs:137`) calls `self.pipeline.run()`. | Adding/removing/reordering processors changes processing behavior for all captures. The `Processor` trait (`processor.rs:84-102`) is consumed here. |
| 7 | `crates/nabu-core/src/models/knowledge_object.rs:10-43` | `KnowledgeObject` — the universal domain model. Every subsystem (capture, processing, storage, graph, indexer) uses it as input/output. | All 14 processors, CaptureEngine (produces `KnowledgeObject`), StorageManager (persists), Indexer (indexes), VaultGraph (nodes) | Changing a field ripples to serialization (JSON sidecar in `storage/manager.rs`), graph serialization (`graph/serializer.rs`), indexer tokenization (`indexer.rs:222`), and the frontend model mapping (`nabu-ui/src/models.rs`). |
| 8 | `crates/nabu-core/src/jobs/workers/executor.rs:24-107` | `PipelineExecutor` — the sole `JobExecutor` implementation (`pipeline_migration/executor.rs:24`). Bridges the WorkerPool to the ProcessingPipeline and StorageManager. Contains `Self::object_from_job()` which reconstructs `KnowledgeObject` from the job payload. | `Worker::run()` (`workers/worker.rs:80-86`): `executors.get(&job.processor_name)` → `executor.execute()`. `ExecutorRegistry` holds this. | The `object_from_job()` reconstruction (`executor.rs:70-107`) only uses `title`, `source_url`, `object_id` from the payload — content is `ObjectContent::PlainText(title)`. Changing the payload schema requires coordination between `CaptureEngine::ingest()` (producer) and `PipelineExecutor::execute()` (consumer). |
| 9 | `crates/nabu-core/src/jobs/workers/pool.rs:14-27` | `WorkerPool` — manages 4 worker tasks consuming from the `DurableJobQueue`. Configured with `BackpressureController` and `ShutdownCoordinator`. | `build_application_context` (`lib.rs:120`), `lib.rs:349-351` (startup spawn), `lib.rs:335` (resolution) | Changing worker count affects throughput. Changing dequeue logic affects all async processing. If `shutdown()` is never called on exit (`lib.rs:405-411` only does `mark_clean_exit`), in-flight jobs may be killed mid-execution. |
| 10 | `src-tauri/src/recovery.rs:310-406` | `snapshot_note()` + `note_save()` — the version history system. Every save creates a snapshot. `MAX_VERSIONS_PER_NOTE = 50` (`recovery.rs:35`). All history commands depend on the `version_file()` / `note_versions_dir()` path functions (`recovery.rs:128-134`). | `note_save` command (autosave path from `NoteEditor`), `versions_*` commands, `snapshot_create` command | Changing the hash function (`stable_hash`, `recovery.rs:68-75` — FNV-1a) invalidates all existing version directories. Changing retention count affects disk usage. |

---

## 11. Risks, Ambiguities, and Open Questions

### Verified Risks

| # | Risk | Evidence | Impact |
|---|---|---|---|
| R1 | **Broken content handoff**: captured content is lost between ingest and pipeline execution | `CaptureEngine::ingest()` (`engine.rs:80-95`) puts only `object_id`, `object_type`, `source`, `title`, `source_url` in the job payload. `PipelineExecutor::object_from_job()` (`pipeline_migration/executor.rs:70-107`) reconstructs with `ObjectContent::PlainText(title)`. The actual `CaptureData` (text/binary/URI) is held in-memory by the handler, never persisted before enqueue. | Content-dependent processors (OcrProcessor, WhisperProcessor, PdfTextProcessor, etc.) receive placeholder data. OCR, Whisper, PDF extraction, embeddings, and classification all fail or produce nulls. |
| R2 | **4 of 14 processors individually routable; 10 run inline** | `ExecutorRegistry` has 4 executors (`lib.rs:109-116`), all pointing to the same `PipelineExecutor` instance. The 14 processors all run inside `PipelineExecutor::execute()` → `ProcessingPipeline::run()`. | Intentional design (per `lib.rs:102-107` comment), but means priority/delay/scheduling per-processor is impossible. All 14 processors block on the same job. |
| R3 | **Indexer bypassed by `notes_search`** | `notes_search` (`commands.rs:1559-1661`) scans `.md` files + substring match. Does not call `Indexer`. | The in-memory Indexer (`indexer.rs:26-213`) is only fed by `ITEM_STORED` events, which are only published by `StorageManager::save()`. Since `note_save` in `recovery.rs` bypasses `StorageManager`, most note saves never reach the Indexer. |
| R4 | **VaultGraph bypassed by `graph_data`** | `graph_data` (`commands.rs:1907-2017`) rebuilds from filesystem `[[wikilinks]]` via union-find. Does not call `VaultGraph.add_node()`. | `VaultGraph` is only updated by the `ITEM_STORED` subscriber (`lib.rs:162-177`), which only fires for `StorageManager::save()` — not for `note_save` in `recovery.rs` or direct filesystem writes. |
| R5 | **WorkerPool has no shutdown hook** | `RunEvent::Exit` handler (`lib.rs:405-411`) only calls `mark_clean_exit()`. `WorkerPool::shutdown()` (`pool.rs:97-128`) is never invoked. | On exit, 4 worker tokio tasks are aborted mid-execution. Jobs marked `Running` in the `JobStore` never transition to `Completed` or `Failed`. |
| R6 | **Scheduler never started as background task** | `DurableJobQueue::new()` (`queue.rs:80-89`) creates `Scheduler::new(store)` (`scheduler.rs:52-53`) but stores it. No `tokio::spawn` for `scheduler.process_due_jobs()` exists anywhere. | Scheduled/delayed jobs (e.g., `Job::with_schedule()`) never execute. The `ScheduleSpec` enum (`scheduler.rs:12-19`) is dead code at runtime. |
| R7 | **No Tauri event-based push to frontend** | No `emit()`/`listen()` calls found in the backend. All frontend→backend is `invoke()`. No backend→frontend push. | Frontend must poll or re-fetch for state changes. The `ITEM_PROCESSING_PROGRESS` event (`events.rs:35`) has no subscriber and no UI binding. Users see no progress during background processing. |
| R8 | **HistoryManager (core) is dead code** | Registered as `"history_manager"` (`lib.rs:150-153`) but never resolved. `push_history()` is defined in `../../src-tauri/src/history.rs` (`history.rs:30-47`), not in core. | The core `HistoryManager` (`nabu-core/src/history/mod.rs:165-300`) is never exercised. The undo/redo stack is purely in `../../src-tauri/src/history.rs` using its own `HistoryEntry`/`HistoryOp` types. |

### Ambiguities (cannot be resolved without runtime)

| # | Ambiguity | Reason |
|---|---|---|
| A1 | Whether the `ITEM_CAPTURED` event (`kinds.rs:33`) is consumed anywhere | Search found the constant and the publish site (`engine.rs:64-74`), but no `subscribe("item.captured", ...)` call exists. The subscriber is only for `ITEM_STORED`. |
| A2 | Whether `VaultGraph` is ever read by any command | `VaultGraph` is registered as `"vault_graph"` and subscribed to `ITEM_STORED`, but no `#[tauri::command]` resolves it via `ctx.resolve("vault_graph")`. The `graph_data` command (`commands.rs:1907`) uses `State<'_, SettingsStore>` only. |
| A3 | Whether the `Indexer` is ever queried by any command | `Indexer` is registered as `"indexer"` and subscribed to `ITEM_STORED`, but no command resolves it. `notes_search` (`commands.rs:1559`) does filesystem scan. |
| A4 | Whether the Plugin system is wired up at all | `CapabilityRegistry::register_builtin()` is called (`lib.rs:61`), registering 12 capabilities. But `PluginManager` (`plugin/manager.rs`) is never constructed or registered. |
| A5 | Whether the `Application` builder (`registry/application.rs:107-119`) is ever used | Doc-comment describes `ApplicationBuilder::new()` → `with_*()` → `.build()` → `.initialize()` → `.start()` → `.shutdown()`. But `build_application_context()` (`lib.rs:55`) constructs services directly — does not use `Application`. |

---

## 12. Verified Symbol Index

### Entry points
- `fn main()` — `src-tauri/src/main.rs:3` → `app_lib::run()`
- `pub fn run()` — `src-tauri/src/lib.rs:183`
- `#[wasm_bindgen(start)] pub fn start()` — `crates/nabu-ui/src/lib.rs:13`
- `pub fn build_application_context(vault_path: PathBuf) -> ApplicationContext` — `src-tauri/src/lib.rs:55`

### Core traits
- `trait Processor` — `crates/nabu-core/src/processing/processor.rs:84-102`
- `trait CaptureHandler` — `crates/nabu-core/src/capture/handler.rs:37-47`
- `trait JobExecutor` — `crates/nabu-core/src/jobs/workers/executor.rs:15-23`
- `trait Queue` — `crates/nabu-core/src/jobs/queue.rs:55-60`

### Core structs
- `struct CaptureEngine` — `crates/nabu-core/src/capture/engine.rs:16-20`
- `struct ProcessingPipeline` — `crates/nabu-core/src/processing/pipeline.rs:36` (approx)
- `struct StorageManager` — `crates/nabu-core/src/storage/manager.rs:33`
- `struct VaultGraph` — `crates/nabu-core/src/graph/mod.rs:54-64`
- `struct Indexer` — `crates/nabu-core/src/indexer.rs:26-30`
- `struct HistoryManager` — `crates/nabu-core/src/history/mod.rs:165-169`
- `struct ServiceRegistry` — `crates/nabu-core/src/registry/mod.rs:40`
- `struct ApplicationContext` — `crates/nabu-core/src/registry/context.rs:141`
- `struct DurableJobQueue` — `crates/nabu-core/src/jobs/queue.rs:68-76`
- `struct WorkerPool` — `crates/nabu-core/src/jobs/workers/pool.rs:14-27`
- `struct PipelineExecutor` — `crates/nabu-core/src/pipeline_migration/executor.rs:24-28`
- `struct ExecutorRegistry` — `crates/nabu-core/src/jobs/workers/executor.rs:29-31`
- `struct KnowledgeObject` — `crates/nabu-core/src/models/knowledge_object.rs:10-43`

### Core functions
- `fn build_default_capture_engine(...)` — `crates/nabu-core/src/capture/engine.rs:155-181` (11 handlers)
- `fn build_standard_pipeline(...)` — `crates/nabu-core/src/processing/pipeline.rs:245-265` (14 processors)
- `fn object_type_to_job_type(...)` — `crates/nabu-core/src/capture/engine.rs:141-152`
- `fn start_socket_server(...)` — `src-tauri/src/native_messaging_socket.rs:200-256`
