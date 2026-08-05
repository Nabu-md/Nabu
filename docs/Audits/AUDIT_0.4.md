# Nabu v0.4 Audit: State, IPC & Event Flow

> Complete semantic trace of how information moves through the Nabu codebase, from user action to every affected subsystem. Based on evidence gathered through RustRover's semantic analysis tools.

---

## 1. Executive Summary

Nabu has **two fundamentally disconnected information-flow paths** that share no common update mechanism:

### Path A — User editing (notes, sessions, settings): Direct filesystem writes
```
User types → NoteEditor (Leptos) → tauri_invoke → note_save / settings_set / session_save
    → std::fs::write (recovery.rs:403) / Mutex<AppSettings> (settings.rs:293)
    → NO EventBus publication
    → NO Indexer update
    → NO VaultGraph update
    → Frontend polls / manual refresh only
```

### Path B — Content capture (files, clipboard, inbox, native messaging socket):
```
User drops file → tauri_invoke("capture_file_drop") → CaptureEngine::ingest
    → ITEM_CAPTURED event → DurableJobQueue (file-persisted)
    → tokio::spawn(WorkerPool) → Worker::dequeue → PipelineExecutor::execute
    → ProcessingPipeline::run → StorageManager::save
    → ITEM_STORED event → Indexer::index_object + VaultGraph::add_node
    → INDEX_UPDATED + GRAPH_UPDATED events
    → Frontend has NO subscription — must poll
```

**Critical finding:** The `note_save` command (`src-tauri/src/recovery.rs:391`) bypasses the entire EventBus pipeline. It writes directly via `std::fs::write` (line 403) and snapshots via `snapshot_note` (line 404). It does **not** call `StorageManager::save()`, does **not** publish `ITEM_STORED`, and therefore does **not** trigger indexing or graph updates. When a user edits and saves a note, the search index and knowledge graph are silently stale until a manual refresh or the next capture event.

**Critical finding:** There is **no Tauri event bridge** from the backend EventBus to the frontend. The `EventBus<PipelineEvent>` (defined in `../../crates/nabu-core/src/event_bus/bus.rs`) is purely backend-internal. Events publish synchronously (Mutex lock, inline handler dispatch). The frontend (`../../crates/nabu-ui`) communicates exclusively through request-response `tauri_invoke()` calls — there is no push mechanism. The `GraphEventBridge` (`../../crates/nabu-core/src/graph/incremental/event_wiring.rs`) exists but is **not wired in production** — it is only used in tests (event_wiring.rs:191-232).

---

## 2. Global State Map

### 2.1 Frontend State (Leptos CSR, `../../crates/nabu-ui/src`)

The frontend has **no centralized state store**. State is decentralized across **six context providers** registered at the `App` component root (`components/app.rs:77-86`):

| Context | Struct | Location | Owner | Lifetime | Persistence |
|---------|--------|----------|-------|----------|-------------|
| **Navigation** | `NavContext` | `components/navigation/state.rs:174-211` | App component (render-time capture) | App lifetime | `settings_set`/`settings_get` IPC |
| **Workspace** | `WorkspaceContext` | `components/workspace.rs:28-44` | App component | App lifetime | Session via `session_save` |
| **History** | `HistoryContext` | `history.rs:47-53` | App component + `OnceLock` | App lifetime | Backend `HistoryManager` |
| **Save Status** | `SaveStatusContext` | `components/recovery/save_status.rs:52-58` | App component | App lifetime | None (transient) |
| **Tasks** | `TaskContext` | `components/ui/feedback.rs:729-730` | App component | App lifetime | None (transient) |
| **Toasts** | `ToastContext` | `components/ui/feedback.rs:80-83` | `ToastProvider` (`feedback.rs:190-197`) | App lifetime | None (transient) |
| **Theme** | theme `Signal<String>` | `components/app.rs:79` | App component | App lifetime | `settings_set` |

#### NavContext — 21 `RwSignal` fields (`state.rs:176-211`)

| Signal | Type | Backing IPC | Mutation Points |
|--------|------|-------------|-----------------|
| `view_mode` | `RwSignal<ViewMode>` | `settings_set("nabu.view_mode")` | `nav.view_mode.set()` — 5+ call sites across components |
| `palette_open` | `RwSignal<bool>` | None (transient) | `nav.palette_open.set()` — command palette toggle |
| `switcher_open` | `RwSignal<bool>` | None | Quick switcher toggle |
| `shortcuts_open` | `RwSignal<bool>` | None | Shortcuts reference toggle |
| `search_query` | `RwSignal<String>` | None | SearchPage input |
| `show_left_sidebar` | `RwSignal<bool>` | `settings_set("nabu.show_left_sidebar")` | Sidebar toggle (RibbonBar) |
| `show_right_inspector` | `RwSignal<bool>` | `settings_set("nabu.show_right_inspector")` | Inspector toggle |
| `recent_notes` | `RwSignal<Vec<String>>` | `settings_set("nabu.recent_notes")` | `record_recent_note()` (app.rs:118, 373) |
| `favourites` | `RwSignal<Vec<String>>` | `settings_set("nabu.favourites")` | Favourite toggle |
| `recent_searches` | `RwSignal<Vec<String>>` | `settings_set("nabu.recent_searches")` | Search execution |
| `saved_searches` | `RwSignal<Vec<SavedSearch>>` | `settings_set("nabu.saved_searches")` | Saved search CRUD |
| `smart_folders` | `RwSignal<Vec<SmartFolder>>` | `settings_set("nabu.smart_folders")` | Smart folder CRUD |
| `recent_commands` | `RwSignal<Vec<String>>` | `settings_set("nabu.recent_commands")` | Command palette |
| `favourite_commands` | `RwSignal<Vec<String>>` | `settings_set("nabu.favourite_commands")` | Command palette |
| `notes_index` | `RwSignal<Vec<NoteIndexEntry>>` | `notes_index` IPC (app.rs:90 → `load_notes_index`) | File tree refresh |
| `dashboard_sections` | `RwSignal<Vec<String>>` | `settings_set("nabu.dashboard.sections")` | Dashboard config |
| `vault_name` | `RwSignal<String>` | None | Vault selection (app.rs:249, 266) |

#### WorkspaceContext — 4 `RwSignal` fields (`workspace.rs:28-44`)

| Signal | Type | Backing IPC | Consumers |
|--------|------|-------------|-----------|
| `tabs` | `RwSignal<Vec<OpenTab>>` | Direct mutation (no IPC) | TabBar, open_tab(), workspace.rs:75-89 |
| `active_path` | `RwSignal<Option<String>>` | Direct mutation | NoteEditor (app.rs:368), file_tree, tab_bar |
| `refresh_tree` | `RwSignal<u32>` | Triggers `tree_list` IPC | FileTree (`components/file_tree.rs`) |
| `content_version` | `RwSignal<(String, u32)>` | Direct mutation | NoteEditor (reload on external change) |

#### HistoryContext — 2 `RwSignal` fields (`history.rs:47-53`)

- `can_undo: RwSignal<bool>` — refreshed via `history_status` IPC (line 106)
- `can_redo: RwSignal<bool>` — refreshed via `history_status` IPC

Backed by: `OnceLock<SharedShortcutState>` (`history.rs:60`) — ensures the same signal set survives App re-mounts.

#### SaveStatusContext (`save_status.rs:52-58`)

- `status: RwSignal<SaveStatus>` — enum: `Idle`, `Saving`, `Saved`, `Failed`, `Retrying`
- `detail: RwSignal<String>` — tooltip text

Driven by: `note_editor.rs:127-152` (sets `Saving` before IPC, `Saved`/`Failed` after response).

#### TaskContext (`feedback.rs:729-730`)

- `tasks: RwSignal<Vec<TaskInfo>>` — in-memory only, **never populated from the backend**

`TaskInfo` fields: `id: String`, `label: String`, `progress: Option<f64>` (`feedback.rs:717-724`).

#### ToastContext (`feedback.rs:80-83`)

- `toasts: RwSignal<Vec<ToastItem>>` — in-memory only
- Methods: `push()`, `dismiss()`, `clear_all()`, `success/info/warning/error()`
- `ToastProvider` (`feedback.rs:190-197`) registers at app root, renders `<ToastRegion />`

### 2.2 Backend State (`../../src-tauri/src`, `crates/nabi-core/src/`)

All backend state is centralized in `ApplicationContext` (`registry/context.rs:141`), constructed once at startup (`src-tauri/src/lib.rs:55-179`) and registered as Tauri managed state (`lib.rs:345: app.manage(ctx)`).

| Service | Type | Container | Accessor |
|---------|------|-----------|----------|
| `event_bus` | `Arc<EventBus<PipelineEvent>>` | `ServiceRegistry` | `ctx.event_bus()` (context.rs:184) |
| `storage_manager` | `Arc<StorageManager>` | `ServiceRegistry` | `ctx.storage_manager()` (context.rs:254) |
| `pipeline` | `Arc<ProcessingPipeline>` | `ServiceRegistry` | `ctx.processing_pipeline()` (context.rs:229) |
| `job_queue` | `Arc<DurableJobQueue>` | `ServiceRegistry` | `ctx.job_queue()` (context.rs:234) |
| `worker_pool` | `Arc<WorkerPool>` | `ServiceRegistry` | `ctx.worker_pool()` (context.rs:239) |
| `capture_engine` | `Arc<CaptureEngine>` | `ServiceRegistry` | `ctx.capture_engine()` (context.rs:224) |
| `vault_graph` | `Arc<RwLock<VaultGraph>>` | `ServiceRegistry` | `ctx.vault_graph()` (context.rs:244) |
| `indexer` | `Arc<Mutex<Indexer>>` | `ServiceRegistry` | `ctx.indexer()` (context.rs:249) |
| `history_manager` | `Arc<RwLock<HistoryManager>>` | `ServiceRegistry` | `ctx.history_manager()` (context.rs:259) |
| `capability_registry` | `CapabilityRegistry` | `ApplicationContext` field | `ctx.capability_registry()` (context.rs:189) |

#### SettingsStore (`src-tauri/src/settings.rs:254-258`)

```rust
pub struct SettingsStore {
    path: PathBuf,
    inner: Mutex<AppSettings>,
}
```
- Registered via Tauri `manage()` (`lib.rs:200`)
- `Mutex` protects `AppSettings` (110 fields)
- Persisted to JSON at `~/.config/nabu/settings.json` (or `NABU_SETTINGS_PATH`)
- Accessed in commands via `State<'_, SettingsStore>`

### 2.3 State Ownership Summary

| State Category | Owner | Lifetime | Persistence |
|---------------|-------|----------|-------------|
| View mode, sidebar, vault name | Frontend (`NavContext`) | App lifetime | `settings_set` IPC → SettingsStore |
| Open tabs, active path | Frontend (`WorkspaceContext`) | App lifetime | `session_save` IPC → `.nabu/session.json` |
| Undo/redo capability | Backend (`HistoryManager`) | App lifetime | In-memory (not persisted) |
| Save status | Frontend (`SaveStatusContext`) | Note lifecycle | None |
| Settings (110 fields) | Backend (`SettingsStore: Mutex<AppSettings>`) | App lifetime | `~/.config/nabu/settings.json` |
| Session state | Backend (`recovery.rs`) | Per session | `.nabu/session.json`, `../../.nabu/.running` |
| Job queue | Backend (`DurableJobQueue`) | App lifetime | `../../.nabu/queue` (file-persisted) |
| Search index | Backend (`Indexer: Mutex`) | App lifetime | `.nabu/search_index.json` |
| Knowledge graph | Backend (`VaultGraph: RwLock`) | App lifetime | `.nabu/graph_state.json` |
| Version snapshots | Backend (`recovery.rs`) | Per note | `.nabu/versions/{id}/*.json` |
| Event bus | Backend (`EventBus`) | App lifetime | In-memory (not persisted) |

---

## 3. Reactive State Map

### 3.1 Leptos Reactivity

The frontend uses Leptos 0.7.8 with `ContextProvider` pattern. All state is `RwSignal` or derived `Memo`. No `Resource` (async data) is used — all async data loading happens through explicit `spawn_local` + `tauri_invoke` calls.

#### Signal Update Propagation

```
NoteEditor content change
    ↓
Debounce (250ms, AUTOSAVE_DELAY_MS, note_editor.rs:124-155)
    ↓
save_status.status.set(Saving)           ← signal update #1
save_status.detail.set(...)               ← signal update #2
spawn_local(async move { tauri_invoke("note_save", ...) })
    ↓
On Ok:  save_status.status.set(Saved)     ← signal update #3
On Err: save_status.status.set(Failed)    ← signal update #4
```

#### Effect-Driven Updates

1. **Active note sync** (`app.rs:111-120`): `Effect::new` watches `workspace.active_path` and updates `active_note` signal + `record_recent_note`.
2. **Session persistence** (`app.rs:164-189`): `Effect::new` watches nav view_mode, sidebar visibility, active_note, cursor, scroll → bumps `session_dirty` signal → 800ms timeout → `session_save` IPC.
3. **TaskIndicator** (`feedback.rs:778-809`): `view!` closure reads `tasks.tasks.get()` on each render tick.

#### Memo Usage

`HistoryContext` uses `Memo` for computed values? No — both `can_undo` and `can_redo` are plain `RwSignal<bool>` updated by explicit `refresh_history_state()` calls (`history.rs:103-112`).

### 3.2 Backend Reactivity

The backend has NO reactive system. It uses **synchronous pub/sub** via `EventBus`:

```rust
// EventBus::publish (bus.rs:69-76)
pub fn publish(&self, event_kind: &str, event: &Events) {
    let inner = self.inner.lock().unwrap();  // ← synchronous Mutex
    if let Some(subscribers) = inner.subscribers.get(event_kind) {
        for subscriber in subscribers {
            (subscriber.handler)(event);  // ← INLINE call, no async
        }
    }
}
```

**All EventBus handlers are synchronous.** The `ITEM_STORED` subscriber in `lib.rs:162-177` calls `storage.load()`, `indexer.lock()`, `indexer.index_object()`, `graph.write()`, `graph.add_node()` — all synchronous operations within a Mutex lock. There is no async dispatch of events.

---

## 4. Backend State Map

### 4.1 StorageManager (`crates/nabu-core/src/storage/manager.rs:33-37`)

```rust
pub struct StorageManager {
    store: RwLock<HashMap<Uuid, KnowledgeObject>>,  // ← in-memory cache
    vault_path: PathBuf,                            // ← vault root
    event_bus: Option<EventBus<PipelineEvent>>,     // ← pub ITEM_STORED
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"storage_manager"`
- **Singleton**: Yes — one instance per vault, created at `lib.rs:73-76`
- **Cache**: `HashMap<Uuid, KnowledgeObject>` under `RwLock` — serves fast lookups
- **Persistence**: Writes to vault root as `.md`/`.txt`/`.html`/`.json`, sidecar metadata under `../../.nabu`
- **`save()` method** (line 142): Writes content file + JSON sidecar → updates cache → publishes `ITEM_STORED`
- **`load()` method** (line 199): Cache hit → return; cache miss → reconstruct from disk

### 4.2 VaultGraph (`../../crates/nabu-core/src/graph/mod.rs`)

```rust
pub struct VaultGraph {
    nodes: RwLock<HashMap<Uuid, KnowledgeObject>>,
    edges: RwLock<Vec<GraphEdge>>,
    adjacency: RwLock<HashMap<Uuid, HashSet<Uuid>>>,
    persistence: Option<GraphPersistence>,
    event_bus: Option<EventBus<PipelineEvent>>,
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"vault_graph"`
- **Singleton**: Yes — created at `lib.rs:143-146`
- **Lock strategy**: Three separate `RwLock`s (`nodes`, `edges`, `adjacency`) — each method scopes its lock to avoid deadlock with persistence (line 336-339: "std::sync::RwLock is not reentrant")
- **`add_node()` method** (line 335): Inserts into `nodes` map → publishes `GRAPH_UPDATED` → auto-persists if `persistence.auto_save`
- **Persistence**: `.nabu/graph_state.json` via `GraphPersistence` (graph/src/persistence.rs)

### 4.3 Indexer (`crates/nabu-core/src/indexer.rs:32-42`)

```rust
pub struct Indexer {
    index: RwLock<InvertedIndex>,
    event_bus: Option<EventBus<PipelineEvent>>,
    vault_path: Option<PathBuf>,
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"indexer"`
- **Singleton**: Yes — created at `lib.rs:139`
- **`index_object()` method** (line 138): Tokenizes object → updates inverted index → publishes `INDEX_UPDATED`
- **Persistence**: `.nabu/search_index.json`

### 4.4 EventBus (`crates/nabu-core/src/event_bus/bus.rs:10-12`)

```rust
pub struct EventBus<Events: Clone + Send + Sync + 'static> {
    inner: Arc<Mutex<BusInner<Events>>>,
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"event_bus"`
- **Singleton**: Yes — created at `lib.rs:57`
- **Subscription**: `subscribe(kind: &str, handler: F)` returns `Subscription` (bus.rs:36-66)
- **Publishing**: `publish(kind: &str, event: &Events)` — synchronous, inline (bus.rs:69-76)
- **Subscribers**: Stored in `HashMap<String, Vec<SubscriberEntry>>` with `Box<dyn Fn(&Events) + Send + Sync>`
- **No async**: Handlers execute within the Mutex lock; no channel, no tokio::spawn

### 4.5 CaptureEngine (`crates/nabu-core/src/capture/engine.rs:52-58`)

```rust
pub struct CaptureEngine {
    handlers: Vec<Arc<dyn CaptureHandler>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    queue: Option<Arc<DurableJobQueue>>,
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"capture_engine"`
- **Singleton**: Yes — created at `lib.rs:124-128`
- **`ingest()` method** (line 54): Routes to handler → publishes `ITEM_CAPTURED` → enqueues job

### 4.6 WorkerPool (`crates/nabu-core/src/jobs/workers/pool.rs:14-27`)

```rust
pub struct WorkerPool {
    worker_count: usize,           // 4 (lib.rs:120)
    queue: Arc<dyn Queue>,
    executors: Arc<ExecutorRegistry>,
    shutdown: Arc<ShutdownCoordinator>,  // 30s drain timeout
    backpressure: Arc<BackpressureController>,
    handles: tokio::sync::Mutex<Vec<JoinHandle<()>>>,
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"worker_pool"`
- **Singleton**: Yes — created at `lib.rs:120`, started via `tauri::async_runtime::spawn` (lib.rs:349)
- **Workers**: 4 `Worker` instances (pool.rs:66), each `tokio::spawn`'d (pool.rs:75)
- **Shutdown**: `ShutdownCoordinator` with 30s drain (`workers/shutdown.rs`)

### 4.7 DurableJobQueue (`../../crates/nabu-core/src/jobs/queue.rs`)

```rust
pub struct DurableJobQueue {
    base_path: PathBuf,
    // file-backed persistence in .nabu/queue/
}
```

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"job_queue"`
- **Singleton**: Yes — created at `lib.rs:93-94`
- **Persistence**: Jobs written to `../../.nabu/queue` as JSON files
- **Methods**: `enqueue()`, `dequeue()`, `peek()`, `cancel()`, `retry()`, `count()`

### 4.8 ProcessingPipeline (`../../crates/nabu-core/src/processing/pipeline.rs`)

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"pipeline"`
- **Singleton**: Yes — created at `lib.rs:80`
- **14 processors**: ContentExtractor, LinkExtractor, TagExtractor, FrontmatterParser, ContentHasher, StorageWriter, TextClassifier, DuplicateDetector, PriorityProcessor, MetadataEnricher, EmbeddingProcessor, ThumbnailProcessor, OcrProcessor, SummaryProcessor
- **`run()` method** (line ~56): Takes `KnowledgeObject` + `ProgressReporter` + `CancellationToken`, runs processors in order

### 4.9 PipelineExecutor (`crates/nabu-core/src/pipeline_migration/executor.rs:24`)

```rust
pub struct PipelineExecutor {
    pipeline: Arc<ProcessingPipeline>,
    event_bus: Option<EventBus<PipelineEvent>>,
    storage: Option<Arc<StorageManager>>,
}
```

- **Owner**: Registered in `ExecutorRegistry` (not `ServiceRegistry`)
- **Registered executor**: For processor names `"ocr_processor"`, `"whisper_processor"`, `"pdf_text_extraction_processor"`, `"metadata_extraction_processor"` (lib.rs:109-116)
- **`execute()` method** (line 112): Reconstructs object → runs pipeline → calls `storage.save()` (line 166) → publishes `ProcessingCompleted`/`ProcessingFailed`

### 4.10 SettingsStore (`src-tauri/src/settings.rs:254`)

```rust
pub struct SettingsStore {
    path: PathBuf,
    inner: Mutex<AppSettings>,
}
```

- **Owner**: Tauri managed state (`app.manage(settings_store)`, lib.rs:200)
- **Singleton**: Yes — loaded once at `lib.rs:196-197`
- **AppSettings**: 110 fields across 20+ categories (Appearance, Editor, Markdown, Search, Graph, Files, OCR, Accessibility, Performance, Privacy, Keyboard, Advanced, Whisper, Experimental)
- **Persistence**: JSON at `~/.config/nabu/settings.json`

### 4.11 HistoryManager (`../../crates/nabu-core/src/history/mod.rs`)

- **Owner**: `ApplicationContext` via `ServiceRegistry` keyed as `"history_manager"`
- **Singleton**: Yes — created at `lib.rs:150-152`
- **In-memory**: `Vec<HistoryEntry>` under `RwLock`
- **Reversible operations**: NoteCreate, NoteRename, NoteMove, NoteDelete, FolderCreate, NoteDuplicate
- **Not persisted** — history is lost on restart

### 4.12 ApplicationContext (`crates/nabu-core/src/registry/context.rs:141`)

```rust
pub struct ApplicationContext {
    registry: Arc<RwLock<ServiceRegistry>>,
    capability_registry: CapabilityRegistry,
    lifecycle: LifecycleManager,
    performance: Arc<PerformanceMonitor>,
}
```

- **Owner**: Tauri managed state (`app.manage(ctx)`, lib.rs:345)
- **Lifecycle**: Created → Initialized → Running → Shutdown (`registry/lifecycle.rs`)
- **Resolution**: `ctx.resolve<T: Send + Sync + 'static>(key: &str) -> Option<Arc<T>>` (context.rs:196)
- **Type erasure**: Services stored as `Arc<dyn Any + Send + Sync>` in `ServiceRegistry`

---

## 5. IPC Command Matrix

The sole frontend→backend communication function is `tauri_invoke()` (`crates/nabu-ui/src/ipc.rs:9-11`):

```rust
pub async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue {
    invoke(cmd, args).await  // → window.__TAURI__.core.invoke(cmd, {args})
}
```

### 5.1 Command Registration (`src-tauri/src/lib.rs:201-311`)

**108 commands** are registered in the `invoke_handler`. They are distributed across three source files:

| File | Logical Command Count | `#[tauri::command]` Count |
|------|----------------------|--------------------------|
| `commands.rs` | 78 | 80 |
| `history.rs` | 17 | 17 |
| `recovery.rs` | 14 | 13* |

*Note: `note_save` and `note_read` appear in recovery.rs but also have variants in commands.rs. The invoke_handler registers the recovery.rs versions (lines 289-290).

### 5.2 Complete Command Matrix

#### commands.rs (lines 357-3373, 78 logical commands)

| Command | Line | Args | Return | State Accessed | Downstream Effects |
|---------|------|------|--------|----------------|-------------------|
| `tree_list` | 357 | `State<SettingsStore>` | `Vec<TreeEntry>` | vault_path | Reads filesystem, returns tree |
| `reveal_in_file_manager` | 371/388/405 | `{path}` (cfg:unix/macos/win) | `Result<(), String>` | AppHandle | OS file manager |
| `check_vault_exists` | 429 | `State<SettingsStore>` | `Option<String>` | settings | Checks settings.last_vault_path |
| `get_current_vault` | 445 | — | `Option<String>` | settings | None |
| `select_vault_dialog` | 450 | — | `Option<String>` | AppHandle | OS dialog |
| `create_vault_dialog` | 478 | — | `Option<String>` | AppHandle | OS dialog |
| `open_dictation_pill` | 505 | `AppHandle` | `Result<(), String>` | AppHandle | Show main window |
| `close_dictation_pill` | 526 | `AppHandle` | `Result<(), String>` | AppHandle | Hide main window |
| `toggle_dictation_pill` | 534 | `AppHandle` | `Result<bool, String>` | AppHandle | Show/hide toggle |
| `start_dictation` | 552 | — | `Result<String, String>` | AppHandle | OS speech-to-text |
| `stop_dictation` | 557 | — | `Result<String, String>` | — | OS speech-to-text stop |
| `complete_setup` | 562 | `AppHandle` | `Result<(), String>` | — | First-run setup |
| `open_settings` | 573 | `AppHandle` | `Result<(), String>` | — | OS settings page |
| `note_create_file` | 581 | `{path, content?, store, ctx}` | `Result<(), String>` | SettingsStore, ApplicationContext | `std::fs::write` + `snapshot_note` + `history::push_history` |
| `note_daily` | 641 | — | `String` | — | Returns date-based filename |
| `get_settings` | 647 | `State<SettingsStore>` | `AppSettings` | settings | None |
| `settings_set` | 652 | `{key, value, store}` | `Result<(), String>` | settings | `extra_settings.insert` + persist |
| `settings_get` | 666 | `{key, store}` | `serde_json::Value` | settings | None |
| `settings_set_all` | 674 | `AppSettings, store` | `Result<(), String>` | settings | Full settings replace + persist |
| `settings_export` | 685 | `State<SettingsStore>` | `Vec<u8>` | settings | None |
| `settings_import` | 693 | `{payload, store}` | `Result<(), String>` | settings | Deserialize + persist |
| `settings_reset` | 704 | `State<SettingsStore>` | `AppSettings` | settings | Reset to defaults |
| `open_app_in_finder` | 713/725 | — | `Result<(), String>` | AppHandle | OS reveal app |
| `show_macos_notification` | 732/746 | `{title, body}` | `Result<(), String>` | AppHandle | macOS notification |
| `pin_to_taskbar` | 753/765 | — | `Result<(), String>` | AppHandle | Windows pin |
| `open_in_explorer` | 772/784 | — | `Result<(), String>` | AppHandle | OS explore |
| `open_in_file_manager` | 790 | — | `Result<(), String>` | AppHandle | OS file manager |
| `show_linux_notification` | 801/811 | `{title, body}` | `Result<(), String>` | AppHandle | Linux notification |
| `install_desktop_entry` | 811 | — | `Result<(), String>` | AppHandle | Linux desktop entry |
| `reveal_vault_in_file_manager` | 840 | `State<SettingsStore>` | `Result<(), String>` | settings | OS reveal vault |
| `inbox_subscribe` | 1000 | `State<ApplicationContext>` | `Vec<InboxItem>` | ctx | Returns inbox items |
| `inbox_get_queue` | 1005 | `State<ApplicationContext>` | `Vec<InboxItem>` | ctx | Reads from KnowledgeInbox |
| `inbox_approve` | 1025 | `{id, ctx}` | `Result<(), String>` | ctx | Routes to CaptureEngine |
| `inbox_reject` | 1067 | `{id, ctx}` | `Result<(), String>` | ctx | Archive/reject |
| `inbox_retry` | 1116 | `{id, ctx}` | `Result<(), String>` | ctx | Re-enqueue job |
| `inbox_delete` | 1129 | `{id, ctx}` | `Result<(), String>` | ctx | Delete from inbox |
| `inbox_batch_approve` | 1137 | `{ids, ctx}` | `Result<(), String>` | ctx | Batch route |
| `inbox_batch_reject` | 1150 | `{ids, ctx}` | `Result<(), String>` | ctx | Batch reject |
| `inbox_batch_delete` | 1164 | `{ids, ctx}` | `Result<(), String>` | ctx | Batch delete |
| `inbox_batch_retry` | 1177 | `{ids, ctx}` | `Result<(), String>` | ctx | Batch retry |
| `inbox_edit_metadata` | 1190 | `{id, metadata, ctx}` | `Result<(), String>` | ctx | Update metadata |
| `inbox_move` | 1226 | `{id, path, ctx}` | `Result<(), String>` | ctx | Move to vault path |
| `capture_file_drop` | 1249 | `{ctx, filename, mime_type, data}` | `Result<String, String>` | ctx | → `engine.ingest()` |
| `queue_get_all` | 1303 | `State<ApplicationContext>` | `Vec<QueueItem>` | ctx | Reads DurableJobQueue |
| `queue_set_status` | 1318 | `{id, status, ctx}` | `Result<(), String>` | ctx | Mutate queue item |
| `queue_set_priority` | 1366 | `{id, priority, ctx}` | `Result<(), String>` | ctx | Mutate queue item |
| `queue_set_progress` | 1384 | `{id, progress, ctx}` | `Result<(), String>` | ctx | Mutate queue item |
| `queue_batch_set_status` | 1404 | `{ids, status, ctx}` | `Result<(), String>` | ctx | Batch mutate |
| `queue_archive_completed` | 1664 | `State<ApplicationContext>` | `Result<usize, String>` | ctx | Archive completed jobs |
| `notes_index` | 1442 | `State<SettingsStore>` | `Vec<NoteIndexEntry>` | settings | Scans vault, builds index |
| `notes_search` | 1560 | `{query, limit, store}` | `Vec<SearchHit>` | settings, Indexer | Full-text search |
| `graph_data` | 1908 | `{limit, store}` | `GraphData` | settings | Reads VaultGraph |
| `note_links` | 2157 | `{path, store}` | `Vec<Link>` | settings | Parses wikilinks |
| `link_mention` | 2373 | `{source, target, ctx}` | `Result<(), String>` | ctx | → StorageManager + graph |
| `mention_ignore` | 2428 | `{source, target, ctx}` | `Result<(), String>` | ctx | Mark mention ignored |
| `mention_ignore_list` | 2415 | `State<ApplicationContext>` | `Vec<(String, String)>` | ctx | Read ignored mentions |
| `archive_note` | 2492 | `{path, store}` | `Result<(), String>` | settings | Move to archive dir |
| `archive_restore` | 2538 | `{path, store}` | `Result<(), String>` | settings | Restore from archive |
| `archive_list` | 2588 | `State<SettingsStore>` | `Vec<ArchiveEntry>` | settings | List archive dir |
| `smart_folders_list` | 2660 | `State<SettingsStore>` | `Vec<SmartFolder>` | settings | Read smart folders |
| `smart_folder_save` | 2674 | `{id, name, query, store}` | `Result<(), String>` | settings | Persist smart folder |
| `smart_folder_delete` | 2694 | `{id, store}` | `Result<(), String>` | settings | Remove smart folder |
| `smart_folder_evaluate` | 2732 | `{id, store}` | `Vec<String>` | settings | Evaluate query → paths |
| `calendar_notes` | 2855 | `{date, store}` | `Vec<NoteCalendarEntry>` | settings | Scan calendar notes |
| `daily_note_for` | 2888 | `{date, store}` | `String` | settings | Generate daily note path |
| `template_list` | 2926 | `State<SettingsStore>` | `Vec<Template>` | settings | Read templates dir |
| `template_save` | 2948 | `{id, name, content, store}` | `Result<(), String>` | settings | Write template file |
| `template_delete` | 2959 | `{id, store}` | `Result<(), String>` | settings | Delete template |
| `template_duplicate` | 2966 | `{id, new_name, store}` | `Result<(), String>` | settings | Copy template |
| `template_set_favourite` | 2990 | `{id, favourite, store}` | `Result<(), String>` | settings | Toggle favourite |
| `inbox_quick_capture` | 3007 | `{text, store}` | `Result<String, String>` | store | → CaptureEngine |
| `canvas_list` | 3142 | `State<SettingsStore>` | `Vec<CanvasEntry>` | settings | Read canvases |
| `canvas_get` | 3148 | `{id, store}` | `Option<Canvas>` | settings | Read canvas file |
| `canvas_save` | 3157 | `{id, data, store}` | `Result<(), String>` | settings | Write canvas file |
| `canvas_delete` | 3172 | `{id, store}` | `Result<(), String>` | settings | Delete canvas |
| `notes_diff` | 3184 | `{path, version_a, version_b, store}` | `Option<DiffResult>` | settings, recovery | Diff versions |
| `statistics_get` | 3373 | `State<ApplicationContext>` | `Statistics` | ctx | Compute vault stats |

#### history.rs (lines 64-918, 17 commands)

| Command | Line | State Accessed | Downstream Effects |
|---------|------|----------------|-------------------|
| `history_status` | 65 | `ctx` (ApplicationContext) | Read HistoryManager: can_undo, can_redo |
| `history_undo` | 82 | `ctx` | → `HistoryManager::undo()` → reverse op + StorageManager |
| `history_redo` | 90 | `ctx` | → `HistoryManager::redo()` → forward op |
| `history_clear` | 97 | `ctx` | Clears history entries |
| `history_set_depth` | 105 | `ctx, {depth}` | Mutates max_depth |
| `note_rename` | 424 | `ctx, {old, new}` | → `push_history` → `std::fs::rename` |
| `note_delete` | 480 | `ctx, {path}` | → `trash_file` → `push_history` |
| `note_restore` | 525 | `ctx, {path}` | → `restore_from_trash` → `push_history` |
| `folder_create` | 687 | `ctx, {name, parent}` | → `std::fs::create_dir` → `push_history` |
| `folder_rename` | 919 | `ctx, {old, new}` | → `std::fs::rename` |
| `note_duplicate` | 787 | `ctx, {src, dst}` | → `copy_tree` → `push_history` |
| `items_move` | 841 | `ctx, {srcs, dst}` | → `std::fs::rename` for each |
| `trash_list` | 575 | `store` (SettingsStore) | Read trash manifest |
| `trash_delete` | 586 | `store, {path}` | Delete from trash |
| `trash_restore_many` | 604 | `store, {paths}` | Restore from trash |
| `trash_purge_expired` | 668 | `store` | Delete expired trash |
| `trash_empty` | 687 | `store` | Clear all trash |

#### recovery.rs (lines 390-731, 14 commands)

| Command | Line | State Accessed | Downstream Effects |
|---------|------|----------------|-------------------|
| `note_save` | 391 | `store` (SettingsStore) | `std::fs::write` + `snapshot_note` — **NO EventBus** |
| `note_read` | 410 | `store` | `std::fs::read_to_string` |
| `versions_list` | 421 | `store, {path}` | Read version manifest |
| `versions_get` | 433 | `store, {id}` | Read version snapshot |
| `versions_restore` | 449 | `store, {id}` | Write version → file |
| `versions_duplicate` | 498 | `store, {id, new_name}` | Copy version file + manifest |
| `versions_diff` | 548 | `store, {id_a, id_b}` | Diff two versions |
| `snapshot_create` | 581 | `store, {path}` | Create version snapshot |
| `versions_all` | 632 | `store` | List all versions in vault |
| `session_save` | 664 | `store, {data}` | Write `.nabu/session.json` |
| `session_load` | 678 | `store` | Read `.nabu/session.json` |
| `session_clear` | 685 | `store` | Delete `.nabu/session.json` |
| `recovery_check` | 693 | `store` | Check `../../.nabu/.running` marker |
| `recovery_discard` | 706 | `store` | Remove recovery markers |

### 5.3 Registration Summary

| Phase | Commands | Source File(s) |
|-------|----------|----------------|
| 1. Vault & workspace | 6 | commands.rs |
| 2. Dictation & capture | 9 | commands.rs |
| 3. Navigation | 4 | commands.rs |
| 4. History (undo/redo) | 5 | history.rs |
| 5. Notes (fs operations) | 6 | history.rs |
| 6. Trash | 6 | history.rs |
| 7. Folders | 2 | history.rs |
| 8. Note duplication/movement | 2 | history.rs |
| 9. Content listing/search | 4 | commands.rs |
| 10. Graph & links | 6 | commands.rs |
| 11. Archive | 3 | commands.rs |
| 12. Smart folders | 4 | commands.rs |
| 13. Calendar & daily | 2 | commands.rs |
| 14. Templates | 5 | commands.rs |
| 15. Inbox | 11 | commands.rs |
| 16. Capture (file drop) | 1 | commands.rs |
| 17. Queue management | 6 | commands.rs |
| 18. Canvas | 4 | commands.rs |
| 19. Notes diff | 1 | commands.rs |
| 20. Statistics | 1 | commands.rs |
| 21. Recovery (save/read) | 2 | recovery.rs |
| 22. Versions | 7 | recovery.rs |
| 23. Sessions | 3 | recovery.rs |
| 24. Crash recovery | 2 | recovery.rs |
| 25. Platform integrations | 7 | commands.rs (cfg variants) |

**Total: 108 registered commands** (some with `#[cfg]` platform variants expanding to additional `#[tauri::command]` definitions).

---

## 6. Event Flow Diagram

### 6.1 EventBus Event Types

`PipelineEvent` (`crates/nabu-core/src/event_bus/events.rs:6-29`) — 10 variants:

| Variant | Struct | Published By | Subscribed By |
|---------|--------|-------------|---------------|
| `ItemCaptured` | `ItemCapturedEvent` (line 62) | `CaptureEngine::ingest` (engine.rs:64-74) | CaptureEngine, UI (inbox) |
| `ItemProcessingStarted` | `ItemProcessingStartedEvent` (line 72) | `PipelineExecutor::execute` (executor.rs:122-125) | UI (progress) |
| `ItemProcessingProgress` | `ItemProcessingProgressEvent` (line 80) | `ProcessingPipeline` processors | UI (progress bars) |
| `ItemProcessingCompleted` | `ItemProcessingCompletedEvent` (line 88) | `PipelineExecutor::execute` (executor.rs:155-157) | — |
| `ItemProcessingFailed` | `ItemProcessingFailedEvent` (line 97) | `PipelineExecutor::execute` (executor.rs:144-147) | UI (error) |
| `ItemStored` | `ItemStoredEvent` (line 108) | `StorageManager::save` (manager.rs:181-189) | Indexer + VaultGraph (lib.rs:162-177) |
| `IndexUpdated` | `IndexUpdatedEvent` (line 116) | `Indexer::index_object` (indexer.rs:153-161) | — |
| `GraphUpdated` | `GraphUpdatedEvent` (line 130) | `VaultGraph::add_node`/`remove_node` (graph/mod.rs:345-353) | — |
| `ItemCancelled` | `ItemCancelledEvent` (line 146) | WorkerPool (job cancellation) | UI (status) |
| `ItemRetried` | `ItemRetriedEvent` (line 153) | WorkerPool (job retry) | UI (status) |

### 6.2 Event Kind Constants (`crates/nabu-core/src/event_bus/events.rs:31-43`)

| Constant | Value | Variant |
|----------|-------|---------|
| `ITEM_CAPTURED` | `"item.captured"` | `ItemCaptured` |
| `ITEM_PROCESSING_STARTED` | `"item.processing.started"` | `ItemProcessingStarted` |
| `ITEM_PROCESSING_PROGRESS` | `"item.processing.progress"` | `ItemProcessingProgress` |
| `ITEM_PROCESSING_COMPLETED` | `"item.processing.completed"` | `ItemProcessingCompleted` |
| `ITEM_PROCESSING_FAILED` | `"item.processing.failed"` | `ItemProcessingFailed` |
| `ITEM_STORED` | `"item.stored"` | `ItemStored` |
| `INDEX_UPDATED` | `"index.updated"` | `IndexUpdated` |
| `GRAPH_UPDATED` | `"graph.updated"` | `GraphUpdated` |
| `ITEM_CANCELLED` | `"item.cancelled"` | `ItemCancelled` |
| `ITEM_RETRIED` | `"item.retried"` | `ItemRetried` |

### 6.3 EventBus Subscription Map (actual production wiring)

**Only subscribers registered at production startup** (`src-tauri/src/lib.rs:155-177`):

```
EventBus (lib.rs:57, Arc<EventBus<PipelineEvent>>)
  │
  └── subscribe("item.stored", closure)  ← lib.rs:162
      │
      ├── On ItemStored event:
      │   ├── storage.load(stored.object_id) → KnowledgeObject   (manager.rs:164)
      │   ├── indexer.lock() → index_object(&object)            (lib.rs:166)
      │   │   └── publishes "index.updated"                     (indexer.rs:154)
      │   ├── graph.write() → add_node(&object)                 (lib.rs:171)
      │   │   └── publishes "graph.updated"                     (graph/mod.rs:346)
      │   └── graph auto-persists if auto_save                (graph/mod.rs:357-359)
      │
      └── NO subscribers for: item.captured, item.processing.*,
          index.updated, graph.updated, item.cancelled, item.retried
```

**GraphEventBridge** (`graph/incremental/event_wiring.rs:26-189`) is **NOT wired in production**. It subscribes to `ITEM_STORED` and translates events into incremental graph updates, but is only invoked in tests (`event_wiring.rs:191-232`). Production uses the inline closure at `lib.rs:162-177`.

### 6.4 Event Propagation Characteristics

| Property | Value | Evidence |
|----------|-------|----------|
| Synchronization | **Synchronous** | `bus.publish()` acquires `Mutex` lock and calls handlers inline (`bus.rs:70-75`) |
| Distribution | **Point-to-point via pub/sub** | `subscribe(kind, handler)` stores `Box<dyn Fn>` in HashMap (`bus.rs:36-51`) |
| Cross-thread | **No** — all on same thread | No `tokio::spawn` or channel in publish path |
| Frontend delivery | **None** | No `window.emit()` / `app.emit()` calls anywhere in src-tauri or nabu-core |
| Persistence | **No** — in-memory only | `BusInner` has no persistence; events lost on crash |
| Unsubscribe | Manual via `Subscription::unsubscribe()` | `bus.rs:79-84`; `Subscription` does NOT auto-unsubscribe on drop (`bus.rs:116-117`) |

---

## 7. Workflow Trace Matrix

### 7.1 Save Note (User Editing)

```
User types in NoteEditor
    ↓  [250ms debounce, note_editor.rs:124-155]
Effect::new(closure)
    ├── Sets save_status.status = Saving    [save_status.rs:55, note_editor.rs:127]
    └── spawn_local(async {
            tauri_invoke("note_save", {path, content})   [ipc.rs:9, note_editor.rs:136]
                ↓
            Tauri command dispatcher (lib.rs:289 → recovery::note_save)
                ↓
            recovery::note_save (recovery.rs:391)
                ├── std::fs::write(&abs, &content)       [recovery.rs:403] ← DIRECT WRITE
                ├── snapshot_note(&vault, &path)         [recovery.rs:404] ← versioning only
                └── Returns Ok(())                       [recovery.rs:405]
                ↓  NO EventBus publication
                ↓  NO StorageManager::save call
                ↓  NO ITEM_STORED event
                ↓  NO Indexer::index_object
                ↓  NO VaultGraph::add_node
                ↓
            On Ok: save_status.status = Saved  [note_editor.rs:139]
            On Err: save_status.status = Failed [note_editor.rs:146]
    })
```

**State mutations**: `file on disk` (direct write), `.nabu/versions/{id}` (snapshot). 
**EventBus events published**: **NONE**
**Frontend signals updated**: `save_status.status`, `save_status.detail`
**Downstream systems notified**: **NONE** — index and graph are stale

### 7.2 Save Note (Content Capture Pipeline)

```
User drops file on DictationPill / Inbox / FileTree
    ↓
tauri_invoke("capture_file_drop", {filename, mime_type, data})  [dictation_pill.rs:104, inbox.rs:255]
    ↓
Tauri command: capture_file_drop (commands.rs:1249)
    ↓
engine.ingest(CaptureRequest { ... })  [commands.rs:1269]
    ↓
CaptureEngine::ingest (engine.rs:54-101)
    ├── route(&request) → handler.process() → KnowledgeObject  [engine.rs:56]
    ├── event_bus.publish(ITEM_CAPTURED, ...)                  [engine.rs:64-74]
    ├── queue.enqueue(Job { ... })                             [engine.rs:78-101]
    └── Returns object_id                                    [engine.rs:103]
    ↓  ITEM_CAPTURED published — NO frontend subscriber
    ↓
DurableJobQueue::enqueue(job) → writes to .nabu/queue/  [engine.rs:101]
    ↓
WorkerPool (started lib.rs:349 via tauri::async_runtime::spawn)
    ├── tokio::spawn(async { worker.run().await })             [pool.rs:75]
    │   ↓
    │   Worker::run (worker.rs:50)
    │   ├── loop { dequeue() → job }                           [worker.rs:71]
    │   ├── lookup executor in ExecutorRegistry                [worker.rs:~110]
    │   └── executor.execute(job, progress, cancellation)     [worker.rs:~130]
    │       ↓
    │       PipelineExecutor::execute (executor.rs:112-176)
    │       ├── publish ProcessingStarted                     [executor.rs:122-125]
    │       ├── object_from_job(job) → KnowledgeObject       [executor.rs:130]
    │       ├── pipeline.run(object, progress, cancel)       [executor.rs:134-137]
    │       │   ├── ContentExtractor → text                  
    │       │   ├── LinkExtractor → [[wikilinks]]           
    │       │   ├── TagExtractor → #tags                    
    │       │   ├── FrontmatterParser → YAML                
    │       │   ├── ContentHasher → checksum               
    │       │   └── StorageWriter → NO direct write (pipeline defers to executor)  [pipeline.rs:208]
    │       ├── publish ProcessingCompleted                  [executor.rs:155-157]
    │       └── storage.save(&result.object)                [executor.rs:166]
    │           ↓
    │           StorageManager::save (manager.rs:142-193)
    │           ├── std::fs::write(content file)              [manager.rs:156]
    │           ├── std::fs::write(JSON sidecar)              [manager.rs:171]
    │           ├── cache.insert(object.id, object)            [manager.rs:176]
    │           └── event_bus.publish(ITEM_STORED, ...)       [manager.rs:181-189]
    │               ↓
    │               ITEM_STORED subscriber (lib.rs:162-177)
    │               ├── storage.load(object_id) → KnowledgeObject   [lib.rs:164]
    │               ├── indexer.index_object(&object)              [lib.rs:166]
    │               │   ├── update inverted index                [indexer.rs:142-150]
    │               │   └── publish INDEX_UPDATED                [indexer.rs:153-161]
    │               ├── graph.add_node(&object)                [lib.rs:171]
    │               │   ├── insert into nodes map              [graph/mod.rs:341-342]
    │               │   ├── publish GRAPH_UPDATED              [graph/mod.rs:345-353]
    │               │   └── auto-persist if enabled            [graph/mod.rs:357-359]
    │               │       └── persistence.save(self)         [graph/mod.rs:359]
    │               └── return                                 [lib.rs:175]
    │           ↓
    │           Returns Ok(vault_rel)                           [manager.rs:192]
    │       ↓
    │       PipelineExecutor returns Ok(completed_job)           [executor.rs:175]
    │   ↓
    │   Worker reports completion to queue                       [worker.rs:~170]
    └── Workers loop, awaiting next dequeue                     [worker.rs:63]
    ↓
Frontend: capture_file_drop IPC returns Ok(id) → toast "Captured"  [commands.rs:1271, inbox.rs:257]
```

### 7.3 Create Note

```
User creates note via FileTree / command palette
    ↓
tauri_invoke("note_create_file", {path, content})   [app.rs:213, commands.rs:581]
    ↓
commands::note_create_file (commands.rs:581-638)
    ├── snapshot_note(vault_path, &path)             [commands.rs:600] ← versioning
    ├── std::fs::write(&safe_path, &content)        [commands.rs:608] ← DIRECT WRITE
    ├── history::push_history(                      [commands.rs:614]
    │   │   HistoryOp::NoteCreate,
    │   │   undo: remove file,
    │   │   redo: write file
    │   └── registers with HistoryManager)
    └── Returns Ok(())                              [commands.rs:637]
    ↓  NO EventBus publication
    ↓  NO StorageManager::save call
    ↓  NO ITEM_STORED event
    ↓  NO Indexer update
    ↓  NO VaultGraph update
```

**State mutations**: `file on disk` (direct write), `.nabu/versions/` (snapshot), HistoryManager (undo entry).
**EventBus events**: **NONE**

### 7.4 Delete Note

```
User deletes note via FileTree context menu
    ↓
tauri_invoke("note_delete", {path})                [via HistoryContext]
    ↓
history::note_delete (history.rs:480)
    ├── trash_file(vault_path, src) → moves to .nabu/trash/   [history.rs:315]
    │   ├── write_trash_manifest                            [history.rs:303]
    │   └── returns trash_path                            [history.rs:315]
    ├── push_history(                                     [history.rs:~490]
    │   undo: restore_from_trash,
    │   redo: trash_file)
    └── Returns                              [history.rs:~490]
    ↓  NO EventBus publication
    ↓  NO Indexer update (stale search)
    ↓  NO VaultGraph update (stale graph)
```

### 7.5 Open Vault

```
App startup (app.rs:235-256)
    ↓
tauri_invoke("check_vault_exists", {})  → recovery::check_vault_exists (commands.rs:429)
    ├── reads settings.last_vault_path                      [settings.rs:297]
    └── returns Option<String>                             [commands.rs:430]
    ↓
On mount (lib.rs:312-386):
    ├── SettingsStore::load(&settings_path)                 [lib.rs:196-197]
    ├── build_application_context(vault_path)               [lib.rs:331]
    │   ├── EventBus::new()                                  [lib.rs:57]
    │   ├── StorageManager::with_event_bus(...)              [lib.rs:73]
    │   ├── build_standard_pipeline(Some(event_bus))        [lib.rs:80]
    │   ├── DurableJobQueue::new(...)                        [lib.rs:93]
    │   ├── PipelineExecutor::with_event_bus(...)           [lib.rs:104]
    │   ├── WorkerPool::new(4, queue, executors)            [lib.rs:120]
    │   ├── build_default_capture_engine(...)               [lib.rs:124]
    │   ├── Indexer::with_event_bus(...)                    [lib.rs:139]
    │   ├── VaultGraph::with_persistence(...)               [lib.rs:143]
    │   ├── HistoryManager::new()                           [lib.rs:150]
    │   └── subscribe ITEM_STORED → Indexer + Graph        [lib.rs:162-177]
    ├── app.manage(ctx)                                     [lib.rs:345]
    ├── tauri::async_runtime::spawn({ pool.start() })       [lib.rs:349]
    ├── tauri::async_runtime::spawn({ socket_server })      [lib.rs:360]
    ├── mark_running(&vault_path)                           [lib.rs:329]
    ├── ctx.initialize()                                    [lib.rs:339]
    └── ctx.start()                                         [lib.rs:342]
    ↓
Frontend (app.rs:235-256):
    ├── If vault exists → set_screen(MainDashboard)
    └── If not → set_screen(VaultSetup)
    ↓
On mount (app.rs:127-160):
    ├── tauri_invoke("recovery_check", {}) → check for crash
    │   └── recovery::recovery_check reads .nabu/.running marker
    └── load_all_nav_state(nav) + load_notes_index(nav)
        ├── tauri_invoke("get_settings", {}) → get_settings
        ├── tauri_invoke("notes_index", {}) → notes_index
        └── tauri_invoke("smart_folders_list", {}) → smart_folders_list
```

### 7.6 Settings Change

```
User changes theme in SettingsPanel
    ↓
tauri_invoke("settings_set", {key: "nabu.theme", value: "dark"})  [app.rs:271-272]
    ↓
commands::settings_set (commands.rs:652-663)
    ├── store.update(|s| s.extra_settings.insert(key, value))  [settings.rs:297]
    │   └── AppSettings mutex locked, field updated          [settings.rs:293-308]
    ├── SettingsStore::save() → write JSON to disk            [settings.rs:310-315]
    └── Returns Ok(())                                        [commands.rs:662]
    ↓  NO event published
    ↓  NO frontend signal update
    ↓  Frontend signal must be updated separately by the calling component
```

### 7.7 Search Query

```
User types query in SearchPage
    ↓
Signal: search_query.set(query)  [navigation/state.rs:186]
    ↓
spawn_local(async { tauri_invoke("notes_search", {query, limit}) })
    ↓
commands::notes_search (commands.rs:1560)
    ├── reads settings (vault_path)                          [commands.rs:~1575]
    ├── ctx.resolve("indexer") → Indexer                   [commands.rs:~1580]
    │   └── indexer.search(&query)                          [indexer.rs:180]
    │       ├── tokenize query                            [indexer.rs:~183]
    │       ├── lookup in inverted index                  [indexer.rs:~190]
    │       └── return matching UUIDs                      [indexer.rs:~195]
    └── returns Vec<SearchHit>                             [commands.rs:~1590]
    ↓
Frontend: serde_wasm_bindgen::from_value → Vec<SearchHit>
    ↓
Signal: search_results.set(results)  → UI re-renders
```

**Note**: Search reads from `Indexer`'s in-memory inverted index (`RwLock<InvertedIndex>`), NOT from the persisted `search_index.json` file. If the index hasn't been built (e.g., first startup, or notes saved via `note_save` bypassing the pipeline), search results will be stale or empty.

### 7.8 Inbox Quick Capture

```
User submits text via DictationPill scratchpad
    ↓
tauri_invoke("inbox_quick_capture", {text})  [dictation_pill.rs:~90]
    ↓
commands::inbox_quick_capture (commands.rs:3007)
    ├── ctx.resolve("capture_engine")                          [commands.rs:~3010]
    └── engine.ingest(CaptureRequest::text(text))             [commands.rs:~3020]
        ↓
        (Same as capture pipeline — see section 7.2)
```

### 7.9 File Tree Refresh

```
User creates/deletes/moves a note
    ↓
history operation completes
    ↓
workspace.refresh_tree.update(|v| *v += 1)  [file_tree.rs:~?]
    ↓
use workspace() → read refresh_tree signal  [file_tree.rs:~?]
    ↓
tauri_invoke("tree_list", {})  [file_tree.rs:~?]
    ↓
commands::tree_list (commands.rs:357)
    ├── read settings.last_vault_path                        [commands.rs:358]
    ├── scan_tree(dir, "")                                    [commands.rs:311]
    │   ├── std::fs::read_dir                               [manager.rs:311]
    │   ├── filter .nabu/, .DS_Store                        [manager.rs:~320]
    │   └── build TreeEntry list                            [manager.rs:~330]
    └── returns Vec<TreeEntry>                              [commands.rs:359]
    ↓
Frontend: serde_wasm_bindgen::from_value → Vec<TreeEntry>
    ↓
Signal: tree_entries.set(new_entries) → FileTree re-renders
```

**Note**: `refresh_tree` is `RwSignal<u32>` — a simple counter that is bumped. The FileTree component must be watching it and re-fetching via `tree_list` IPC. There is no event-based tree update.

---

## 8. Event Cascade Analysis

### 8.1 Save via Capture Pipeline (Full Cascade)

When a file is captured via `capture_file_drop`:

```
capture_file_drop IPC → engine.ingest()
    │
    ├── [E1] ITEM_CAPTURED ← engine.rs:64
    │   └── (no subscribers in production)
    │
    ├── DurableJobQueue::enqueue(job) → write to .nabu/queue/
    │
    └── (async, via tokio::spawn in WorkerPool)
        Worker::dequeue → PipelineExecutor::execute
            │
            ├── [E2] ProcessingStarted ← executor.rs:124
            │   └── (no subscribers)
            │
            ├── ProcessingPipeline::run (14 processors)
            │   ├── [various] progress events ← emitted to ProgressReporter
            │   └── StorageWriter → deferred to executor
            │
            ├── [E3] ProcessingCompleted ← executor.rs:156
            │   └── (no subscribers)
            │
            └── StorageManager::save(object)
                │
                ├── std::fs::write (content + sidecar)
                ├── cache.insert (in-memory)
                │
                └── [E4] ITEM_STORED ← manager.rs:181  ← THE CASCADE ROOT
                    │
                    ├── Subscriber 1: Indexer::index_object
                    │   ├── update inverted index
                    │   ├── cache insert
                    │   └── [E5] INDEX_UPDATED ← indexer.rs:154
                    │       └── (no subscribers)
                    │
                    └── Subscriber 2: VaultGraph::add_node
                        ├── insert into nodes map
                        ├── [E6] GRAPH_UPDATED ← graph/mod.rs:346
                        │   └── (no subscribers)
                        └── auto-persist → graph_state.json
```

**Cascade depth**: 6 event levels (E1→E2→E3→E4→E5/E6)
**Cascade breadth**: 1 → 1 → 1 → 1 → 2 parallel → 2 terminal
**Frontend participation**: **ZERO** — no events reach the frontend

### 8.2 Save via Note Editor (No Cascade)

When a user edits and saves a note:

```
note_save IPC → recovery::note_save
    │
    ├── std::fs::write (direct to disk)
    ├── snapshot_note (version snapshot)
    └── Returns Ok
    │
    └── [NO EVENTS PUBLISHED]
    └── [NO ITEM_STORED]
    └── [NO Indexer update]
    └── [NO VaultGraph update]
```

**Cascade depth**: 0
**Frontend participation**: Local signal updates only (`SaveStatus::Saving → Saved/Failed`)

### 8.3 Undo/Redo (Partial Cascade)

```
User presses Cmd+Z → tauri_invoke("history_undo")
    ↓
history::history_undo (history.rs:82)
    ├── HistoryManager::undo() → pops entry, calls reverse closure
    │   ├── reverse closure → e.g., std::fs::remove_file or restore_from_trash
    │   └── NO ITEM_STORED published
    └── Returns Option<String> (label)
    ↓
Frontend: toasts.info("Undo", label)
    ↓
notify_history_changed() → CustomEvent "nabu:history-changed" (DOM, frontend-only)
    ↓
Trash screen / FileTree listens for event via window_event_listener_untyped  [file_tree.rs:419]
    ↓
Manual re-fetch: tauri_invoke("tree_list") or tauri_invoke("trash_list")
```

**Key insight**: The undo/redo system uses **frontend custom events** (DOM-level `CustomEvent`) to notify components, NOT the backend EventBus. This is a separate notification mechanism entirely.

---

## 9. Data Ownership Analysis

### 9.1 KnowledgeObject

| Aspect | Details |
|--------|---------|
| **Canonical owner** | Backend `StorageManager` (`HashMap<Uuid, KnowledgeObject>` under `RwLock`, manager.rs:34) |
| **Construction** | `CaptureEngine::route()` produces `KnowledgeObject` (engine.rs:56) |
| **Mutation** | `StorageManager::save()` (manager.rs:142) — only writer |
| **Frontend representation** | `NoteEntry` (`state.rs:131`) — separate serde DTO, NOT the same type |
| **Caching** | StorageManager in-memory `HashMap` + `../../.nabu` sidecars on disk |
| **Derivation** | `NoteEntry` derived from filesystem scan by `collect_notes()` (commands.rs:1705) |

### 9.2 AppSettings

| Aspect | Details |
|--------|---------|
| **Canonical owner** | Backend `SettingsStore` (`Mutex<AppSettings>`, settings.rs:256-257) |
| **Mutation** | `settings_set` (commands.rs:652) → `store.update()` → `store.save()` |
| **Frontend representation** | None — frontend reads via `get_settings` IPC, writes via `settings_set` IPC |
| **Persistence** | `~/.config/nabu/settings.json` |
| **Duplication** | Frontend `NavContext` caches some settings values (e.g., `notes_index`) in `RwSignal<Vec<NoteIndexEntry>>` — these are derived, not the source of truth |

### 9.3 Session State

| Aspect | Details |
|--------|---------|
| **Canonical owner** | Backend `recovery.rs` (`session_save`, line 664) → `.nabu/session.json` |
| **Frontend representation** | `SessionState` struct (`app.rs:49-74`) — separate serde DTO |
| **Duplication** | Both frontend and backend have session state; frontend persists on timer (800ms debounce, app.rs:175-188) and on `beforeunload` (app.rs:193-203); backend persists per-save via `session_save` command |

### 9.4 Vault Paths

| Aspect | Details |
|--------|---------|
| **Canonical source** | `SettingsStore.last_vault_path` (settings.rs:59) |
| **Frontend access** | `NavContext.vault_name` (derived display name) — frontend never stores the full path |
| **Backend access** | `vault_path(&store)` helper (recovery.rs:42) reads from SettingsStore |
| **Duplication** | `vault_path` is recomputed from settings in every command that needs it — no caching |

### 9.5 Graph Data

| Aspect | Details |
|--------|---------|
| **Canonical owner** | Backend `VaultGraph` (`HashMap<Uuid, KnowledgeObject>` in `nodes: RwLock`, graph/mod.rs) |
| **Persistence** | `.nabu/graph_state.json` via `GraphPersistence` |
| **Frontend representation** | `GraphData` DTO (commands.rs:1908 returns `GraphData`) |
| **Duplication** | Frontend re-fetches via `graph_data` IPC on demand — no live sync |

### 9.6 Index Data

| Aspect | Details |
|--------|---------|
| **Canonical owner** | Backend `Indexer` (`RwLock<InvertedIndex>`, indexer.rs:33-42) |
| **Persistence** | `.nabu/search_index.json` |
| **Frontend representation** | `Vec<SearchHit>` / `Vec<NoteIndexEntry>` — DTOs fetched via IPC |
| **Duplication** | Frontend `NavContext.notes_index` (`RwSignal<Vec<NoteIndexEntry>>`, state.rs:206) is populated by `load_notes_index()` which calls `notes_index` IPC (app.rs:90) — NOT by Indexer directly |

---

## 10. Communication Bottlenecks

### 10.1 Frontend-Backend Event Gap (CRITICAL)

**Problem**: Backend publishes 10 event types via `EventBus`, but the frontend has **zero** subscription mechanisms.

**Evidence**: 
- `tauri_invoke()` (`ipc.rs:9-11`) — only supports `invoke` (request-response)
- No `window.emit()` or `app.emit()` calls anywhere in `../../src-tauri` or `../../crates/nabu-core`
- `EventBus::publish()` (`bus.rs:69-76`) calls handlers synchronously within a Mutex lock — handlers are `Box<dyn Fn(&Events)>` with no async capability

**Impact**: 
- When a note is captured via the pipeline, the `INDEX_UPDATED` and `GRAPH_UPDATED` events are fired but the frontend's file tree, search results, and graph view are never refreshed automatically
- The frontend must explicitly re-fetch data via `tree_list`, `notes_search`, `graph_data`, etc.
- No progress indication for long-running captures (OCR, Whisper, PDF)
- No real-time notifications for inbox items

### 10.2 Dual Save Paths (CRITICAL)

**Problem**: Two separate code paths for persisting notes with completely different event semantics.

| Path | Command | Storage | Events | Index Updated? | Graph Updated? |
|------|---------|---------|--------|----------------|----------------|
| User editing | `note_save` (recovery.rs:391) | `std::fs::write` | None | **NO** | **NO** |
| Content capture | `capture_file_drop` → `storage.save` (manager.rs:142) | `storage.save` | `ITEM_STORED` | Yes | Yes |
| Note creation | `note_create_file` (commands.rs:581) | `std::fs::write` | None | **NO** | **NO** |

**Impact**: Search index and knowledge graph are only updated when content flows through the capture pipeline. Direct note edits are invisible to search and graph until a manual refresh or re-import.

### 10.3 Synchronous EventBus (MEDIUM)

**Problem**: `EventBus::publish()` holds a Mutex lock during handler execution.

**Evidence**: `bus.rs:70-75`:
```rust
let inner = self.inner.lock().unwrap();
if let Some(subscribers) = inner.subscribers.get(event_kind) {
    for subscriber in subscribers {
        (subscriber.handler)(event);  // ← synchronous, within lock
    }
}
```

**Impact**: If any handler is slow (e.g., `Indexer::index_object` on a large document), all other subscribers and all other `publish()` calls block. No concurrency between event handlers.

### 10.4 Worker Pool Polling (LOW)

**Problem**: Workers use `tokio::time::sleep(100ms)` polling when no jobs are available.

**Evidence**: `worker.rs:73-76`:
```rust
Ok(None) => {
    tokio::time::sleep(Duration::from_millis(100)).await;
    continue;
}
```

**Impact**: 100ms latency for job pickup when queue transitions from empty to non-empty. Could use `tokio::sync::Notify` for immediate wakeup.

### 10.5 No Async Channels for Frontend Updates (CRITICAL)

**Problem**: There are no mpsc or broadcast channels connecting the backend to the frontend.

**Evidence**: `grep` for `mpsc`, `oneshot`, `async_channel`, `broadcast` in `../../crates/nabu-core/src` and `../../src-tauri/src`:
- `mpsc` used only in `WorkerPool` (worker_channel.rs:3, worker.rs:180) — internal to nabu-core
- `oneshot` used only in job cancellation (errors.rs:38)
- No channels cross the Tauri IPC boundary

**Impact**: The only mechanism for backend→frontend communication is synchronous request-response IPC. The frontend cannot receive asynchronous events.

### 10.6 Native Messaging Socket — Fire-and-Forget (MEDIUM)

**Problem**: The native messaging socket server spawns per-connection tasks that call `capture_engine.ingest` and then discard the result.

**Evidence**: `native_messaging_socket.rs:237`:
```rust
tokio::spawn(async move {
    if let Err(e) = handle_connection(stream, engine).await {
        eprintln!("Connection error: {}", e);
    }
});
```
And `handle_connection` (line 259) calls `engine.ingest()` and writes a response back to the socket, but does not push progress events back.

**Impact**: Native messaging clients (Safari extension, browser extension) receive only a success/failure response, not real-time processing progress.

---

## 11. State Consistency Assessment

### 11.1 Consistency Model

| System | Consistency | Mechanism | Evidence |
|--------|------------|-----------|----------|
| **Search index** | Event-driven (partial) | `ITEM_STORED` → `Indexer::index_object` | lib.rs:162-177 |
| **Knowledge graph** | Event-driven (partial) | `ITEM_STORED` → `VaultGraph::add_node` | lib.rs:162-177 |
| **File tree** | Polling | `refresh_tree` signal → `tree_list` IPC | workspace.rs:37, app.rs:90 |
| **Session state** | Event-driven (timer) | 800ms debounce → `session_save` IPC | app.rs:173-189 |
| **Settings** | Synchronous write | `settings_set` IPC → Mutex → disk | commands.rs:652-663 |
| **History (undo/redo)** | Event-driven (frontend) | DOM CustomEvent → manual re-fetch | history.rs:152-159 |
| **Inbox** | Polling | Explicit `inbox_query`/poll | inbox.rs frontmatter |

### 11.2 Inconsistency Scenarios

1. **Note edit → search results stale**: User edits note content → `note_save` writes to disk → no `ITEM_STORED` → Indexer never updates → search returns stale results until manual re-index.

2. **Note edit → graph edges stale**: User adds a `[[wikilink]]` in note editor → `note_save` writes to disk → no `ITEM_STORED` → `VaultGraph::add_node` never called → graph doesn't reflect new link.

3. **Note creation via editor → not indexed**: User creates a new note via `note_create_file` → `std::fs::write` → no `ITEM_STORED` → note invisible to search and graph.

4. **Settings change → frontend signals stale**: User changes a setting → `settings_set` IPC updates `SettingsStore` → returns `Ok(())` → frontend component must independently update its local signal. No automatic signal propagation.

5. **History undo → no event**: User undoes a note creation → `HistoryManager::undo()` calls reverse closure (delete file) → no `ITEM_STORED` → Indexer and Graph still reference the deleted note.

### 11.3 Consistency Enforcement Points

- **`capture_file_drop` / `inbox_approve` / `inbox_convert`**: These are the ONLY pathways that maintain full consistency (Storage → Index → Graph).
- **All other commands** (`note_save`, `note_create_file`, `note_delete`, `note_rename`, `folder_create`): Direct filesystem operations with **no** downstream state synchronization.

### 11.4 EventBus Subscription Completeness

| Event Kind | Producers | Subscribers (production) | Coverage |
|------------|-----------|--------------------------|----------|
| `item.captured` | CaptureEngine | **None** | 0% |
| `item.processing.started` | PipelineExecutor | **None** | 0% |
| `item.processing.progress` | PipelineExecutor, Pipeline | **None** | 0% |
| `item.processing.completed` | PipelineExecutor | **None** | 0% |
| `item.processing.failed` | PipelineExecutor | **None** | 0% |
| `item.stored` | StorageManager.save | Indexer + VaultGraph | ~67% (missing: frontend, HistoryManager) |
| `index.updated` | Indexer | **None** | 0% |
| `graph.updated` | VaultGraph | **None** | 0% |
| `item.cancelled` | WorkerPool | **None** | 0% |
| `item.retried` | WorkerPool | **None** | 0% |

**Only 1 of 10 event kinds has subscribers in production.** And the subscribers are hardcoded closures in `lib.rs:162-177` — they are not discoverable or configurable.

---

## 12. Capability Platform Compatibility

### 12.1 Current Foundation for Capabilities

The codebase has a `CapabilityRegistry` (`../../crates/nabu-core/src/plugin`) that is already wired into `ApplicationContext`:

- **Construction**: `lib.rs:60-61` — `capability_registry.register_builtin()`
- **Storage**: `ApplicationContext.capability_registry()` (context.rs:189)
- **Built-in capabilities**: `nabu:event_bus`, `nabu:storage`, `nabu:capture`, `nabu:processor`, `nabu:graph`, `nabu:export`, `nabu:search` (context.rs:28-35)

### 12.2 Extensibility Analysis

| Capability | Can Support Today? | Required Changes | Evidence |
|-----------|-------------------|-----------------|----------|
| **Syncthing sidecar events** | **Partial** | Add `tokio::process::Command` support in nabu-core + Tauri command to start sidecar + EventBus publication for sync events | `WorkerPool` uses `tokio::spawn` (pool.rs:75); nabu-core Cargo.toml does NOT enable `tokio` process features |
| **Harper diagnostics** | **No** | Need `spawn_blocking` for CPU-bound linter + EventBus event for diagnostics + Tauri→frontend event bridge for real-time updates | No `spawn_blocking` found in nabu-core; EventBus handlers are synchronous only |
| **ACP streaming responses** | **No** | Need async streaming channel from capability → EventBus → frontend; current EventBus is synchronous and not exposed to frontend | `PipelineEvent` is `Serialize + Deserialize` but EventBus has no async publish |
| **Background capability status** | **No** | Need capability lifecycle events → EventBus → frontend subscription | No EventBus→frontend bridge exists |
| **Live notifications** | **No** | Same as above — need backend→frontend event bridge | `notify_history_changed()` (history.rs:152) uses DOM CustomEvent, not Tauri events |

### 12.3 Required Architectural Changes

To support long-running capability modules, the communication architecture needs:

1. **EventBus → Tauri event bridge**: A bridge that subscribes to `EventBus` events and forwards them to the frontend via `window.emit()` or `app.emit_all()`. Currently no such bridge exists.

2. **Async EventBus**: The current `EventBus::publish()` is synchronous (Mutex + inline handlers). For async capability handlers, either:
   - Wrap handlers in `tokio::spawn` (loses ordering guarantees), or
   - Add an async variant of EventBus with `tokio::sync::broadcast` channels

3. **tokio process features in nabu-core**: `../../crates/nabu-core/Cargo.toml` currently has `tokio` with only `sync`, `time`, `io`, `net` features. Adding `process` would enable `tokio::process::Command` for spawning sidecar binaries.

4. **Capability lifecycle management**: The `CapabilityRegistry` exists but `register_builtin()` only registers built-in capabilities. A pattern for dynamic capability registration (loading external capability manifests) needs to be added.

5. **Progress channel**: The `ProgressReporter` (`jobs/workers/progress.rs`) exists for pipeline progress but is not exposed to the frontend. A capability could report progress through it, but there's no IPC path for the frontend to read it.

### 12.4 Existing Supporting Infrastructure

| Component | Supports Capabilities? | Notes |
|-----------|----------------------|-------|
| `WorkerPool` (4 workers, 30s shutdown) | **Yes** | Can execute capability jobs via `JobExecutor` trait |
| `JobType` enum | **Yes** | Has `Sync` and `Embedding` variants (job.rs:141-159) |
| `Processor` trait | **Yes** | Capabilities can implement as processors |
| `CaptureHandler` trait | **Yes** | Capabilities can register as capture handlers |
| `ExecutorRegistry` | **Yes** | Can register capability executors |
| `ServiceRegistry` | **Yes** | Capabilities can register services |
| Native messaging socket | **Yes** | Already spawns per-connection `tokio::spawn` tasks |
| `spawn_blocking` usage | **No** | Not found in nabu-core for CPU-bound offload |

---

## 13. Highest-Priority Architectural Observations

### 13.1 CRITICAL: Dual Persistence Paths Cause Stale Indexes and Graphs

**Finding**: `note_save` (recovery.rs:391) and `note_create_file` (commands.rs:581) both write directly to disk via `std::fs::write`, bypassing `StorageManager::save()` and the `ITEM_STORED` event. The capture pipeline (file drops, clipboard, inbox) correctly routes through `StorageManager::save()` (manager.rs:142).

**Impact**: Search and graph views are silently stale after any note edit. Users will search for content they just typed and get no results. The knowledge graph will not reflect new wikilinks added during editing.

**File references**: 
- `src-tauri/src/recovery.rs:391-406` (note_save — direct fs::write)
- `src-tauri/src/commands.rs:581-638` (note_create_file — direct fs::write)
- `src-tauri/src/lib.rs:162-177` (ITEM_STORED subscriber — only fires from StorageManager)
- `crates/nabu-core/src/storage/manager.rs:142-193` (StorageManager::save — the canonical path)

### 13.2 CRITICAL: No Backend→Frontend Event Bridge

**Finding**: `EventBus<PipelineEvent>` is strictly backend-internal. It uses synchronous `Mutex` + inline handler dispatch (`bus.rs:70-75`). No `window.emit()` calls exist anywhere in `../../src-tauri` or `../../crates/nabu-core`. The only cross-boundary mechanism is `tauri_invoke()` (request-response IPC, `ipc.rs:9-11`).

**Impact**: The frontend cannot receive asynchronous events. All 10 event types (`ITEM_CAPTURED`, `PROCESSING_PROGRESS`, `ITEM_STORED`, `INDEX_UPDATED`, etc.) are effectively dead for frontend purposes. Users get no progress indication during OCR/Whisper/PDF processing, no live inbox updates, and no graph/index refresh after backend operations.

**File references**:
- `crates/nabu-ui/src/ipc.rs:1-11` (sole IPC function)
- `crates/nabu-core/src/event_bus/bus.rs:69-76` (synchronous publish)
- `src-tauri/src/lib.rs:162-177` (only subscribers are backend-internal closures)
- Grep for `emit` in src-tauri/src and crates/nabu-core/src: zero `window.emit` / `app.emit` matches

### 13.3 CRITICAL: GraphEventBridge Dead Code

**Finding**: `GraphEventBridge` (`graph/incremental/event_wiring.rs:26-189`) provides an incremental graph update mechanism that translates `ITEM_STORED` events into targeted graph updates. However, `wire_incremental_graph_updates()` (line 178) is **never called in production code** — it appears only in test code (`event_wiring.rs:191-232`).

**Impact**: The incremental update infrastructure is complete but unused. Production falls back to the inline closure in `lib.rs:162-177`, which calls `VaultGraph::add_node()` directly — the same as what `GraphEventBridge` would do, but without snapshot management, transaction batching, or region-based incremental rebuilds.

**File references**:
- `crates/nabu-core/src/graph/incremental/event_wiring.rs:178-189` (convenience function)
- `crates/nabu-core/src/graph/incremental/event_wiring.rs:191-232` (only call sites are in tests)
- `src-tauri/src/lib.rs:162-177` (production uses inline closure instead)

### 13.4 HIGH: Frontend State Duplication Without Sync

**Finding**: `NavContext.notes_index` (`navigation/state.rs:206`) caches a `Vec<NoteIndexEntry>` in the frontend, populated by `load_notes_index()` (called at app.rs:90). The backend `Indexer` also maintains an index. These are **completely independent** — `load_notes_index` calls `commands::notes_index` which scans the filesystem directly (`collect_notes`, commands.rs:1705), not the `Indexer`.

**Impact**: The frontend's `notes_index` and the backend `Indexer` can diverge. The frontend index is only refreshed on explicit calls to `load_notes_index()`. Adding a note via `note_create_file` (which bypasses `StorageManager`) will be reflected in the filesystem scan but not in the `Indexer`.

**File references**:
- `crates/nabu-ui/src/components/navigation/state.rs:206` (frontend notes_index signal)
- `crates/nabu-ui/src/components/navigation/state.rs:148-160` (load_notes_index function)
- `src-tauri/src/commands.rs:1442` (notes_index command — filesystem scan, not Indexer)
- `src-tauri/src/commands.rs:1705` (collect_notes — scans vault directory)

### 13.5 HIGH: No Progress Events Reach Frontend

**Finding**: The `PipelineExecutor::execute` method publishes `ProcessingStarted`, `ProcessingProgress`, `ProcessingCompleted`, and `ProcessingFailed` events (`executor.rs:122-157`). It also uses a `ProgressReporter` to emit progress (`pipeline.rs:56, 136`). However, **no subscribers exist for any processing events**, and there is no IPC mechanism to forward progress to the frontend.

**Impact**: Long-running captures (OCR on PDFs, Whisper transcription, PDF text extraction) provide zero feedback to the user. The UI shows no progress bar, no spinner, no status update. The user has no indication that processing is happening until they manually check the inbox or queue.

**File references**:
- `crates/nabu-core/src/pipeline_migration/executor.rs:122-157` (event publication)
- `crates/nabu-core/src/processing/pipeline.rs:56` (ProgressReporter in Processor trait)
- `src-tauri/src/lib.rs:162-177` (only ITEM_STORED subscribers, nothing for processing events)
- `crates/nabu-ui/src/components/ui/feedback.rs:710-730` (TaskContext exists but is never populated from backend)

### 13.6 MEDIUM: Settings Mutations Have No Event Propagation

**Finding**: `settings_set` (commands.rs:652-663) updates `AppSettings` via `Mutex` and persists to disk. It returns `Ok(())` with no event. The frontend component that triggers the change must independently update its local signal.

**Impact**: If a setting is changed from one code path (e.g., settings panel), other components that depend on that setting (e.g., NavContext for view_mode) will not automatically update. The settings panel and NavContext signals can diverge.

**File references**:
- `src-tauri/src/commands.rs:652-663` (settings_set)
- `src-tauri/src/settings.rs:293-315` (update + save)
- `src-tauri/src/lib.rs:200` (settings_store managed state — no event listener)

### 13.7 MEDIUM: Worker Polling Instead of Notify

**Finding**: `Worker::run()` (worker.rs:63-89) uses `tokio::time::sleep(100ms)` polling when the queue is empty, rather than a `tokio::sync::Notify` for immediate wakeup.

**Impact**: 100ms latency for job pickup when the queue transitions from empty to non-empty. During high-throughput capture bursts, this adds unnecessary latency.

**File references**:
- `crates/nabu-core/src/jobs/workers/worker.rs:73-76` (sleep polling)
- `../../crates/nabu-core/src/jobs/workers/pool.rs` (no Notify-based wakeup)
```
