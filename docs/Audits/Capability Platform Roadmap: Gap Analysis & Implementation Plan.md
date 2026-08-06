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

## 2. Phase 0 — UI Framework Migration to Dioxus

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

## 3. Implementation Matrix

| Roadmap Phase | Existing Infrastructure | Can Be Extended | Must Be Built | Readiness | Estimated Complexity |
|---|---|---|---|---|---|
| **Phase 0 — UI Framework Migration** | `nabu-core`, `src-tauri` (64 IPC commands, 113 call sites), IPC layer (`ipc.rs`, 5 lines), Tailwind CSS pipeline, theme system (CSS `data-theme`), canvas rendering (`web_sys::CanvasRenderingContext2d`), App Blocks (HTML iframe), clipboard/drag-drop/keyboard (`web_sys`) | `lucide-leptos` → `dioxus-icon` (80+ icon re-exports); Trunk → `cargo-dioxus` build pipeline; `view!` → `rsx!`, `RwSignal` → `Signal`, `Callback` → closures, `into_any()` elimination | Project setup & build pipeline; UI primitives migration; layout/navigation migration; all core view components | PLANNED | LOW (mechanical syntax changes) |
| **Phase 1 — Capability Framework Foundation** | `ServiceRegistry`, `ApplicationContext`, `LifecycleManager`, `CapabilityRegistry`, `SettingsStore`, `build_application_context()`, `validate_core_services()` | Implement `Lifecycle` trait on 5 production services; add `serde` to `Capability`; add IPC commands for capability queries | Capability-to-service binding API; `Capability` serialization for IPC; capability config schema | PARTIALLY READY | LOW |
| **Phase 2 — Syncthing Capability** | `native_messaging.rs`, `native_messaging_socket.rs`, `EventBus`, `PipelineEvent`, `ProgressReporter`, `SettingsStore`, `std::process::Command` (11 call sites), `WorkerPool.start()`, `ShutdownCoordinator` | Extend `EventBus` with event-to-IPC bridge; fix socket handle lifecycle; add capability IPC commands | Process supervisor (restart policy, health checks); folder sync state model; conflict resolution types; persistent process state | NOT READY | HIGH |
| **Phase 3 — Harper Capability** | `Processor` trait, `ProcessingPipeline` (14 processors), `CaptureHandler` (9 implementations), `JobExecutor`, `ExecutorRegistry`, `PipelineExecutor`, `ProgressReporter`, `CancellationToken`, OCR/Whisper/AI settings | Register new Harper processors via `ProcessingPipeline::register()`; use existing `ProgressReporter` for progress | Diagnostic data model (Diagnostic, Decoration, Suggestion types); editor integration bridge; debounce controller; text range model | PARTIALLY READY | MEDIUM |
| **Phase 4 — ACP Client Capability** | `std::process::Command` (11 call sites), `native_messaging.rs` (JSON protocol), `tauri::async_runtime::spawn()`, `EventBus`, `CancellationToken`, `HistoryManager`, `WorkerPool` | Use `WorkerPool` pattern for agent task execution; use `CancellationToken` for cancellation | JSON-RPC implementation; stdin/stdout bidirectional pipe abstraction; streaming message handling; conversation thread state model; tool calling abstraction | NOT READY | VERY HIGH |
| **Phase 5 — Capability UI** | `SettingsPanel` (13 tabs), `ToastProvider`/`use_toast()` (86 usages), `StatusDot`/`TaskIndicator`, `ViewMode` (16 variants), `NavContext`, `AppSettings` (117 fields), Settings IPC (7 commands) | Add capability tab to SettingsPanel; route events via event-to-IPC bridge to toast system | Capability management UI; capability status indicators; activity panel; streaming interface components; diagnostics panel; `Capability` serialization | NOT READY | MEDIUM |
| **Phase 6 — Capability SDK** | `plugin/` module (8 files, ~670 lines): `PluginManager`, `CapabilityRegistry`, `PluginManifest`, `PluginLifecycle`, `PermissionEvaluator`, `FeatureRegistry`, `DependencyGraph`, `Version`; `Processor`/`CaptureHandler`/`JobExecutor` trait patterns; `ServiceRegistry` with categories | Extend `plugin/` module to integrate into production; add `CapabilityProvider` trait on existing patterns | Plugin loader/runtime (Wasm or native); `CapabilityProvider` trait; shared event contracts; plugin-to-host IPC abstraction | PARTIALLY READY | VERY HIGH |
| **Phase 7 — Production Readiness** | `diagnostics/` (init, metrics, spans, performance); `ShutdownCoordinator` (30s drain); `HistoryManager` (undo/redo); `Recovery` (sessions, snapshots); `DurableJobQueue`; `VaultGraph::persist()`, `Indexer::persist()`; `validate_core_services()`; 112 test modules | Extend Exit handler to call `ApplicationContext::shutdown()`; persist VaultGraph/Indexer/StorageManager | Application shutdown sequence; socket server lifecycle handle; metrics endpoint for UI; health check endpoint; lifecycle integration tests | PARTIALLY READY | MEDIUM |

---

## 4. Risk Assessment

| Phase | Risk | Rationale | Key Risk Factors |
|---|---|---|---|
| Phase 0 — UI Migration | MEDIUM | Largest by line count (22k lines) but mechanical; UI is being redesigned anyway | Build pipeline (Trunk→dioxus-cli); icon library swap; no existing Dioxus patterns |
| Phase 1 — Framework Foundation | LOW | All abstractions exist; work is trait impls + serde + 3 IPC commands | Dead `Application` struct confusion vs `ApplicationContext` |
| Phase 2 — Syncthing | HIGH | New `ProcessSupervisor` from scratch; discarded socket handle; cross-platform | `0o777` socket perms; cross-platform process management; no conflict resolution |
| Phase 3 — Harper | MEDIUM | Pipeline is extensible; Harper is a new `Processor`. Editor integration is the gap | No diagnostic data model; no editor↔backend IPC; `ProcessingResult` lacks diagnostics |
| Phase 4 — ACP Client | VERY HIGH | Zero JSON-RPC infrastructure; new stdin/stdout, conversation model, streaming, tools | Entirely new abstraction; async routing; depends on Phase 2 + Phase 1 bridge |
| Phase 5 — Capability UI | MEDIUM | UI foundation solid (SettingsPanel 13 tabs, ToastProvider, ViewMode). Risk is bridge dependency | Hard dependency on event-to-IPC bridge; hardcoded SettingsPanel tabs |
| Phase 6 — Capability SDK | VERY HIGH | 670-line dead `plugin/` module must be integrated or rebuilt; Wasm runtime decision | Architectural commitment (extend vs rebuild); `Capability` is metadata-only |
| Phase 7 — Production Readiness | MEDIUM | Diagnostics/mechanisms exist; fix is mechanical. Socket perms = security vuln | `0o777` socket perms; `note_save` bypass; no health/metrics IPC; no shutdown sequence |

## 5. Codebase Guardrails — Existing Code, Blockers, Dead Code

### Dead Code (Don't Extend)

| Component | Location | Why Dead |
|---|---|---|
| `plugin/` module | `crates/nabu-core/src/plugin/` (8 files, ~670 lines) | `PluginManager` only instantiated in tests; production path bypasses it |
| `Application` struct | `registry/application.rs:514` | `Application::builder()` only called in tests; production uses `ApplicationContext::new()` directly |
| `ApplicationContextBuilder` | `context.rs:428-478` | Complete builder, but `build_application_context()` constructs `ApplicationContext` directly |
| `graph/incremental/` | `graph/incremental/` (~1157 lines) | Incremental graph subsystem, never used in production |
| `native_messaging_socket.rs` handle | `lib.rs:363` (`Ok(_handle)`) | `SocketServerHandle` discarded — socket server is a zombie |

### Phase-Specific Blockers

| Backend Phase | Hard Blocker | Location |
|---|---|---|
| Phase 1 | `Lifecycle` trait defined but **zero implementations** in production | `registry/lifecycle.rs:202-230` |
| Phase 1 | `CapabilityRegistry` registered but **never queried** — no IPC to frontend | `lib.rs:60-61`; no `#[tauri::command]` |
| Phase 2 | `ProcessSupervisor` must be built from scratch (no existing pattern) | None — new module needed |
| Phase 2 | Socket handle discarded + `0o777` perms (security vuln) | `lib.rs:363`; `native_messaging_socket.rs:356` |
| Phase 3 | No diagnostic data model, no editor↔backend IPC bridge | None — must build |
| Phase 4 | No JSON-RPC, no stdin/stdout abstraction, no conversation model, no tool calling | None — entire `rpc/` module needed |
| Phase 5 | Zero `#[listen]` calls in frontend — no event-driven UI possible | `crates/nabu-ui/src/` (grep returns 0) |
| Phase 5 | `Capability` struct not `Serialize`/`Deserialize` | `plugin/capability.rs:15-49` |
| Phase 6 | `PluginManager` never loads/executes plugin code (`manager.rs:4`) | `plugin/manager.rs:3-4` |
| Phase 7 | Exit handler only calls `mark_clean_exit` — no coordinated shutdown | `lib.rs:402-412` |

### Shared Infrastructure — Existing Code Reference

| Infrastructure | Location | Phases That Use It |
|---|---|---|
| `ServiceRegistry` (register, categories, resolve) | `registry/mod.rs:41-217` | 1-6 + Phase 0 |
| `ApplicationContext` (DI, typed accessors) | `registry/context.rs:141-422` | 1, 2, 3, 7 |
| `EventBus<PipelineEvent>` (8 event types) | `event_bus/bus.rs`, `events.rs:8-29` | 1, 2, 3, 4, 5 |
| `SettingsStore` (CRUD, feature toggles) | `src-tauri/src/settings.rs:254-390` | 1, 2, 3, 5, 6 |
| `build_application_context()` (composition root) | `src-tauri/src/lib.rs:55-180` | 1, 2, 3, 7 |
| `ProgressReporter` (callback progress) | `jobs/workers/progress.rs:8-155` | 2, 3, 5 |
| `ShutdownCoordinator` (30s drain) | `jobs/workers/shutdown.rs:11-111` | 2, 4, 7 |
| `WorkerPool` (async task pool) | `jobs/workers/pool.rs:14-191` | 1, 2, 4, 7 |
| `HistoryManager` (undo/redo, persisted) | `history/mod.rs:14` | 5, 7 |
| `Recovery` (sessions, snapshots, crash detection) | `src-tauri/src/recovery.rs` | 7 |
| `ToastProvider` / `use_toast()` | `components/ui/feedback.rs:78-197` | 5, 7 |
| `LifecycleManager` (stage transitions) | `registry/lifecycle.rs:94-196` | 1, 2, 4, 7 |
| `Lifecycle` trait (unimplemented) | `registry/lifecycle.rs:202-230` | Phase 1 |

### Shared Infrastructure — Must Be Built

| Infrastructure | Blocks These Phases | Location to Create |
|---|---|---|
| Event-to-IPC bridge (`EventBus`→`window.emit_all`) | 2, 3, 4, 5 | `src-tauri/src/lib.rs` |
| Application shutdown sequence (Exit handler) | 7 | `src-tauri/src/lib.rs:402-412` |
| `Capability` serialization (`Serialize`/`Deserialize`) | 1, 5, 6 | `plugin/capability.rs` |
| ProcessSupervisor (restart, health, crash) | 2, 4 | `crates/nabu-core/src/process/supervisor.rs` |
| JSON-RPC abstraction (Request/Response/Router) | 4 | `crates/nabu-core/src/rpc/mod.rs` |
| Conversation model (Thread/Message/Turn) | 4, 5 | New types in `rpc/` |
| Capability UI panels (per-capability settings) | 5 | `crates/nabu-ui/src/components/` |

---

## 6. Wave Execution Plan

**13 waves. 46 prompts. Max 8 agents (Wave 4).** Phase 6 parallelizes with Phases 2-5 from Wave 3 onward.

### Wave 0 — UI Migration Foundation

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P0.1: Project setup (`lib.rs`, `ipc.rs`, `app.rs`, `icons.rs`). Dioxus+Tauri, dioxus-cli pipeline, icon swap, root shell, shared contexts | `cargo check`; app launches; `settings_get` IPC works; theme toggle works |
| **Agent 2** | P0.2: 12 UI primitives. `view!`→`rsx!`, `RwSignal`→`Signal`, `Callback`→closures, `into_any()` elimination | `cargo check`; ARIA attrs preserved |
| **Agent 3** | P0.3: Layout (5 files) + Navigation (13 files). Sidebar/inspector/tabs/ribbon, navbar, command palette, shortcuts | Layout switches views; ⌘K opens palette; ⌘Z/⌘Y work |
| **Agent 4** | P0.4 Views A: `file_tree`, `note_editor`, `note_view`, `property_editor` | `cargo check`; editor saves via IPC; file tree drag-drop works |
| **Agent 5** | P0.4 Views B + Docs: Settings (15 tabs), recovery (6 files), inbox, dictation, templates, stats + update AGENTS.md/README.md | `cargo check`; all 15 settings tabs render; AGENTS.md updated with Dioxus commands |

### Wave 1 — Lifecycle Bootstrap

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P1.1.1: `impl Lifecycle for WorkerPool` (start→spawn, shutdown→drain) | `cargo check`; `cargo test lifecycle_integration` |
| **Agent 2** | P1.1.2: `impl Lifecycle` for `CaptureEngine` + `PipelineExecutor` | `cargo check`; both implement init/start/shutdown |
| **Agent 3** | P1.1.3 + P1.4.1: `impl Lifecycle` for VaultGraph + Indexer + StorageManager + fix `note_save` bypass | `cargo check`; `note_save` calls `storage.save()`; `cargo test note_save_pipeline` |
| **(spare)** | P1.2.1: `#[derive(Serialize, Deserialize)]` on `Capability` | `cargo test capability_serialization` |

### Wave 2 — Bridge + IPC

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P1.1.4: Wire `ctx.initialize()` + `ctx.start()` into `build_application_context()` | `cargo check`; `grep "impl Lifecycle"` → 6+ |
| **Agent 2** | P1.3.1 + P1.2.2: EventBus→`emit_all` subscriber + `capability_list` IPC | `listen("nabu-event")` receives events; IPC returns JSON array |
| **Agent 3** | P1.2.3: `capability_enable`/`disable` IPC | `cargo check`; returns `Result<(), String>` |

### Wave 3 — Frontend Events + Phase 6 Start

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P1.3.2: Frontend event listener hooks (`tauri::listen` wrapper) | `note_save` → frontend receives `ITEM_STORED` |
| **Agent 2** | P1.5.1: `health_check` IPC + lifecycle test | Returns `ServiceHealth`; test passes |
| **Agent 3** | P6.1.1: Integrate `PluginManager` into `build_application_context()` | `cargo check`; PluginManager in production path |
| **Agent 4** | P6.1.2 (start): Add `CapabilityProvider` trait | `cargo test plugin_integration` |
| **Agent 5** | P6.2.1: Shared event contracts for plugins | `cargo check`; trait compiles |

### Wave 4 — Core Capabilities (Peak: 8 agents)

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P2.1.1: `ProcessSupervisor` + `ProcessState` + `RestartPolicy` | `cargo check`; subprocess spawn/monitor |
| **Agent 2** | P2.2.1: `SyncFolder`/`SyncStatus`/`ConflictResolution`/`SyncProgress` | `cargo test sync_model` |
| **Agent 3** | P3.1.1: `Diagnostic`/`Decoration`/`Suggestion`/`TextRange` | Types compile; events constructable |
| **Agent 4** | P3.1.2: Diagnostic severity enums + styles | `cargo check` |
| **Agent 5** | P3.1.3: Diagnostic event types + `DiagnosticBatch` | Events publish to EventBus |
| **Agent 6** | P6.1.2 (cont): Plugin integration continued | `cargo test` passes |
| **Agent 7** | P6.2.1 (cont): Event contracts + `CapabilityProvider` | `cargo check` |
| **Agent 8** | P6.2.2: Plugin event contract types | Types serialize; tests pass |

### Wave 5 — Process + Pipeline Implementation

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P2.1.2: Process lifecycle (start/stop/restart/health) | Crash triggers restart |
| **Agent 2** | P2.2.2: Sync settings in AppSettings + SettingsStore + IPC | Settings round-trip |
| **Agent 3** | P3.2.1: Harper Processor implementation | Registers with pipeline |
| **Agent 4** | P6.2.2 (cont): Event contracts finalized | Used in integration tests |
| **Agent 5** | P6.3.1: Plugin-to-host IPC (`plugin_call`) | Frontend invokes plugins; `cargo test plugin_ipc` |

### Wave 6 — Socket Fix + Harper + JSON-RPC Start

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P2.3.1: Fix socket handle lifecycle + `0o600` permissions | Socket shuts down on exit; no zombies |
| **Agent 2** | P2.3.2: `SyncStatusChanged` event + subscriber | Events through IPC bridge |
| **Agent 3** | P3.2.2: Harper ProcessingResult with diagnostics | Emits diagnostics via events |
| **Agent 4** | P3.3.1: Editor integration bridge | `diagnostic_requested` IPC returns diagnostics |
| **Agent 5** | P4.1.1: JSON-RPC core (Request/Response/Router) | `cargo test jsonrpc_core` |
| **Agent 6** | P6.3.1 (cont): Finalize plugin IPC | End-to-end invocation works |

### Wave 7 — JSON-RPC + Conversation Start

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P4.1.2: stdin/stdout bidirectional pipe | Async I/O works; `cargo test io_stream` |
| **Agent 2** | P4.2.1: Thread/Message/Turn conversation types | Types serde-serialize |
| **Agent 3** | P4.2.2 (start): Conversation persistence | Persist across restarts |
| **Agent 4** | P6.3.1 (cont): Complete plugin IPC | Integration test passes |

### Wave 8 — Agent Management

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P4.2.2 (cont): Conversation persistence | Threads persist across restarts |
| **Agent 2** | P4.3.1: Agent process management | Spawn/monitor/restart |

### Wave 9 — Streaming + Tools

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P4.4.1: Streaming (token streaming via EventBus) | Tokens reach frontend |
| **Agent 2** | P4.4.2: Tool calling (`Tool` trait, `ToolCall`/`Result`) | Tools register + execute via IPC |

### Wave 10 — Capability UI

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P5.1.1: Capability management UI (Settings panel) | Listed; enable/disable from UI |
| **Agent 2** | P5.1.2: Capability status indicators | Real-time updates |
| **Agent 3** | P5.2.1: Event-driven UI (EventBus→toast) | Events trigger toasts |
| **Agent 4** | P5.3.1: Activity panel | Events display |
| **Agent 5** | P5.3.2: Streaming interface components | Tokens stream to UI |

### Wave 11 — Shutdown + Metrics Start

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P7.1.1: Exit handler graceful shutdown | Clean exit; no data loss |
| **Agent 2** | P7.2.1: `health_check` IPC | Returns ServiceHealth |
| **Agent 3** | P7.2.2: `metrics` IPC | Returns timer/gauge/counter |
| **Agent 4** | P5.3.2 (cont): Finalize streaming UI | Streaming panel passes validation |
| **Agent 5** | P7.2.2 (cont): Complete metrics endpoint | End-to-end: metrics IPC → frontend |

### Wave 12 — Lifecycle Testing

| Agent | Scope | Validation |
|-------|-------|------------|
| **Agent 1** | P7.3.1: Full lifecycle integration tests | `cargo test lifecycle_full` passes; crash recovery restores session |

---

## 7. Per-Prompt Quick Reference

| Prompt | Deliverable | Validation |
|--------|-------------|------------|
| P0.1 | Dioxus+Tauri project, dioxus-cli, icon swap, app shell, contexts | `cargo check`; app launches; IPC works |
| P0.2 | 12 UI primitives in Dioxus | `cargo check`; ARIA preserved |
| P0.3 | Layout + navigation migrated | View switching; ⌘K/⌘Z/⌘Y work; nav persists |
| P0.4 | 15+ core views migrated | `cargo check`; editor/graph/settings work |
| P1.1.1 | `impl Lifecycle for WorkerPool` | `cargo test lifecycle_integration` |
| P1.1.2 | `impl Lifecycle` CaptureEngine + PipelineExecutor | `cargo check` |
| P1.1.3 | `impl Lifecycle` VaultGraph + Indexer + StorageManager | `cargo check`; persist on shutdown |
| P1.1.4 | Wire init/start into `build_application_context()` | `grep "impl Lifecycle"` → 6+ |
| P1.2.1 | `Serialize`/`Deserialize` on `Capability` | `cargo test capability_serialization` |
| P1.2.2 | `capability_list` IPC | Returns JSON array |
| P1.2.3 | `capability_enable/disable` IPC | Returns `Result<(), String>` |
| P1.3.1 | EventBus→`emit_all` subscriber | Events in frontend devtools |
| P1.3.2 | Frontend event listener hooks | `note_save`→frontend receives |
| P1.4.1 | Route `note_save` through `StorageManager.save()` | `cargo test note_save_pipeline` |
| P1.5.1 | `health_check` IPC + lifecycle test | Returns ServiceHealth; test passes |
| P2.1.1 | ProcessSupervisor + ProcessState + RestartPolicy | `cargo check`; spawn/monitor |
| P2.1.2 | Process lifecycle (start/stop/restart/health) | Crash triggers restart |
| P2.2.1 | SyncFolder/SyncStatus/ConflictResolution/SyncProgress | `cargo test sync_model` |
| P2.2.2 | Sync settings integration | Settings round-trip |
| P2.3.1 | Fix socket handle + `0o600` permissions | Shut down on exit; no zombies |
| P2.3.2 | SyncStatusChanged event + subscriber | Events through IPC bridge |
| P3.1.1 | Diagnostic/Decoration/Suggestion/TextRange types | `cargo check` |
| P3.1.2 | Diagnostic severity enums + styles | `cargo check` |
| P3.1.3 | Diagnostic event types + DiagnosticBatch | Events publish |
| P3.2.1 | Harper Processor implementation | Registers with pipeline |
| P3.2.2 | ProcessingResult with diagnostics | Emits diagnostics via events |
| P3.3.1 | Editor integration bridge | `diagnostic_requested` IPC returns diagnostics |
| P4.1.1 | JSON-RPC core (Request/Response/Router) | `cargo test jsonrpc_core` |
| P4.1.2 | stdin/stdout bidirectional pipe | Async I/O works; `cargo test io_stream` |
| P4.2.1 | Thread/Message/Turn conversation types | Types serde-serialize |
| P4.2.2 | Conversation persistence | Persist across restarts |
| P4.3.1 | Agent process management | Spawn/monitor/restart |
| P4.4.1 | Streaming message handling | Tokens reach frontend |
| P4.4.2 | Tool calling (`Tool` trait, `ToolCall`/`Result`) | Tools register + execute via IPC |
| P5.1.1 | Capability management UI | Listed; enable/disable from UI |
| P5.1.2 | Capability status indicators | Real-time updates |
| P5.2.1 | Event-driven UI (EventBus→toast) | Events trigger toasts |
| P5.3.1 | Activity panel | Events display |
| P5.3.2 | Streaming interface components | Tokens stream to UI |
| P6.1.1 | Integrate PluginManager into production | In `build_application_context()` |
| P6.1.2 | CapabilityProvider trait | `cargo test plugin_integration` |
| P6.2.1 | Shared event contracts for plugins | Types serialize |
| P6.2.2 | Plugin event contracts finalized | Used in integration tests |
| P6.3.1 | Plugin-to-host IPC (`plugin_call`) | Frontend invokes plugins; `cargo test plugin_ipc` |
| P7.1.1 | Exit handler graceful shutdown | Clean exit; no data loss |
| P7.2.1 | `health_check` IPC | Returns ServiceHealth |
| P7.2.2 | `metrics` IPC | Returns timer/gauge/counter |
| P7.3.1 | Full lifecycle integration tests | `cargo test lifecycle_full` |

---
## 8. Master Implementation Tables

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

## 9. Overall Program Summary

**13 waves · 46 prompts · max 8 agents**

**Critical path:** Wave 0 → Wave 2 → Wave 4 → Wave 6 → Wave 7 → Wave 9 → Wave 10 → Wave 11 → Wave 12
(11 prompts: P0.1 → P1.3.1 → P2.1.1 → P4.1.1 → P4.3.1 → P4.4.1 → P5.2.1 → P7.1.1 → P7.3.1)

**Highest-risk work:** Phase 4 JSON-RPC (entirely new abstraction) → Phase 6 Plugin SDK (integrate or rebuild 670-line dead module) → Phase 2 Process Supervisor (new from scratch, security perms).

> All execution details are in **Section 6 (Wave Execution Plan)** and **Section 7 (Per-Prompt Quick Reference)**.
