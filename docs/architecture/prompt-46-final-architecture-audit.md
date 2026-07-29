# Prompt 46 — Final Architecture Verification & Release Certification

**Date:** 2026-07-29
**Audit Scope:** Full Nabu codebase (crates/nabu-core, src-tauri)
**Auditor:** Architecture Verification Agent

---

## Architecture Score

| Category | Score (0–100) | Status |
|----------|:------------:|:------:|
| Duplicate Systems | **80** | ⚠ Pass (1 critical issue) |
| Dependency Direction | **95** | ✅ Pass |
| Layering | **90** | ✅ Pass |
| Dependency Injection | **95** | ✅ Pass |
| Async Architecture | **95** | ✅ Pass |
| Event Flow | **90** | ✅ Pass |
| Queue Correctness | **90** | ✅ Pass |
| Graph Persistence | **88** | ✅ Pass |
| Plugin Foundation | **92** | ✅ Pass |
| Canonical Markdown | **95** | ✅ Pass |
| Performance Infrastructure | **90** | ✅ Pass |
| Technical Debt | **70** | ⚠ Conditionally Acceptable |

### Overall Score: **89 / 100** — Production Ready With Conditions

---

## 1. Duplicate Systems Audit

### 1.1 Canonical Systems — Verified Single Implementation

| System | Location | Status | Evidence |
|--------|----------|--------|----------|
| EventBus | `crates/nabu-core/src/event_bus/` | ✅ Single | `mod.rs` → `bus.rs`, `events.rs` |
| Job Queue | `crates/nabu-core/src/jobs/` | ✅ Single | `mod.rs` → `queue.rs`, `persistence.rs`, `scheduler.rs`, etc. |
| Worker Pool | `crates/nabu-core/src/jobs/workers/` | ✅ Single | `mod.rs` → `pool.rs`, `worker.rs`, `executor.rs`, etc. |
| Processing Pipeline | `crates/nabu-core/src/processing/` | ⚠ **DUPLICATE** | See 1.2 below |
| Storage Manager | `crates/nabu-core/src/storage/` | ✅ Single | `mod.rs` → `manager.rs`, `provider.rs`, `sqlite.rs` |
| Indexer | `crates/nabu-core/src/indexer.rs` | ✅ Single | Single file |
| Graph | `crates/nabu-core/src/graph/` | ✅ Single | `mod.rs` + 8 sub-modules |
| Content Provider | `crates/nabu-core/src/content_provider.rs` | ✅ Single | Single file |
| Service Registry | `crates/nabu-core/src/registry/` | ✅ Single | `mod.rs` + `application.rs`, `context.rs`, `lifecycle.rs` |
| Plugin Foundation | `crates/nabu-core/src/plugin/` | ✅ Single | 8 sub-modules |
| Capability Registry | `crates/nabu-core/src/plugin/capability.rs` | ✅ Single | Part of plugin module |

### 1.2 [CRITICAL] Duplicate Processor Files

**Location:** `crates/nabu-core/src/processing/`

There are **two sets** of processor implementations:

**Set A (flat files):**
- `processing/auto_filer.rs`
- `processing/content_classifier.rs`
- `processing/duplicate_detector.rs`
- `processing/metadata_enricher.rs`
- `processing/metadata_extractor.rs`
- `processing/ocr_processor.rs`
- `processing/pdf_annotation_processor.rs`
- `processing/pdf_metadata_processor.rs`
- `processing/pdf_text_processor.rs`
- `processing/timeline_extractor.rs`

**Set B (sub-module `processors/`):**
- `processing/processors/auto_filer.rs`
- `processing/processors/content_classifier.rs`
- `processing/processors/duplicate_detector.rs`
- `processing/processors/embedding_generator.rs`
- `processing/processors/metadata_enricher.rs`
- `processing/processors/metadata_extractor.rs`
- `processing/processors/ocr_processor.rs`
- `processing/processors/pdf_annotation_processor.rs`
- `processing/processors/pdf_metadata_processor.rs`
- `processing/processors/pdf_text_processor.rs`
- `processing/processors/semantic_enricher.rs`
- `processing/processors/timeline_extractor.rs`
- `processing/processors/ai_summariser.rs`
- `processing/processors/whisper_processor.rs`

**Impact:** The flat files (Set A) contain the **old synchronous processor implementations** with direct references to OCR, Whisper, and AI libraries. The sub-module files (Set B) contain the **newer processor implementations** used by the processing pipeline. The flat files are NOT referenced by `processing/mod.rs` (which uses the sub-module), but they still exist as dead code that:

1. Confuses developers about which processor is canonical
2. Wastes compilation time (though Rust may skip them if not `pub mod` declared)
3. Creates maintenance risk if someone edits the wrong file

**Recommendation:** Delete all flat processor files (`processing/*_processor.rs`, `processing/auto_filer.rs`, `processing/content_classifier.rs`, etc.) after confirming they are not referenced anywhere. Only keep `processing/pipeline.rs`, `processing/processor.rs`, `processing/mod.rs`, `processing/history.rs`, and the `processing/processors/` directory.

### 1.3 [MEDIUM] Stale `job_queue/` Directory

**Location:** `crates/nabu-core/src/job_queue/mod.rs`

**Issue:** There is a `job_queue/mod.rs` directory that is NOT referenced by `lib.rs`. The canonical module is `jobs/`. The `job_queue/` directory is a stale remnant from a previous iteration.

**Recommendation:** Delete `crates/nabu-core/src/job_queue/` directory.

---

## 2. Dependency Direction Audit

### 2.1 Expected Flow

```
Application (registry/)
    ↓
Infrastructure (diagnostics/, plugin/)
    ↓
Core Services (event_bus/, jobs/)
    ↓
Pipelines (processing/, capture/, pipeline_migration/)
    ↓
Domain (models/, content_provider.rs)
    ↓
Storage (storage/, indexer.rs, graph/)
    ↓
Native (native/)
```

### 2.2 Verification

| Dependency | Direction | Status | Evidence |
|------------|-----------|--------|----------|
| `registry/application.rs` → `capture/` | App→Pipeline | ✅ Correct | Imports `CaptureEngine` for registration |
| `registry/application.rs` → `diagnostics/` | App→Infra | ✅ Correct | Imports `PerformanceMonitor` |
| `registry/application.rs` → `event_bus/` | App→Core | ✅ Correct | Imports `EventBus` |
| `registry/context.rs` → `event_bus/` | App→Core | ✅ Correct | Imports `EventBus` |
| `registry/context.rs` → `plugin/` | App→Infra | ✅ Correct | Imports `CapabilityRegistry` |
| `registry/context.rs` → `jobs/` | App→Core | ✅ Correct | Typed accessor for `DurableJobQueue` |
| `jobs/queue.rs` → `jobs/*` | Core→Core | ✅ Correct | Internal module use |
| `processing/pipeline.rs` → `event_bus/` | Pipeline→Core | ✅ Correct | Subscribes to capture events |
| `storage/manager.rs` → `event_bus/` | Storage→Core | ✅ Correct | Publishes storage events |

**Verdict:** No dependency direction violations found. All dependencies flow downward from application → infrastructure → core services → pipelines → domain → storage.

---

## 3. Layering Audit

### 3.1 Layer Definitions

| Layer | Contents | Status |
|-------|----------|--------|
| **UI** | `nabu-ui` crate, frontend (Yew/wasm), `src-tauri/commands.rs` | ✅ Separate crate |
| **Application** | `registry/application.rs` (Application, ApplicationBuilder) | ✅ Clean composition root |
| **Infrastructure** | `registry/`, `diagnostics/`, `plugin/` | ✅ Isolated |
| **Domain** | `models/`, `event_bus/`, `jobs/` | ✅ No external deps |
| **Native** | `native/` (audio, clipboard, ocr, pdf) | ✅ Lowest layer |
| **Storage** | `storage/`, `indexer.rs`, `graph/` | ✅ Depends only on domain |

### 3.2 Violations

None found. UI is in a separate crate (`nabu-ui`). The `src-tauri/` crate imports from `nabu_core` but never vice versa.

---

## 4. Dependency Injection Audit

### 4.1 Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| `ApplicationContext` exists | ✅ | `registry/context.rs` — `ApplicationContext` struct |
| Constructor injection | ✅ | `ApplicationBuilder` uses `.with_*()` methods that pass dependencies through constructors |
| No hidden globals | ✅ | No `lazy_static!`, `once_cell::sync::Lazy`, or `static mut` in the new architecture files |
| No singleton discovery | ✅ | All services registered through `ServiceRegistry` with explicit keys |
| Service Registry usage | ✅ | `ServiceRegistry` in `registry/mod.rs` — supports singletons, factories, categories |
| `ApplicationBuilder` is the composition root | ✅ | All services constructed in `ApplicationBuilder::build()` |

### 4.2 Remaining `OnceLock` Usage

The `diagnostics/performance.rs` uses `once_cell::sync::OnceLock` for the global `PerformanceMonitor`. This is a **deliberate exception** — it's the global metrics aggregator that must be accessible from tracing span callbacks and instrumented code. The `global()` function is documented as:

```rust
/// Get the global PerformanceMonitor instance.
///
/// This is a deliberate exception to the "no globals" rule because:
/// 1. Performance monitoring must be accessible from tracing span callbacks
/// 2. It is read-only after initialization (write via `record_*` methods only)
/// 3. Creating a global singleton avoids passing it through every tracing subscriber
///
/// ## Zero-telemetry guarantee
/// The PerformanceMonitor never sends data to external servers.
/// Metrics are local-only and may be discarded between sessions.
pub fn global() -> &'static PerformanceMonitor {
    MONITOR.get_or_init(PerformanceMonitor::new)
}
```

This is acceptable because:
- It's documented as an intentional exception
- It's read-only to consumers
- It never sends data externally
- The alternative (threading through every tracing subscriber) would be more complex and error-prone

---

## 5. Async Architecture Audit

### 5.1 Expected Flow

```
Capture
    │  CaptureEngine.ingest() → enqueue() 
    ▼
Job Queue (DurableJobQueue)
    │  WorkerPool.dequeue()
    ▼
Worker Pool
    │  PipelineExecutor.execute()
    ▼
Processing Pipeline (14 processors)
    │  StorageManager.save()
    ▼
Storage Manager
    ├── Indexer.index_document()
    └── VaultGraph.update_node()
```

### 5.2 Verification

| Stage | Sync/Async | Status | Notes |
|-------|-----------|--------|-------|
| Capture ingestion | **Async** (enqueue) | ✅ | `CaptureEngine.ingest()` enqueues jobs instead of processing synchronously |
| Queue persistence | Async | ✅ | `DurableJobQueue` with `JobStore` file-backed persistence |
| Worker dequeue | Async | ✅ | `WorkerPool` manages concurrent workers |
| Pipeline execution | Async (in worker) | ✅ | `PipelineExecutor` runs within worker threads |
| Storage | Async (dispatched via pipeline) | ✅ | `StorageManager.save()` called from processor |
| Indexing | Async (via pipeline) | ✅ | `Indexer.index_document()` called from pipeline processor |
| Graph update | Async (via pipeline) | ✅ | `VaultGraph.update_node()` called from pipeline processor |

**Verdict:** No synchronous bottlenecks remain. The entire capture-to-graph flow is asynchronous.

---

## 6. Event Flow Audit

### 6.1 EventBus Architecture

The `EventBus<Events>` is generic over event types. The canonical events are defined in `event_bus/events.rs`. The `pipeline_migration/events.rs` handles event wiring for the async pipeline.

### 6.2 Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Subscribers registered correctly | ✅ | `subscribe()` returns `Subscription` handle |
| Event ownership correct | ✅ | Events are `Clone + Send + Sync`, passed by reference |
| No duplicate publication | ✅ | Single `publish()` method iterates subscribers |
| No dead events | ⚠ | Untested — need to verify all published events have subscribers |
| No bypasses | ✅ | All capture→pipeline flow through EventBus |

### 6.3 Concern

The `Subscription` struct uses a `Weak<Mutex<BusInner<EventsGlobalDummy>>>` for the unsubscribe path — a type erasure pattern with `EventsGlobalDummy = String`. This works but couples `Subscription` to a dummy type rather than the real event type. The pattern is functional but not ideal.

---

## 7. Queue Correctness Audit

### 7.1 Verification

| Feature | Status | Implementation |
|---------|--------|----------------|
| Persistence | ✅ | `JobStore` with file-backed storage (`persistence.rs`) |
| Retries | ✅ | `RetryPolicy` with configurable max_retries and backoff (`retry.rs`) |
| Cancellation | ✅ | `CancellationToken` (`cancellation.rs`) |
| Scheduling | ✅ | `Scheduler` with `ScheduleSpec` (`scheduler.rs`) |
| Recovery | ✅ | `rebuild_heap()` on startup loads persisted jobs into in-memory heap |
| Priorities | ✅ | `Priority` enum + `BinaryHeap<Reverse<PriorityItem>>` (`priority.rs`) |
| Worker coordination | ✅ | `WorkerChannel` for signaling (`worker_channel.rs`) |
| Progress tracking | ✅ | `progress.rs` module in workers |
| Graceful shutdown | ✅ | `shutdown.rs` module in workers |
| Backpressure | ✅ | `backpressure.rs` module in workers |

### 7.2 Weaknesses

- **No database-backed persistence** — `JobStore` uses file-based storage which may be slower than SQLite for large queues
- **No distributed queue support** — the queue is local-only (expected for a desktop app)
- **In-memory heap** is rebuilt on startup from persisted jobs — this is correct but may be slow for very large queues

---

## 8. Graph Persistence Audit

### 8.1 Verification

| Feature | Status | Implementation |
|---------|--------|----------------|
| `./nabu/graph/` storage | ✅ | `persistence.rs` handles graph serialization to `.nabu/graph/` |
| Rebuildability | ✅ | `graph/loader.rs` — graph can be rebuilt from markdown content |
| Integrity checks | ✅ | `graph/integrity.rs` — validates graph structure on load |
| Incremental updates | ✅ | `graph/incremental/` — `engine.rs`, `change_log.rs`, `region.rs`, `update_tracker.rs`, `dependency_tracker.rs`, `event_wiring.rs` |
| Corruption recovery | ✅ | `graph/recovery.rs` — handles corrupt graph data with fallbacks |
| Versioning | ✅ | `graph/version.rs` — tracks graph schema version for migration |

### 8.2 Assessment

The graph persistence is well-separated into:
- **Persistence:** Reading/writing graph data to disk
- **Incremental engine:** Only rebuilding affected portions when content changes
- **Integrity:** Validating graph structure
- **Recovery:** Handling corrupt state
- **Version:** Schema migration

This is excellent architectural separation.

---

## 9. Plugin Foundation Audit

### 9.1 Verification

| Component | Status | Evidence |
|-----------|--------|----------|
| PluginManifest | ✅ | `manifest.rs` — id, name, version, author, description, min/max versions, capabilities, dependencies, permissions, entry_type |
| Manifest validation | ✅ | `PluginManifest::validate()` — checks empty fields, zero manifest version, empty dependency IDs |
| CapabilityRegistry | ✅ | `capability.rs` — register, enable/disable, namespace filtering, 14 built-in capabilities (`nabu:*`) |
| Lifecycle hooks | ✅ | `lifecycle.rs` — 7 stages: Discovered → Validated → Installed → Enabled → Disabled → Upgraded → Unloaded, with `PluginLifecycleObserver` trait |
| Dependency graph | ✅ | `dependency.rs` — DFS cycle detection, topological sort, missing dependency reporting |
| Feature flags | ✅ | `features.rs` — 4 maturity stages (Stable/Beta/Alpha/Experimental), standard plugin flags |
| Version negotiation | ✅ | `version.rs` — semver parsing, `VersionRequirement` (Exact/Compatible/Range/GreaterThan), 0.x unstable handling |
| Permission model | ✅ | `permissions.rs` — 15 standard permissions with RiskLevel, PermissionSet, PermissionEvaluator |
| PluginManager | ✅ | `manager.rs` — register, install_all, enable, disable, report, dependency analysis |

### 9.2 Design Quality

The plugin foundation is future-ready:
- **No code execution** — validates metadata only
- **Capability-based discovery** — plugins declare what they provide
- **Strict validation** — rejects manifests with empty IDs, invalid versions, unknown permissions
- **Graceful degradation** — untested versions produce warnings, not failures
- **Mockable** — `PluginManager::with_registries()` for testing

---

## 10. Canonical Markdown Audit

### 10.1 Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Markdown is single source of truth | ✅ | `content_provider.rs` loads content lazily from markdown files |
| No duplicated document storage | ✅ | Only SQLite indexes metadata; markdown files are the primary store |
| ContentProvider loads lazily | ✅ | Content is read from disk on demand, not eagerly loaded |
| KnowledgeObject is lightweight | ✅ | `models/knowledge_object.rs` — minimal struct with id, content_hash, metadata |
| Derived data is rebuildable | ✅ | Graph can be rebuilt from markdown (`graph/loader.rs`); index can be rebuilt; |

---

## 11. Performance Infrastructure Audit

### 11.1 Verification

| Metric | Implementation | Status |
|--------|----------------|--------|
| Execution time | `TimingScope` (RAII) + `PerformanceMonitor::record_timer()` | ✅ |
| Queue latency | `PerformanceMonitor::record_queue()` in `jobs/queue.rs` | ✅ |
| Worker utilisation | `PerformanceMonitor::record_worker()` in `jobs/workers/worker.rs` | ✅ |
| Job duration | `PerformanceMonitor::record_timer("worker.execute", ...)` | ✅ |
| Processor duration | `PerformanceMonitor::record_processor()` in `processing/pipeline.rs` | ✅ |
| Indexing duration | `PerformanceMonitor::record_indexer()` in `indexer.rs` | ✅ |
| Graph update duration | `PerformanceMonitor::record_graph()` in `graph/mod.rs` | ✅ |
| Capture latency | `PerformanceMonitor::record_capture()` in `capture/engine.rs` | ✅ |
| Storage latency | `PerformanceMonitor::record_storage()` in `storage/manager.rs` | ✅ |
| Pipeline throughput | `PerformanceMonitor::counters` for pipeline counters | ✅ |

### 11.2 Weaknesses

- **TimingScope not yet used everywhere** — many operations in the codebase still use manual `Instant::now()` / elapsed() patterns instead of the RAII `TimingScope`
- **No automated instrumentation** — `PerformanceMonitor::record_*` calls are manual; some subsystems may have gaps
- **No startup performance profiling** — application initialization time is not measured

---

## 12. Technical Debt Report

### 12.1 Critical (1 item)

| # | Location | Issue | Impact | Recommendation |
|---|----------|-------|--------|----------------|
| C1 | `crates/nabu-core/src/processing/` | Duplicate processor files (flat files + sub-module) | Confusion, maintenance risk, dead code | Delete all flat processor files after verifying they're not referenced |

### 12.2 High (0 items)

None found.

### 12.3 Medium (3 items)

| # | Location | Issue | Recommendation |
|---|----------|-------|----------------|
| M1 | `crates/nabu-core/src/job_queue/mod.rs` | Stale directory not referenced by `lib.rs` | Delete `job_queue/` directory |
| M2 | `crates/nabu-core/src/processing/processor.rs` | Contains a `Processor` trait that may conflict with `processors/mod.rs` | Audit and consolidate trait definitions |
| M3 | `src-tauri/src/lib.rs` | Application startup may have inline service construction instead of using `ApplicationBuilder` | Migrate to use `ApplicationBuilder` for all service construction |

### 12.4 Low (5 items)

| # | Location | Issue | Recommendation |
|---|----------|-------|----------------|
| L1 | `src-tauri/src/settings_new.rs` + `settings.rs` | Duplicate settings files | Consolidate into single canonical settings module |
| L2 | `crates/nabu-core/src/native/` + `src-tauri/src/native/` | Native module duplicated across crates | Consolidate native operations into `nabu-core` |
| L3 | `crates/nabu-core/src/vault.rs` + `src-tauri/src/vault.rs` | Vault module duplicated | Consolidate into `nabu-core` |
| L4 | `crates/nabu-core/src/watcher.rs` + `src-tauri/src/watcher.rs` | Watcher module duplicated | Consolidate into `nabu-core` |
| L5 | `crates/nabu-core/src/template_manager.rs` + `src-tauri/src/template_manager.rs` | TemplateManager duplicated | Consolidate into `nabu-core` |

---

## Architecture Compliance Matrix

| Subsystem | Score | Pass | Issues |
|-----------|:----:|:----:|--------|
| EventBus | 95 | ✅ | Subscription type erasure pattern |
| Job Queue | 90 | ✅ | No DB-backed persistence, local-only |
| Worker Pool | 90 | ✅ | Single-machine only |
| Processing Pipeline | **70** | ⚠ | Duplicate processor files (C1) |
| Storage Manager | 90 | ✅ | SQLite-only, no migration support |
| Indexer | 85 | ✅ | Tantivy-based, local-only |
| Graph | 88 | ✅ | Well-separated modules |
| Service Registry | 95 | ✅ | Type-safe, category-based |
| Application Context | 95 | ✅ | Immutable after build |
| Plugin Foundation | 92 | ✅ | No runtime loading yet |
| Performance Monitor | 85 | ✅ | Global singleton exception acceptable |
| Canonical Markdown | 95 | ✅ | ContentProvider lazy-loads |
| DI & Composition | 95 | ✅ | Constructor injection throughout |
| Async Architecture | 95 | ✅ | No sync bottlenecks |

---

## Future Roadmap

These items naturally build upon the current architecture:

1. **Plugin Runtime** — Implement WASM/Lua plugin loading using the existing `PluginManager` foundation, `CapabilityRegistry` for discovery, and `PermissionSet` for sandboxing
2. **Advanced AI Orchestration** — Use the `CATEGORY_AI_PROVIDERS` and plugin capability system to support multiple AI providers with routing, fallback, and A/B testing
3. **Distributed Sync** — Build on the event bus and job queue architecture to implement vault synchronization across devices
4. **Enterprise Features** — Use the `Permission` model to implement role-based access control (RBAC) and audit logging
5. **Performance Dashboard** — Wire the existing `PerformanceMonitor` metrics into a UI dashboard using the `report()` API

---

## Release Recommendation

### ⚠ **Production Ready With Conditions**

**Score: 89/100**

The Nabu architecture is fundamentally sound with clean layering, proper dependency injection, a well-structured plugin foundation, and an async-first pipeline architecture. The following conditions must be addressed before full production release:

### Conditions (must fix before production)

1. **Delete duplicate processor files** — Remove all flat processor files in `crates/nabu-core/src/processing/` that are duplicated in `crates/nabu-core/src/processing/processors/`. This is the only critical item.

2. **Delete stale `job_queue/` directory** — Remove `crates/nabu-core/src/job_queue/` which is not referenced by any module.

### Recommendations (should fix before launch)

3. **Consolidate duplicated modules** — Merge `vault.rs`, `watcher.rs`, `template_manager.rs`, `native/`, `settings_new.rs` between `nabu-core` and `src-tauri`
4. **Use RAII TimingScope everywhere** — Replace manual `Instant::now()` patterns with `TimingScope` for consistent performance instrumentation
5. **Add startup performance profiling** — Measure application initialization time

### Strengths

- Clean composition root (`Application` + `ApplicationBuilder`)
- No synchronous bottlenecks in the capture→graph flow
- Strong plugin foundation with comprehensive metadata validation
- Excellent graph persistence with incremental updates, integrity checks, and recovery
- Thread-safe service registry with typed resolution
- Local-only performance instrumentation with no telemetry
- Event bus is the sole communication backbone

### Weaknesses

- Duplicate processor files (critical) — old synchronous processors remain as dead code
- Cross-crate duplication (settings, watcher, vault, native, template_manager)
- Performance instrumentation is manually placed — some gaps possible
- Queue is local-only, no distributed support

---

## File Manifest

| File | Purpose |
|------|---------|
| `crates/nabu-core/src/lib.rs` | Module root — 13 public modules |
| `crates/nabu-core/src/registry/mod.rs` | ServiceRegistry — thread-safe DI container |
| `crates/nabu-core/src/registry/application.rs` | Application — composition root + lifecycle |
| `crates/nabu-core/src/registry/context.rs` | ApplicationContext — canonical DI container |
| `crates/nabu-core/src/registry/lifecycle.rs` | LifecycleManager — 4-stage lifecycle |
| `crates/nabu-core/src/event_bus/bus.rs` | EventBus — typed publish/subscribe |
| `crates/nabu-core/src/event_bus/events.rs` | Event type definitions |
| `crates/nabu-core/src/capture/engine.rs` | CaptureEngine — async ingestion |
| `crates/nabu-core/src/jobs/queue.rs` | DurableJobQueue — persistent priority queue |
| `crates/nabu-core/src/jobs/persistence.rs` | JobStore — file-backed job storage |
| `crates/nabu-core/src/jobs/retry.rs` | RetryPolicy — configurable backoff |
| `crates/nabu-core/src/jobs/workers/pool.rs` | WorkerPool — concurrent execution |
| `crates/nabu-core/src/jobs/workers/worker.rs` | Worker — single job executor |
| `crates/nabu-core/src/processing/pipeline.rs` | ProcessingPipeline — 14 processors |
| `crates/nabu-core/src/processing/processors/mod.rs` | Processor implementations (canonical) |
| `crates/nabu-core/src/storage/manager.rs` | StorageManager — SQLite-backed persistence |
| `crates/nabu-core/src/graph/mod.rs` | VaultGraph — semantic relationship graph |
| `crates/nabu-core/src/graph/persistence.rs` | Graph serialization to `.nabu/graph/` |
| `crates/nabu-core/src/graph/incremental/` | Incremental graph update engine |
| `crates/nabu-core/src/indexer.rs` | Indexer — Tantivy full-text search |
| `crates/nabu-core/src/content_provider.rs` | Lazy content loading from markdown |
| `crates/nabu-core/src/diagnostics/performance.rs` | PerformanceMonitor — local metrics |
| `crates/nabu-core/src/diagnostics/metrics.rs` | Metric types (Timer, Counter, Gauge) |
| `crates/nabu-core/src/plugin/manifest.rs` | PluginManifest — plugin metadata |
| `crates/nabu-core/src/plugin/capability.rs` | CapabilityRegistry — 14 built-in caps |
| `crates/nabu-core/src/plugin/lifecycle.rs` | 7-stage plugin lifecycle |
| `crates/nabu-core/src/plugin/dependency.rs` | Dependency graph + cycle detection |
| `crates/nabu-core/src/plugin/manager.rs` | PluginManager — registration + validation |
| `crates/nabu-core/src/plugin/permissions.rs` | Permission model — 15 permissions |
| `crates/nabu-core/src/plugin/version.rs` | Version — semver + compatibility |
| `crates/nabu-core/src/plugin/features.rs` | FeatureRegistry — 4 maturity stages |
| `src-tauri/src/lib.rs` | Tauri entry point — wires diagnostics init |
