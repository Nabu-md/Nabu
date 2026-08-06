# Capability Platform Roadmap: Gap Analysis & Implementation Plan

**Date:** 2026-08-05
**Author:** Chief Architect
**Methodology:** Semantic analysis via RustRover (Find Usages, Call Hierarchy, Type Hierarchy, Trait Implementations, Data Flow Analysis, Module Dependency Graph)
**Status:** Complete

---

## 1. Executive Summary

The Nabu codebase contains **significant existing infrastructure** relevant to every phase of the Capability Platform Roadmap. The canonical composition root at `src-tauri/src/lib.rs:55-180` (`build_application_context`) wires a single `EventBus`, `ServiceRegistry`, `CaptureEngine`, `DurableJobQueue`, `WorkerPool`, `ProcessingPipeline`, `StorageManager`, `Indexer`, `VaultGraph`, and `CapabilityRegistry`. A complete plugin foundation (`plugin/` module, ~670 lines) provides `CapabilityRegistry`, `PluginManager`, `PluginManifest`, `PluginLifecycle`, `PermissionEvaluator`, `FeatureRegistry`, `DependencyGraph`, and `Version` types. The UI layer has a full `SettingsPanel` with 13 tabs and a `ToastProvider`/`use_toast()` notification system.

However, **three architectural blockers** prevent the Capability Platform from functioning:

1. **No event-to-IPC bridge** — `EventBus` publishes 8 `PipelineEvent` variants (`event_bus/events.rs:11-29`) but the frontend has **zero** `#[listen]` calls (`grep` across `crates/nabu-ui/src/` returns no results). Backend events never reach the UI, so status events, notifications, conversation streaming, and progress updates are impossible.

2. **No graceful shutdown sequence** — `ApplicationContext::shutdown()` exists at `registry/context.rs:410` but is never called in the Tauri `Exit` handler. `lib.rs:402-412` only calls `mark_clean_exit` (removes a `.running` marker file). No coordination exists for stopping `WorkerPool`, persisting `VaultGraph`, persisting `Indexer`, or flushing `SettingsStore`.

3. **`note_save` bypasses the canonical pipeline** — `src-tauri/src/recovery.rs:391-406` writes directly to disk via `std::fs::write` without publishing `ITEM_STORED` events. This means `Indexer.index_object()` (`lib.rs:162-177`) and `VaultGraph::add_node()` never execute for autosaved notes, breaking search and graph integrity for the most frequently written path.

Beyond these blockers, the codebase has **dead architecture** that must be resolved before the Capability Platform can proceed:

- The entire `plugin/` module (8 files, ~670 lines) is **complete but unused** in production — `PluginManager` is never instantiated outside tests (`crates/nabu-core/tests/plugin_foundation_integration.rs:502`).
- The `NativeMessagingSocket` (`native_messaging_socket.rs`) starts a Unix socket server but **discards the handle** (`Ok(_handle)` at `lib.rs:363`, `lib.rs:357-369`), making it impossible to shut down or interact with.
- The `Application`/`ApplicationContextBuilder` in `registry/application.rs` (514 lines) is **test-only** — grep shows `Application::builder()` is called only in test files, never in `src-tauri/src/`.
- The `Lifecycle` trait (`registry/lifecycle.rs:202-230`) is **defined but never implemented** by any production service.

### Readiness Assessment Summary

| Phase | Readiness | Key Finding |
|-------|-----------|-------------|
| Phase 0 — UI Framework Migration | PLANNED | LePtos 0.7.8 → Dioxus migration; entire nabu-ui crate (22,000 lines, 76 files) being rewritten; must complete before Phase 1 capability UI |
| Phase 1 — Framework Foundation | PARTIALLY READY | ServiceRegistry, ApplicationContext, LifecycleManager, CapabilityRegistry all exist; Lifecycle trait unimplemented |
| Phase 2 — Syncthing | NOT READY | Native messaging exists but handle discarded; no event-to-IPC bridge; no process supervision |
| Phase 3 — Harper | PARTIALLY READY | CaptureHandler/Processor/JobExecutor traits exist; no editor diagnostic rendering pipeline |
| Phase 4 — ACP Client | NOT READY | subprocess spawning exists; no JSON-RPC, no conversation state, no streaming |
| Phase 5 — Capability UI | NOT READY | SettingsPanel + Toast system exist; no capability panels, no event-driven updates |
| Phase 6 — Capability SDK | PARTIALLY READY | Complete plugin foundation exists but dead code; no plugin loading/execution |
| Phase 7 — Production Readiness | PARTIALLY READY | Tracing, metrics, ShutdownCoordinator exist; no app-level shutdown sequence |

### Critical Prerequisites (before Phase 2 can proceed)

1. **Complete Phase 0** — Dioxus migration must finish before any Capability UI work can begin.
2. **Fix `note_save` pipeline bypass** — Route note saves through `StorageManager.save()` so `ITEM_STORED` events propagate to Indexer and VaultGraph.
3. **Implement event-to-IPC bridge** — Subscribe backend `EventBus` events to Tauri's `emit_all` so the frontend can receive them via `#[listen]`.
4. **Implement graceful shutdown** — Call `ApplicationContext::shutdown()` in the Tauri `Exit` handler; stop `WorkerPool`, persist `VaultGraph`/`Indexer`, flush `SettingsStore`.
5. **Decide on the plugin system** — Either integrate the dead `plugin/` module or remove it entirely.

---

## 2. Overall Capability Platform Readiness

**Score: PARTIALLY READY**

The architecture provides **foundation-level infrastructure** for every capability described in the roadmap. The core abstractions are sound:

- Single-event-bus design with typed `PipelineEvent` enum (`event_bus/events.rs:8-29`)
- Service registry with category-based discovery (`registry/mod.rs:41-189`)
- Lifecycle stages with one-way transitions (`registry/lifecycle.rs:25-36`)
- Trait-based handler/processor/executor patterns (`capture/handler.rs:37`, `processing/processor.rs:84`, `jobs/workers/executor.rs:14`)
- Plugin foundation with manifest validation, dependency graphs, and feature flags (`plugin/` module)

**However, the readiness is undermined by:**

- **Critical integration gaps**: The event-to-IPC bridge and `note_save` pipeline bypass are the two largest architectural risks. They break the contract documented in `crates/nabu-core/src/lib.rs:8-26` (which claims `ITEM_STORED → Indexer.index_document() + VaultGraph.update_node()`, but production code at `src-tauri/src/lib.rs:162-177` uses `Indexer.index_object()` + `VaultGraph.add_node()`).

- **Dead architecture debt**: 670+ lines of complete plugin system (`plugin/`), 514+ lines of dead `Application` struct (`registry/application.rs`), and 1,157 lines of dead incremental graph subsystem (`graph/incremental/`) compete for the same design space as the roadmap's Phase 1 and Phase 6.

- **No frontend-backend event channel**: The `EventBus` at `event_bus/bus.rs` is fully functional — `subscribe()` is called at `lib.rs:162` for `ITEM_STORED`. But there is no mechanism to forward events to the Tauri frontend. This is a **systemic gap** that blocks Status Events (Phase 2), Notifications (Phase 5), Conversation Threads (Phase 4), and Streaming Interfaces (Phase 5).

### Shared Infrastructure Already Present

| Infrastructure | Location | Phases Supported |
|---|---|---|
| `ServiceRegistry` + categories | `registry/mod.rs:41` | 1, 2, 3, 4, 5, 6 |
| `ApplicationContext` (DI container) | `registry/context.rs:141` | 1, 2, 3, 7 |
| `LifecycleManager` + `Lifecycle` trait | `registry/lifecycle.rs:94, 202` | 1, 2, 4, 7 |
| `EventBus<PipelineEvent>` + 8 event types | `event_bus/` | 1, 2, 3, 4, 5 |
| `SettingsStore` (CRUD + export/import) | `src-tauri/src/settings.rs:254` | 1, 2, 3, 5, 6 |
| `ToastProvider` + `use_toast()` | `components/ui/feedback.rs:78-197` | 5, 7 |
| `ProgressReporter` (callback-based) | `jobs/workers/progress.rs:8` | 2, 3, 5 |
| `ShutdownCoordinator` (worker pool) | `jobs/workers/shutdown.rs:11` | 2, 4, 7 |
| `HistoryManager` (undo/redo) | `history/mod.rs:14` | 5, 7 |
| `Recovery` (session restore, snapshots) | `src-tauri/src/recovery.rs` | 7 |
| Diagnostics (tracing, metrics, spans) | `diagnostics/` | 5, 7 |

### Shared Infrastructure Missing

| Infrastructure | Why Needed |
|---|---|
| Event-to-IPC bridge | Backend EventBus → Tauri frontend (blocks Phases 2, 4, 5) |
| Application-level shutdown sequence | Coordinated stop of all services (blocks Phase 7) |
| Capability metadata serialization | Frontend needs to query enabled/disabled capabilities (blocks Phase 5) |
| Capability-to-service binding API | Map capability IDs to actual runtime services (blocks Phase 1, 6) |
| JSON-RPC / stdin-stdout abstraction | Core protocol for ACP agent communication (blocks Phase 4) |
| Conversation thread state model | Thread/Message/Turn types for ACP (blocks Phase 4) |
| Capability-specific UI panels | Per-capability settings and status (blocks Phase 5) |
| Process supervision framework | Sidecar lifecycle, restart policies, health checks (blocks Phase 2) |

---

## 3. Phase 0 — UI Framework Migration to Dioxus

### Rationale

The current UI (LePtos 0.7.8) is being redesigned as part of the Capability Platform roadmap. Since the entire `nabu-ui` crate (22,000 lines, 76 files) is being rewritten, the framework migration is a **prerequisite** to Phase 1 — it must be completed before capability UI can be developed.

**Why Dioxus**: Nabu's App Block system (HTML iframe sandboxing), canvas-based GraphView, Tailwind CSS pipeline, and direct `web_sys` DOM access (clipboard, drag-drop, keyboard) all require a framework that renders real HTML DOM. Only Dioxus meets this requirement among the evaluated frameworks (Iced renders to GPU canvas; GPUI has no WASM target). The backend (`nabu-core`, `src-tauri`) is completely unaffected — the IPC abstraction (`crate::ipc::tauri_invoke()`, 5 lines) is framework-agnostic.

### Existing Infrastructure (Reusable)

| Component | Reusable | What Changes |
|-----------|----------|-------------|
| IPC layer (`ipc.rs`, 5 lines) | ✅ 100% | None — pure `wasm_bindgen` |
| 64 Tauri commands, 113 call sites | ✅ 100% | None — command names and args unchanged |
| `nabu-core` (22,285 lines) | ✅ 100% | None |
| `src-tauri` backend (7,086 lines) | ✅ 100% | None |
| Tailwind CSS pipeline (`app.css`, 2,923 lines) | ✅ 100% | None — external CSS, framework-agnostic |
| `tailwind.config.js` | ✅ 100% | None — scans Rust source for class names |
| Theme system (CSS `data-theme` attribute) | ✅ 100% | None — raw `web_sys` DOM access |
| Icon enum (80+ variants) | ✅ 90% | Only `render_icon_view()` dispatch changes |
| Canvas rendering (GraphView, 1,300 lines) | ✅ 100% | None — raw `web_sys::CanvasRenderingContext2d` |
| App Blocks (iframe sandbox) | ✅ 100% | None — HTML `<iframe>` element |
| Clipboard / drag-drop / keyboard | ✅ 100% | None — raw `web_sys` APIs |

### Changes Required

| Change | Count | Scope |
|--------|-------|-------|
| `view!` → `rsx!` macro syntax (`on:click` → `onclick`, `prop:value` → `value`) | 689 invocations | All 76 component files |
| `RwSignal<T>` → `Signal<T>` (Dioxus 0.5+) | 169 instances | Signal read/write patterns |
| `into_any()` elimination | 449 calls | Dioxus `rsx!` returns `VNode` natively |
| `Callback<T>` → Rust closures / `EventHandler` | 350 uses | Event handler pattern migration |
| `collect_view()` → Dioxus iteration | 100 calls | `.to_vec()` pattern in `rsx!` |
| Icon library `lucide-leptos` → `dioxus-icon` or inline SVG | 80+ re-exports | `icons.rs` dispatch table only |
| Build pipeline Trunk → `cargo-dioxus` | 1 pipeline | `run-trunk.sh` / `build-trunk.sh` replacement |
| `window_event_listener_untyped` → `web_sys` or `use_event_listener` | 7 uses | Direct listener pattern |

### Phase 0 Subphases

**P0.1 — Project Setup & Foundation**
- Initialize Dioxus + Tauri integration (shared IPC remains identical)
- Replace Trunk build pipeline with `cargo-dioxus`
- Migrate icon system: `lucide-leptos` → `dioxus-icon` (preserving the `Icon` enum; only `render_icon_view()` dispatch changes)
- Set up Tailwind CSS pipeline in Dioxus context
- Migrate the root app shell: `lib.rs` (mount_to_body → dioxus launch), `app.rs` (AppScreen, AppScreen rendering)
- Migrate shared contexts: `provide_theme`/`use_theme`, `provide_workspace`/`use_workspace`, `provide_navigation`/`use_nav`, `provide_history`, `provide_tasks`, `provide_toast` — same API names, different signal types
- **Deliverable**: A compiling Dioxus + Tauri project with the same IPC layer, theme system, and shared contexts working. Boot splash and loading screen functional.
- **Validation**: `cargo check` passes; app launches with Tauri webview; `settings_get` IPC works; theme toggles between dark/light.
- **Dependencies**: None — this is the starting point.

**P0.2 — UI Primitives Layer**
- Migrate the 10 UI primitive modules (`button.rs`, `input.rs`, `menu.rs`, `dialog.rs`, `card.rs`, `feedback.rs`, `icons.rs`, `info.rs`, `layout.rs`, `nav.rs`, `selection.rs`)
- Focus on: `view!` → `rsx!` syntax, `RwSignal` → `Signal`, `Callback` → closures, `ChildrenFn` → `Element`, `into_any()` → native type erasure
- Each primitive agent owns 2-3 files with no overlap
- **Deliverable**: All 12 UI primitive modules compile and render correctly in Dioxus with identical visual output and ARIA attributes.
- **Validation**: `cargo check` passes; button/input/menu/dialog components render with correct styling and behavior in isolation.
- **Dependencies**: P0.1 (project setup)

**P0.3 — Layout & Navigation Layer**
- Migrate the 5 layout components (`left_sidebar.rs`, `ribbon_bar.rs`, `right_inspector.rs`, `tab_bar.rs`, `mod.rs`)
- Migrate the 10 navigation components (`navbar.rs`, `command_palette.rs`, `quick_switcher.rs`, `shortcuts.rs`, `search_page.rs`, `dashboard.rs`, `home_screen.rs`, `breadcrumb.rs`, `smart_folders.rs`, `archive_page.rs`, `calendar_page.rs`, `commands.rs`, `state.rs`)
- Each agent owns one layout or navigation component
- **Deliverable**: Layout renders with sidebar/inspector/tabs/ribbon; navbar with undo/redo/search/settings toggles; command palette with fuzzy search; shortcuts reference dialog; navigation state context fully migrated.
- **Validation**: Layout switches between view modes correctly; keyboard shortcuts (⌘K, ⌘P, ⌘⇧F, ⌘Z/Y) fire IPC calls; navigation state persists across reloads.
- **Dependencies**: P0.2 (primitives)

**P0.4 — Core View Components**
- Migrate the 15+ core view components: `file_tree.rs`, `note_editor.rs`, `graph_view.rs`, `inbox.rs`, `settings/settings_panel.rs`, `recovery/*` (6 files), `reading_queue.rs`, `reader.rs`, `canvas.rs`, `comparison.rs`, `statistics.rs`, `trash.rs`, `template_editor.rs`, `template_picker.rs`, `dictation_pill.rs`, `property_editor.rs`, `relation_editor.rs`, `vault_setup_wizard.rs`
- Each agent owns 3-4 files — no crossing ownership:
  - **Agent A** (4 files): `file_tree.rs` + `note_editor.rs` + `note_view.rs` + `property_editor.rs`
  - **Agent B** (3 files): `graph_view.rs` + `canvas.rs` + `comparison.rs`
  - **Agent C** (4 files): `settings_panel.rs` + `trash.rs` + `reading_queue.rs` + `reader.rs`
  - **Agent D** (5 files): `inbox.rs` + recovery suite (`diff_view.rs`, `recovery_banner.rs`, `recovery_manager.rs`, `save_status.rs`, `session.rs`, `version_history.rs`) + `dictation_pill.rs` + `template_editor.rs` + `template_picker.rs` + `vault_setup_wizard.rs` + `statistics.rs`
- **Deliverable**: All core views render and interact correctly — file tree with drag-drop/rename/context menu; editor with autosave/debounce/cursor-restore; graph view with canvas pan/zoom/minimap; settings with all 15 tabs; inbox with status management; recovery with version diffing.
- **Validation**: `cargo check` passes; editor saves notes via `note_save` IPC; graph renders with zoom/pan; settings persist changes; inbox items approve/reject; recovery snapshots list and restore.
- **Dependencies**: P0.3 (layout/navigation)

### Phase 0 Wave Structure

| Wave | Phase | Subphases | Agents | Deliverable |
|------|-------|-----------|--------|-------------|
| **0** | Phase 0 | P0.1, P0.2, P0.3, P0.4 | **5** | Complete Dioxus migration of all UI components |

- **Agent 1**: P0.1 — Project setup, build pipeline, icon system, root app shell, shared contexts (4 files: `lib.rs`, `ipc.rs`, `app.rs`, `icons.rs`)
- **Agent 2**: P0.2 — UI primitives (12 files: `button.rs`, `input.rs`, `menu.rs`, `dialog.rs`, `card.rs`, `feedback.rs`, `info.rs`, `layout.rs`, `nav.rs`, `selection.rs`, `mod.rs`, `icons.rs` support)
- **Agent 3**: P0.3 — Layout + navigation (15 files: `layout/*`, `navigation/*`, `theme_toggle.rs`, `workspace.rs`, `tree.rs`)
- **Agent 4**: P0.4 (Views A+B) — File tree, editor, property editor, graph view, canvas, comparison
- **Agent 5**: P0.4 (Views C+D) + Documentation — Settings, trash, reading queue, reader, inbox, recovery suite, dictation pill, templates, vault wizard, statistics + update AGENTS.md and docs

**Ownership boundaries**: Agent 1 owns root files and build pipeline. Agent 2 owns all `ui/` primitives. Agent 3 owns `layout/` + `navigation/` + `workspace.rs` + `tree.rs`. Agent 4 owns file tree + editor + graph + canvas + comparison. Agent 5 owns settings + recovery + inbox + dictation + templates + documentation. No file is touched by more than one agent.

### Phase 0 Validation Gate

**Compilation**: `cargo check` passes for the full `nabu-ui` crate under Dioxus.

**Integration**: The app launches via Tauri, loads the Dioxus WASM bundle, IPC calls succeed, theme toggles work, and the boot splash renders correctly.

**Functional parity**: All 64 IPC commands are callable from the Dioxus UI with the same serialization (`serde_wasm_bindgen`) and the same command names. The `src-tauri` backend requires zero changes.

**Documentation**: `AGENTS.md` and `README.md` updated to reflect Dioxus (build commands, architecture diagram, component structure).

---

## 4. Phase 1 Gap Analysis — Capability Framework Foundation

### Existing Infrastructure

**READY — Service Registration**
- `ServiceRegistry` (`crates/nabu-core/src/registry/mod.rs:41-189`) — Thread-safe registry with singleton registration (`register()` at line 66), transient factories (`register_factory()` at line 78), and category-based discovery (`register_in_category()` at line 117, `resolve_category()` at line 183). Supports type-erased `Arc<dyn Any + Send + Sync>` storage with downcast on resolve.
- Standard category constants: `CATEGORY_CAPTURE_HANDLERS` (line 196), `CATEGORY_PROCESSORS` (line 199), `CATEGORY_AI_PROVIDERS` (line 202), `CATEGORY_OCR_PROVIDERS` (line 205), `CATEGORY_EMBEDDING_PROVIDERS` (line 208), `CATEGORY_EXPORTERS` (line 211), `CATEGORY_STORAGE_PROVIDERS` (line 214), `CATEGORY_CONTENT_PROVIDERS` (line 217).

**READY — Shared State / Runtime Initialization**
- `ApplicationContext` (`registry/context.rs:141-422`) — Holds `registry: Arc<RwLock<ServiceRegistry>>`, `event_bus: Arc<EventBus<PipelineEvent>>`, `capability_registry: CapabilityRegistry`, `lifecycle: LifecycleManager`. Provides typed accessors: `capture_engine()` (line 224), `processing_pipeline()` (line 229), `job_queue()` (line 234), `worker_pool()` (line 239), `vault_graph()` (line 244), `indexer()` (line 249), `storage_manager()` (line 254), `history_manager()` (line 259), `performance_monitor()` (line 264).
- `ApplicationContextBuilder` (`context.rs:428-478`) — Builder pattern with `with_event_bus()`, `with_registry()`, `with_capability_registry()`, `build()`. Auto-registers `event_bus` in the registry.
- `build_application_context()` (`src-tauri/src/lib.rs:55-180`) — The **canonical production composition root**. Constructs EventBus → StorageManager → ProcessingPipeline → DurableJobQueue → PipelineExecutor → WorkerPool → CaptureEngine → VaultGraph → Indexer. Registers all services in the `ServiceRegistry`. Subscribes `ITEM_STORED` event to Indexer + VaultGraph at `lib.rs:162-177`.

**READY — Settings Integration**
- `SettingsStore` (`src-tauri/src/settings.rs:254-390`) — Full CRUD: `load()` (line 267), `get()` (line 289), `set()` (line 293), `update()` (line 297), `save()` (line 310), `reset()` (line 316), `export_settings()` (line 321), `import_settings()` (line 333). Feature toggle support via `get_feature_toggles()` (line 377) and `set_feature_toggle()` (line 381). Uses `Mutex<AppSettings>` internally with file persistence to JSON.
- `AppSettings` struct (`settings.rs:1-117`) — 60+ fields covering Appearance, Editor, Markdown, Search, Graph, Files & Vaults, Import & Export, OCR, Accessibility, Performance, Privacy, Keyboard Shortcuts, Advanced, Experimental.

**READY — Capability Registry**
- `CapabilityRegistry` (`plugin/capability.rs:91-260`) — Thread-safe registry storing `capabilities: HashMap<String, Capability>`, `providers: HashMap<String, String>`, `enabled: HashSet<String>`. Methods: `register()` (line 110), `register_builtin()` (line 117), `has()` (line 128), `get()` (line 133), `provider()` (line 138), `enable()` (line 143), `disable()` (line 149), `is_enabled()` (line 155), `list()` (line 160), `list_enabled()` (line 167), `capability_count()` (line 174), `enabled_count()` (line 178), `by_namespace()` (line 184), `namespace_has()` (line 192), `provider_capabilities()` (line 198).
- `Capability` struct (`capability.rs:15-49`) — `{namespace, name, description, required}`, with `id()` returning `"{namespace}:{name}"`.
- `builtin_capabilities()` (`capability.rs:62-79`) — Returns 14 built-in capabilities: `nabu:event_bus`, `nabu:storage`, `nabu:capture`, `nabu:processor`, `nabu:graph`, `nabu:export`, `nabu:search`, `nabu:ocr`, `nabu:ai`, `nabu:embedding`, `nabu:template`, `nabu:sync`, `nabu:watch`, `nabu:plugin`.
- `Application::new()` (`context.rs:156-180`) — Constructs `ApplicationContext` with registry, event bus, and capability registry. The capability registry is passed in from `build_application_context()` at `lib.rs:60-63`.

**READY — Lifecycle Management**
- `LifecycleManager` (`registry/lifecycle.rs:94-196`) — Uses `AtomicU8` for lock-free stage transitions. `transition_to()` (line 139) validates one-way transitions: `Created→Initialized`, `Initialized→Running`, `Running→Shutdown`, plus skip transitions. `stage()` (line 114), `is_at_least()` (line 177), `is_shutdown()` (line 182), `is_running()` (line 188).
- `LifecycleStage` enum (`lifecycle.rs:26-36`) — `Created(0)`, `Initialized(1)`, `Running(2)`, `Shutdown(3)`. Ordered for comparison.
- `Lifecycle` trait (`lifecycle.rs:202-230`) — `initialize()`, `start()`, `shutdown()` with default no-op implementations. Takes `&self` (not `&mut self`).
- `ApplicationContext::initialize()` (`context.rs:371-391`) — Calls `validate_core_services()` (`context.rs:324`), transitions to `Initialized`.
- `ApplicationContext::start()` (`context.rs:394-405`) — Transitions to `Running`.
- `ApplicationContext::shutdown()` (`context.rs:410-421`) — Transitions to `Shutdown`. **Never called in production** (see Missing).

**READY — Validation**
- `validate_core_services()` (`context.rs:324-329`) — Checks `["event_bus", "capture_engine", "pipeline", "storage_manager"]` (required) and `["job_queue", "worker_pool", "vault_graph", "indexer"]` (optional).
- `ServiceHealth` enum and `check_health()` (`context.rs:273-279`) — Per-service health check.

### Partial Infrastructure

**PARTIAL — Lifecycle Trait Not Implemented**
- `Lifecycle` trait is defined at `registry/lifecycle.rs:202-230` but **no production service implements it**. The canonical services (`CaptureEngine`, `StorageManager`, `VaultGraph`, `Indexer`, `WorkerPool`, `DurableJobQueue`) in `src-tauri/src/lib.rs:55-180` do not implement `initialize()`, `start()`, or `shutdown()`. Only `ApplicationContext::initialize()` and `ApplicationContext::start()` are called at `lib.rs:342`. The `Lifecycle` trait remains a dead abstraction.
- Evidence: Search for `impl Lifecycle` returns **zero** results across the codebase. `Lifecycle` is only referenced in test code and the trait definition itself.

**PARTIAL — CapabilityRegistry Registered But Unused**
- `CapabilityRegistry::new()` + `register_builtin()` is called at `lib.rs:60-61` and the registry is passed to `ApplicationContext::new()` at `lib.rs:63`. However, the registry is **never queried** after registration — no IPC command exposes capabilities to the frontend, no service checks `is_enabled()` before executing.
- Evidence: `capability_registry` field in `ApplicationContext` (`context.rs:147`) is `pub` but grep for `capability_registry.` across `src-tauri/src/` returns zero results outside the constructor.

**PARTIAL — ApplicationContextBuilder Not Used in Production**
- `ApplicationContextBuilder` at `context.rs:428-478` is a complete, working builder, but `build_application_context()` at `src-tauri/src/lib.rs:55` constructs `ApplicationContext` directly via `::new()`. The builder is used only in tests (e.g., `context.rs:530`, `context.rs:590`).
- `build_standard_application_context()` at `context.rs:501-517` is another factory that registers built-in capabilities but **only includes tests** (`#[cfg(test)]` at line 523). The production path duplicates its functionality manually.

**PARTIAL — ApplicationContextBuilder Not Used in Production**
- `ApplicationContextBuilder` at `context.rs:428-478` is a complete, working builder, but `build_application_context()` at `src-tauri/src/lib.rs:55` constructs `ApplicationContext` directly via `::new()`. The builder is used only in tests (e.g., `context.rs:530`, `context.rs:590`).
- `build_standard_application_context()` at `context.rs:501-517` is another factory that registers built-in capabilities but **only includes tests** (`#[cfg(test)]` at line 523). The production path duplicates its functionality manually.

### Missing Infrastructure

- **No capability-to-service binding API**: `CapabilityRegistry` stores metadata (name, description, provider) but there's no API to map a capability ID (e.g., `nabu:sync`) to actual runtime services or handlers. The registry has `has()`, `get()`, `is_enabled()` but no `resolve_service()` or `get_handler()`.
- **No IPC command to query capabilities**: No `tauri::command` exposes `CapabilityRegistry` to the frontend. The frontend cannot discover what capabilities are available or their enabled state.
- **No capability metadata serialization**: The `Capability` struct (`plugin/capability.rs:15-49`) is not `Serialize`/`Deserialize`. It cannot cross the IPC boundary to the UI.
- **No runtime capability lifecycle**: `CapabilityRegistry::enable()`/`disable()` (`capability.rs:143-152`) mutate the internal set, but there is no mechanism to start/stop the underlying service when a capability is toggled.
- **No capability configuration schema**: No `CapabilityConfig` type linking capabilities to `AppSettings` fields.

### Architectural Blockers

- **Lifecycle trait unimplemented**: Without implementing `Lifecycle` on production services, Phase 1's "Capability Lifecycle" requirement (start/stop/restart individual capabilities) cannot be met. The trait exists but is dead code.
- **CapabilityRegistry never queried**: The registry is populated but never consulted — capabilities are decorative metadata, not functional hooks.
- **Application startup panics on service failure**: `lib.rs:94` panics if `DurableJobQueue` construction fails; `lib.rs:145` panics if `VaultGraph` construction fails. No graceful degradation path.

---

## 5. Phase 2 Gap Analysis — Syncthing Capability

### Existing Infrastructure

**READY — Subprocess / Native Messaging**
- `std::process::Command` is used in `commands.rs` for process spawning: lines 378 (`open`), 395 (`explorer`), 420 (`xdg-open`), 715 (`open`), 733 (`terminal-notifier`), 774 (`explorer`), 792 (`xdg-open`), 802 (`notify-send`), 847 (`open`), 853 (`explorer`), 858 (`xdg-open`). Pattern: `Command::new(cmd).args(args).spawn()`.
- `native_messaging.rs` (`src-tauri/src/native_messaging.rs:1-281`) — Complete native messaging protocol implementation: `Message` struct with `request_id`, `command`, `payload` (line 45-60), `read_message()`/`write_message()` length-prefixed JSON I/O (lines 61-180), `MAX_PAYLOAD_SIZE` 1MB validation (line 12), `ALLOWED_COMMANDS` whitelist (line 16).
- `native_messaging_socket.rs:1-350` — `SocketServerState` struct (`line 220`), `start_socket_server()` (`line 222`), `SocketServerHandle` with `shutdown()` (`line 356-362`). Uses `tokio::sync::Notify` (`line 222`) for shutdown signaling. Spawns in Tauri runtime at `lib.rs:360-369`.

**READY — Settings**
- `SettingsStore` (`src-tauri/src/settings.rs:254-390`) — Full settings persistence. Can store sync configuration, device pairing keys, folder mappings.
- `get_feature_toggles()` / `set_feature_toggle()` (`settings.rs:377-389`) — Feature flags can gate sync experiments.

**READY — Event Routing**
- `EventBus` (`event_bus/bus.rs`) — `subscribe()` (line 151), `publish()` (line 133), `unsubscribe()` (line 165). Used at `lib.rs:162` for `ITEM_STORED` events.
- 8 event kinds defined (`event_bus/events.rs:32-43`): `item.captured`, `item.processing.started`, `item.processing.progress`, `item.processing.completed`, `item.processing.failed`, `item.stored`, `index.updated`, `graph.updated`.

**READY — Status Reporting**
- `ProgressReporter` (`jobs/workers/progress.rs:8-155`) — Callback-based progress with `set_progress()` (line 39), `report()` (line 47), `noop()` (line 33). Used by `PipelineExecutor` (line 44-110 of `pipeline_migration/executor.rs`) to report per-processor progress.

**READY — Notifications**
- `show_macos_notification` (`commands.rs:732`) — Uses `terminal-notifier` binary
- `show_linux_notification` (`commands.rs:801`) — Uses `notify-send` binary
- `open_app_in_finder` (`commands.rs:847`) — Uses `open` on macOS

### Partial Infrastructure

**PARTIAL — Process Lifecycle (Socket Server)**
- `native_messaging_socket.rs:362` — `start_socket_server()` returns a `SocketServerHandle` from which the caller can call `.shutdown()`. **However**, the return value is discarded at `src-tauri/src/lib.rs:362`: `Ok(_handle) => { tracing::info!(...); }`. The handle is never stored, so the socket server **cannot be shut down** during application exit.
- Evidence: `lib.rs:357-369` creates `SocketServerState` and spawns `start_socket_server` but binds the handle with `Ok(_handle)` — the `let` binding name is literally `_handle`.

**PARTIAL — Status Events (No Event-to-IPC Bridge)**
- `EventBus.subscribe()` is called at `lib.rs:162` for `ITEM_STORED`, which triggers `Indexer.index_object()` and `VaultGraph.add_node()`. However, `PipelineEvent::ItemProcessingProgress` (`event_bus/events.rs:14`) and other events are **published but have no subscribers** that forward them to the Tauri frontend.
- No `#[listen]` calls exist in `crates/nabu-ui/src/` — grep returns zero results. The frontend has no mechanism to receive real-time backend events.

**PARTIAL — Progress Reporting (No UI Integration)**
- `ProgressReporter` (`jobs/workers/progress.rs:8-155`) uses callback `|f64|` (line 11) but the only callback registered is in `PipelineExecutor::execute()` (`pipeline_migration/executor.rs:51-110`), which writes progress to the job queue via `queue.report_progress()`. There is no IPC emission of progress to the UI.
- `commands.rs` has `queue_get_all` (`lib.rs:277`), `queue_set_status`, `queue_set_progress` — but these are command-response, not streaming.

### Missing Infrastructure

- **No process supervisor for sidecar processes**: There is no general-purpose subprocess management abstraction. The `native_messaging.rs` module handles stdin/stdout protocol but does not supervise process lifecycle (restart on crash, health checks, crash recovery).
- **No crash detection / restart policy**: If a sync-related subprocess dies, there is no mechanism to detect or restart it.
- **No persistent process state**: No PID file tracking, no restart-count tracking, no state persistence for sidecar processes across app restarts.
- **No event-to-IPC bridge**: This is the single largest gap. The `EventBus` fully implements publish/subscribe for `PipelineEvent`, but there is no subscriber that calls `tauri::Emitter::emit_all()` to forward events to the frontend. Without this, Syncthing's Status Events cannot reach the capability UI (Phase 5).
- **No folder sync state model**: No `SyncFolder` struct, no `SyncStatus` enum, no `ConflictResolution` type. No tracking of synced folders, last sync timestamps, or conflict states.
- **No conflict resolution mechanism**: No types or logic for detecting or resolving file conflicts during sync.

### Architectural Blockers

- **Discarded socket handle** (`lib.rs:363`): The `SocketServerHandle` returned by `start_socket_server()` is discarded, making the socket server a zombie — it cannot be cleanly shut down. This violates the production-readiness requirement of Phase 7.
- **No event-to-IPC bridge**: Blocks real-time status events, progress, and notifications for ALL phases that depend on backend-to-frontend communication.

---

## 6. Phase 3 Gap Analysis — Harper Capability

### Existing Infrastructure

**READY — Content Processing Pipeline**
- `Processor` trait (`processing/processor.rs:84-102`) — `#[async_trait]` with `name()`, `async process()` (takes `&ProcessingContext`, `ProgressReporter`, `CancellationToken`), `supports()` (returns `true` by default). Implements `Send + Sync`.
- `ProcessingPipeline` (`processing/pipeline.rs:15-290`) — Holds `Vec<Arc<dyn Processor>>`, runs them sequentially with per-processor progress (`progress.set_progress()` at line 116), cancellation checks (`cancellation.is_cancelled()` at line 82), and child spans (`processor_span` at line 122).
- `ProcessingContext` (`processor.rs:1-79`) — Bundles object reference, event bus, vault path. Used by processors.
- 14 processors in `processing/processors/`: `OcrProcessor` (`ocr_processor.rs:17`), `PdfAnnotationProcessor` (`pdf_annotation_processor.rs:14`), `PdfMetadataProcessor` (`pdf_metadata_processor.rs:9`), `PdfTextProcessor` (`pdf_text_processor.rs:9`), `WhisperProcessor` (`whisper_processor.rs:16`), plus metadata extractors, tag extractors, link analyzers, etc.
- `build_standard_pipeline()` (`processing/pipeline.rs`) — Factory that constructs the 14-processor pipeline in order.

**READY — Capture Handlers (Extensible Source Points)**
- `CaptureHandler` trait (`capture/handler.rs:37-47`) — `#[async_trait]` with `name()`, `source()`, `async capture()`. 9 implementations: `ClipboardHandler`, `BrowserCaptureHandler`, `ArticleCaptureHandler`, `YouTubeCaptureHandler`, `GitHubRepositoryHandler`, `EmailCaptureHandler`, `ScreenshotHandler`, `FileDropHandler`, `WatchFolderHandler`.
- `CaptureEngine` (`capture/engine.rs`) — Routes captures to handlers by `CaptureSource`, enqueues `Job`s for processing.

**READY — Job Execution**
- `JobExecutor` trait (`jobs/workers/executor.rs:15-23`) — `#[async_trait]` with `async execute()`. Takes `&Job`, `ProgressReporter`, `CancellationToken`.
- `ExecutorRegistry` (`jobs/workers/executor.rs:29-70`) — `register()` (line 41), `get()` (line 46), `has_executor()` (line 51), `processor_names()` (line 61).
- `PipelineExecutor` (`pipeline_migration/executor.rs:24-177`) — Bridges `WorkerPool` to `ProcessingPipeline`. Implements `JobExecutor`. Calls `pipeline.run()` then `storage.save()`.
- `NoopExecutor` (`executor.rs:73-88`) and `FallbackExecutor` (`executor.rs:92-107`) — test/error executors that are never registered in production.

**READY — Configuration**
- `AppSettings` includes Harper-relevant settings: `ocr_language` (line 183), `ocr_auto_process_scanned_pdfs` (line 184), `ocr_confidence_threshold` (line 185), `whisper_model` (line 217), `enable_ai_summarization` (line 218), `enable_semantic_search` (line 219).
- `SettingsStore.get_feature_toggles()` / `set_feature_toggle()` for gating experimental analysis.

**READY — Progress Reporting**
- `ProgressReporter` (`jobs/workers/progress.rs:8-155`) — `set_progress()`, `report()`, `set_message()`, `noop()`.

### Partial Infrastructure

**PARTIAL — Editor Integration**
No editor integration exists. The `nabu-ui` crate uses a Markdown editor component (likely a Leptos wrapper around a textarea or CodeMirror-like component), but there is **no diagnostic rendering pipeline** connecting Harper's processor output back to the editor.

Evidence: Grep for `diagnostic` in `crates/nabu-ui/src/` returns only unrelated matches. No `Decoration`, `Diagnostic`, `Suggestion`, `InlineDecoration`, or `EditorExtension` types exist in the UI layer.

**PARTIAL — Processor Extensibility**
`ProcessingPipeline.register()` (`pipeline.rs:39`) exists but `build_standard_pipeline()` constructs a fixed 14-processor pipeline. There is no mechanism for a capability/plugin to dynamically register additional processors at runtime. The `Processor` trait is extensible, but the registration mechanism is not exposed through the capability framework.

**PARTIAL — Text Processing**
The existing processors handle OCR, PDF extraction, Whisper transcription, metadata extraction, and embedding generation, but there are no text annotation or writing-analysis processors (e.g., spelling correction, grammar checking, readability analysis).

### Missing Infrastructure

- **No diagnostic rendering pipeline**: No `Decoration` type, no `Diagnostic` type, no `Suggestion` type, no inline decoration model for displaying analysis results in the editor.
- **No editor extension points**: No `ExtensionPoint` trait, no `register_processor()` IPC command for dynamic processor registration from capabilities.
- **No debounce system**: No `DebounceController` or timer-based throttle for editor-triggered analysis. Harper-style analysis needs to debounce on keystroke pause — no existing infrastructure for this.
- **No annotation model**: No types for representing text ranges, severity levels, diagnostic codes, or suggested fixes that Harper processors could produce.
- **No editor ↔ backend bridge**: No IPC command for requesting Harper diagnostics for a specific note. No event-to-IPC bridge to push diagnostics from background processors to the frontend.
- **No text range model**: No `TextRange` / `TextPosition` types for specifying which parts of a document contain issues.

### Architectural Blockers

- **No diagnostic data model**: Without `Diagnostic`/`Decoration`/`Suggestion` types, Harper cannot report analysis results in a structured way.
- **No editor integration bridge**: The processing pipeline produces `ProcessingResult` (`processor.rs:2-78`) but there is no mechanism to connect processor output to editor rendering.

---

## 7. Phase 4 Gap Analysis — ACP Client Capability

### Existing Infrastructure

**READY — Subprocess Management**
- `std::process::Command` usage in `commands.rs` (lines 378, 395, 420, 715, 733, 774, 792, 802, 847, 853, 858) — Basic process spawning with `Command::new(cmd).args(args).spawn()`.
- `native_messaging.rs` (`src-tauri/src/native_messaging.rs:1-281`) — Length-prefixed JSON message protocol for stdin/stdout communication with external processes. `read_message()` (line 61), `write_message()` (line 100).

**READY — Asynchronous Communication**
- `tokio` runtime via `tauri::async_runtime::spawn()` (used at `lib.rs:349`, `lib.rs:360`, `lib.rs:377`).
- `EventBus` pub/sub pattern (`event_bus/bus.rs:133-165`).
- `CancellationToken` (`jobs/cancellation.rs`) — For cancelling async operations mid-flight.

**READY — JSON Serialization**
- `serde`/`serde_json` used throughout: `CaptureRequest` (`capture/handler.rs:50-60`), `KnowledgeObject`, `ObjectMetadata`, `PipelineEvent` variants.
- `serde_wasm_bindgen` for frontend ↔ backend serialization (`crates/nabu-ui/src/lib.rs:57`, `components/app.rs:128`).

**READY — Shared State**
- `HistoryManager` (`history/mod.rs:14-461`) — Command-based undo/redo with `HistoryEntry` (line 103), `HistoryAction` closure type (line 100), undo/redo stacks.
- `SettingsStore` — Persistent state for conversation configuration.

**READY — Worker Pool**
- `WorkerPool.start()` (`jobs/workers/pool.rs:52-120`) — Spawns worker tasks on tokio runtime. Called at `lib.rs:348-352`.

### Partial Infrastructure

**PARTIAL — Background Process Lifecycle**
- `WorkerPool` has explicit async `start()` (`pool.rs:52`) and `ShutdownCoordinator` (`shutdown.rs:11`) but this is specific to job queue workers, not general-purpose subprocess management.
- No general-purpose process supervisor that can start/stop/restart external processes with health checks.

**PARTIAL — JobExecutor Pattern**
- `JobExecutor` trait (`jobs/workers/executor.rs:15-23`) — The pattern (`async execute(&self, job, progress, cancellation) -> JobResult<Job>`) is a good abstraction that could be extended for ACP agent execution, but it is tightly coupled to the `Job` data model (`jobs/job.rs`) and `ExecutorRegistry`.

### Missing Infrastructure

- **No JSON-RPC implementation**: No `jsonrpc` types, no request/response framing, no method routing. The `native_messaging.rs` module handles arbitrary JSON messages but provides no JSON-RPC structure.
- **No stdin/stdout communication abstraction**: While `native_messaging.rs` has `read_message()`/`write_message()`, there is no general-purpose `StdioPipe` or `BufferedIOPipe` abstraction for bidirectional async communication with external processes.
- **No streaming message handling**: No `Stream`-based async message receiver. No `AsyncRead`/`AsyncWrite` wrapper for subprocess stdin/stdout.
- **No message routing**: No router to dispatch incoming messages from ACP processes to specific handlers by method name.
- **No conversation thread state model**: No `Thread`, `Message`, `Turn`, `Participant` types. No thread history persistence.
- **No tool calling abstraction**: No `Tool` trait, no `ToolCall` type, no tool result marshaling.
- **No event-to-IPC bridge**: ACP conversation events from the backend cannot reach the frontend for display in a sidebar or conversation view.

### Architectural Blockers

- **No JSON-RPC or streaming abstraction**: Phase 4's core requirement (JSON-RPC, stdin/stdout, streaming) requires building an entirely new abstraction layer. The existing `native_messaging.rs` protocol handles length-prefixed JSON but does not implement JSON-RPC framing or streaming message handling.
- **No process lifecycle management**: External agent processes need start/stop/restart/health-check lifecycle management that does not exist outside the `WorkerPool` context.

---

## 8. Phase 5 Gap Analysis — Capability UI

### Existing Infrastructure

**READY — Capability Settings**
- `SettingsPanel` (`components/settings/settings_panel.rs:114-179`) — 13-tab settings UI with tabs: Appearance, Editor, Markdown, Search, Graph, Files & Vaults, Import & Export, OCR, Accessibility, Performance, Privacy, Keyboard Shortcuts, Advanced, Experimental, About.
- Individual setting sub-components: `AppearanceSettings`, `EditorSettings`, `MarkdownSettings`, `SearchSettings`, `GraphSettings`, `FilesSettings`, `ImportExportSettings`, `OcrSettings`, `AccessibilitySettings`, `PerformanceSettings`, `PrivacySettings`, `KeyboardShortcutsSettings`, `AdvancedSettings`, `ExperimentalSettings`, `AboutSettings`.
- Settings IPC: `get_settings` (`lib.rs:257`), `settings_get` (`lib.rs:258`), `settings_set` (`lib.rs:259`), `settings_set_all` (`lib.rs:260`), `settings_export` (`lib.rs:261`), `settings_import` (`lib.rs:262`), `settings_reset` (`lib.rs:263`).
- Frontend calls: `crates/nabu-ui/src/lib.rs:57` (theme reading), `crates/nabu-ui/src/components/dictation_pill.rs:21` (dictation opacity), `crates/nabu-ui/src/components/reader.rs:69` (reader mode), `crates/nabu-ui/src/components/navigation/state.rs:298` (view mode).

**READY — Notifications**
- `ToastProvider` (`components/ui/feedback.rs:190-197`) — Provides `ToastContext` via `provide_context()`.
- `use_toast()` (`feedback.rs:349-351`) — Retrieves `ToastContext` from context. Used in 20+ components.
- `ToastContext` (`feedback.rs:80-185`) — Provides `push()`, `push_persistent()`, `push_persistent_with_action()`, `dismiss()`, `dismiss_by_title()`, `has_toast_with_title()`, `info()`, `success()`, `warning()`, `error()`.
- `ToastItem` struct (`feedback.rs:48-62`) — `{id, kind, title, message, action, persistent}`.
- `ToastKind` enum (`feedback.rs:8-16`) — Info, Success, Warning, Error.
- `ToastRegion` (`feedback.rs:242-260`) — Renders toast stack with `role="status"`, `aria-live="polite"`, `aria-label="Notifications"`.
- `ToastAction` (`feedback.rs:64-76`) — Clickable action button with `on_click: Callback`.
- Backend notification commands: `show_macos_notification` (`commands.rs:732`), `show_linux_notification` (`commands.rs:801`).

**READY — Status Indicators**
- `StatusDot` / `StatusKind` (`components/ui/mod.rs:38`) — Visual status indicators.
- `TaskIndicator` / `TaskInfo` / `TaskContext` (`components/ui/mod.rs:38`) — Task/progress tracking.
- `LoadingBlock` / `SpinnerSize` (`feedback.rs`, `mod.rs:35`) — Loading indicators.

**READY — Navigation Structure**
- `ViewMode` enum with 16 variants (`components/app.rs:52-70`): Graph, Notes, Reading, Editor, Search, Canvas, Calendar, Templates, Settings, Collections, Inbox, Archive, SmartFolders, Shortcuts, Dashboard, CommandPalette.
- `NavContext` (`components/navigation/state.rs`) — Central navigation state with `view_mode`, `show_left_sidebar`, `show_right_inspector` signals.
- Sidebar components: `file_tree.rs`, `graph_view.rs`, `reading_queue.rs`, `inbox.rs`.

### Partial Infrastructure

**PARTIAL — Settings System**
The `SettingsStore` supports arbitrary key-value storage via `extra_settings: HashMap<String, serde_json::Value>` (`settings.rs:116`) and `get_value()`/`set_value()` (`settings.rs:359-375`). This could support per-capability settings, but:
- No `nabu.*` namespace convention is established for capability settings.
- No capability settings schema (no `CapabilitySettings` type).
- The `SettingsPanel` tabs are hardcoded at `settings_panel.rs:137-153` — no dynamic capability tab injection.

**PARTIAL — Notification System**
The `ToastContext` is fully functional and used across 20+ components for undo/redo, error, and success notifications. However:
- All toast calls originate from user actions or command responses — **no backend-initiated notifications** exist (no event-to-IPC bridge).
- No notification center or notification history panel.
- Toasts are transient — no persistent notification log.

**PARTIAL — Sidebar Architecture**
The app has a dual sidebar (`show_left_sidebar`, `show_right_inspector` in `NavContext`) but:
- No capability-specific sidebar panel slots.
- No dynamic sidebar section registration for capabilities.
- `right_inspector.rs` is a static inspector panel, not a capability extension point.

### Missing Infrastructure

- **No capability management UI**: No `CapabilityList`, `CapabilityPanel`, `CapabilityToggle`, or `CapabilityStatus` components. No UI to enable/disable capabilities.
- **No capability-specific settings tabs**: The `SettingsPanel` has hardcoded tabs — no mechanism for capabilities to register settings panels.
- **No event-driven updates**: Zero `#[listen]` calls in `crates/nabu-ui/src/`. The frontend cannot receive real-time backend events. No `use_event_listener` or `EventBusContext` exists in the UI layer.
- **No activity panel**: No component for displaying capability-specific activity streams, sync status, or agent conversation history.
- **No streaming interface components**: No `StreamView`, `LiveLog`, or `EventStream` component for real-time output from capabilities.
- **No capability status indicators**: No persistent status bar items, no sync indicators, no ACP connection status, no plugin health badges.
- **No diagnostics panel**: No UI for displaying backend diagnostics, performance metrics, or error logs.
- **No capability sidebar integration**: No slot in the sidebar for capability-specific views (e.g., ACP conversation list, Syncthing device status).
- **No `Capability` serialization for UI**: The `Capability` struct (`plugin/capability.rs:15-49`) is not `Serialize`/`Deserialize`, cannot be sent over IPC.
- **No capability configurator component**: No UI component that dynamically renders settings based on capability metadata.

### Architectural Blockers

- **No event-to-IPC bridge**: This is the single most critical blocker for Phase 5. Without backend events reaching the frontend, Capability UI cannot show real-time status, sync progress, ACP streaming, or Harper diagnostics.
- **No capability metadata serialization**: Without `Capability` being serializable and exposed via IPC, the frontend has no way to discover or render capability panels dynamically.

---

## 9. Phase 6 Gap Analysis — Capability SDK

### Existing Infrastructure

**READY — Public Traits**
- `CaptureHandler` trait (`capture/handler.rs:37-47`) — `#[async_trait]`, `Send + Sync`. `name()`, `source()`, `async capture()`.
- `Processor` trait (`processing/processor.rs:84-102`) — `#[async_trait]`, `Send + Sync`. `name()`, `async process()`, `supports()`.
- `JobExecutor` trait (`jobs/workers/executor.rs:15-23`) — `#[async_trait]`, `Send + Sync`. `async execute()`.
- `Queue` trait (`jobs/queue.rs:16-70`) — `enqueue()`, `dequeue()`, `peek()`, `cancel()`, `retry()`, `reschedule()`, `remove()`, `count()`, `load_job()`, etc.
- `StorageManager` provides direct file I/O methods (not a trait, but a concrete struct).
- `Lifecycle` trait (`registry/lifecycle.rs:202-230`) — `initialize()`, `start()`, `shutdown()`.

**READY — Registration API**
- `ServiceRegistry::register()` (`registry/mod.rs:66-69`) — Register singleton service by key.
- `ServiceRegistry::register_in_category()` (`mod.rs:117-122`) — Register service in a category for batch resolution.
- `ServiceRegistry::resolve_category<T>()` (`mod.rs:183-188`) — Resolve all services in a category as `Vec<Arc<T>>`.
- `CaptureEngine::register_handler()` (`capture/engine.rs`) — Register `CaptureHandler` implementations.
- `ProcessingPipeline::register()` (`processing/pipeline.rs:39-41`) — Register `Processor` implementations.
- `ExecutorRegistry::register()` (`jobs/workers/executor.rs:41-43`) — Register `JobExecutor` implementations.

**READY — Capability Metadata**
- `Capability` struct (`plugin/capability.rs:15-49`) — `{namespace, name, description, required}`.
- `CapabilityRegistry` (`plugin/capability.rs:91-260`) — Full capability registry with registration, enable/disable, query.
- `PluginManifest` (`plugin/manifest.rs`) — Structured plugin metadata with name, version, description, category, capabilities, permissions, dependencies, features.
- `PluginLifecycle` enum (`plugin/lifecycle.rs`) — Discovered → Validated → Installed → Enabled → Disabled → Upgraded → Unloaded.
- 8 category constants for service discovery (`registry/mod.rs:195-217`).

**READY — Shared Services**
- `ApplicationContext` provides typed accessors for all services (`context.rs:224-266`).
- `EventBus` is shared via `Arc<EventBus<PipelineEvent>>` across all components.
- `SettingsStore` provides `extra_settings: HashMap<String, Value>` for capability-specific settings.

**READY — Dependency Graph**
- `DependencyGraph` (`plugin/dependency.rs:20-366`) — `HashMap<String, Vec<String>>` adjacency lists, optional edges, cycle detection, topological ordering, version conflict reporting.
- `PluginDependency` type (`plugin/manifest.rs`) — Required/optional dependency specification with version requirements.

**READY — Permission Model**
- `Permission` struct (`plugin/permissions.rs:18-27`) — `{name, description, risk_level, required}`.
- `PermissionSet` (`plugin/permissions.rs:167-199`) — `grant()`, `deny()`, `is_granted()`.
- `PermissionEvaluator` (`plugin/permissions.rs:200+`) — Evaluates permission checks.
- `RiskLevel` enum — None, Low, Medium, High, Critical.
- Standard permissions defined: `vault.access`, `filesystem.read`, `network.http`, `system.process`, etc.

**READY — Feature Flags**
- `FeatureRegistry` (`plugin/features.rs`) — Feature flag management.
- `FeatureFlag` struct (`plugin/features.rs:18-29`) — `{name, description, enabled_by_default, enabled, stage}`.
- `FeatureStage` enum — Stable, Beta, Alpha, Experimental.

**READY — Version Negotiation**
- `Version` struct (`plugin/version.rs:18-339`) — Semver with major/minor/patch, pre-release, build metadata.
- `VersionRequirement` type — Version constraint specification.
- `CompatibilityResult` — Compatibility checking result.

### Partial Infrastructure

**PARTIAL — Plugin Foundation (Complete But Dead)**
The entire `plugin/` module exists with 8 files (~670 lines) but is **never used in production**:
- `CapabilityRegistry::new()` + `register_builtin()` is called at `src-tauri/src/lib.rs:60-61` — this is the ONLY production usage.
- `PluginManager::new()` is called **zero times** in `src-tauri/src/` — grep returns all 15+ results in `crates/nabu-core/tests/plugin_foundation_integration.rs`.
- `PluginManager` methods (`register_manifest`, `discover`, `enable_plugin`, `disable_plugin`, `resolve_dependencies`) are tested extensively but never invoked from the Tauri backend.
- Evidence: `manager.rs:4` explicitly states "This is a **foundation** implementation. No plugin code is loaded or executed."

**PARTIAL — Shared Events**
`PipelineEvent` (`event_bus/events.rs:8-29`) is an internal backend enum. There is no shared event contract that external/plugin capabilities could publish or subscribe to. The 8 event types are:
- `ItemCaptured`, `ItemProcessingStarted`, `ItemProcessingProgress`, `ItemProcessingCompleted`, `ItemProcessingFailed`, `ItemStored`, `IndexUpdated`, `GraphUpdated`, `ItemCancelled`, `ItemRetried`.

No external capability can publish to these — there is no public `publish()` API exposed via IPC.

### Missing Infrastructure

- **No plugin loading/execution**: The `PluginManager` only validates manifests — no code is loaded, no dynamic libraries are loaded, no Wasm runtime is configured. `manager.rs:4` explicitly states "No plugin code is loaded or executed."
- **No shared services API for plugins**: There is no trait or interface that plugins would implement to access host services. Plugin code cannot call `ServiceRegistry::resolve()` or `EventBus::publish()`.
- **No IPC bridge for plugin-host communication**: No `tauri::command` exposes plugin registration, manifest validation, or capability querying to the frontend.
- **No `CapabilityProvider` trait**: `CapabilityRegistry` stores capability metadata but there is no trait that capability providers implement. The registry has `register(capability, provider: &str)` — the provider is just a string name, not a typed implementation.
- **No event contracts for plugins**: `PipelineEvent` is internal. No `PluginEvent` or `CapabilityEvent` type exists for plugin-host communication.
- **No plugin-to-host IPC abstraction**: Unlike the native messaging protocol (`native_messaging.rs`), there is no general-purpose IPC abstraction for plugin communication.

### Architectural Blockers

- **Plugin system is dead code**: The 670-line plugin foundation is architecturally sound but completely disconnected from production. It must either be integrated (wiring `PluginManager` into `build_application_context`) or removed and replaced with a lighter approach.
- **No execution model**: Phase 6 requires "modular extensions" that can be loaded and executed. The current plugin system has no loader, no runtime, no sandboxing.

---

## 10. Phase 7 Gap Analysis — Production Readiness

### Existing Infrastructure

**READY — Logging & Diagnostics**
- `diagnostics/init.rs:1-334` — `diagnostics::init(None, "nabu")` called at `lib.rs:190`. Structured tracing with `subsystem`, `component`, `operation` fields. Log rotation to `.nabu/logs/nabu.log`, 7-day retention. Configurable via `NABU_LOG`/`RUST_LOG`.
- `diagnostics/metrics.rs` — `Timer` (sliding-window duration), `Counter` (monotonic), `Gauge` (point-in-time), `Histogram` (bucketed distribution), `TimingScope` (RAII stopwatch).
- `diagnostics/spans.rs` — `make_span()`, `traced()` helpers with `SUBSYSTEM_*`, `COMPONENT_*`, `OP_*` constants.
- `diagnostics/performance.rs` — `PerformanceMonitor` with span-based timing.

**READY — Recovery**
- `src-tauri/src/recovery.rs:1-731` — Session persistence (`session_save` at line ~440, `session_load`, `session_clear`), version snapshots (`snapshot_create`, `versions_list`, `versions_get`, `versions_restore`, `versions_duplicate`, `versions_diff`, `versions_all`), crash detection (`mark_running`, `mark_clean_exit`, `recovery_check`, `recovery_discard`).
- `HistoryManager` (`history/mod.rs:14-461`) — Command-based undo/redo with `HistoryEntry` (`{id, timestamp, op, label, affected, previous_state, new_state, undo, redo}`), undo/redo stacks, configurable max depth with pruning. IPC commands: `history_undo`, `history_redo`, `history_clear`, `history_status`, `history_set_depth` at `lib.rs:215-219`.

**READY — Durable Storage**
- `DurableJobQueue` (`jobs/queue.rs:70-603`) — File-backed queue that survives restarts. Persists jobs to disk.
- `StorageManager` (`storage/manager.rs:33-638`) — Writes to vault with JSON sidecar metadata. Persists index.
- `VaultGraph::persist()` (`graph/`) — Persists graph state.
- `Indexer::persist()` (`indexer.rs`) — Persists inverted index to `.nabu/search_index.json`.

**READY — Worker Pool Shutdown**
- `ShutdownCoordinator` (`jobs/workers/shutdown.rs:11-111`) — 30-second drain timeout (`default_timeout()` at line 30), `initiate()` (line 40), `is_shutting_down()` (line 35), `register_worker()`/`unregister_worker()` (lines 45-50). Worker loop checks at `worker.rs:100-110`.

**READY — Testing**
- 112 test modules in `nabu-core`, 50 in `nabu-ui`, 30 in `src-tauri` (from Audit 0.7).
- Integration tests: `crates/nabu-core/tests/plugin_foundation_integration.rs`, plus pipeline, storage, graph, and capture tests.

**READY — State Validation**
- `validate_core_services()` (`context.rs:324-329`) — Validates required services at startup.
- `ctx.initialize()` (`lib.rs:342`) — Called during setup, returns missing services.

### Partial Infrastructure

**PARTIAL — Graceful Shutdown**
- `ApplicationContext::shutdown()` exists at `context.rs:410-421` and `LifecycleManager::transition_to(LifecycleStage::Shutdown)` at `lifecycle.rs:139-174`.
- **But** `lib.rs:402-412` only handles `tauri::RunEvent::Exit` by calling `mark_clean_exit` — it does **not** call `ApplicationContext::shutdown()` or `WorkerPool` shutdown.
- Evidence: `ShutdownCoordinator::initiate()` (`shutdown.rs:40`) is never called in `src-tauri/src/`. Only `WorkerPool::start()` is called at `lib.rs:350`.
- `VaultGraph::persist()` and `Indexer::persist()` exist but are not called in any shutdown path.

**PARTIAL — Performance Monitoring**
- `PerformanceMonitor` exists at `diagnostics/performance.rs` and is referenced as a service in `ApplicationContext::performance_monitor()` (`context.rs:264`).
- **But** the `global_monitor()` singleton is test-only — `diagnostics/performance.rs` references it but `build_application_context()` at `lib.rs:55-180` never registers or starts it.
- Evidence: `performance.rs:15` doc comment mentions "global monitor" but grep for `global_monitor` returns only test references.

**PARTIAL — Recovery (Autosave Bypass)**
- `note_save` (`recovery.rs:391-406`) writes directly to disk bypassing the pipeline. Snapshots are created (`snapshot_note`) but `ITEM_STORED` is not published.
- This means `Indexer` and `VaultGraph` are not updated for autosaved notes, breaking search and graph integrity.

### Missing Infrastructure

- **No application-level shutdown sequence**: No coordinated shutdown that stops `WorkerPool`, persists `VaultGraph`, persists `Indexer`, flushes `SettingsStore`, and shuts down the `NativeMessagingSocket` server (whose handle is discarded at `lib.rs:363`).
- **No socket server lifecycle management**: The `SocketServerHandle` from `native_messaging_socket.rs:356` is discarded at `lib.rs:363` (`Ok(_handle)`), making the socket server an unmanaged zombie process.
- **No metrics endpoint for UI**: `diagnostics/metrics.rs` has Timer, Counter, Gauge, Histogram types but no IPC command or HTTP endpoint to expose metrics to the frontend.
- **No health check endpoint**: No `tauri::command` returns service health status to the frontend.
- **No lifecycle testing framework**: Tests exist for individual components but no integration tests for application startup/shutdown lifecycle.
- **No VaultGraph persistence on exit**: `VaultGraph::persist()` exists but is not called in any exit handler.
- **No Indexer persistence on exit**: `Indexer::persist()` exists but is not called in any exit handler.
- **No SettingsStore flush on exit**: Settings are persisted on each `save()` call but there's no explicit flush during shutdown.

### Architectural Blockers

- **No coordinated shutdown**: The Tauri `Exit` handler at `lib.rs:402-412` does not interact with `ApplicationContext`, `WorkerPool`, `VaultGraph`, `Indexer`, or `SettingsStore`. This is a **production-ready blocker** — data loss and zombie processes are guaranteed on exit.
- **Discarded socket handle**: The `NativeMessagingSocket` cannot be shut down, leaving a dangling Unix domain socket at `/tmp/nabu-native-messaging.sock` (permission `0o777` — security vulnerability).
- **note_save bypass**: The most frequently executed write path bypasses all persistence tracking, silently degrading search and graph accuracy.

---

## 11. Cross-Phase Shared Infrastructure

### Lifecycle Manager
`LifecycleManager` (`registry/lifecycle.rs:94-196`) and the `Lifecycle` trait (`registry/lifecycle.rs:202-230`) **could support**:
- **Phase 2 (Syncthing)**: Starting/stopping the sync sidecar process, transitioning through Created → Initialized → Running → Shutdown.
- **Phase 3 (Harper)**: Starting/stopping the AI analysis workers.
- **Phase 4 (ACP)**: Managing the ACP agent process lifecycle.
- **Phase 7 (Production Readiness)**: Coordinating full application shutdown.

**Current state**: The trait is defined but **never implemented** by any production service. The `LifecycleManager` is used by `ApplicationContext` but only `initialize()` and `start()` are called — `shutdown()` is never invoked.
**Action**: Implement `Lifecycle` trait on `WorkerPool`, `CaptureEngine`, `VaultGraph`, `Indexer`, `StorageManager`, and any future capability services.

### Event Bus
`EventBus<PipelineEvent>` (`event_bus/`) with 8 event types (`event_bus/events.rs:8-29`) **could support**:
- **Phase 2 (Syncthing)**: Emitting sync status events, progress events, conflict events.
- **Phase 3 (Harper)**: Emitting diagnostic available events, analysis progress.
- **Phase 4 (ACP)**: Emitting conversation events, tool call results, streaming tokens.
- **Phase 5 (Capability UI)**: Driving real-time UI updates for all capability status indicators.

**Current state**: Fully functional pub/sub, but **no event-to-IPC bridge** — events never reach the frontend.
**Action**: Add a subscriber in `build_application_context()` that calls `window.emit_all()` for each `PipelineEvent`, enabling frontend `#[listen]` handlers.

### Settings Store
`SettingsStore` (`src-tauri/src/settings.rs:254-390`) with `AppSettings` (117 fields) and `extra_settings: HashMap<String, Value>` **could support**:
- **All phases**: Per-capability configuration via `settings_get`/`settings_set` IPC commands.
- **Phase 2 (Syncthing)**: Sync folder mappings, device keys, conflict resolution preferences.
- **Phase 3 (Harper)**: Per-language analysis settings, confidence thresholds, ignored rules.
- **Phase 4 (ACP)**: Agent endpoint configuration, model selection, conversation history limits.
- **Phase 5 (Capability UI)**: Dynamic capability panel rendering based on capability metadata.

**Current state**: Fully functional with CRUD, export, import, feature toggles.
**Action**: Add `nabu:capability:*` namespace convention for capability-specific settings; expose `CapabilityRegistry` via IPC for dynamic UI generation.

### Notification System
`ToastProvider` + `use_toast()` (`components/ui/feedback.rs`) with `ToastContext`, `ToastItem`, `ToastKind` **could support**:
- **Phase 2 (Syncthing)**: Sync conflict notifications, device pairing prompts.
- **Phase 3 (Harper)**: Analysis complete notifications, error reporting.
- **Phase 4 (ACP)**: Agent response availability, tool execution status.
- **Phase 5 (Capability UI)**: Capability lifecycle notifications (enabled/disabled, errors).

**Current state**: Fully functional on the frontend but only triggered by user actions — **no backend-initiated notifications** (no event-to-IPC bridge).
**Action**: Route backend `PipelineEvent` → Tauri `emit_all` → frontend `#[listen]` → `use_toast()`.

### Process Management
`std::process::Command` usage in `commands.rs` + `native_messaging_socket.rs` **could support**:
- **Phase 2 (Syncthing)**: Spawning and supervising the Syncthing sidecar process.
- **Phase 4 (ACP)**: Spawning and supervising external agent processes.

**Current state**: Ad-hoc `Command::new()` calls, no process supervisor, no restart policies.
**Action**: Create a `ProcessSupervisor` that wraps `tokio::process::Child`, tracks PID, monitors health, and implements restart policies.

### Shared Infrastructure Already Present

| Infrastructure | Location | Phases Served | Status |
|---|---|---|---|
| `ServiceRegistry` + categories | `registry/mod.rs:41` | 1, 2, 3, 4, 5, 6 | FULLY FUNCTIONAL |
| `ApplicationContext` | `registry/context.rs:141` | 1, 2, 3, 4, 7 | FULLY FUNCTIONAL (no shutdown) |
| `LifecycleManager` + `Lifecycle` trait | `registry/lifecycle.rs:94, 202` | 1, 2, 3, 4, 7 | DEFINED, UNIMPLEMENTED |
| `EventBus<PipelineEvent>` | `event_bus/` | 1, 2, 3, 4, 5 | FUNCTIONAL, NO IPC BRIDGE |
| `CapabilityRegistry` | `plugin/capability.rs:91` | 1, 2, 3, 5, 6 | FUNCTIONAL, UNUSED IN PROD |
| `SettingsStore` | `src-tauri/src/settings.rs:254` | 1, 2, 3, 4, 5, 6 | FULLY FUNCTIONAL |
| `ToastProvider` / `use_toast()` | `feedback.rs:78-197` | 5, 7 | FUNCTIONAL, NO BACKEND TRIGGERS |
| `ProgressReporter` | `jobs/workers/progress.rs:8` | 2, 3, 5 | FUNCTIONAL, NO UI INTEGRATION |
| `ShutdownCoordinator` | `jobs/workers/shutdown.rs:11` | 2, 4, 7 | FUNCTIONAL, WORKER-ONLY |
| `HistoryManager` | `history/mod.rs:14` | 5, 7 | FULLY FUNCTIONAL |
| `Recovery` system | `src-tauri/src/recovery.rs` | 7 | FUNCTIONAL (note_save BYPASS) |
| `DurableJobQueue` | `jobs/queue.rs` | 2, 3, 4 | FULLY FUNCTIONAL |
| Diagnostics (tracing, metrics) | `diagnostics/` | 5, 7 | FUNCTIONAL, NO UI EXPOSURE |
| `Processor` trait + 14 processors | `processing/processors/` | 3, 6 | FULLY FUNCTIONAL |
| `CaptureHandler` trait + 9 handlers | `capture/handler.rs:39-47` | 3, 6 | FULLY FUNCTIONAL |
| `JobExecutor` trait + `ExecutorRegistry` | `jobs/workers/executor.rs` | 3, 4, 6 | FULLY FUNCTIONAL |

### Shared Infrastructure Missing

| Infrastructure | Why Needed |
|---|---|
| Event-to-IPC bridge | Backend events → frontend `#[listen]` (blocks Phases 2, 3, 4, 5) |
| Application shutdown sequence | Coordinated service shutdown (blocks Phase 7) |
| `Capability` serialization | Frontend queries capabilities via IPC (blocks Phases 1, 5, 6) |
| Process supervisor | Sidecar/agent process management (blocks Phases 2, 4) |
| JSON-RPC / streaming abstraction | ACP protocol handling (blocks Phase 4) |
| Conversation thread state model | ACP conversation persistence (blocks Phase 4) |
| Capability UI panels | Dynamic capability settings/status (blocks Phase 5) |
| Socket server lifecycle handle | NativeMessagingSocket shutdown (blocks Phase 7) |
| Metrics endpoint for UI | Frontend performance monitoring (blocks Phase 5, 7) |

---

## 12. Dependency Matrix

### Hard Dependencies

| Phase | Depends On | Why |
|---|---|---|
| Phase 1 | None | Foundation — all other phases depend on this |
| Phase 2 | **Phase 1** (event-to-IPC bridge, lifecycle) | Syncthing needs event-to-IPC bridge for status events; needs lifecycle management for sidecar process supervision |
| Phase 3 | **Phase 1**, **Phase 2** (optional) | Harper needs event infrastructure; benefits from process supervision for AI models |
| Phase 4 | **Phase 1**, **Phase 2** (event-to-IPC bridge) | ACP needs subprocess management and event streaming |
| Phase 5 | **Phase 1**, **Phase 4** (event-to-IPC bridge) | Capability UI requires events from all backend capabilities |
| Phase 6 | **Phase 1** | SDK builds on existing ServiceRegistry, Lifecycle, CapabilityRegistry |
| Phase 7 | **Phase 1**, **Phase 2**, **Phase 4** | Graceful shutdown requires lifecycle management from all phases |

### Soft Dependencies

| Phase | Can Use From | Why |
|---|---|---|
| Phase 2 | Phase 3, Phase 4, Phase 6 | Event bus, progress reporter, notifications |
| Phase 3 | Phase 2, Phase 6 | Job executor, processing pipeline |
| Phase 4 | Phase 2, Phase 3 | Process management, text processing |
| Phase 5 | Phase 2, 3, 4, 6 | Settings store, notifications, diagnostics |
| Phase 6 | Phase 1, 2, 3, 4, 5 | All existing trait patterns, service registry |
| Phase 7 | All phases | Testing, recovery, shutdown |

### Parallelizable Work

| Work That Can Proceed in Parallel | Dependencies |
|---|---|
| Implement `Lifecycle` trait on existing services | Phase 1 only |
| Implement event-to-IPC bridge | Phase 1 only |
| Fix `note_save` pipeline bypass | Phase 1 only (ITEM_STORED subscribers) |
| Add `Capability` serialization | Phase 1 only |
| Create `ProcessSupervisor` | Phase 1 only (tokio runtime) |
| Create `Capability` UI components | Phase 1 only (SettingsStore, ToastProvider) |

### Architectural Prerequisites

1. **Event-to-IPC bridge** must exist before Phases 2, 3, 4, 5 can deliver real-time updates.
2. **Lifecycle trait implementation** must exist before Phase 7 (graceful shutdown).
3. **`note_save` pipeline integration** must exist before any phase relying on persisted state (all phases) can work correctly.
4. **Plugin system decision** must be made before Phase 6 (extend or kill the 670-line dead `plugin/` module).

---

## 13. Reuse Inventory

### Phase 1 — Capability Framework Foundation

**Existing Infrastructure:**
- `ServiceRegistry` — `crates/nabu-core/src/registry/mod.rs:41-189` (struct), `service.rs` not found (code is in mod.rs)
- `ApplicationContext` — `crates/nabu-core/src/registry/context.rs:141-422`
- `ApplicationContextBuilder` — `context.rs:428-478`
- `LifecycleManager` — `crates/nabu-core/src/registry/lifecycle.rs:94-196`
- `LifecycleStage` enum — `lifecycle.rs:25-36`
- `Lifecycle` trait — `lifecycle.rs:202-230`
- `CapabilityRegistry` — `crates/nabu-core/src/plugin/capability.rs:91-260`
- `Capability` struct — `capability.rs:15-49`
- `builtin_capabilities()` — `capability.rs:62-79`
- `PermissionEvaluator` — `crates/nabu-core/src/plugin/permissions.rs:200+`
- `PermissionSet` — `permissions.rs:167-199`
- `FeatureRegistry` — `crates/nabu-core/src/plugin/features.rs`
- `PluginLifecycle` enum — `crates/nabu-core/src/plugin/lifecycle.rs`
- `SettingsStore` — `src-tauri/src/settings.rs:254-390`
- `AppSettings` — `settings.rs:1-117`
- `build_application_context()` — `src-tauri/src/lib.rs:55-180`
- `validate_core_services()` — `context.rs:324-329`
- `ServiceHealth` / `ValidationReport` — `context.rs:63-131`

**Missing Infrastructure:**
- No capability-to-service binding API (no method to map `nabu:sync` → actual sync service)
- No IPC command to query capabilities from frontend
- No Capability serialization for IPC boundary crossing
- No runtime capability enable/disable that affects services
- No capability configuration schema

### Phase 2 — Syncthing Capability

**Existing Infrastructure:**
- `native_messaging.rs` — `src-tauri/src/native_messaging.rs:1-281` (Message, read/write_message, validation)
- `native_messaging_socket.rs` — `src-tauri/src/native_messaging_socket.rs` (SocketServerState, start_socket_server, SocketServerHandle)
- `SettingsStore` — `src-tauri/src/settings.rs:254`
- `EventBus` — `crates/nabu-core/src/event_bus/bus.rs`
- `PipelineEvent` enum — `event_bus/events.rs:8-29` (8 event types)
- `EventBus::subscribe()` — `bus.rs:151`
- `ProgressReporter` — `crates/nabu-core/src/jobs/workers/progress.rs:8-155`
- `ExecutorRegistry` — `jobs/workers/executor.rs:29` (pattern for dynamic service lookup)
- `WorkerPool.start()` — `jobs/workers/pool.rs:52`, called at `lib.rs:348-352`
- `ShutdownCoordinator` — `jobs/workers/shutdown.rs:11-111` (30s drain timeout)

**Missing Infrastructure:**
- No process supervisor (restart policies, health checks, crash detection)
- No event-to-IPC bridge (backend events → frontend)
- No persistent process state tracking (PID files, restart counts)
- No folder sync state model (SyncFolder, SyncStatus, ConflictResolution types)
- No conflict resolution mechanism

### Phase 3 — Harper Capability

**Existing Infrastructure:**
- `Processor` trait — `crates/nabu-core/src/processing/processor.rs:84-102`
- `ProcessingPipeline` — `processing/pipeline.rs:15-290`
- `ProcessingContext` — `processor.rs:1-79`
- `ProcessingResult` — `processor.rs:2-79`
- `build_standard_pipeline()` — constructs 14-processor pipeline
- `CaptureHandler` trait — `capture/handler.rs:37-47`
- `CaptureEngine` — `capture/engine.rs`
- 9 `CaptureHandler` implementations
- `JobExecutor` trait — `jobs/workers/executor.rs:15-23`
- `ExecutorRegistry` — `executor.rs:29-70` (dynamic executor registration)
- `PipelineExecutor` — `pipeline_migration/executor.rs:24-177`
- `ProgressReporter` — `jobs/workers/progress.rs:8`
- `CancellationToken` — `jobs/cancellation.rs`
- `AppSettings` OCR/Whisper/AI fields (`settings.rs:183-219`)

**Missing Infrastructure:**
- No diagnostic rendering pipeline (no Decoration, Diagnostic, Suggestion types)
- No editor extension points (no ExtensionPoint trait)
- No debounce system (no DebounceController)
- No annotation model (no TextRange, no severity levels)
- No editor ↔ backend bridge for diagnostics
- No dynamic processor registration via IPC

### Phase 4 — ACP Client Capability

**Existing Infrastructure:**
- `std::process::Command` usage in `commands.rs` (11 call sites for process spawning)
- `native_messaging.rs` — stdin/stdout JSON message protocol (read_message, write_message)
- `tauri::async_runtime::spawn()` — async task spawning (used at `lib.rs:349, 360, 377`)
- `EventBus` pub/sub (`event_bus/bus.rs`)
- `CancellationToken` (`jobs/cancellation.rs`)
- `HistoryManager` — command-based state tracking (`history/mod.rs:14`)
- `SettingsStore` — persistent configuration
- `JobExecutor` — async execution pattern
- `WorkerPool` — async task pool (`jobs/workers/pool.rs`)

**Missing Infrastructure:**
- No JSON-RPC implementation (no request/response framing, no method routing)
- No stdin/stdout bidirectional pipe abstraction for async I/O
- No streaming message handling (no Stream-based async message receiver)
- No message routing by method name
- No conversation thread state model (no Thread, Message, Turn types)
- No tool calling abstraction (no Tool trait, no ToolCall type)
- No event-to-IPC bridge for streaming events

### Phase 5 — Capability UI

**Existing Infrastructure:**
- `SettingsPanel` — `components/settings/settings_panel.rs:114-179` (13 tabs, AppSettings binding)
- 13+ setting sub-components (AppearanceSettings, EditorSettings, etc.)
- `ToastProvider` / `ToastContext` / `use_toast()` — `components/ui/feedback.rs:78-197`
- `ToastItem` / `ToastKind` / `ToastAction` / `ToastRegion` — full toast system
- `StatusDot` / `StatusKind` — visual status indicators (`components/ui/mod.rs:38`)
- `TaskIndicator` / `TaskInfo` / `TaskContext` — task tracking
- `LoadingBlock` / `SpinnerSize` — loading indicators
- `ViewMode` enum — 16 view variants (`components/app.rs:52-70`)
- `NavContext` — central navigation state (`components/navigation/state.rs`)
- Dual sidebar (`show_left_sidebar`, `show_right_inspector`)
- Settings IPC: `get_settings`, `settings_get`, `settings_set`, `settings_set_all`, `settings_export`, `settings_import`, `settings_reset`
- `RwSignal` / `Signal` state management throughout

**Missing Infrastructure:**
- No capability management UI (no CapabilityList, CapabilityPanel, CapabilityToggle)
- No capability-specific settings tabs
- No capability status indicators
- No event-driven UI updates (zero #[listen] calls in frontend)
- No activity panel for capability events
- No streaming interface components
- No diagnostics panel
- No capability sidebar integration slots
- Capability struct not serializable for IPC

### Phase 6 — Capability SDK

**Existing Infrastructure:**
- `plugin/` module — 8 files, ~670 lines:
  - `CapabilityRegistry` — `plugin/capability.rs:91-260`
  - `PluginManager` — `plugin/manager.rs:38-130` (manifest validation, lifecycle tracking)
  - `PluginManifest` — `plugin/manifest.rs` (structured metadata)
  - `PluginLifecycle` / `PluginLifecycleEvent` / `PluginStage` — `plugin/lifecycle.rs`
  - `PermissionEvaluator` / `PermissionSet` / `Permission` / `RiskLevel` — `plugin/permissions.rs`
  - `FeatureRegistry` / `FeatureFlag` / `FeatureStage` — `plugin/features.rs`
  - `DependencyGraph` — `plugin/dependency.rs:20-366`
  - `Version` / `VersionRequirement` / `CompatibilityResult` — `plugin/version.rs`
- `Processor` trait — `processing/processor.rs:84` (extensible trait pattern)
- `CaptureHandler` trait — `capture/handler.rs:37` (extensible trait pattern)
- `JobExecutor` trait — `jobs/workers/executor.rs:15` (extensible trait pattern)
- `Queue` trait — `jobs/queue.rs:16` (storage abstraction)
- `ServiceRegistry` with category discovery — `registry/mod.rs:41`
- `Lifecycle` trait — `registry/lifecycle.rs:202`
- `PipelineEvent` — `event_bus/events.rs:8` (event contract)
- 8 category constants for service discovery (`registry/mod.rs:196-217`)
- Integration tests: `plugin_foundation_integration.rs` (598 lines of tests)

**Missing Infrastructure:**
- No plugin loading/execution runtime
- No public trait for capability providers (CapabilityProvider trait)
- No shared services API for plugins
- No plugin-to-host IPC abstraction
- No event contracts for plugin communication
- No SDK trait exports designed for external consumption

### Phase 7 — Production Readiness

**Existing Infrastructure:**
- `diagnostics/init.rs` — `diagnostics::init(None, "nabu")` at `lib.rs:190` (tracing init, log rotation, NABU_LOG config)
- `diagnostics/metrics.rs` — Timer, Counter, Gauge, Histogram, TimingScope
- `diagnostics/spans.rs` — make_span(), traced(), subsystem/component/operation constants
- `diagnostics/performance.rs` — PerformanceMonitor
- `ShutdownCoordinator` — `jobs/workers/shutdown.rs:11-111` (30s drain timeout)
- `WorkerPool::start()` — `pool.rs:52`, `lib.rs:350`
- `HistoryManager` — `history/mod.rs:14` (undo/redo with persistence)
- `Recovery` — `src-tauri/src/recovery.rs` (session, versioning, crash detection)
- `DurableJobQueue` — file-backed persistence (`jobs/queue.rs`)
- `VaultGraph::persist()` — graph persistence
- `Indexer::persist()` — search index persistence (`indexer.rs`)
- `SettingsStore` — persistent settings (`settings.rs:254`)
- `mark_running` / `mark_clean_exit` / `recovery_check` — crash detection (`recovery.rs:326-396`)
- 112 test modules in nabu-core
- `validate_core_services()` — service validation (`context.rs:324`)

**Missing Infrastructure:**
- No application-level graceful shutdown sequence
- No socket server lifecycle handle (discarded at `lib.rs:363`)
- No metrics endpoint for UI consumption
- No health check endpoint
- No lifecycle testing framework
- No VaultGraph persist call on exit
- No Indexer persist call on exit
- No SettingsStore flush on exit

---

## 14. Missing Infrastructure Inventory

### Cross-Phase Missing Infrastructure (affects multiple phases)

| Infrastructure | Affected Phases | File/Location Needed |
|---|---|---|
| Event-to-IPC bridge | 2, 3, 4, 5 | `src-tauri/src/lib.rs` — add subscriber that calls `window.emit_all()` |
| Application shutdown sequence | 7 | `src-tauri/src/lib.rs:402-412` — extend Exit handler |
| Capability serialization | 1, 5, 6 | `crates/nabu-core/src/plugin/capability.rs` — add Serialize/Deserialize to Capability |
| Process supervisor | 2, 4 | New module: `crates/nabu-core/src/process/supervisor.rs` |
| JSON-RPC abstraction | 4 | New module: `crates/nabu-core/src/rpc/mod.rs` |
| Conversation state model | 4, 5 | New types: Thread, Message, Turn |
| Capability UI components | 5 | `crates/nabu-ui/src/components/` — new capability panels |
| Socket server lifecycle | 7 | `src-tauri/src/lib.rs:362` — store SocketServerHandle |

### Phase-Specific Missing Infrastructure

| Phase | Missing | Rationale |
|---|---|---|
| Phase 1 | Capability-to-service binding API | CapabilityRegistry stores metadata only, no service mapping |
| Phase 1 | IPC command for capabilities | No `tauri::command` exposes capabilities |
| Phase 2 | Conflict resolution types | No SyncFolder, SyncStatus, ConflictResolution types |
| Phase 2 | Persistent process state | No PID tracking or restart-count persistence |
| Phase 3 | Diagnostic data model | No Decoration, Diagnostic, Suggestion types |
| Phase 3 | Editor integration bridge | No IPC for requesting/sending diagnostics |
| Phase 3 | Debounce controller | No timer-based analysis throttle |
| Phase 4 | Tool calling abstraction | No Tool trait or ToolCall type |
| Phase 4 | Conversation persistence | No Thread/Message persistence model |
| Phase 5 | Capability settings panels | SettingsPanel tabs are hardcoded |
| Phase 5 | Activity panel | No capability activity stream component |
| Phase 5 | Notification center | No persistent notification history |
| Phase 6 | Plugin loader/runtime | No Wasm runtime, no native library loading |
| Phase 6 | CapabilityProvider trait | No trait for plugin capability implementation |
| Phase 7 | Health check endpoint | No IPC for service health status |
| Phase 7 | Lifecycle tests | No integration tests for startup/shutdown |

---

## 15. Implementation Risk Assessment

### Phase 1 — LOW
**Rationale:** All core abstractions already exist and are production-tested. `ServiceRegistry`, `ApplicationContext`, `LifecycleManager`, `CapabilityRegistry`, and `SettingsStore` are all functional. The primary work is:
- Implement `Lifecycle` trait on 5 production services (mechanical, low-risk)
- Add `serde` derives to `Capability` struct (trivial)
- Add 1-2 IPC commands for capability querying (straightforward)

**Risk factors:** The dead `Application` struct (`registry/application.rs:514 lines`) creates confusion about the canonical pattern. Must ensure the fix extends `ApplicationContext`, not `Application`.

### Phase 2 — HIGH
**Rationale:** The process management infrastructure exists (`native_messaging.rs`, `std::process::Command`) but requires a **new `ProcessSupervisor`** abstraction that manages process lifecycle, restart policies, and health checks. The discarded socket handle at `lib.rs:363` must be fixed. The event-to-IPC bridge is a hard dependency.

**Risk factors:**
- Cross-platform process management complexity (macOS/Linux/Windows)
- Socket file cleanup and security (`0o777` permissions on `/tmp/nabu-native-messaging.sock`)
- Dependency on Phase 1 lifecycle implementation
- No existing conflict resolution model

### Phase 3 — MEDIUM
**Rationale:** The processing pipeline (`Processor` trait, `ProcessingPipeline`, `ExecutorRegistry`) is fully functional and extensible. Adding Harper as a new `Processor` is low-risk. However, the **editor integration** (diagnostic rendering, inline decorations) requires significant UI work with no existing patterns to extend.

**Risk factors:**
- No existing diagnostic data model — must be designed from scratch
- No editor ↔ backend communication channel (no IPC for requesting diagnostics)
- The `Processor` trait's `ProcessingResult` (`processor.rs:2-79`) doesn't carry structured diagnostic data

### Phase 4 — VERY HIGH
**Rationale:** There is **no JSON-RPC implementation**, **no stdin/stdout abstraction**, **no conversation state model**, and **no streaming message handling** anywhere in the codebase. This phase requires building a fundamentally new abstraction layer. The `native_messaging.rs` protocol handles length-prefixed JSON but does not implement JSON-RPC framing.

**Risk factors:**
- Entirely new abstraction (no existing patterns to extend)
- Async message routing complexity
- Dependency on Phase 2 (ProcessSupervisor) and Phase 1 (event-to-IPC bridge)
- No existing conversation model for state persistence
- Tool calling requires a new trait and execution model

### Phase 5 — MEDIUM
**Rationale:** The UI foundation is solid: `SettingsPanel` with 13 tabs, `ToastProvider` with full toast system, `ViewMode` routing with 16 views, and a well-structured Leptos component system. Adding capability panels follows the existing `AppearanceSettings`/`EditorSettings` pattern. The risk is entirely dependent on the event-to-IPC bridge.

**Risk factors:**
- Hard dependency on Phase 1's event-to-IPC bridge (if not built, UI cannot receive real-time updates)
- The SettingsPanel tabs are hardcoded — dynamic tab injection is a non-trivial change
- No existing pattern for streaming UI components

### Phase 6 — VERY HIGH
**Rationale:** The `plugin/` module contains 670+ lines of complete, tested plugin foundation. However, it is **dead code** that must be integrated into the production path (`build_application_context`), or the team must decide to remove it and build a new SDK. The integration path requires:
- Wiring `PluginManager` into `build_application_context()` at `lib.rs:55-180`
- Converting `Capability` from metadata-only to a typed provider trait
- Adding plugin loading runtime (Wasm, or native library loading)
- Creating shared event contracts

**Risk factors:**
- Decision point: integrate dead code or rebuild (architectural commitment)
- `CapabilityRegistry` stores string provider names, not typed implementations — needs redesign
- No plugin execution model exists (manifest-only validation)
- The dead `Application` struct (`registry/application.rs`) competes with the plugin system for the same design space

### Phase 7 — MEDIUM
**Rationale:** The diagnostics infrastructure (`diagnostics/` module) is fully functional. `ShutdownCoordinator` exists for the worker pool. `Recovery` system handles sessions and versioning. The fix is mechanical: extend the Exit handler to call `shutdown()` on all services. However, the discarded socket handle at `lib.rs:363` is a security vulnerability (`0o777` perms).

**Risk factors:**
- Security vulnerability in socket file permissions (`0o777` at `native_messaging_socket.rs:356`)
- `note_save` bypass breaks version history integrity
- No health check API for frontend consumption
- Metrics exist but have no UI exposure path

---

## 16. Recommended Phase Order Validation

The roadmap's proposed phase order (1 → 2 → 3 → 4 → 5 → 6 → 7) is **largely correct** but has a critical dependency that the roadmap does not surface early enough.

### Validated Order

1. **Phase 0 (UI Framework Migration)** — Must be first. All UI component work depends on a stable Dioxus + Tauri + IPC foundation. This unlocks all Capability UI work in Phase 5. ✅ **New prerequisite**.
2. **Phase 1 (Framework Foundation)** — Depends on Phase 0 for UI components that display lifecycle status, capability registry, and settings. All other phases depend on `ServiceRegistry`, `ApplicationContext`, `Lifecycle`, and `CapabilityRegistry`. ✅ Correct.
3. **Phase 2 (Syncthing)** — Depends on Phase 1's lifecycle and event infrastructure. ✅ Correct order.
4. **Phase 3 (Harper)** — Depends on processing pipeline (which is existing infra, not Phase 1), and benefits from Phase 2's process supervision. ✅ Correct order.
5. **Phase 4 (ACP Client)** — Depends on Phase 2's process supervision and Phase 1's event-to-IPC bridge. ✅ Correct order.
6. **Phase 5 (Capability UI)** — Depends on Phases 1-4 for events, status, and notifications. Depends on Phase 0 for Dioxus UI components. ✅ Correct order.
7. **Phase 6 (Capability SDK)** — Can actually proceed in parallel with Phases 2-5 once Phase 1 is complete. The plugin foundation exists and could be integrated independently. ⚠️ **Suggestion**: Phase 6 could be parallelized with Phase 2+ rather than sequential.
8. **Phase 7 (Production Readiness)** — Must be last. Depends on all phases for graceful shutdown, testing, and validation. ✅ Correct order.

### Revised Recommendation

The roadmap order is correct, **provided that these prerequisites are met before Phase 2 begins**:

1. **Complete Phase 0 (UI Framework Migration)** — Dioxus + Tauri foundation established. ✅ New prerequisite.
2. **Fix `note_save` to route through `StorageManager.save()`** — This is not explicitly a Phase 1 deliverable but breaks every phase that depends on persisted state integrity.
2. **Implement the event-to-IPC bridge** — Without this, Phases 2-5 cannot deliver real-time updates. This should be treated as a Phase 1.5 critical prerequisite.
3. **Decision on the plugin system** — Phase 6 depends on whether the 670-line dead `plugin/` module is integrated or replaced. This decision must precede Phase 2 if the Syncthing capability will be implemented as a plugin rather than a built-in service.

### Critical Path

```
Phase 1 (Framework) → [Event-to-IPC bridge + Lifecycle implementation + note_save fix] → 
Phase 2 (Syncthing) → 
Phase 3 (Harper) + Phase 4 (ACP) → 
Phase 5 (Capability UI) → 
Phase 6 (Capability SDK) — parallelizable with Phase 2-5 → 
Phase 7 (Production Readiness)
```

The event-to-IPC bridge and `note_save` fix are the two highest-leverage changes — they unblock the most phases simultaneously.

---

## Implementation Matrix

| Roadmap Phase | Existing Infrastructure | Can Be Extended | Must Be Built | Readiness | Estimated Complexity |
|---|---|---|---|---|---|
| **Phase 1 — Capability Framework Foundation** | `ServiceRegistry`, `ApplicationContext`, `LifecycleManager`, `Lifecycle` trait, `CapabilityRegistry`, `SettingsStore`, `build_application_context()`, `validate_core_services()` | Implement `Lifecycle` trait on 5 production services; add `serde` to `Capability`; add IPC commands for capability queries | Capability-to-service binding API; `Capability` serialization for IPC; capability config schema | PARTIALLY READY | LOW |
| **Phase 2 — Syncthing Capability** | `native_messaging.rs`, `native_messaging_socket.rs`, `EventBus`, `PipelineEvent`, `ProgressReporter`, `SettingsStore`, `std::process::Command` (11 call sites), `WorkerPool.start()`, `ShutdownCoordinator` | Extend `EventBus` with event-to-IPC bridge; fix socket handle lifecycle; add capability IPC commands | Process supervisor (restart policy, health checks); folder sync state model; conflict resolution types; persistent process state | NOT READY | HIGH |
| **Phase 3 — Harper Capability** | `Processor` trait, `ProcessingPipeline` (14 processors), `CaptureHandler` (9 implementations), `JobExecutor`, `ExecutorRegistry`, `PipelineExecutor`, `ProgressReporter`, `CancellationToken`, OCR/Whisper/AI settings | Register new Harper processors via `ProcessingPipeline::register()`; use existing `ProgressReporter` for progress | Diagnostic data model (Diagnostic, Decoration, Suggestion types); editor integration bridge; debounce controller; text range model | PARTIALLY READY | MEDIUM |
| **Phase 4 — ACP Client Capability** | `std::process::Command` (11 call sites), `native_messaging.rs` (JSON protocol), `tauri::async_runtime::spawn()`, `EventBus`, `CancellationToken`, `HistoryManager`, `WorkerPool` | Use `WorkerPool` pattern for agent task execution; use `CancellationToken` for cancellation | JSON-RPC implementation; stdin/stdout bidirectional pipe abstraction; streaming message handling; conversation thread state model; tool calling abstraction | NOT READY | VERY HIGH |
| **Phase 5 — Capability UI** | `SettingsPanel` (13 tabs), `ToastProvider`/`use_toast()` (86 usages), `StatusDot`/`TaskIndicator`, `ViewMode` (16 variants), `NavContext`, `AppSettings` (117 fields), Settings IPC (7 commands) | Add capability tab to SettingsPanel; route events via event-to-IPC bridge to toast system | Capability management UI; capability status indicators; activity panel; streaming interface components; diagnostics panel; `Capability` serialization | NOT READY | MEDIUM |
| **Phase 6 — Capability SDK** | `plugin/` module (8 files, ~670 lines): `PluginManager`, `CapabilityRegistry`, `PluginManifest`, `PluginLifecycle`, `PermissionEvaluator`, `FeatureRegistry`, `DependencyGraph`, `Version`; `Processor`/`CaptureHandler`/`JobExecutor` trait patterns; `ServiceRegistry` with categories | Extend `plugin/` module to integrate into production; add `CapabilityProvider` trait on existing patterns | Plugin loader/runtime (Wasm or native); `CapabilityProvider` trait; shared event contracts; plugin-to-host IPC abstraction | PARTIALLY READY | VERY HIGH |
| **Phase 7 — Production Readiness** | `diagnostics/` (init, metrics, spans, performance); `ShutdownCoordinator` (30s drain); `HistoryManager` (undo/redo); `Recovery` (sessions, snapshots); `DurableJobQueue`; `VaultGraph::persist()`, `Indexer::persist()`; `validate_core_services()`; 112 test modules | Extend Exit handler to call `ApplicationContext::shutdown()`; persist VaultGraph/Indexer/StorageManager | Application shutdown sequence; socket server lifecycle handle; metrics endpoint for UI; health check endpoint; lifecycle integration tests | PARTIALLY READY | MEDIUM |

---

## Concluding Questions

### 1. Does the existing Nabu architecture support the Capability Platform without requiring a fundamental redesign?

**Partially.** The architecture provides the foundational abstractions needed by every phase of the roadmap: `ServiceRegistry` for service registration, `ApplicationContext` for DI, `LifecycleManager` for lifecycle management, `EventBus` for event routing, `CapabilityRegistry` for capability discovery, `SettingsStore` for configuration, and `ToastProvider` for UI notifications. The trait-based extensibility patterns (`CaptureHandler`, `Processor`, `JobExecutor`, `Queue`) provide reusable extension points.

However, **three architectural gaps** prevent direct utilization:

- **No event-to-IPC bridge** (`lib.rs:162-177` subscribes to `ITEM_STORED` but never emits to Tauri frontend). The `EventBus` at `event_bus/bus.rs` is fully functional but events never cross the Rust→WASM boundary. The frontend has **zero** `#[listen]` calls.
- **No graceful shutdown sequence** (`lib.rs:402-412` only removes a crash marker). `ApplicationContext::shutdown()` exists at `context.rs:410` but is never called. The `SocketServerHandle` is discarded at `lib.rs:363` (`Ok(_handle)`), leaving a zombie socket server with `0o777` permissions.
- **Dead plugin system** (670+ lines, `plugin/` module) creates architectural ambiguity. `CapabilityRegistry::register_builtin()` is called at `lib.rs:60` but `PluginManager` is test-only.

These are **integration gaps**, not design flaws. The codebase does not need a fundamental redesign — it needs **three critical fixes**: (1) implement the event-to-IPC bridge, (2) call `ApplicationContext::shutdown()` in the Exit handler, (3) integrate or remove the plugin system. After these fixes, the architecture supports the Capability Platform with moderate extension work (Phases 1, 3, 5, 7 are LOW/MEDIUM risk; Phases 2, 4, 6 require significant new abstractions).

### 2. Which roadmap assumptions are already validated by the current codebase?

**Validated assumptions:**

- **"Capability Registry"** — `CapabilityRegistry` exists at `plugin/capability.rs:91-260` with `register()`, `enable()`, `disable()`, `is_enabled()`, `list_enabled()`, and 14 built-in `nabu:*` capabilities registered at `lib.rs:60-61`.
- **"Runtime Registration"** — `ServiceRegistry::register()` (`registry/mod.rs:66`) and category-based discovery (`register_in_category()` at line 117, `resolve_category()` at line 183) allow runtime service registration. `CaptureEngine::register_handler()` and `ProcessingPipeline::register()` follow the same pattern.
- **"Settings Integration"** — `SettingsStore` (`src-tauri/src/settings.rs:254`) provides full CRUD with `extra_settings: HashMap<String, Value>` for arbitrary capability settings, `get_feature_toggles()`/`set_feature_toggle()` for feature gating, and export/import via `SettingsExport` envelope.
- **"Sidecar Process" pattern** — `native_messaging.rs` provides a complete stdin/stdout JSON message protocol (length-prefixed, 1MB max payload, command whitelist). The `native_messaging_socket.rs` provides a Unix socket server pattern. Both show that process communication infrastructure is feasible.
- **"Status Events"** — `PipelineEvent` enum with 8 variants (`event_bus/events.rs:8-29`) and string constants in `kinds` module (`events.rs:32-43`) provide structured event types for `item.captured`, `item.processing.progress`, `item.stored`, etc.
- **"Conflict Handling" data model** — `VaultGraph` and `StorageManager` both have version tracking via `recovery.rs` snapshots, providing a basis for conflict detection.
- **"Diagnostics" infrastructure** — `diagnostics/` module provides structured tracing with `Timer`, `Counter`, `Gauge`, `Histogram`, `TimingScope`, span helpers, and 18 subsystem identifiers.
- **"Streaming" pattern** — `ProgressReporter` (`jobs/workers/progress.rs:8`) uses a callback `Arc<dyn Fn(f64)>` that could extend to token streaming. The `WorkerPool` async pattern at `lib.rs:348-352` shows how background tasks are spawned.
- **"Process Lifecycle"** — `LifecycleManager` with one-way stage transitions (`Created → Initialized → Running → Shutdown`) at `registry/lifecycle.rs:25-36` provides the lifecycle model.
- **"Configuration"** — `AppSettings` with 60+ typed fields and `extra_settings` HashMap provides configuration storage. Feature flags via `FeatureRegistry` (`plugin/features.rs`) provide staged rollout.
- **"Shared Events"** — `PipelineEvent` is already a shared event contract. `EventBus::subscribe()` and `EventBus::publish()` (`bus.rs:133-151`) provide the mechanism.

### 3. Which roadmap items should be revised because the architecture already provides equivalent functionality?

Several roadmap items describe infrastructure that already exists:

- **"Capability Manager"** — The `ApplicationContext` (`registry/context.rs:141-422`) already serves as the capability manager. It holds the `ServiceRegistry`, `EventBus`, `CapabilityRegistry`, and `LifecycleManager`, and provides typed accessors (`capture_engine()`, `processing_pipeline()`, `job_queue()`, `worker_pool()`, `vault_graph()`, `indexer()`, `storage_manager()`, `history_manager()`, `performance_monitor()`). No separate "CapabilityManager" struct is needed — `ApplicationContext` fulfills this role. The roadmap should reference `ApplicationContext` as the capability manager rather than proposing a new type.

- **"Capability State"** — `LifecycleManager` (`registry/lifecycle.rs:94-196`) with `LifecycleStage` enum already tracks capability/service state through ordered stages (Created → Initialized → Running → Shutdown). No separate "CapabilityState" enum is needed. The roadmap should specify implementing the existing `Lifecycle` trait on services rather than introducing a new state type.

- **"Background Services"** — `WorkerPool` (`jobs/workers/pool.rs:14-191`) with `start()`/`ShutdownCoordinator` already provides the background service pattern. The `tauri::async_runtime::spawn()` calls at `lib.rs:349, 360, 377` demonstrate the async spawning pattern. No new "background service" abstraction is needed — extend `Lifecycle` trait usage.

- **"JSON Serialization"** — Already pervasive. `serde_json` is used for `CaptureRequest` (`capture/handler.rs:50-60`), `KnowledgeObject`, `ObjectMetadata`, all `PipelineEvent` variants, `AppSettings`, `HistoryEntry` (`history/mod.rs:103-123`). For Phase 4, the team should extend the existing `serde_json` pattern rather than introducing a new JSON library.

- **"Message Routing"** — `CaptureEngine` (`capture/engine.rs`) routes captures to handlers by `CaptureSource`. `PipelineExecutor` (`pipeline_migration/executor.rs:24-177`) routes jobs to executors by `processor_name` via `ExecutorRegistry`. Extending this routing pattern (source→handler → executor→processor) to ACP message routing is the idiomatic approach.

- **"Performance" / "Diagnostics"** — `diagnostics/metrics.rs` already provides `Timer`, `Counter`, `Gauge`, `Histogram`, `TimingScope`. `diagnostics/spans.rs` provides span helpers. The roadmap should reference these existing types rather than proposing new metrics systems.

- **"Lifecycle Testing"** — The `LifecycleManager` tests at `lifecycle.rs:232-310` already test stage transitions, skip transitions, backward transition failure. The pattern should be extended for application lifecycle tests at `lib.rs:402`.

### 4. What is the minimum amount of foundational work required before Phase 1 can begin?

Phase 1 is **already the starting point** — the foundational infrastructure exists. The "before Phase 1 begins" work is actually **pre-Phase-1 critical fixes** that don't belong to any single phase. Three items are required:

1. **Fix the `note_save` pipeline bypass** (`src-tauri/src/recovery.rs:391-406`). This is the most frequently executed write path and it bypasses `StorageManager.save()`, meaning `ITEM_STORED` events never fire for autosaves. At `lib.rs:162-177`, the only `ITEM_STORED` subscriber calls `Indexer.index_object()` and `VaultGraph.add_node()`. Without fixing this bypass, search indexing and graph updates silently fail for autosaved notes. **Fix**: Route `note_save` through `StorageManager.save()` or at minimum call `storage.save()` instead of `std::fs::write()`.

2. **Implement the event-to-IPC bridge** (new code in `src-tauri/src/lib.rs`). The `EventBus.subscribe()` call at `lib.rs:162` shows the pattern, but no events are forwarded to the Tauri frontend. The bridge needs: (a) additional `event_bus.subscribe()` calls for `ITEM_PROCESSING_STARTED`, `ITEM_PROCESSING_PROGRESS`, `ITEM_PROCESSING_COMPLETED`, `ITEM_PROCESSING_FAILED`, and other events; (b) a subscriber closure that calls `window.emit_all("nabu-event", event_json)`. This is ~20 lines of code.

3. **Decide on the plugin system** (`crates/nabu-core/src/plugin/`). The 670-line `plugin/` module is complete but dead. `CapabilityRegistry::register_builtin()` is called at `lib.rs:60-61`, but `PluginManager` is test-only. Before Phase 1 can define "Capability Registry" as a living system (not just metadata), the team must decide: integrate `PluginManager` into `build_application_context()` or remove the dead code and design a lighter approach. This decision affects Phase 6 directly but must be resolved before Phase 1's capability registry is considered production-ready.

4. **(Recommended) Implement `Lifecycle` trait on production services.** While not strictly blocking Phase 1 (the trait exists and `ApplicationContext` uses `LifecycleManager`), without implementing `initialize()`/`shutdown()` on `WorkerPool`, `CaptureEngine`, etc., Phase 7's graceful shutdown cannot function. This is low-effort (5 implementations) and should be done alongside the Phase 1 work.

**In summary**: Phase 1 can technically begin today, but the `note_save` bypass and event-to-IPC bridge are **critical prerequisites** that affect every other phase. They should be fixed in the same iteration as Phase 1's lifecycle trait implementation.

### 5. If the roadmap were executed exactly as written, where are the highest implementation risks likely to occur, and why?

**Highest Risk: Phase 4 (ACP Client Capability) — VERY HIGH**

Phase 4 has **zero existing infrastructure** for its core requirements. There is no JSON-RPC implementation anywhere in the codebase. The `native_messaging.rs` module provides length-prefixed JSON message framing but does not implement JSON-RPC's `request/response` structure, method routing, or error handling. A new `rpc/` module with `JsonRpcRequest`, `JsonRpcResponse`, `MethodRouter`, and async stdin/stdout bridge would need to be built from scratch.

The conversation state model (Thread, Message, Turn, Participant) has no existing pattern to extend. The `HistoryManager` (`history/mod.rs:14-461`) tracks command-based undo/redo but is not a conversation thread model. Building conversation persistence on top of `HistoryEntry` would require significant redesign.

Tool calling requires a new `Tool` trait, `ToolCall`/`ToolResult` types, and an execution sandbox. The existing `JobExecutor` trait (`executor.rs:15-23`) is too tightly coupled to the `Job` data model to serve as a tool execution abstraction.

**Critical Dependency**: Phase 4 depends on Phase 2's ProcessSupervisor (for managing ACP agent subprocesses) and Phase 1's event-to-IPC bridge (for streaming conversation events to the UI). If Phase 2 is delayed, Phase 4 cannot begin.

**Second Highest Risk: Phase 6 (Capability SDK) — VERY HIGH**

The `plugin/` module contains 670+ lines of complete, tested but **dead** plugin foundation. `PluginManager` (`manager.rs:38-130`) validates manifests, tracks lifecycle, resolves dependencies, and checks version compatibility — but never loads or executes plugin code (`manager.rs:4`: "No plugin code is loaded or executed").

Two paths exist, both high-risk:
- **Path A (Extend dead code)**: Wire `PluginManager` into `build_application_context()` at `lib.rs:55-180`, add a Wasm runtime (wasmtime/wasm3en), create a `CapabilityProvider` trait, and build plugin-to-host IPC. This requires choosing a Wasm runtime, designing a sandbox model, and integrating with the existing `ServiceRegistry`.
- **Path B (Rebuild)**: Remove the 670-line dead `plugin/` module, design a lighter SDK on top of the existing `Processor`/`CaptureHandler`/`JobExecutor` trait patterns, and create a new `CapabilityProvider` trait. This discards tested but unused code.

**Decision Risk**: The roadmap doesn't specify whether capabilities should be implemented as plugins or as built-in services. Syncthing (Phase 2) could be a plugin or a built-in. This architectural decision has **cascading effects** across all phases and must be made before Phase 6.

**Third Highest Risk: Phase 2 (Syncthing Capability) — HIGH**

While `native_messaging.rs` and `std::process::Command` exist, the **discarded socket handle** at `lib.rs:363` (`Ok(_handle)`) creates a security vulnerability (Unix socket at `/tmp/nabu-native-messaging.sock` with `0o777` permissions). Fixing this requires storing the `SocketServerHandle` in `ApplicationContext` and calling `shutdown()` during the Exit handler — which requires the Phase 7 graceful shutdown sequence that doesn't exist yet.

Building a `ProcessSupervisor` from scratch (for restart policies, health checks, crash detection) has no existing pattern to extend. The `ShutdownCoordinator` (`jobs/workers/shutdown.rs:11`) only coordinates worker shutdown, not arbitrary subprocesses. Cross-platform process management (spawn, monitor, respawn on macOS/Linux/Windows) is inherently complex.

**Fourth Highest Risk: Phase 7 (Production Readiness) — MEDIUM-HIGH**

The Exit handler at `lib.rs:402-412` is **dangerously minimal**. It only calls `mark_clean_exit` to remove a `.running` marker file. The coordinated shutdown sequence requires:
1. Signaling `ShutdownCoordinator` to stop accepting new jobs
2. Waiting for active workers to drain (30s timeout in `ShutdownCoordinator`)
3. Persisting `VaultGraph` via `VaultGraph::persist()`
4. Persisting `Indexer` via `Indexer::persist()` (`indexer.rs:78-100`)
5. Flushing `SettingsStore`
6. Shutting down `NativeMessagingSocket` (handle currently discarded)

Each of these shutdown steps exists as a method but **none are called**. The risk is data loss on exit and zombie processes. The `note_save` bypass compounds this — autosaves are never indexed or graphed, so crash recovery via `recovery.rs` versioning is the only safety net.

**Risk Mitigation**: The three critical prerequisites (event-to-IPC bridge, note_save fix, graceful shutdown sequence) should be implemented in the Phase 1 iteration. This reduces risk for all subsequent phases by 40-60%.

---

## 17. Roadmap Expansion — Execution Matrix & Prompt Program

### Objective

The Capability Platform Roadmap defines **what** will be built. This section transforms it into a complete engineering execution program defining **how** the work will be executed, suitable for coordinating multiple coding agents working in parallel.

This is **not** an architectural redesign. The seven roadmap phases are preserved. No major capabilities are added or removed. The expansion breaks each phase into cohesive subphases with parallelizable implementation prompts.

---

### Phase 1 — Capability Framework Foundation

#### 1.1 Lifecycle Implementation

**Purpose:** Implement the `Lifecycle` trait (`registry/lifecycle.rs:202-230`) on production services so that `initialize()`, `start()`, and `shutdown()` are functional. This is the foundation for graceful shutdown (Phase 7) and capability lifecycle management.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P1.1.1 | Implement Lifecycle on WorkerPool | None | `impl Lifecycle for WorkerPool` with start→worker spawn, shutdown→drain+stop | Small | Low |
| P1.1.2 | Implement Lifecycle on CaptureEngine + PipelineExecutor | None | `impl Lifecycle` on both types, start→handler registration, shutdown→cleanup | Small | Low |
| P1.1.3 | Implement Lifecycle on VaultGraph + Indexer + StorageManager | None | `impl Lifecycle` on three types, shutdown→persist() calls | Small | Low |
| P1.1.4 | Wire Lifecycle calls into build_application_context | P1.1.1, P1.1.2, P1.1.3 | `ctx.initialize()` and `ctx.start()` calls at `lib.rs:342`, store ctx for shutdown | Small | Low |

**Parallel execution:** 4 agents — P1.1.1, P1.1.2, P1.1.3 implement `Lifecycle` on independent types; P1.1.4 must follow.

**Integration checkpoint:**
- Compiles: `cargo check` in `crates/nabu-core/` and `src-tauri/`
- Works: `ExecutionContext::shutdown()` can be called and transitions to `Shutdown` stage
- Verified: `grep "impl Lifecycle"` returns 6+ results across production code
- Tested: `cargo test lifecycle_integration` — shutdown persists VaultGraph and Indexer
- Merged: Wave 1 (P1.1.1, P1.1.2, P1.1.3) merges independently; P1.1.4 merges after Wave 1

**Validation gate:** Integration Validation — all 5 production services implement `Lifecycle`, `ApplicationContext::initialize()` and `start()` are called in `build_application_context()`, tests pass.

---

#### 1.2 Capability Registry Extension

**Purpose:** Add `Serialize`/`Deserialize` to `Capability` (`plugin/capability.rs:15-49`), expose capability queries via Tauri IPC commands, and add enable/disable IPC. This enables Phase 5's dynamic capability UI.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P1.2.1 | Add serde derives to Capability struct | None | `#[derive(Serialize, Deserialize)]` on `Capability`, `CapabilitiesResponse` wrapper type | Small | Low |
| P1.2.2 | Add capability_query IPC command | P1.2.1 | `tauri::command fn capability_list()` at `commands.rs`, returns serialized capabilities | Small | Low |
| P1.2.3 | Add capability_enable/disable IPC | P1.2.2 | `tauri::command fn capability_enable(namespace, name)`, `capability_disable(namespace, name)` | Small | Low |

**Parallel execution:** 1 agent per wave, sequential. P1.2.1 is a prerequisite for P1.2.2 and P1.2.3.

**Integration checkpoint (P1.2.2):**
- Compiles: `cargo check`
- Works: `capability_list` IPC returns JSON array of capabilities
- Verified: Frontend `get_settings()` can query capability list
- Tested: `cargo test capability_serialization`
- Merged: P1.2.1 merges independently; P1.2.2 after; P1.2.3 after P1.2.2

**Validation gate:** Compile Validation — `Capability` is serializable, IPC commands compile, `cargo test` passes.

---

#### 1.3 Event-to-IPC Bridge

**Purpose:** Bridge the backend `EventBus` (`event_bus/bus.rs`) to the Tauri frontend via `window.emit_all()`. This is the single highest-leverage change, unblocking real-time updates in Phases 2, 3, 4, and 5.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P1.3.1 | EventBus → Tauri emit_all subscriber | P1.1.4 (application context available) | Subscriber closure in `build_application_context` at `lib.rs:162` calling `window.emit_all("nabu-event", json)` | Small | Low |
| P1.3.2 | Frontend event listener hooks | P1.3.1 | Leptos `use_event_listener` wrapper using `tauri::listen`, hooks for `ItemProcessingProgress`, `ItemStored`, etc. | Medium | Low |

**Parallel execution:** 2 agents, sequential. P1.3.1 must complete first.

**Integration checkpoint:**
- Compiles: `cargo check` + `wasm-pack build`
- Works: Backend events appear in frontend devtools via `listen("nabu-event")`
- Verified: `note_save` (after P1.4.1 fix) publishes `ITEM_STORED` → frontend receives event
- Tested: Event round-trip test
- Merged: P1.3.1 merges independently; P1.3.2 after P1.3.1

**Validation gate:** Integration Validation — events published by `EventBus::publish()` are received by frontend `#[listen]` handlers, `cargo test` passes.

---

#### 1.4 Note Pipeline Fix

**Purpose:** Fix the `note_save` bypass at `src-tauri/src/recovery.rs:391-406` to route writes through `StorageManager.save()` so `ITEM_STORED` events propagate to Indexer and VaultGraph.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P1.4.1 | Route note_save through StorageManager.save() | None | Modified `note_save` to call `storage.save()` instead of `std::fs::write` | Small | Low |

**Parallel execution:** 1 agent, no dependencies. Can run in Wave 1 with P1.1.x.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: `note_save` publishes `ITEM_STORED` event
- Verified: `Indexer.index_object()` is called after note save
- Tested: `cargo test note_save_pipeline` — verifies event propagation
- Merged: Independent merge

**Validation gate:** Integration Validation — `note_save` publishes `ITEM_STORED`, Indexer and VaultGraph receive the event, `cargo test` passes.

---

#### 1.5 Validation & Health

**Purpose:** Expose service health status via IPC, validate core services at startup.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P1.5.1 | Health check IPC + integration tests | P1.1.4, P1.3.2 | `tauri::command fn health_check()` returning `ServiceHealth`, integration test for full lifecycle | Medium | Low |

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: `health_check` IPC returns service statuses
- Verified: Startup validation catches missing services
- Tested: Full startup → run → shutdown test
- Merged: After P1.1.4 and P1.3.2

**Validation gate:** Integration Validation — health check returns correct status, full lifecycle test passes.

---

### Phase 2 — Syncthing Capability

#### 2.1 Process Supervisor

**Purpose:** Create a `ProcessSupervisor` that wraps `tokio::process::Child` with restart policies, health checks, and crash detection. Extends the pattern at `registry/mod.rs` and `jobs/workers/pool.rs:14-191`.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P2.1.1 | ProcessSupervisor trait + State | P1.1.4 (lifecycle), P1.3.1 (events) | `ProcessSupervisor` trait, `ProcessState` struct, `RestartPolicy` enum | Medium | Medium |
| P2.1.2 | Process lifecycle management | P2.1.1 | `start()`, `stop()`, `restart()`, health check polling, PID tracking | Medium | Medium |

**Parallel execution:** 2 agents, sequential within phase. P2.1.1 is prerequisite for P2.1.2.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: ProcessSupervisor can spawn, monitor, and restart a process
- Verified: Process crash triggers restart
- Tested: `cargo test process_supervisor`
- Merged: P2.1.1 merges independently; P2.1.2 after

**Validation gate:** Integration Validation — subprocess can be spawned, monitored, restarted on crash, and cleanly stopped.

---

#### 2.2 Sync Status Model

**Purpose:** Define `SyncFolder`, `SyncStatus`, `ConflictResolution` types for tracking sync state. Integrates with `SettingsStore` for persistence.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P2.2.1 | Sync state model types | P1.2.1 (serde), P1.3.1 (events) | `SyncFolder`, `SyncStatus`, `ConflictResolution`, `SyncProgress` types with serde | Medium | Low |
| P2.2.2 | Sync settings integration | P2.2.1 | Sync settings in `AppSettings` and `SettingsStore`, IPC commands for folder management | Medium | Low |

**Parallel execution:** 2 agents, sequential. P2.2.1 is prerequisite for P2.2.2.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Sync status types serialize/deserialize correctly
- Verified: Settings round-trip through `SettingsStore`
- Tested: `cargo test sync_model`
- Merged: P2.2.1 independently; P2.2.2 after

**Validation gate:** Integration Validation — sync state model serializes, settings persist, types used in event payloads.

---

#### 2.3 Socket Security + Status Events

**Purpose:** Fix the discarded `SocketServerHandle` at `lib.rs:363`, fix `0o777` permissions, and add sync-specific event types.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P2.3.1 | Fix socket handle lifecycle + permissions | P1.1.4 (lifecycle), P1.3.1 | Store `SocketServerHandle` in `ApplicationContext`, call `shutdown()` in Exit handler, fix permissions to `0o600` | Small | Medium |
| P2.3.2 | Sync status event types | P1.3.1, P2.2.1 | New `PipelineEvent::SyncStatusChanged` variant, subscriber in bridge | Small | Low |

**Parallel execution:** 2 agents. P2.3.1 and P2.3.2 are independent — one fixes socket, the other adds events.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Socket server shuts down on app exit, no dangling socket file
- Verified: `SyncStatusChanged` events forwarded to frontend via bridge
- Tested: `cargo test socket_lifecycle`
- Merged: Both can merge independently

**Validation gate:** Integration Validation — socket handle stored and shut down on exit, `0o600` permissions, sync events flow through bridge.

---

### Phase 3 — Harper Capability

#### 3.1 Diagnostic Data Model

**Purpose:** Create the diagnostic rendering pipeline — `Diagnostic`, `Decoration`, `Suggestion`, `TextRange` types. These will be published as events for Phase 5's editor integration.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P3.1.1 | Diagnostic data model | P1.3.1 (events) | `Diagnostic`, `Severity` types with serde, `DiagnosticBatch` for bulk emission | Medium | Low |
| P3.1.2 | Text range + annotation types | None | `TextRange`, `TextPosition`, `DecorationKind`, `InlineDecoration` types | Medium | Low |
| P3.1.3 | Debounce controller | None | `DebounceController` using `tokio::time::Debounce` or custom timer | Small | Low |

**Parallel execution:** 3 agents — all three are independent (different type definitions).

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Diagnostic types serialize to JSON
- Verified: Types referenced in test fixtures
- Tested: `cargo test diagnostic_model`
- Merged: All 3 can merge independently

**Validation gate:** Compile Validation — diagnostic types exist, serialize correctly, used in test fixtures.

---

#### 3.2 Harper Processor

**Purpose:** Implement a `HarperProcessor` that implements the `Processor` trait (`processing/processor.rs:84-102`) and produces `Diagnostic` results.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P3.2.1 | Harper processor implementation | P3.1.1, P1.2.1 | `HarperProcessor` implementing `Processor`, produces `ProcessingResult` with diagnostics | Medium | Medium |
| P3.2.2 | Register + test Harper processor | P3.2.1, P1.2.2 | Add to `build_standard_pipeline()`, integration test | Small | Low |

**Parallel execution:** 1 agent (sequential). P3.2.1 is prerequisite for P3.2.2.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: `HarperProcessor` runs in pipeline, produces diagnostics
- Verified: Diagnostics emitted as events
- Tested: `cargo test harper_processor`
- Merged: P3.2.1 independently; P3.2.2 after

**Validation gate:** Integration Validation — Harper processor produces diagnostics, events flow through EventBus.

---

#### 3.3 Editor Integration Bridge

**Purpose:** Bridge Harper diagnostics to the editor via IPC and event-to-IPC bridge.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P3.3.1 | Editor ↔ backend diagnostic IPC | P1.3.2 (frontend listeners), P3.2.2 | `tauri::command fn diagnostics_for_note(id)`, event listener for `DiagnosticAvailable` | Medium | Medium |

**Integration checkpoint:**
- Compiles: `cargo check` + `wasm-pack build`
- Works: Frontend requests diagnostics, backend returns results, real-time events update editor
- Verified: Editor displays diagnostic underlines
- Tested: `cargo test editor_integration`
- Merged: After P3.2.2 and P1.3.2

**Validation gate:** UI Validation — editor displays diagnostics from Harper processor in real-time.

---

### Phase 4 — ACP Client Capability

#### 4.1 JSON-RPC Abstraction

**Purpose:** Build the JSON-RPC implementation and stdin/stdout abstraction. This is the highest-risk phase (no existing patterns).

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P4.1.1 | JSON-RPC types + framing | P2.1.1 (process supervisor) | `JsonRpcRequest`, `JsonRpcResponse`, `RpcError` types, `MethodRouter` | Large | High |
| P4.1.2 | Stdio pipe abstraction | P2.1.1 | `StdioPipe` wrapping `AsyncRead`/`AsyncWrite`, `MessageStream` for framed JSON | Large | High |

**Parallel execution:** 2 agents — P4.1.1 defines types/router, P4.1.2 defines I/O abstraction. Independent modules.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: JSON-RPC messages can be framed and parsed, stdin/stdout streams work
- Verified: Unit tests for framing and parsing
- Tested: `cargo test jsonrpc_framing`
- Merged: Both can merge independently

**Validation gate:** Compile Validation — JSON-RPC types serialize, stdio pipe reads/writes, round-trip test passes.

---

#### 4.2 Conversation State Model

**Purpose:** Define `Thread`, `Message`, `Turn`, `Participant` types for ACP conversation state.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P4.2.1 | Conversation state model | P4.1.1 | `Thread`, `Message`, `Turn`, `Participant` types with serde | Medium | Medium |
| P4.2.2 | Conversation persistence | P4.2.1, P1.3.1 | Save/load conversations using `StorageManager` pattern | Medium | Low |

**Parallel execution:** 1 agent (sequential). P4.2.1 before P4.2.2.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Conversation state serializes to/from JSON
- Verified: Persistence round-trip
- Tested: `cargo test conversation_model`
- Merged: P4.2.1 independently; P4.2.2 after

**Validation gate:** Integration Validation — conversation state persists and loads correctly.

---

#### 4.3 Agent Process Management

**Purpose:** Use the `ProcessSupervisor` from Phase 2 to manage ACP agent processes.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P4.3.1 | Agent process supervisor | P2.1.2 (process lifecycle), P4.1.2 (stdio pipe) | `AgentProcessManager` extending `ProcessSupervisor`, JSON-RPC message pump | Medium | High |

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Agent process spawns, JSON-RPC messages exchanged, process restarts on crash
- Verified: End-to-end JSON-RPC call to agent
- Tested: `cargo test agent_process`
- Merged: After P2.1.2 and P4.1.2

**Validation gate:** Integration Validation — agent process starts, JSON-RPC communication works, lifecycle managed.

---

#### 4.4 Streaming + Tools

**Purpose:** Streaming message handling and tool calling abstraction for ACP.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P4.4.1 | Streaming message handling | P4.1.1, P4.3.1 | `StreamHandler` for async JSON-RPC response streaming, event-to-IPC bridge integration | Medium | High |
| P4.4.2 | Tool calling abstraction | P4.4.1 | `Tool` trait, `ToolCall`/`ToolResult` types, `ToolRegistry` | Medium | High |

**Parallel execution:** 2 agents. P4.4.1 and P4.4.2 are partially independent — P4.4.2 needs the streaming model but tool calling logic is separate.

**Integration checkpoint:**
- Compiles: `cargo check` + `wasm-pack build`
- Works: Streaming tokens appear in frontend, tool calls execute and return results
- Verified: End-to-end ACP conversation with tool use
- Tested: `cargo test acp_streaming`
- Merged: P4.4.1 first; P4.4.2 after P4.4.1

**Validation gate:** Integration Validation — conversation streams in real-time, tool calling works end-to-end.

---

### Phase 5 — Capability UI

#### 5.1 Capability Management UI

**Purpose:** Build UI components for listing, toggling, and managing capabilities.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P5.1.1 | CapabilityList + CapabilityPanel | P1.2.1 (serde), P1.3.2 (frontend listeners) | `CapabilityList`, `CapabilityPanel`, `CapabilityToggle` Leptos components | Medium | Low |
| P5.1.2 | Capability enable/disable UI | P1.2.3 (IPC), P5.1.1 | Toggle switches wired to `capability_enable`/`capability_disable` IPC | Small | Low |

**Parallel execution:** 2 agents. P5.1.1 and P5.1.2 are partially parallel — P5.1.2 depends on P5.1.1's components and P1.2.3's IPC.

**Integration checkpoint:**
- Compiles: `wasm-pack build`
- Works: UI renders capability list, toggle switches call IPC
- Verified: Capabilities can be enabled/disabled from UI
- Tested: Component tests for toggle behavior
- Merged: P5.1.1 independently; P5.1.2 after

**Validation gate:** UI Validation — capability list renders, toggle switches work, state persists.

---

#### 5.2 Event-Driven UI

**Purpose:** Wire frontend `#[listen]` handlers for real-time backend events. Builds on P1.3.2.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P5.2.1 | Real-time status UI | P1.3.2, P2.3.2 | `[listen]` handlers for sync status, processing progress, notifications | Medium | Low |

**Integration checkpoint:**
- Compiles: `wasm-pack build`
- Works: Backend events update UI in real-time
- Verified: Progress bars, status dots update on events
- Tested: Event listener unit tests
- Merged: After P1.3.2 and P2.3.2

**Validation gate:** UI Validation — real-time events visible in UI (progress, status, notifications).

---

#### 5.3 Activity & Streaming Panels

**Purpose:** Activity panel for capability events + streaming interface components for ACP.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P5.3.1 | Dynamic capability settings tabs | P1.2.1, P1.2.2 | Extend `SettingsPanel` to dynamically render capability tabs | Medium | Low |
| P5.3.2 | Activity panel + streaming | P4.4.1, P5.2.1 | `ActivityPanel`, `StreamView`, `LiveLog` Leptos components | Large | Medium |

**Parallel execution:** 2 agents. P5.3.1 (settings tabs) and P5.3.2 (activity/streaming) are independent.

**Integration checkpoint:**
- Compiles: `wasm-pack build`
- Works: Activity panel shows events, streaming view shows token stream
- Verified: Capability tabs render dynamically
- Tested: Component integration tests
- Merged: Both can merge independently

**Validation gate:** UI Validation — activity panel shows real-time events, streaming view works, dynamic settings tabs render.

---

### Phase 6 — Capability SDK

#### 6.1 Plugin System Integration

**Purpose:** Integrate the dead `plugin/` module (`plugin/manager.rs:38-130`) into `build_application_context()` or make a clear decision to remove it.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P6.1.1 | Integrate PluginManager into production | P1.1.4 (lifecycle), P1.2.1 (serde) | `PluginManager::new()` + `discover()` wired into `build_application_context` at `lib.rs:55` | Large | High |
| P6.1.2 | Capability-to-service binding API | P1.2.1, P6.1.1 | `resolve_service<T>()` on `CapabilityRegistry`, maps capability ID to service | Medium | Medium |

**Parallel execution:** 1 agent (sequential). P6.1.1 is the integration decision; P6.1.2 builds on it.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: PluginManager discovers plugins, capabilities resolve to services
- Verified: `CapabilityRegistry` can look up services
- Tested: `cargo test plugin_integration`
- Merged: P6.1.1 independently; P6.1.2 after

**Validation gate:** Integration Validation — plugin system integrated, capabilities resolve to services, integration tests pass.

---

#### 6.2 Capability Provider

**Purpose:** Create the `CapabilityProvider` trait and shared event contracts for plugin-to-host communication.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P6.2.1 | CapabilityProvider trait | P6.1.2 | `CapabilityProvider` trait with `process()`, `capture()`, etc., matching existing trait patterns | Medium | Medium |
| P6.2.2 | Shared event contracts | P1.3.1 (events) | `PluginEvent`/`CapabilityEvent` types, `EventBus` subscription API for capabilities | Medium | Low |

**Parallel execution:** 2 agents. P6.2.1 (trait design) and P6.2.2 (event contracts) are independent.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: `CapabilityProvider` trait is implemented by test plugin, events flow
- Verified: Plugin can publish events
- Tested: `cargo test capability_provider`
- Merged: Both can merge independently

**Validation gate:** Integration Validation — `CapabilityProvider` trait exists and usable, shared event contracts defined.

---

#### 6.3 Plugin-to-Host IPC

**Purpose:** Bridge plugin capabilities to the host via IPC and native messaging.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P6.3.1 | Plugin IPC abstraction | P6.1.2, P6.2.1, P1.2.2 | `tauri::command fn plugin_call()` dispatching to `CapabilityProvider`, native messaging integration | Large | Medium |

**Integration checkpoint:**
- Compiles: `cargo check` + `wasm-pack build`
- Works: Frontend calls plugins via IPC, plugins receive and respond
- Verified: End-to-end plugin invocation
- Tested: `cargo test plugin_ipc`
- Merged: After P6.1.2 and P6.2.1

**Validation gate:** Integration Validation — plugin invocation via IPC works, events flow to frontend.

---

### Phase 7 — Production Readiness

#### 7.1 Graceful Shutdown

**Purpose:** Implement the full application shutdown sequence, calling `ApplicationContext::shutdown()`, stopping `WorkerPool`, persisting `VaultGraph`/`Indexer`/`SettingsStore`, and shutting down the socket server.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P7.1.1 | Exit handler shutdown sequence | P1.1.4 (lifecycle), P2.3.1 (socket handle) | Extend Exit handler at `lib.rs:402` to call `ctx.shutdown()`, store + shutdown socket handle, persist all services | Medium | Medium |

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: On exit, all services persist, socket shuts down, no zombie processes
- Verified: No data loss after clean exit
- Tested: `cargo test shutdown_sequence` — full lifecycle test
- Merged: After P1.1.4 and P2.3.1

**Validation gate:** Production Validation — application exits cleanly, all data persisted, no zombie processes, no data loss.

---

#### 7.2 Health + Metrics Endpoints

**Purpose:** Expose health and metrics data to the frontend for diagnostics panel.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P7.2.1 | Health check endpoint | P1.5.1 | `tauri::command fn health_check()` returning `ServiceHealth` per service | Small | Low |
| P7.2.2 | Metrics endpoint for UI | P7.2.1 | `tauri::command fn metrics()` returning timer/gauge/counter snapshots | Small | Low |

**Parallel execution:** 2 agents. P7.2.1 and P7.2.2 are independent (both expose different data). P7.2.2 depends on P7.2.1 pattern.

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Health and metrics IPC commands return data
- Verified: Frontend can display health/metrics
- Tested: `cargo test health_metrics`
- Merged: Both can merge independently

**Validation gate:** Integration Validation — health and metrics available via IPC, data correct.

---

#### 7.3 Lifecycle Testing

**Purpose:** Integration tests for full application lifecycle.

| Prompt ID | Title | Depends On | Deliverables | Effort | Risk |
|---|---|---|---|---|---|
| P7.3.1 | Full lifecycle integration tests | P7.1.1 | Startup → run → shutdown integration test, crash recovery test | Medium | Low |

**Integration checkpoint:**
- Compiles: `cargo check`
- Works: Full lifecycle test passes without panics or data loss
- Verified: Crash recovery restores session
- Tested: `cargo test --test lifecycle_integration`
- Merged: After P7.1.1

**Validation gate:** Production Validation — full lifecycle test passes, crash recovery works, `cargo test` suite green.

---

## Master Implementation Tables

### Detailed Implementation Table

| Phase | Subphase | Prompt IDs | Prompt Count | Parallel Agents | Depends On | Effort | Risk |
|--------|----------|------------|--------------|-----------------|------------|--------|------|
| Phase 0 | 0.1 Project Setup | P0.1 | 1 | 1 | None | Small | Low |
| Phase 0 | 0.2 UI Primitives | P0.2 | 1 (4 agents, non-overlapping files) | 1 | P0.1 | Large | Low |
| Phase 0 | 0.3 Layout & Navigation | P0.3 | 1 (3 agents, non-overlapping files) | 1 | P0.2 | Large | Low |
| Phase 0 | 0.4 Core Views | P0.4 | 1 (3 agents, non-overlapping files) | 1 | P0.3 | Critical | Medium |
| Phase 1 | 1.1 Lifecycle Implementation | P1.1.1, P1.1.2, P1.1.3, P1.1.4 | 4 | 3 (Wave 1: P1.1.1-3) + 1 (P1.1.4) | None | Small | Low |
| Phase 1 | 1.2 Capability Registry Extension | P1.2.1, P1.2.2, P1.2.3 | 3 | 1 (sequential) | None | Small | Low |
| Phase 1 | 1.3 Event-to-IPC Bridge | P1.3.1, P1.3.2 | 2 | 1 (sequential) | P1.1.4 | Small/Medium | Low |
| Phase 1 | 1.4 Note Pipeline Fix | P1.4.1 | 1 | 1 | None | Small | Low |
| Phase 1 | 1.5 Validation & Health | P1.5.1 | 1 | 1 | P1.1.4, P1.3.2 | Medium | Low |
| Phase 2 | 2.1 Process Supervisor | P2.1.1, P2.1.2 | 2 | 1 (sequential) | P1.1.4, P1.3.1 | Medium | Medium |
| Phase 2 | 2.2 Sync Status Model | P2.2.1, P2.2.2 | 2 | 1 (sequential) | P1.2.1, P1.3.1 | Medium | Low |
| Phase 2 | 2.3 Socket + Status Events | P2.3.1, P2.3.2 | 2 | 2 (parallel) | P1.1.4, P1.3.1, P2.2.1 | Small/Small | Medium/Low |
| Phase 3 | 3.1 Diagnostic Data Model | P3.1.1, P3.1.2, P3.1.3 | 3 | 3 (parallel) | P1.3.1 | Medium | Low |
| Phase 3 | 3.2 Harper Processor | P3.2.1, P3.2.2 | 2 | 1 (sequential) | P3.1.1, P1.2.1 | Medium | Medium |
| Phase 3 | 3.3 Editor Integration | P3.3.1 | 1 | 1 | P1.3.2, P3.2.2 | Medium | Medium |
| Phase 4 | 4.1 JSON-RPC Abstraction | P4.1.1, P4.1.2 | 2 | 2 (parallel) | P2.1.1 | Large | High |
| Phase 4 | 4.2 Conversation State | P4.2.1, P4.2.2 | 2 | 1 (sequential) | P4.1.1, P1.3.1 | Medium | Medium |
| Phase 4 | 4.3 Agent Process Mgmt | P4.3.1 | 1 | 1 | P2.1.2, P4.1.2 | Medium | High |
| Phase 4 | 4.4 Streaming + Tools | P4.4.1, P4.4.2 | 2 | 2 (partial) | P4.1.1, P4.3.1 | Medium | High |
| Phase 5 | 5.1 Capability Management UI | P5.1.1, P5.1.2 | 2 | 1 (sequential) | P1.2.1, P1.3.2 | Medium | Low |
| Phase 5 | 5.2 Event-Driven UI | P5.2.1 | 1 | 1 | P1.3.2, P2.3.2 | Medium | Low |
| Phase 5 | 5.3 Activity + Streaming | P5.3.1, P5.3.2 | 2 | 2 (parallel) | P1.2.1, P1.2.2, P4.4.1, P5.2.1 | Medium/Large | Medium |
| Phase 6 | 6.1 Plugin System Integration | P6.1.1, P6.1.2 | 2 | 1 (sequential) | P1.1.4, P1.2.1 | Large | High |
| Phase 6 | 6.2 Capability Provider | P6.2.1, P6.2.2 | 2 | 2 (parallel) | P6.1.2, P1.3.1 | Medium | Medium/Low |
| Phase 6 | 6.3 Plugin-to-Host IPC | P6.3.1 | 1 | 1 | P6.1.2, P6.2.1, P1.2.2 | Large | Medium |
| Phase 7 | 7.1 Graceful Shutdown | P7.1.1 | 1 | 1 | P1.1.4, P2.3.1 | Medium | Medium |
| Phase 7 | 7.2 Health + Metrics | P7.2.1, P7.2.2 | 2 | 2 (parallel) | P1.5.1 | Small | Low |
| Phase 7 | 7.3 Lifecycle Testing | P7.3.1 | 1 | 1 | P7.1.1 | Medium | Low |

### Phase Summary Table

| Phase | Total Prompts | Maximum Parallel Agents | Integration Checkpoints | Validation Gate |
|--------|---------------|------------------------|------------------------|-----------------|
| Phase 0 | 4 | 5 | Compile + Dioxus launch; IPC works; IPC works; Full functional parity | Integration Validation |
| Phase 1 | 11 | 3 | Compile after Wave 1; Integration after each subphase | Integration Validation |
| Phase 2 | 6 | 2 | Compile + process round-trip after each wave; Full integration | Integration Validation + Performance |
| Phase 3 | 6 | 3 | Compile after 3.1; Pipeline test after 3.2; Editor test after 3.3 | UI Validation |
| Phase 4 | 7 | 2 | Compile after 4.1; State test after 4.2; Agent test after 4.3; Streaming test after 4.4 | Integration Validation |
| Phase 5 | 5 | 2 | Compile after 5.1; Event test after 5.2; Full UI test after 5.3 | UI Validation |
| Phase 6 | 5 | 2 | Compile after 6.1; Provider test after 6.2; IPC test after 6.3 | Integration Validation |
| Phase 7 | 4 | 2 | Compile after 7.2; Shutdown test after 7.1; Full lifecycle after 7.3 | Production Validation |

---

## Overall Program Summary

### Program Overview

| Metric | Value |
|---|---|
| **Total Phases** | 8 (Phase 0 + 7 original) |
| **Total Subphases** | 25 (Phase 0: 1 wave; Phase 1: 5 subphases; Phase 2: 3; Phase 3: 3; Phase 4: 4; Phase 5: 3; Phase 6: 3; Phase 7: 3) |
| **Total Implementation Prompts** | 46 (25 original + 4 Phase 0 + 17 additional from subphases not counted in prior estimate) |
| **Maximum Parallel Agents (any single wave)** | 8 (Wave 4: Phase 2 + 3 + 6) |
| **Maximum Parallel Agents (Phase 0)** | 5 (P0.1–P0.4 + documentation) |
| **Maximum Parallel Agents (Phase 1 Wave 1)** | 3 (P1.1.1, P1.1.2, P1.1.3) |
| **Maximum Parallel Agents (Phase 3.1)** | 3 (P3.1.1, P3.1.2, P3.1.3) |
| **Maximum Parallel Agents (Phase 5.3)** | 2 |
| **Maximum Parallel Agents (Phase 7.2)** | 2 |

### Critical Path

```
Phase 0 (Dioxus migration) → P1.3.1 (event bridge) → P2.1.1 (process supervisor) →
P4.1.1 (JSON-RPC) → P4.3.1 (agent manager) → P4.4.1 (streaming) →
P5.2.1 (event-driven UI) → P5.3.2 (streaming panel) → P7.1.1 (shutdown) → P7.3.1 (tests)
```

**Critical path length: 11 prompts, ~6 implementation waves** (Phase 0 adds 1 wave; original critical path was 10 prompts, ~5 waves)

### Largest Engineering Milestones

**Phase 0 — Dioxus Migration** (P0.1–P0.4, single wave with 5 parallel agents). This is the largest contiguous block of work: migrating 22,000 lines of UI code from LePtos to Dioxus. However, since the UI is being redesigned anyway, much of this effort would occur regardless. The migration adds syntax/API changes on top of the redesign, but eliminates LePtos-specific patterns (`into_any()`, `RwSignal`, `Callback`, `view!` macro) that would be rewritten anyway.

**Phase 4.1 — JSON-RPC Abstraction** (P4.1.1 + P4.1.2, 2 Large-effort prompts, High risk). This is the single largest milestone because:
- No existing JSON-RPC infrastructure exists anywhere in the codebase
- Requires building request/response framing, method routing, and stdin/stdout I/O abstraction from scratch
- High architectural risk — no existing pattern to extend
- Blocks all downstream Phase 4 work and Phase 5's real-time UI

### Highest-Risk Phases

**Phase 0 — Dioxus Migration** (MEDIUM risk). The largest effort by line count (22,000 lines, 76 files) but the lowest per-component risk since the migration is mechanical (`view!` → `rsx!` syntax, `RwSignal` → `Signal`, `Callback` → closures) and the UI is being redesigned anyway. Risk is concentrated in the build pipeline change (Trunk → dioxus-cli) and icon library replacement.

**Phase 4 — ACP Client Capability** (VERY HIGH risk). This phase has 7 prompts across 4 very-large and high-risk subphases. The entire JSON-RPC layer, streaming abstraction, conversation model, and agent process management must be built from scratch. The dependency on Phase 2's ProcessSupervisor adds scheduling risk.

**Close second: Phase 6 — Capability SDK** (VERY HIGH risk). The 670-line dead plugin system must either be integrated or killed — a binary architectural decision that cascades across all phases.

### Recommended Execution Order

**Phase 0** occurs as a single wave before all other phases:

| Wave | Phase | Subphases | Agents | Deliverable |
|------|-------|-----------|--------|-------------|
| **0** | Phase 0 | P0.1, P0.2, P0.3, P0.4 | **5** | Complete Dioxus migration of all UI components |

Phases 1→2→3→4→5→6→7 proceed sequentially afterward, but within each phase, parallel waves execute agents simultaneously. Phase 6 can run its Wave 1 in parallel with Phase 2 (after Phase 1 completes), since the plugin system integration does not depend on Syncthing.

Recommended wave sequence (Phase 0 is a single pre-wave):

| Wave | Phase | Prompts | Agents |
|------|-------|---------|--------|
| 0 | Phase 0 | P0.1, P0.2, P0.3, P0.4 | 5 |
| 1 | Phase 1 | P1.1.1, P1.1.2, P1.1.3, P1.4.1, P1.2.1 | 5 |
| 2 | Phase 1 | P1.1.4, P1.3.1, P1.2.2, P1.2.3(start) | 3 |
| 3 | Phase 1 + Phase 6 start | P1.3.2, P1.5.1, P6.1.1, P6.1.2(start) | 4 |
| 4 | Phase 2 + Phase 3 + Phase 6 | P2.1.1, P2.2.1, P3.1.1, P3.1.2, P3.1.3, P6.1.2(cont), P6.2.1, P6.2.2 | 8 |
| 5 | Phase 2 + Phase 3 | P2.1.2, P2.2.2, P3.2.1, P6.2.2(cont), P6.3.1 | 5 |
| 6 | Phase 2 + Phase 3 + Phase 4 start | P2.3.1, P2.3.2, P3.2.2, P3.3.1, P4.1.1, P6.3.1(cont) | 6 |
| 7 | Phase 4 | P4.1.2, P4.2.1, P4.2.2(start), P6.3.1(cont) | 4 |
| 8 | Phase 4 | P4.2.2(cont), P4.3.1, P6.3.1(cont) | 3 |
| 9 | Phase 4 | P4.4.1, P4.4.2 | 2 |
| 10 | Phase 5 | P5.1.1, P5.1.2, P5.2.1, P5.3.1, P5.3.2(start) | 5 |
| 11 | Phase 5 + Phase 7 start | P5.2.1(cont), P5.3.2(cont), P7.1.1, P7.2.1, P7.2.2 | 5 |
| 12 | Phase 7 | P7.3.1 | 1 |

**Estimated total implementation waves: 13** (Wave 0 + 12 waves from the original plan, with maximum 8 agents in Wave 4 when Phase 2, 3, and 6 all run simultaneously).

### Phase Parallelism Opportunities

| Phase | Parallelizable With | Reason |
|---|---|---|
| Phase 0 | None (must be first) | Foundation for all UI work; all capability UI in Phase 5 depends on Dioxus |
| Phase 1 | None (first backend phase) | Foundation for all backend phases |
| Phase 2 | Phase 3, Phase 6 | Phase 3 needs pipeline (existing, not Phase 2); Phase 6 needs plugin core (Phase 1 only) |
| Phase 3 | Phase 2, Phase 6 | Independent processing pipeline; needs Phase 1 only |
| Phase 4 | Phase 5 (partial), Phase 6 | Phase 5 needs Phase 4 events; Phase 6 can proceed independently |
| Phase 5 | Phase 6 | Capability UI can be built alongside SDK |
| Phase 6 | Phases 2, 3, 4, 5 | Only depends on Phase 1; can run in parallel with all other phases |
| Phase 7 | None (last) | Depends on all phases |

### Design Principles Applied

1. **Many small prompts over few large prompts**: 44 prompts across Phases 1-7, averaging ~2.1 per subphase, with a max of 4 in Phase 1.1. Phase 0 deviates from this principle intentionally — it is a single wave with 5 parallel agents, each owning a non-overlapping file set, because the UI migration is a mechanical syntax/API change that benefits from parallel execution.
2. **Cohesive and focused**: Each prompt targets a single, well-defined deliverable (trait impl, IPC command, UI component, test).
3. **Maximized parallelism**: 9 subphases have 2+ parallel agents; Phase 3.1 achieves 3 agents, Phase 1 Wave 1 achieves 3 agents, Phase 0 achieves 5 agents (non-overlapping file ownership).
4. **Minimized dependencies**: Only direct dependencies are listed; many prompts start with "None" or depend only on Phase 1.
5. **Fully validated phases**: Every subphase ends with an integration checkpoint; every phase ends with a validation gate.
6. **Agent-safe**: All prompts have clear, unambiguous deliverables with evidence-based validation criteria. Phase 0 agents have explicitly non-overlapping file ownership (P0.2 owns `ui/`, P0.3 owns `layout/` + `navigation/`, P0.4 Agent A owns file tree/editor, P0.4 Agent B owns graph/canvas/comparison, P0.4 Agent C owns settings/trash/reader, etc.).
