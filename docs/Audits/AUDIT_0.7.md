# Audit 0.7 — Technical Debt & Dead Architecture

## 1. Executive Summary

This audit performs a semantic reverse-engineering of the Nabu codebase to identify architectural debt, abandoned implementations, dead code paths, duplicate subsystems, migration artifacts, and obsolete abstractions.

### Key Findings

1. **Dead Code**: 10+ unused frontend components (~600 lines), 1 dead module (`crate::tree`), and 6-file disconnected `collections/` module
2. **Dead Architecture**: `Application` struct + `ApplicationBuilder` and full `PluginManager` stack are never instantiated in production; `IncrementalUpdateEngine` / `GraphEventBridge` / `WireIncrementalGraphUpdates` exist only as test-only code
3. **Duplicate Architecture**: Two competing composition roots (`ApplicationBuilder` vs `build_application_context` in `src-tauri/src/lib.rs`); `WireJobEventsToEventBus` is a no-op stub where `PipelineExecutor` publishes events directly
4. **Migration Artifact**: `collections/` module retains Yew-style `Props` type instead of Leptos `#[component]`; `NativeMessagingSocket` is a vestigial Unix socket server for a Safari extension host bridge that may not exist
5. **Temporary Implementation**: `wire_job_events_to_event_bus` is an empty stub with a comment "In production, this would subscribe"; `note_save` bypasses the canonical pipeline (captured in AUDIT_0.5)
6. **Architectural Drift**: Frontend uses Leptos CSR with `Signal` context providers; backend uses a manual `ApplicationContext` built without `ApplicationBuilder`; two state management patterns compete
7. **Trait & Abstraction Quality**: `Lifecycle` trait defined but never implemented by any service; `Queue` trait has only 2 implementations (1 production, 1 test); `JobExecutor` trait has 3 implementations but 2 are test stubs
8. **Service Lifecycle**: Two lifecycle systems (`LifecycleManager` for `Application`, `Lifecycle` trait for services) but neither is used for actual service startup in `lib.rs::run()` — services are initialized manually
9. **Dependency Smells**: `nabu-core` exports both `ApplicationBuilder` (unused) and `build_standard_application_context` (used) — two APIs for the same purpose
10. **Error Handling**: `lib.rs` panics on queue/graph creation failures; no graceful degradation or recovery for critical services
11. **Documentation Drift**: The `Application` architecture document describes `ContentProvider`, `ExportEngine`, `TemplateManager` as part of the composition root — none exist
12. **Capability Platform Risk**: The plugin system exists as a complete but unregistered dead architecture; `PluginManager` is never instantiated anywhere

---

## 2. Technical Debt Overview

| Category | Count | Severity Distribution |
|----------|-------|----------------------|
| Dead Code | 15+ items | High: 6, Medium: 5, Low: 4 |
| Dead Architecture | 4 systems | Critical: 2, High: 1, Medium: 1 |
| Duplicate Architecture | 4 systems | High: 2, Medium: 2 |
| Migration Artifacts | 3 items | High: 1, Medium: 2 |
| Temporary Implementations | 5 items | High: 3, Medium: 2 |
| Architectural Drift | 7 patterns | High: 3, Medium: 4 |
| Trait/Abstraction Issues | 6 traits | High: 2, Medium: 3, Low: 1 |
| Service Lifecycle | 3 patterns | Medium: 3 |
| Dependency Smells | 5 issues | Medium: 4, Low: 1 |
| Error Handling | 3 patterns | Medium: 3 |
| Documentation Drift | 4 items | Medium: 4 |
| Capability Platform Risk | 6 items | High: 3, Medium: 3 |

### Debt Classification Summary

**Dead Code** (unused implementation with no verified execution path)
**Dead Architecture** (subsystem whose architectural purpose no longer exists)
**Duplicate Architecture** (two or more systems solving the same problem)
**Migration Artifact** (temporary compatibility layer or leftover from previous architecture)
**Temporary Implementation** (placeholder intended to be replaced)
**Architectural Drift** (implementation no longer follows the dominant architecture)
**Maintainability Risk** (working code that will likely become difficult to extend)

---

## 3. Dead Code Inventory

### 3.1. Frontend Dead Components (nabu-ui)

#### `components/template_picker.rs` — **TemplatePicker**
- **Category**: Dead Code
- **Severity**: Medium
- **File**: `crates/nabu-ui/src/components/template_picker.rs`
- **Module**: `components::template_picker`
- **Struct/Function**: `TemplatePicker` (component)
- **Evidence**: Component is declared in `mod.rs` and defined in its own file (78 lines), but has zero `use` or render references anywhere in `src/`. The file tree component (`components/file_tree.rs`) renders a template button that calls `template_list` IPC — not `TemplatePicker`.
- **Semantic References**: Only referenced at `components/mod.rs:22` (`pub mod template_picker;`)
- **Current Usage**: Never instantiated, never rendered
- **Impact**: Dead weight in component module; future developers may confuse it with the actual template functionality in `template_editor.rs` which IS used
- **Recommended Action**: Remove

#### `components/pdf_viewer.rs` — **PdfViewer**
- **Category**: Dead Code
- **Severity**: Low
- **File**: `crates/nabu-ui/src/components/pdf_viewer.rs`
- **Module**: `components::pdf_viewer`
- **Struct/Function**: `PdfViewer` (component)
- **Evidence**: 15-line component file declared in `mod.rs:13`. Zero references outside its declaration. No PDF viewing functionality exists in `note_editor.rs` or `reader.rs` — the PDF processors exist in the backend but have no frontend counterpart.
- **Semantic References**: Only at `components/mod.rs:13`
- **Current Usage**: Never instantiated, never rendered
- **Impact**: Dead code, but indicates missing PDF viewing UI capability
- **Recommended Action**: Remove

#### `components/theme_toggle.rs` — **ThemeToggle**
- **Category**: Dead Code
- **Severity**: Low
- **File**: `crates/nabu-ui/src/components/theme_toggle.rs`
- **Module**: `components::theme_toggle`
- **Struct/Function**: `ThemeToggle` (component)
- **Evidence**: 24-line component declared in `mod.rs:23`. Zero references outside declaration. Theme management is handled through `settings_panel.rs` → `settings_set_all` IPC, not through a dedicated toggle component.
- **Semantic References**: Only at `components/mod.rs:23`
- **Current Usage**: Never instantiated, never rendered
- **Impact**: Minor dead code; the settings panel handles theme changes
- **Recommended Action**: Remove

#### `components/relation_editor.rs` — **RelationEditor**
- **Category**: Dead Code
- **Severity**: High
- **File**: `crates/nabu-ui/src/components/relation_editor.rs`
- **Module**: `components::relation_editor`
- **Struct/Function**: `RelationEditor` (276 lines)
- **Evidence**: Largest dead component. Declared in `mod.rs:17`. Zero references outside declaration. The `VaultGraph` exists in the backend and `VaultGraph` events are published, but there is no frontend component that renders or edits graph relationships visually.
- **Semantic References**: Only at `components/mod.rs:17`
- **Current Usage**: Never instantiated, never rendered
- **Impact**: 276 lines of dead code; represents a missing graph relationship editing UI capability
- **Recommended Action**: Remove (or finish wiring into graph_view)

#### `components/sandboxed_html.rs` — **SandboxedHtml**
- **Category**: Dead Code
- **Severity**: Low
- **File**: `crates/nabu-ui/src/components/sandboxed_html.rs`
- **Module**: `components::sandboxed_html`
- **Struct/Function**: `SandboxedHtml` (component)
- **Evidence**: 8-line component declared in `mod.rs:18`. Zero references outside declaration. No sandboxed HTML rendering is used anywhere in the UI.
- **Semantic References**: Only at `components/mod.rs:18`
- **Current Usage**: Never instantiated, never rendered
- **Impact**: Trivial dead code
- **Recommended Action**: Remove

#### `components/sandbox.rs` — **SandboxContainer**
- **Category**: Dead Code
- **Severity**: Low
- **File**: `crates/nabu-ui/src/components/sandbox.rs`
- **Module**: `components::sandbox`
- **Struct/Function**: `SandboxContainer` (component)
- **Evidence**: Module is NOT declared in `components/mod.rs` at all (not in the list of 25 modules). However, the file exists at `components/sandbox.rs`. This makes it completely invisible to the compiler — it is not even compiled.
- **Semantic References**: None (not declared in mod.rs)
- **Current Usage**: Never compiled, never instantiated
- **Impact**: Dead file that doesn't even participate in compilation
- **Recommended Action**: Remove the file

### 3.2. Frontend Dead Module

#### `tree.rs` — **crate::tree module**
- **Category**: Dead Code
- **Severity**: High
- **File**: `crates/nabu-ui/src/tree.rs`
- **Module**: `tree`
- **Struct/Function**: `TreeNode`, `FileTree` (duplicate of `components/file_tree.rs`)
- **Evidence**: `lib.rs:10` exports `pub mod tree;` but grep analysis confirms zero `use crate::tree::` imports anywhere in `src/`. The module is a full duplicate of the active `components/file_tree.rs` functionality, containing its own `FileTree` component, `TreeNode` struct, and recursive rendering logic.
- **Semantic References**: Only at `lib.rs:10` (`pub mod tree;`)
- **Current Usage**: Exported as public API but never imported anywhere
- **Impact**: 600+ lines of duplicate tree implementation dead code
- **Recommended Action**: Remove

### 3.3. Disconnected Collections Module

#### `components/collections/` — Full 6-file module
- **Category**: Dead Code (partially) / Dead Architecture
- **Severity**: High
- **Files**: `mod.rs`, `container.rs`, `view_switcher.rs`, `table_view.rs`, `board_view.rs`, `gallery_view.rs`, `calendar_view.rs`, `shared/mod.rs`, `shared/context.rs`, `shared/types.rs`
- **Structs/Functions**: `CollectionContainer`, `CollectionViewSwitcher`, `CollectionTableView`, `CollectionBoardView`, `CollectionGalleryView`, `CollectionCalendarView`
- **Evidence**: The collections module is declared in `components/mod.rs:2` (`pub mod collections;`) but `CollectionContainer` is never rendered by `app.rs`. The module exports a `fetch_objects` function that calls an IPC command `"fetch_objects"` which does NOT exist in the Tauri backend's `invoke_handler` list (220+ commands registered, `fetch_objects` is not among them). This means the collections UI would panic at runtime with an IPC error if ever rendered.
- **Semantic References**: Only at `components/mod.rs:2` — no render path from `app.rs`
- **Current Usage**: Module is compiled but never mounted. Any invocation would fail IPC calls
- **Impact**: 6 files of code that cannot work; the Yew-style `Props` in `view_switcher.rs` represents a migration artifact (see Section 6.1)
- **Recommended Action**: Remove or complete the integration

---

## 4. Dead Feature Inventory

### 4.1. Event Bus Without Frontend Bridge (Dead Feature)

- **Category**: Dead Feature
- **Severity**: Critical
- **File**: `src-tauri/src/lib.rs:162-177` (ITEM_STORED subscriber); `crates/nabu-ui/src/ipc.rs`
- **Module**: Event system vs IPC layer
- **Struct/Function**: `event_bus.subscribe(ITEM_STORED, ...)` in `lib.rs`; `tauri_invoke` in `ipc.rs`
- **Evidence**: The backend `EventBus` publishes 8 event types (`ITEM_CAPTURED`, `ITEM_PROCESSING_STARTED/PROGRESS/COMPLETED/FAILED`, `ITEM_STORED`, `INDEX_UPDATED`, `GRAPH_UPDATED`, `ITEM_CANCELLED`, `ITEM_RETRIED`). The `ITEM_STORED` event triggers `Indexer.index_object()` and `VaultGraph.add_node()`. However, grep analysis confirms **zero `#[listen]` calls or event subscriptions** in the frontend (`crates/nabu-ui/src/`). The frontend has no mechanism to react to backend events — it only calls IPC commands imperatively and never receives push notifications for pipeline progress, indexing, or graph updates.
- **Semantic References**: `event_bus::kinds::*` published in `lib.rs:162`; no `#[listen]` in `nabu-ui/src/`
- **Current Usage**: Backend events fire for internal subsystem coordination (Storage → Indexer → Graph) but never reach the UI
- **Impact**: Users cannot see background processing progress, indexing status, or real-time graph updates. This directly blocks the Capability Platform's Syncthing integration (see Section 14)
- **Recommended Action**: Finish (implement event-to-IPC bridge from backend → frontend)

### 4.2. note_save Pipeline Bypass (Dead Architecture)

- **Category**: Dead Architecture
- **Severity**: Critical
- **File**: `src-tauri/src/recovery.rs:391-406`
- **Module**: `recovery::note_save`
- **Struct/Function**: `note_save` (tauri::command)
- **Evidence**: The `note_save` command directly writes file content to disk via `std::fs::write()` and calls `snapshot_note()` for versioning. It does NOT route through `CaptureEngine` → `JobQueue` → `ProcessingPipeline` → `StorageManager`. The canonical pipeline exists in `lib.rs` for captured content, but editor autosave bypasses it entirely. The module doc at `recovery.rs:9` acknowledges this: "Autosave feedback — `note_save` persists note content and records a version snapshot."
- **Semantic References**: Called from `note_editor.rs:136`; does not call `StorageManager::save()`, `ProcessingPipeline::run()`, or any pipeline stage
- **Current Usage**: Active (autosave works) but architecturally disconnected from the canonical pipeline
- **Impact**: Autosaved notes skip 14 pipeline stages (content classification, duplicate detection, metadata extraction, etc.). This means notes created via autosave never get indexed, never enter the graph, and never trigger AI processing.
- **Recommended Action**: Finish (route `note_save` through pipeline or at minimum publish `ITEM_STORED` event)

### 4.3. Incremental Graph Update Engine (Dead Architecture)

- **Category**: Dead Architecture
- **Severity**: High
- **File**: `crates/nabu-core/src/graph/incremental/`
- **Module**: `graph::incremental`
- **Struct/Function**: `IncrementalUpdateEngine`, `GraphEventBridge`, `wire_incremental_graph_updates`
- **Evidence**: The incremental graph module is 7 files, 1,157 lines of sophisticated change-tracking logic. `GraphEventBridge::wire()` subscribes to `ITEM_STORED` events and translates them into incremental graph updates. However, grep analysis confirms that `wire_incremental_graph_updates` is **never called** from `src-tauri/src/lib.rs::build_application_context()`. The production code at `lib.rs:162-177` subscribes to `ITEM_STORED` directly and calls `graph.add_node(&object)` — the simple full-graph path, completely bypassing the entire incremental subsystem.
- **Semantic References**: `wire_incremental_graph_updates` defined at `incremental/event_wiring.rs:178`; only called in tests at `incremental_graph_integration.rs`. `lib.rs` uses inline subscription at line 162
- **Current Usage**: Only in `incremental_graph_integration.rs` test; production uses direct `graph.add_node()` calls
- **Impact**: 1,157 lines of dead incremental update logic; the `VaultGraph` grows unboundedly without delta-based updates; future Syncthing integration will require incremental updates for performance
- **Recommended Action**: Remove or integrate into `build_application_context`

### 4.4. Platform Integration Commands (Dead Feature)

- **Category**: Dead Feature
- **Severity**: Medium
- **File**: `src-tauri/src/lib.rs:303-310`
- **Module**: `commands`
- **Functions**: `open_app_in_finder`, `show_macos_notification`, `pin_to_taskbar`, `open_in_explorer`, `open_in_file_manager`, `show_linux_notification`, `install_desktop_entry`
- **Evidence**: 7 platform-specific commands are registered in the Tauri `invoke_handler` but grep confirms **zero calls** from the frontend (`crates/nabu-ui/src/`). These commands exist in `commands.rs` but are never invoked via `tauri_invoke()` from any component.
- **Semantic References**: Registered at `lib.rs:303-310`; zero `tauri_invoke("open_app_in_finder")` or similar calls in `nabu-ui/src/`
- **Current Usage**: Registered as available IPC commands but never called
- **Impact**: 7 backend commands serving no purpose; potential attack surface if IPC is exposed
- **Recommended Action**: Remove from `invoke_handler` or wire into UI

---

## 5. Duplicate Architecture Analysis

### 5.1. Composition Roots

#### **Application vs build_application_context**

- **Category**: Duplicate Architecture
- **Severity**: High
- **File**: `crates/nabu-core/src/registry/application.rs` vs `src-tauri/src/lib.rs:55-180`
- **Module**: `registry::application::Application` vs `lib.rs::build_application_context`
- **Evidence**: Two complete implementations of the same composition root:
  - `Application` / `ApplicationBuilder` (in `nabu-core`) — 291 lines with full lifecycle management, builder pattern, typed service resolution
  - `build_application_context` (in `lib.rs`) — 125 lines that manually constructs and registers the same services (EventBus, StorageManager, ProcessingPipeline, DurableJobQueue, WorkerPool, CaptureEngine, Indexer, VaultGraph, HistoryManager)
  
  Grep confirms `Application::builder()` is **never called** in production code — only in tests (`application_integration.rs`). The Tauri entry point uses `build_application_context` exclusively. This means the `Application` struct, `ApplicationBuilder`, and all associated lifecycle management (`LifecycleManager`, `Lifecycle` trait, `LifecycleStage`) are production dead code.
- **Semantic References**: `Application::builder()` only in tests at `application_integration.rs`
- **Current Usage**: `Application`/`ApplicationBuilder` — test-only; `build_application_context` — production
- **Impact**: 514 lines of `Application` + 270 lines of `ApplicationContextBuilder` + 341 lines of `LifecycleManager` are dead in production; the two implementations drift independently
- **Recommended Action**: Merge — remove `Application`/`ApplicationBuilder` and standardize on `build_application_context`

### 5.2. Job Event Wiring

#### **wire_job_events_to_event_bus vs PipelineExecutor direct publish**

- **Category**: Duplicate Architecture
- **Severity**: Medium
- **File**: `crates/nabu-core/src/pipeline_migration/events.rs` vs `crates/nabu-core/src/pipeline_migration/executor.rs`
- **Module**: `pipeline_migration::events` vs `pipeline_migration::executor`
- **Evidence**: The `wire_job_events_to_event_bus` function at `events.rs:15` is an **empty stub** — the body contains only a comment: "In production, this would subscribe to internal queue events. For now, the PipelineExecutor publishes events directly." Meanwhile, `PipelineExecutor::execute()` at `executor.rs:110-125` directly calls `events::publish_processing_started`, `events::publish_processing_failed`, and `events::publish_processing_completed`. This creates two code paths for the same event publishing responsibility.
- **Semantic References**: `wire_job_events_to_event_bus` defined but never called (grep confirms only definition exists)
- **Current Usage**: Function is a no-op stub; `PipelineExecutor` publishes events directly
- **Impact**: Dead wiring infrastructure; if someone implements `wire_job_events_to_event_bus` later, it could create duplicate event subscriptions
- **Recommended Action**: Remove the stub, consolidate event publishing in `PipelineExecutor`

### 5.3. Settings Storage

#### **SettingsStore vs AppSettings**

- **Category**: Architectural Drift (appears as duplicate)
- **Severity**: Medium
- **File**: `src-tauri/src/settings.rs` vs `crates/nabu-ui/src/components/settings/settings_panel.rs`
- **Module**: `settings::SettingsStore` vs `settings_panel`
- **Evidence**: The Tauri backend uses `SettingsStore` (`settings.rs`) — a file-backed `RwLock<AppSettings>` with 27 fields. The frontend `SettingsPanel` component (`settings_panel.rs:9-112`) mirrors this same 27-field struct on the Rust/WASM side and also holds a `leptos::RwSignal<AppSettings>`. Settings round-trip: frontend state → `settings_set_all` IPC → `SettingsStore` → JSON file → reload on next startup. The frontend maintains its own copy via `settings_get` on startup.
- **Semantic References**: `SettingsStore::get()` called from 15+ Tauri commands; `settings_set_all` called from `settings_panel.rs:132`
- **Current Usage**: Both actively used but maintain two separate copies of the same data
- **Impact**: No data inconsistency currently because the frontend re-fetches on startup and writes-through for changes. However, the dual ownership pattern is fragile — external changes to the settings file are not reflected in the running frontend
- **Recommended Action**: Keep (architecturally sound — frontend cache is necessary for CSR WASM); document the sync strategy

---

## 6. Migration Artifact Analysis

### 6.1. Yew-style Props in Collections Module

- **Category**: Migration Artifact
- **Severity**: High
- **File**: `crates/nabu-ui/src/components/collections/view_switcher.rs`
- **Module**: `components::collections::view_switcher`
- **Struct/Function**: `Props` struct, `ViewSwitcher` function component
- **Evidence**: The `collections/view_switcher.rs:4-8` defines `Props` as a struct with fields and implements a function component that takes `Props` by value — this is the Yew 0.20+ function component pattern. The rest of the UI uses Leptos `#[component]` with `Signal` parameters (e.g., `app.rs`, `file_tree.rs`, `note_editor.rs` all use Leptos's `#[component]` macro with `leptos::Signal` props). The `collections/shared/context.rs` also uses `use leptos::Signal` but `view_switcher.rs` uses `Props` struct pattern. This indicates an incomplete migration from Yew to Leptos — the collections module was started in the old framework but never converted.
- **Semantic References**: `Props` at `view_switcher.rs:4`; Leptos `#[component]` at `app.rs:3`, `file_tree.rs:1`, `note_editor.rs:1`
- **Current Usage**: Module is dead (see Section 3.3); if it were wired in, it would fail to compile due to framework mismatch
- **Impact**: Migration artifact that will cause compile errors if the module is activated without conversion; blocks the Collections feature rollout
- **Recommended Action**: Remove or convert to Leptos `#[component]` pattern

### 6.2. Native Messaging Socket (Vestigial Safari Extension Host)

- **Category**: Migration Artifact
- **Severity**: Medium
- **File**: `src-tauri/src/native_messaging_socket.rs`
- **Module**: `native_messaging_socket`
- **Struct/Function**: `SocketServerState`, `start_socket_server`, `SocketServerHandle`, `Message`, `validate_capture_message`, `message_to_capture_request`, `handle_connection`
- **Evidence**: The module implements a Unix socket server that listens at `/tmp/nabu-native-messaging.sock` for connections from a native messaging host binary. It validates capture messages and routes them through the canonical `CaptureEngine::ingest()`. The module doc at lines 1-9 states it "matches the shared `native_messaging::Message` type used by the Safari extension host." However, grep analysis shows **zero references** to `SocketServerState` or `start_socket_server` outside `lib.rs:357-369` where it is spawned at startup. There is no corresponding Safari extension in the repository — `native_messaging.rs` is a separate message type module, and `native_messaging_host.rs` exists in `src-tauri/src/bin/` but its relationship to this socket is undocumented.
- **Semantic References**: Only at `lib.rs:357-369` (started) and `native_messaging.rs` (shared Message type)
- **Current Usage**: Socket server starts on every application launch but no client is known to connect
- **Impact**: Potential security exposure (open Unix socket with 0o777 permissions at `/tmp/`); maintenance burden if the Safari extension is deprecated
- **Recommended Action**: Audit — determine if the Safari extension host is still a supported capture source; if not, remove the socket server

### 6.3. Orphaned Native Messaging Host Binary

- **Category**: Migration Artifact
- **Severity**: Low
- **File**: `src-tauri/src/bin/native_messaging_host.rs`
- **Module**: `bin::native_messaging_host`
- **Evidence**: A standalone binary exists that implements the native messaging host protocol (stdin/stdout JSON). It references `native_messaging::Message` and `CaptureEngine`. However, there is no `.desktop` file, no Safari extension manifest, and no Tauri configuration that registers it as a native messaging host. Grep for `native_messaging_host` in configuration files returns nothing.
- **Semantic References**: Only at `src-tauri/src/bin/native_messaging_host.rs`; no configuration references
- **Current Usage**: Compiled but unreachable by any known client
- **Impact**: Build artifact with no clear integration path
- **Recommended Action**: Determine if this is for the Safari extension; if the socket approach is the new path, remove the binary

---

## 7. Temporary Implementation Inventory

### 7.1. Empty Event Wiring Stub

- **Category**: Temporary Implementation
- **Severity**: High
- **File**: `crates/nabu-core/src/pipeline_migration/events.rs:15-22`
- **Module**: `pipeline_migration::events`
- **Function**: `wire_job_events_to_event_bus`
- **Evidence**: The function signature accepts `&EventBus<PipelineEvent>` but the body is entirely empty with only comments:
  ```rust
  pub fn wire_job_events_to_event_bus(_event_bus: &EventBus<PipelineEvent>) {
      // Subscribe to queue lifecycle events via the event bus.
      // This function sets up subscriptions that bridge job state transitions
      // to typed pipeline events.
      //
      // In production, this would subscribe to internal queue events.
      // For now, the PipelineExecutor publishes events directly.
  }
  ```
  The parameter is even named `_event_bus` (underscore prefix), explicitly signaling it is unused.
- **Semantic References**: Never called anywhere (grep confirms only the definition exists)
- **Current Usage**: Dead — never invoked
- **Reachability**: Not reachable
- **Impact**: Blocks clean architectural separation of event wiring from job execution; if someone relies on this function, event subscriptions will silently not happen
- **Recommended Action**: Remove or implement — if kept as placeholder for future work, add `#[deprecated]` attribute

### 7.2. note_save Bypasses Pipeline

- **Category**: Temporary Implementation (confirmed in AUDIT_0.5)
- **Severity**: Critical
- **File**: `src-tauri/src/recovery.rs:391-406`
- **Module**: `recovery::note_save`
- **Function**: `note_save`
- **Evidence**: (Detailed in AUDIT_0.5 Section 4.1) The `note_save` command directly writes to disk without routing through the pipeline. The comment at line 389 states: "The autosave path — never pushes a history entry (typing would flood the undo stack)." This is a deliberate temporary bypass documented in the code.
- **Semantic References**: Called from `note_editor.rs:136`; documented at `recovery.rs:9`
- **Current Usage**: Active — all note edits route through this path
- **Reachability**: Users encounter this on every autosave (every few seconds during editing)
- **Blocked Feature**: Full pipeline processing (indexing, metadata extraction, AI enrichment) for editor-edited notes
- **Recommended Action**: Finish — publish `ITEM_STORED` event after write, or route through PipelineExecutor

### 7.3. WorkerChannel Stub Implementation

- **Category**: Temporary Implementation
- **Severity**: Medium
- **File**: `crates/nabu-core/src/jobs/worker_channel.rs:30-31, 42-56, 66-68`
- **Module**: `jobs::worker_channel`
- **Functions**: `WorkerChannel::new`, `create_receiver`, `recv_result`
- **Evidence**: 
  - `WorkerChannel::new()` (line 30-31) creates channels but immediately discards the receivers: `let (_tx, _job_rx): (_, mpsc::UnboundedReceiver<Job>) = mpsc::unbounded_channel();` — the `job_rx` is never stored, meaning jobs sent through `send_job()` are silently dropped.
  - `create_receiver()` (line 42-56) spawns a tokio task that does nothing: `tokio::spawn(async move { std::mem::drop(rx); });` with comment "This is simplified — in production this would fan-out to multiple receivers"
  - `recv_result()` (line 66-68) always returns `None` — the result channel receiver is never stored
  - `WorkerReceiver.rx` field is typed as `mpsc::UnboundedSender<Job>` (line 79) — a sender named `rx`, indicating copy-paste confusion
  
  Meanwhile, the production `Worker` (`worker.rs:71-89`) polls the queue with `self.queue.dequeue()` directly — it never uses `WorkerChannel`. The `DurableJobQueue` stores a `WorkerChannel` field but never reads from it.
- **Semantic References**: `WorkerChannel::new()` called at `queue.rs:87`; channel methods never used by `worker.rs`
- **Current Usage**: WorkerChannel is constructed but dead — workers use direct queue polling instead
- **Reachability**: Workers are the production path; WorkerChannel is bypassed
- **Impact**: 108 lines of dead communication infrastructure; workers poll the queue with `tokio::time::sleep(100ms)` instead of being notified — this is a performance and latency issue
- **Recommended Action**: Finish (implement channel-based notification) or Remove (and use polling pattern consistently)

### 7.4. VaultGraph Missing Incremental Engine Wiring

- **Category**: Temporary Implementation
- **Severity**: Low
- **File**: `src-tauri/src/lib.rs:159-177`
- **Module**: `lib.rs::build_application_context`
- **Evidence**: The ITEM_STORED subscriber at `lib.rs:162` directly calls `graph.add_node(&object)` on the `VaultGraph` — a full insert. The `IncrementalUpdateEngine` (`incremental/engine.rs`) exists with `node_added`, `node_modified`, `node_removed`, transaction support, but is never wired. The comment at line 155 states "StorageManager.save() publishes ITEM_STORED after persistence. These subscribers are the ONLY consumers" — but the subscriber uses the simplest possible graph mutation, not the incremental engine.
- **Semantic References**: `VaultGraph::add_node` at `lib.rs:171`; `IncrementalUpdateEngine` only in `incremental/` module and tests
- **Current Usage**: Active (full-graph path) but not using available incremental machinery
- **Reachability**: Users encounter this on every captured item
- **Blocked Feature**: Performance at scale — graph rebuilds on every ITEM_STORED
- **Recommended Action**: Finish (wire IncrementalUpdateEngine) or document the trade-off

### 7.5. VaultGraph Version Recovery Panic

- **Category**: Temporary Implementation
- **Severity**: Medium
- **File**: `crates/nabu-core/src/graph/recovery.rs:270`
- **Function**: `panic!("Expected Recovered, got {:?}", other)`
- **Evidence**: 
  ```rust
  other => panic!("Expected Recovered, got {:?}", other),
  ```
  This panic occurs in a match arm for a `GraphVersion` recovery path. If an unexpected version state is encountered during graph loading/recovery, the application panics. This is reachable at application startup when loading a vault graph from disk.
- **Semantic References**: In `graph/recovery.rs:270`; called during graph load sequence
- **Current Usage**: Reachable — if a corrupted or unrecognized version state is encountered, the app crashes
- **Reachability**: Users can encounter this during vault loading with corrupted data
- **Blocked Feature**: Robust error handling for graph corruption
- **Recommended Action**: Replace panic with error propagation and recovery

---

## 8. Architectural Drift Assessment

### 8.1. Two Service Registration Patterns

#### Manual Registration in `lib.rs` vs `Application::register()`

- **Category**: Architectural Drift
- **Severity**: High
- **File**: `src-tauri/src/lib.rs:55-180` vs `crates/nabu-core/src/registry/application.rs:336-384`
- **Evidence**: The production `build_application_context()` manually constructs services and calls `ctx.register("key", service)` at each step (e.g., `lib.rs:77` `ctx.register("storage_manager", storage.clone())`). The `Application` struct has a `register()` method on `ApplicationContext` but this is only used in tests. The production code does NOT use `ApplicationBuilder`'s `with_*()` methods — it bypasses the entire builder pattern and constructs services directly.
- **Semantic References**: `ctx.register()` used in `lib.rs`; `ApplicationBuilder::with_*()` never called in production
- **Current Usage**: Production uses manual construction; `ApplicationBuilder` is test-only
- **Impact**: Two competing patterns for service registration that will drift; the `ApplicationBuilder` pattern is cleaner but unused

### 8.2. Context Provider Patterns

#### Signal-Captured Copy vs expect_context()

- **Category**: Architectural Drift (acceptable)
- **Severity**: Low
- **File**: `crates/nabu-ui/src/components/` (various)
- **Evidence**: All 7 context providers in the frontend use a consistent pattern documented at `smart_folders.rs:15-19` and `save_status.rs:7-10`: contexts are `Copy` types captured at render time and threaded into `spawn_local` async blocks. This avoids `expect_context()` panics by explicitly passing the context through closure captures. Grep confirms no `expect_context()` calls exist in the codebase.
- **Semantic References**: Zero `expect_context` calls; all context provides use explicit `provide_*` functions
- **Current Usage**: Consistent pattern across all components
- **Impact**: None — this is an intentional, consistent architectural decision. The pattern is well-documented with explanatory comments in `smart_folders.rs` and `save_status.rs`.
- **Recommended Action**: Keep

### 8.3. Inconsistent IPC Invocation Patterns

- **Category**: Architectural Drift
- **Severity**: Medium
- **File**: Various components in `crates/nabu-ui/src/`
- **Evidence**: Three different patterns for IPC invocation exist:
  1. **Direct invoke with `JsValue` args**: `note_editor.rs:136` — `tauri_invoke("note_save", args).await` where `args` is built with `serde_wasm_bindgen::to_value()`
  2. **Direct invoke with JSON**: `template_editor.rs:54` — uses `serde_json::json!()` macros for args
  3. **Invoke with empty args**: `settings_panel.rs:119` — `tauri_invoke("settings_get", empty_args)` where `empty_args = JsValue::NULL`
  
  The `ipc.rs:9` wrapper function does no type checking — it accepts any `&str` command and any `JsValue` args, returning `JsValue`. There is no typed IPC layer. Each component constructs its own argument format independently.
- **Semantic References**: `tauri_invoke` defined at `ipc.rs:9`; used 50+ times across 15+ components
- **Current Usage**: All three patterns actively used
- **Impact**: No compile-time safety for IPC commands — typos in command names or wrong argument shapes only surface at runtime; making a typed IPC layer would be a breaking change across 50+ call sites
- **Recommended Action**: Keep (too many call sites to refactor safely); document the patterns

### 8.4. Settings Mutation Strategy

- **Category**: Architectural Drift
- **Severity**: Medium
- **File**: `settings_panel.rs:119-132` vs `state.rs:256-262` vs `lib.rs:95-99`
- **Evidence**: Settings are mutated through three different mechanisms:
  1. **Bulk save**: `settings_panel.rs:132` — calls `settings_set_all` with the entire `AppSettings` struct
  2. **Individual key**: `state.rs:259` — calls `settings_set` with a single key/value pair (e.g., `view_mode`)
  3. **Startup load**: `lib.rs:68` — calls `get_settings` to load the full settings on app start
  
  The `SettingsStore` backend (`settings.rs`) implements both granular `set(key, value)` and bulk `set_all(AppSettings)` operations, but the granularity of mutation is determined by the caller, not enforced by the API.
- **Semantic References**: `settings_set` at `state.rs:259`, `settings_set_all` at `settings_panel.rs:132`, `get_settings` at `lib.rs:68`
- **Current Usage**: All three patterns coexist
- **Impact**: Inconsistent mutation granularity; granular `set` calls could leave the settings in a half-applied state if one fails
- **Recommended Action**: Keep (acceptable for current usage scale); standardize on `settings_set` for granular changes

---

## 9. Trait & Abstraction Review

### 9.1. Queue Trait (Single Non-Test Implementation)

- **Category**: Maintainability Risk
- **Severity**: Medium
- **File**: `crates/nabu-core/src/jobs/queue.rs:16`
- **Trait**: `Queue`
- **Evidence**: The `Queue` trait is defined at `queue.rs:16` with 13 methods (`enqueue`, `dequeue`, `peek`, `cancel`, `retry`, `reschedule`, `remove`, `count`, `count_by_status`, `load_job`, `list_jobs`, `mark_running`, `mark_completed`, `mark_failed`, `report_progress`). Only one production implementation exists: `DurableJobQueue` (`queue.rs:173`). One test implementation exists (`MockQueue`) inside `#[cfg(test)]`. The trait abstraction allows for in-memory, SQL-backed, or distributed queue implementations, but only the file-backed `DurableJobQueue` is ever used.
- **Semantic References**: `impl Queue for DurableJobQueue` at `queue.rs:173`; no other non-test implementations
- **Current Usage**: Single implementation
- **Impact**: The trait adds indirection without polymorphism benefits. However, it provides a clean interface for future queue implementations (important for testing and potential cloud sync).
- **Recommended Action**: Keep (strategic abstraction for future extensibility)

### 9.2. JobExecutor Trait (Multiple Implementations, Partially Redundant)

- **Category**: Architectural Drift
- **Severity**: Medium
- **File**: `crates/nabu-core/src/jobs/workers/executor.rs:15`
- **Trait**: `JobExecutor`
- **Evidence**: The `JobExecutor` trait (line 15) has exactly 3 implementations:
  1. `PipelineExecutor` (executor.rs:111) — production: runs `ProcessingPipeline` → `StorageManager`
  2. `NoopExecutor` (executor.rs:76) — test stub: returns empty job
  3. `FallbackExecutor` (executor.rs:95) — test stub: returns error "No executor registered"
  
  The `ExecutorRegistry` dispatches by processor name. In `lib.rs:104-116`, a single `PipelineExecutor` instance is registered under all 4 processor names (`ocr_processor`, `whisper_processor`, `pdf_text_extraction_processor`, `metadata_extraction_processor`). The `FallbackExecutor` exists for unregistered processor names but is never registered in the `ExecutorRegistry` — it's only constructed in its own `impl` block.
- **Semantic References**: `NoopExecutor` and `FallbackExecutor` — never registered in any `ExecutorRegistry` (grep confirms)
- **Current Usage**: Only `PipelineExecutor` is registered in `lib.rs:115`
- **Impact**: Two dead executor implementations; `FallbackExecutor` could serve as a safety net but is never wired in
- **Recommended Action**: Remove `NoopExecutor` (test-only); wire `FallbackExecutor` into the registry or remove it

### 9.3. Lifecycle Trait (Dead Abstraction)

- **Category**: Dead Code (trait)
- **Severity**: High
- **File**: `crates/nabu-core/src/registry/lifecycle.rs:202`
- **Trait**: `Lifecycle`
- **Evidence**: The `Lifecycle` trait at `lifecycle.rs:202` defines `name()`, `initialize()`, `start()`, `shutdown()` methods. Grep confirms **zero implementations** of this trait outside its own `#[cfg(test)]` test (`lifecycle_trait_default_impls` test only). The `LifecycleManager` struct (which manages `LifecycleStage`) is used by `ApplicationContext` but manages stages via atomic counters, not via the `Lifecycle` trait. The trait exists as a conceptual contract but no service implements it.
- **Semantic References**: Trait defined at `lifecycle.rs:202`; only test implementation at `lifecycle.rs:313-327`
- **Current Usage**: Never implemented in production
- **Impact**: Dead abstraction that suggests services should implement lifecycle hooks but doesn't enforce it
- **Recommended Action**: Remove (or implement on services that need it)

### 9.4. PerformanceMonitor (Singleton Pattern with Global)

- **Category**: Maintainability Risk
- **Severity**: Medium
- **File**: `crates/nabu-core/src/diagnostics/performance.rs:50`
- **Struct**: `PerformanceMonitor`
- **Evidence**: `PerformanceMonitor` has a `static GLOBAL_MONITOR: OnceLock<Arc<PerformanceMonitor>>` at `performance.rs:50` with a `global_monitor()` function (`performance.rs:61`). This creates a global singleton that violates the dependency injection pattern used everywhere else. However, grep shows that `global_monitor()` is called only within `diagnostics/` module itself (for span/metrics collection), and the `PerformanceMonitor` is also registered in the `ServiceRegistry` via `ApplicationBuilder` (but `ApplicationBuilder` is dead code).
- **Semantic References**: `global_monitor()` at `performance.rs:61`; called only within `diagnostics/` module
- **Current Usage**: Global singleton for diagnostics instrumentation; not used for actual monitoring in production
- **Impact**: Two access patterns (global singleton vs registry); the global pattern contradicts the DI architecture
- **Recommended Action**: Keep (diagnostics instrumentation needs low-overhead global access); remove `ApplicationBuilder`'s performance_monitor wiring since it's dead

---

## 10. Service Lifecycle Consistency

### 10.1. WorkerPool Startup (Inconsistent with VaultGraph)

- **Category**: Architectural Drift
- **Severity**: Medium
- **File**: `src-tauri/src/lib.rs:342-352`
- **Evidence**: The `WorkerPool` is started with `tauri::async_runtime::spawn(async move { pool.start().await; })` at `lib.rs:349-351`. The `VaultGraph` is initialized with `VaultGraph::with_persistence(...)` at `lib.rs:143-147` but `start()` is never called on it — the `VaultGraph` constructor performs eager loading from disk and the graph is used synchronously. The `WorkerPool` is the only service with an explicit async `start()` call; all others (StorageManager, Indexer, CaptureEngine, etc.) are initialized synchronously in the factory function.
- **Semantic References**: `pool.start().await` at `lib.rs:350`; no `.start()` calls for other services
- **Current Usage**: Only WorkerPool has explicit startup
- **Impact**: Inconsistent lifecycle — some services need async startup but only WorkerPool does it; the native messaging socket server (line 360-369) also gets its own `spawn`

### 10.2. Missing Shutdown Handler

- **Category**: Maintainability Risk
- **Severity**: Medium
- **File**: `src-tauri/src/lib.rs:402-412`
- **Evidence**: The Tauri `run` event handler at `lib.rs:402-412` only handles `RunEvent::Exit` — it calls `mark_clean_exit()` to remove the running marker file. It does NOT call any graceful shutdown on services: `WorkerPool::shutdown()`, `DurableJobQueue::shutdown()`, `NativeMessagingSocket::shutdown()`, `VaultGraph::persist()`, or `Indexer::persist()`. All services are dropped implicitly when the application exits.
- **Semantic References**: `RunEvent::Exit` handler at `lib.rs:405-411`; no `shutdown()` calls for any service
- **Current Usage**: Services are not gracefully shut down
- **Impact**: Potential data loss on abrupt exit; worker jobs may be interrupted; graph/index may not be persisted
- **Recommended Action**: Finish (add graceful shutdown for all services)

### 10.3. Orphan Service: Native Messaging Socket

- **Category**: Dead Architecture (service lifecycle)
- **Severity**: Medium
- **File**: `src-tauri/src/native_messaging_socket.rs:200`
- **Evidence**: The `start_socket_server()` function spawns a tokio task at `lib.rs:360-369` that runs indefinitely until a `Notify` signal is received. The `SocketServerHandle` has a `shutdown()` method but it is never stored — the handle is created with `let _handle = start_socket_server(...)` and immediately discarded. The server cannot be shut down because the handle is dropped. The tokio task is detached.
- **Semantic References**: `let _handle = start_socket_server(socket_state)` at `lib.rs:362`; `SocketServerHandle::shutdown` — never called
- **Current Usage**: Server runs for application lifetime but cannot be cleanly stopped
- **Impact**: Resource leak on shutdown; socket file may persist at `/tmp/nabu-native-messaging.sock`
- **Recommended Action**: Store the handle and call `shutdown()` on exit

---

## 11. Dependency & Coupling Analysis

### 11.1. Frontend Direct Access to nabu-core

- **Category**: Dependency Smell
- **Severity**: Medium
- **File**: `crates/nabu-ui/src/lib.rs:57`
- **Evidence**: `lib.rs:57` imports `nabu_core` types directly for theme loading: `crate::ipc::tauri_invoke("settings_get", theme_args).await`. This is actually IPC, not direct dependency. However, the `components/collections/view_switcher.rs:4` imports `super::Props` which references `leptos::Signal` — this is framework-internal. The real problem is: `lib.rs` imports `crate::ui::Icon` which is defined in `components/ui/icons.rs` — this creates a dependency from the crate root to a deeply nested UI component.
- **Semantic References**: `lib.rs:57` uses `crate::ipc::tauri_invoke`; `lib.rs:55` uses `crate::ui::Icon`
- **Current Usage**: Crate-root module depends on UI component module
- **Impact**: Tight coupling between app bootstrap and component hierarchy; makes refactoring component structure difficult
- **Recommended Action**: Keep (common pattern in Leptos apps); consider re-exporting Icon from crate root

### 11.2. Application Struct Not Used in Production

- **Category**: Dependency Smell
- **Severity**: Medium
- **File**: `crates/nabu-core/src/registry/application.rs` vs `src-tauri/src/lib.rs`
- **Evidence**: The `Application` struct imports `CaptureEngine`, `ProcessingPipeline`, `PerformanceMonitor`, `ServiceRegistry`, `LifecycleManager`, `LifecycleStage` — pulling in a large dependency surface. But `Application` is only used in tests. The production `build_application_context` function constructs services directly without the `Application` wrapper, meaning the `Application` struct and its imports are dead weight in the dependency graph.
- **Semantic References**: `Application` only at `registry/application.rs`; `build_application_context` at `lib.rs:55`
- **Current Usage**: `Application` — dead; `build_application_context` — production
- **Impact**: 3 modules (`application.rs`, `lifecycle.rs`, part of `context.rs`) are compiled into the production binary but never used
- **Recommended Action**: Remove `Application`/`ApplicationBuilder` from production builds (feature-gate or remove)

### 11.3. Factory Functions vs Builder Pattern

- **Category**: Dependency Smell
- **Severity**: Medium
- **File**: `crates/nabu-core/src/capture/engine.rs:155` vs `crates/nabu-core/src/processing/pipeline.rs:235`
- **Evidence**: Two different factory patterns exist:
  - `build_default_capture_engine()` at `engine.rs:155` — returns a configured `CaptureEngine`
  - `build_standard_pipeline()` at `pipeline.rs:235` — returns a configured `ProcessingPipeline`
  
  Both are used in production. However, `build_standard_application_context()` at `context.rs:501` also exists as a partial factory that only builds EventBus + Registry + Capabilities — it doesn't build the full app context. So there are THREE factory functions with overlapping responsibility.
- **Semantic References**: `build_default_capture_engine` at `lib.rs:124`; `build_standard_pipeline` at `lib.rs:80`; `build_standard_application_context` only in tests at `context.rs:657`
- **Current Usage**: Two factories actively used; third is test-only
- **Impact**: `build_standard_application_context` is dead code that could confuse developers about the canonical construction path
- **Recommended Action**: Remove `build_standard_application_context`

---

## 12. Error Handling Consistency

### 12.1. Production Panics in build_application_context

- **Category**: Maintainability Risk
- **Severity**: High
- **File**: `src-tauri/src/lib.rs:94-98, 145-146`
- **Evidence**: Two panics in production code:
  - `lib.rs:94-98` — `DurableJobQueue::new()` failure panics: `"Failed to create job queue at {}: {}"`
  - `lib.rs:145-146` — `VaultGraph::with_persistence()` failure panics: `"Failed to initialize VaultGraph: {}"`
  
  Both of these are called during `build_application_context()` which runs in the Tauri `setup` callback. If either service fails to initialize (e.g., permission error writing to vault path, corrupted queue directory), the entire application crashes at startup.
- **Semantic References**: Panics at `lib.rs:94` and `lib.rs:145`; both in `build_application_context`
- **Current Usage**: Reachable — any I/O error during queue or graph initialization crashes the app
- **Impact**: No graceful degradation; application becomes unusable if vault has issues; difficult to recover from corrupted state
- **Recommended Action**: Replace panics with error propagation and user-facing error messages

### 12.2. Inconsistent Error Handling for Service Resolution

- **Category**: Architectural Drift
- **Severity**: Medium
- **File**: `src-tauri/src/history.rs:24-27` vs `src-tauri/src/lib.rs:332-334`
- **Evidence**: 
  - `history.rs:24-27` — uses `ctx.history_manager()` with proper `ok_or_else()` error handling, returning `String` error
  - `lib.rs:332-334` — uses `ctx.resolve("capture_engine").expect("CaptureEngine must be registered")` — panics on None
  - `lib.rs:335` — `ctx.worker_pool()` returns `Option<Arc<WorkerPool>>`, handled with `if let Some(pool) = pool`
  
  Three different error handling patterns for service resolution: expect/panic, Result with ok_or_else, and Option with if-let
- **Semantic References**: `.expect()` at `lib.rs:333`; `ok_or_else` at `history.rs:26`; `if let Some` at `lib.rs:335`
- **Current Usage**: All three patterns coexist
- **Impact**: Inconsistent — some service lookups crash on missing dependency while others return errors
- **Recommended Action**: Standardize on `Result<_, String>` for all service lookups that could fail

### 12.3. No Event-to-IPC Bridge for Backend Events

- **Category**: Dead Architecture
- **Severity**: Critical
- **File**: `src-tauri/src/lib.rs:162-177` vs `crates/nabu-ui/src/ipc.rs:9`
- **Evidence**: (Confirmed in AUDIT_0.5) The backend publishes 8 event types through `EventBus`, and subscribes to `ITEM_STORED` for indexer/graph updates. However, there is no `tauri::Event::emit` or `window.emit` call anywhere in the backend that forwards `PipelineEvent` to the frontend. The frontend's `ipc.rs` only wraps `invoke` — it has no `#listen` capability for receiving backend events.
- **Semantic References**: `event_bus.publish()` calls throughout `crates/nabu-core/src/`; zero `emit()` calls in `src-tauri/src/` that forward events to frontend
- **Current Usage**: Backend events are internal-only; frontend never receives push notifications
- **Impact**: Critical gap in UI-backend communication; blocks real-time updates for capture progress, indexing status, graph changes
- **Recommended Action**: Finish (implement event bridge using Tauri's `emit` API)

---

## 13. Documentation Drift Review

### 13.1. Application Architecture Document Lists Non-existent Services

- **Category**: Documentation Drift
- **Severity**: Medium
- **File**: `crates/nabu-core/src/registry/application.rs:1-29`
- **Evidence**: The architecture doc at `application.rs:1-29` describes the composition root with these child services:
  - `ContentProvider` — resolves file content
  - `ExportEngine` — export to HTML/Markdown/etc.
  - `TemplateManager` — note templates
  - `VaultGraph` — semantic relationship graph (exists ✓)
  - `PerformanceMonitor` — local metrics aggregation (exists ✓)
  
  Grep confirms `ContentProvider`, `ExportEngine`, and `TemplateManager` do NOT exist anywhere in the codebase. These are aspirational services that were never implemented. The doc comments suggest they should be part of the architecture.
- **Semantic References**: `ContentProvider` at `application.rs:22` (doc only); `ExportEngine` at `application.rs:26` (doc only); `TemplateManager` at `application.rs:27` (doc only); no corresponding `pub mod` in `lib.rs`, no struct definitions anywhere
- **Current Usage**: Documentation references services that don't exist
- **Impact**: Misleads developers about the system's actual capabilities; `ApplicationBuilder` documentation shows `.with_*` methods for these services
- **Recommended Action**: Update documentation to reflect reality; remove `Application` struct references to non-existent services

### 13.2. PerformanceMonitor Docs Claim Integration That Doesn't Exist

- **Category**: Documentation Drift
- **Severity**: Low
- **File**: `crates/nabu-core/src/diagnostics/performance.rs:1`
- **Evidence**: The module doc states "PerformanceMonitor — Local Performance Instrumentation" and describes it as producing "performance metrics and health snapshots." However, the only usage in production code is `nabu_core::diagnostics::init(None, "nabu")` at `lib.rs:190` which initializes logging/tracing layers. The `PerformanceMonitor` itself is never resolved, queried, or used for actual monitoring in the production code path. It exists as a struct with methods but no consumers.
- **Semantic References**: `PerformanceMonitor` registered in `ApplicationBuilder` (dead code); `global_monitor()` only used within `diagnostics/` module for span metadata
- **Current Usage**: Struct exists but is not meaningfully integrated; `global_monitor()` is called internally for tracing spans but no metrics are exposed to the UI
- **Impact**: Dead monitoring infrastructure; documentation over-promises
- **Recommended Action**: Either integrate or remove; update docs

### 13.3. ApplicationDoc Lifecycle Claims Unused Transition API

- **Category**: Documentation Drift
- **Severity**: Low
- **File**: `crates/nabu-core/src/registry/application.rs:38-60`
- **Evidence**: The lifecycle doc shows `ApplicationBuilder::new()` → `.build()` → `Application { Created }` → `.initialize()` → `.start()` → `.shutdown()`. In reality, `build_application_context()` constructs services directly without an `Application` wrapper. The lifecycle stages (`Created → Initialized → Running → Shutdown`) are managed by `LifecycleManager` but the `Application::initialize()` and `Application::start()` methods are only called in tests. The Tauri `setup` callback calls `ctx.initialize()` and `ctx.start()` on the `ApplicationContext` directly (not through `Application`).
- **Semantic References**: `Application::initialize()` at `application.rs:164`; in production, `ApplicationContext::initialize()` is called at `lib.rs:339`
- **Current Usage**: Lifecycle doc describes `Application` API that is never used in production
- **Impact**: Developer confusion — the canonical architecture documentation describes a path that doesn't exist in production
- **Recommended Action**: Update documentation or migrate production to `Application` pattern

### 13.4. Pipeline Doc Claims ITEM_STORED Drives Indexer + VaultGraph (Partially True)

- **Category**: Documentation Drift
- **Severity**: Medium
- **File**: `crates/nabu-core/src/lib.rs:8-26`
- **Evidence**: The architecture doc shows:
  ```
  StorageManager.save()
      ├── Indexer.index_document()
      └── VaultGraph.update_node()
  ```
  The actual implementation at `lib.rs:162-177` subscribes to `ITEM_STORED` and calls `Indexer.index_object(&object)` — correct. But it calls `VaultGraph::add_node(&object)` — not `update_node`. The doc says `update_node` but the code says `add_node`. Additionally, the doc shows `GraphEventBridge` as the path for graph updates, but the production code bypasses it entirely (uses direct `add_node`).
- **Semantic References**: Doc at `lib.rs:21-22` (`Indexer.index_document()`, `VaultGraph.update_node()`); code at `lib.rs:166` (`indexer.index_object`), `lib.rs:171` (`graph.add_node`)
- **Current Usage**: Partially matches reality
- **Impact**: Method names don't match; developers searching for `update_node` won't find it
- **Recommended Action**: Update doc to match actual method names

---

## 14. Capability Platform Risk Assessment

The Capability Platform roadmap introduces Syncthing, Harper, ACP Client, and additional future capabilities. The following existing technical debt directly complicates their implementation:

### 14.1. Fragmented Event System (High Priority)

- **Risk**: Syncthing requires real-time notification of vault changes. The backend `EventBus` publishes `ITEM_STORED`, `INDEX_UPDATED`, and `GRAPH_UPDATED` events, but there is **no bridge** from `EventBus` to Tauri's `emit` / frontend `#listen`. The `GraphEventBridge` (incremental graph update system) exists but is dead code (see Section 4.3). Syncthing would need to subscribe to vault changes, but the event system has no path to the UI or to external sync services.
- **Evidence**: Zero `tauri::Event::emit()` calls in `src-tauri/src/`; zero `#[listen]` in frontend
- **Action Before Capability Platform**: Implement event-to-IPC bridge

### 14.2. note_save Pipeline Bypass (High Priority)

- **Risk**: Harper (AI assistant) and ACP Client need to process all content through the canonical pipeline. But `note_save` at `recovery.rs:391` bypasses the pipeline entirely — it writes directly to disk without publishing `ITEM_STORED`. If Harper needs to process notes saved via the editor, those notes will never trigger pipeline processing, indexing, or graph updates.
- **Evidence**: `note_save` at `recovery.rs:391-406`; no `ITEM_STORED` published, no `StorageManager::save()` called
- **Action Before Capability Platform**: Route `note_save` through pipeline or publish `ITEM_STORED`

### 14.3. Plugin System is Completely Dead (High Priority)

- **Risk**: The entire `plugin/` module (670+ lines: `manager.rs`, `capability.rs`, `dependency.rs`, `features.rs`, `lifecycle.rs`, `manifest.rs`, `permissions.rs`, `version.rs`) defines a complete plugin architecture but is never instantiated. The `Application` struct (the intended composition root for plugins) is dead code. The `PluginManager`, `CapabilityRegistry`, `FeatureRegistry`, and `PermissionEvaluator` are only constructed in tests. For the Capability Platform, plugins need to register capabilities dynamically — but the entire plugin infrastructure is test-only.
- **Evidence**: `PluginManager` only at `plugin/manager.rs:527` (test); `Application::builder()` only in tests; `CapabilityRegistry::new()` only at `lib.rs:60` (and `manager.rs:56` in tests)
- **Action Before Capability Platform**: Decide — integrate plugin system or remove it; at minimum document the intended plugin integration path

### 14.4. Incremental Graph Updates Not Wired (Medium Priority)

- **Risk**: Syncthing will generate many incremental vault changes. The `IncrementalUpdateEngine` exists (1,157 lines across 7 files) with change tracking, transaction batching, and region management, but is never connected to the production event flow. Every `ITEM_STORED` event triggers a full `VaultGraph::add_node()` instead of an incremental diff.
- **Evidence**: `wire_incremental_graph_updates` only called in `incremental_graph_integration.rs` test
- **Action Before Capability Platform**: Wire incremental engine or document scalability limits

### 14.5. No Graceful Shutdown for Services (Medium Priority)

- **Risk**: Syncthing and any sync service must be cleanly stopped on application exit to prevent data corruption. The `RunEvent::Exit` handler at `lib.rs:405-411` only removes a marker file. No service shutdown is performed. The `SocketServerHandle` is discarded (line 362: `let _handle = ...`), making it impossible to stop the native messaging socket.
- **Evidence**: Zero `shutdown()` calls in `RunEvent::Exit` handler
- **Action Before Capability Platform**: Implement graceful shutdown for all services

### 14.6. Platform Integration Commands Unimplemented on Frontend (Low Priority)

- **Risk**: ACP Client and future platform integrations may need to interact with OS features. 7 platform-specific commands are registered (`open_app_in_finder`, `show_macos_notification`, etc.) but none are called from the frontend. The UI has no way to trigger these platform interactions.
- **Evidence**: `tauri_invoke` calls for these commands: zero in `nabu-ui/src/`
- **Action Before Capability Platform**: Wire into UI or remove

---

## 15. Prioritized Remediation Backlog

### Immediate (Must Address Before Capability Platform)

| Priority | Issue | Category | Effort | Rationale |
|----------|-------|----------|--------|-----------|
| P0 | Implement Event-to-IPC Bridge | Dead Feature (4.1) | 3-5 days | Syncthing cannot function without real-time vault change notifications reaching the UI or sync services |
| P0 | Route note_save through pipeline or publish ITEM_STORED | Dead Architecture (4.2) | 2-3 days | Harper ACP Client will miss editor-saved notes if they bypass the pipeline entirely |
| P0 | Decide on Plugin System integration or removal | Dead Architecture, Dead Code | 5-10 days | The entire plugin/ module (670+ lines) is test-only; Capability Platform needs a clear plugin story |
| P1 | Remove Application/ApplicationBuilder | Duplicate Architecture (5.1) | 3-4 days | 514 lines of production-dead code causing API confusion; `build_application_context` is the actual production path |
| P1 | Remove wire_job_events_to_event_bus stub | Temporary Implementation (7.1) | 0.5 day | Empty function will cause silent failures if someone tries to use it |
| P1 | Implement graceful service shutdown | Service Lifecycle (10.2) | 2-3 days | Data integrity risk for all services; required for clean Syncthing lifecycle |

### Short-Term (Address in 2-week window)

| Priority | Issue | Category | Effort | Rationale |
|----------|-------|----------|--------|-----------|
| P1 | Remove dead frontend components | Dead Code (3.1) | 1 day | 600+ lines of dead code in 6+1 components; trivial removal |
| P1 | Remove collections/ module or finish wiring | Dead Code (3.3) | 3-5 days | 6 files with Yew-style Props that can't work in Leptos; either remove or convert |
| P2 | Remove NoopExecutor and wire FallbackExecutor | Trait Quality (9.2) | 1 day | 2 dead executor implementations; FallbackExecutor should be the safety net |
| P2 | Remove build_standard_application_context | Dependency Smell (11.3) | 0.5 day | Test-only factory that duplicates `build_application_context` logic |
| P2 | Remove Application doc references to non-existent services | Documentation Drift (13.1) | 1 day | `ContentProvider`, `ExportEngine`, `TemplateManager` don't exist; fix docs |
| P2 | Store SocketServerHandle for shutdown | Service Lifecycle (10.3) | 1 day | Currently impossible to stop the native messaging socket |

### Medium-Term (Deferred to later sprints)

| Priority | Issue | Category | Effort | Rationale |
|----------|-------|----------|--------|-----------|
| P2 | Wire IncrementalUpdateEngine into production | Dead Architecture (4.3) | 3-5 days | 1,157 lines of sophisticated code exists but unused; needed for graph performance at scale |
| P2 | Replace panics in build_application_context | Error Handling (12.1) | 1-2 days | App crashes on vault I/O errors; should degrade gracefully |
| P3 | Remove Lifecycle trait (never implemented) | Dead Code (trait) | 0.5 day | Dead abstraction with no implementations |
| P3 | Remove ApplicationBuilder's performance_monitor wiring | Dependency Smell (11.2) | 0.5 day | PerformanceMonitor registered through dead ApplicationBuilder |
| P3 | Remove native_messaging_host.rs binary | Migration Artifact (6.3) | 1 day | Verify no Safari extension uses it; if socket is the new path, binary is dead |
| P3 | Audit NativeMessagingSocket permissions | Migration Artifact (6.2) | 1 day | 0o777 permissions on /tmp socket is a security concern |
| P3 | Remove WorkerChannel stub code | Temporary Implementation (7.3) | 1 day | 108 lines of dead communication code replaced by queue polling |

### Leave Untouched

| Priority | Issue | Category | Rationale |
|----------|-------|----------|-----------|
| P3 | SettingsStore dual-copy pattern | Architectural Drift (3.3) | Acceptable — frontend cache is necessary for CSR WASM; sync strategy is documented and functional |
| P3 | Three IPC invocation patterns | Architectural Drift (8.3) | 50+ call sites make standardization a high-risk refactor with no functional benefit |
| P3 | Three settings mutation mechanisms | Architectural Drift (8.4) | Working correctly; standardizing would be cosmetic |
| P3 | Signal-captured Copy pattern | Architectural Drift (8.2) | Intentional, consistent, and well-documented architectural decision — not debt |
| P3 | Two factory functions (build_default_* and build_standard_*) | Dependency Smell (11.3) | Both are actively used and serve clear purposes |
| P3 | Queue trait with single implementation | Trait Quality (9.1) | Strategic abstraction for future in-memory/SQL queue implementations |

---

## 16. Conclusion

> **If the Nabu team had two weeks to reduce technical debt before beginning the Capability Platform roadmap, which issues should be addressed first, which should be deferred, and which should be left untouched because they are architecturally sound despite appearing imperfect?**

### Address First (P0 — blocks Capability Platform):

1. **Event-to-IPC Bridge (P0)**: Syncthing cannot function without backend→frontend event propagation. Implement `tauri::Event::emit()` in the backend for `ITEM_CAPTURED`, `ITEM_STORED`, `INDEX_UPDATED`, and `GRAPH_UPDATED` events. Add frontend `#[listen]` subscriptions. This is the single biggest blocker.

2. **note_save Pipeline Integration (P0)**: Harper and ACP Client will miss 100% of editor-written notes. Modify `note_save` to either route through `StorageManager::save()` (which publishes `ITEM_STORED`) or manually publish the event after the write completes.

3. **Plugin System Decision (P0)**: The 670-line plugin module is entirely test-only. Either (a) integrate `PluginManager` into `build_application_context` and register it in the `ServiceRegistry`, or (b) mark the entire module as `#[cfg(test)]` only. The Capability Platform needs to know which path was chosen.

4. **ApplicationBuilder/Architecture Consolidation (P1)**: Remove the dead `Application` struct, `ApplicationBuilder`, and `BuildContext` to eliminate the dual composition root. This also removes the `Lifecycle` trait (dead abstraction, 9.3) and the `build_standard_application_context` test-only factory (11.3) in one stroke. The production `build_application_context` in `lib.rs` becomes the single, canonical composition root.

5. **Graceful Shutdown (P1)**: Implement a `ShutdownCoordinator` that calls `worker_pool.shutdown()`, `queue.shutdown()`, `socket_server.shutdown()`, `indexer.persist()`, and `vault_graph.persist()` on `RunEvent::Exit`. Currently, the app cannot cleanly stop services — this is a data integrity risk.

6. **Remove Dead Frontend Components (P1)**: 6 dead components (600+ lines) and 1 dead module (`tree.rs`, 48 lines) can be removed in a single afternoon. This also includes removing the dead `sandbox.rs` file that isn't even compiled.

7. **Finish or Remove collections/ Module (P1)**: The 6-file collections module is dead code using Yew-style `Props`. Either finish the Leptos migration and wire `CollectionContainer` into `app.rs`, or remove the entire module. Leaving it as dead code with a broken framework pattern will confuse future developers.

### Defer (P2 — important but not blocking):

- **IncrementalUpdateEngine wiring**: Move to P1 if Syncthing will generate high-velocity vault changes
- **WorkerChannel cleanup**: The polling-based worker loop works; channel-based notification is a performance optimization
- **Error handling standardization**: Replace `.expect()` and `panic!()` in `build_application_context` with proper `Result` returns
- **VaultGraph version recovery panic**: Replace with graceful error recovery for corrupted graph state

### Leave Untouched:

- **SettingsStore dual-copy**: The frontend maintains its own `AppSettings` copy because CSR WASM cannot synchronously read backend state on every render. The `settings_set_all` / `settings_set` / `get_settings` trio is the correct pattern for this architecture.
- **Multiple IPC invocation patterns**: The three argument-construction styles (`JsValue`, `serde_json::json!`, `JsValue::NULL`) all go through the same `tauri_invoke` wrapper. Standardizing them is cosmetic and touches 50+ call sites for zero behavioral change.
- **Queue trait with single implementation**: The `Queue` trait provides a clean abstraction boundary for future in-memory or distributed queue implementations. This is not premature generalization — it's an interface for future extensibility.
- **Signal-captured Copy pattern**: The decision to use `Copy` types captured in closures rather than `expect_context()` is intentional and consistent across all 7 context providers. It is documented with explanatory comments in `smart_folders.rs:15` and `save_status.rs:7`.
- **Factory functions**: `build_default_capture_engine()` and `build_standard_pipeline()` are both actively used in production and serve distinct purposes. The test-only `build_standard_application_context()` should be removed, but the two production factories are fine.

### Two-Week Plan:

**Week 1**: 
- Remove all dead code (components, tree.rs, sandbox.rs, ApplicationBuilder/Application, Lifecycle trait)
- Fix the event bridge (P0 — requires understanding Tauri's `emit` API)
- Fix `note_save` to publish `ITEM_STORED`

**Week 2**: 
- Implement graceful shutdown
- Remove `wire_job_events_to_event_bus` stub and `NoopExecutor`
- Remove `build_standard_application_context`
- Decide on collections module (remove or finish)
- Begin plugin system integration planning

This plan addresses all P0 issues, most P1 issues, and removes ~1,000 lines of dead code. The Capability Platform roadmap can then proceed with a clean, well-understood foundation.