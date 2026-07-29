# Nabu Platform Architecture

## Overview

Nabu is a local-first, Markdown-canonical, privacy-first knowledge management application. This document describes the platform architecture at the point of completing architectural phase 3 (Platform Composition & Future Architecture).

## Core Architecture Principles

1. **Markdown is the source of truth (Principle 1)**
2. **KnowledgeObject is the universal runtime model (Principle 2)**
3. **Single pipeline: Capture → Process → Store → EventBus → UI (Principle 3)**
4. **Services never own canonical data (Principle 4)**
5. **Views are projections, never duplicates (Principle 5)**
6. **One search engine — Tantivy (Principle 6)**
7. **One relationship graph — Petgraph/VaultGraph (Principle 7)**
8. **Derived data is rebuildable (Principle 9)**
9. **Local-first (Principle 10)**
10. **Privacy-first — no telemetry sent externally (Principle 11)**

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        ApplicationContext                           │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    ServiceRegistry                            │   │
│  │  ┌────────────┐  ┌──────────────┐  ┌────────────────────┐    │   │
│  │  │ EventBus   │  │ CaptureEngine│  │ ProcessingPipeline │    │   │
│  │  ├────────────┤  ├──────────────┤  ├────────────────────┤    │   │
│  │  │ JobQueue   │  │ StorageManager│  │ Indexer (Tantivy) │    │   │
│  │  ├────────────┤  ├──────────────┤  ├────────────────────┤    │   │
│  │  │ VaultGraph │  │ WorkerPool   │  │ OcrProcessor      │    │   │
│  │  └────────────┘  └──────────────┘  └────────────────────┘    │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                  CapabilityRegistry                           │   │
│  │  nabu:event_bus │ nabu:storage │ nabu:capture │ nabu:processor │   │
│  │  nabu:graph     │ nabu:export  │ nabu:search  │ nabu:import   │   │
│  │  nabu:content_provider │ nabu:theme                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                 LifecycleManager                              │   │
│  │    Created → Initialized → Running → Shutdown                │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         Event Bus                                    │
│                                                                      │
│  ItemCaptured ──► ItemProcessed ──► ItemStored                       │
│       │               │                  │                           │
│       ▼               ▼                  ▼                           │
│  ProcessingP.    StorageMgr.       Indexer, Graph,                   │
│  (via JobQueue)                     Future Plugins                   │
└──────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    Plugin Infrastructure (v0)                        │
│                                                                      │
│  PluginManifest │ PluginMetadata │ FeatureFlags                      │
│  DependencyGraph │ VersionNegotiation │ LifecycleHooks                │
│                                                                      │
│  NOTE: No third-party plugin loading is implemented.                 │
│  The infrastructure is prepared for future plugins.                 │
└──────────────────────────────────────────────────────────────────────┘
```

## ApplicationContext

The `ApplicationContext` is the central composition point. Every major subsystem is registered in the `ServiceRegistry` under a well-known string key. The context provides:

- **Typed accessors** — `capture_engine()`, `processing_pipeline()`, `job_queue()`, `vault_graph()`, `indexer()`
- **Service validation** — `validate_core_services()` checks that required services (event_bus, capture_engine, pipeline, storage_manager) are present
- **Capability registration** — built-in capabilities are registered via the `CapabilityRegistry`
- **Lifecycle management** — `initialize()` → `start()` → `shutdown()` with transition validation

### Service Registry Categories

| Category | Description | Services |
|----------|-------------|----------|
| `capture_handlers` | Capture handlers implementing `CaptureHandler` trait | BrowserCaptureHandler, ArticleCaptureHandler, YouTubeCaptureHandler, GitHubRepositoryHandler, ClipboardHandler, ScreenshotHandler |
| `processors` | Processing pipeline processors implementing `Processor` trait | ContentClassifier, DuplicateDetector, TimelineExtractor, MetadataExtractor, MetadataEnricher, AutoFiler, OcrProcessor, PdfTextProcessor, PdfMetadataProcessor, PdfAnnotationProcessor |
| `ai_providers` | Future AI provider services | — (reserved) |
| `ocr_providers` | Future OCR engine services | — (reserved) |
| `embedding_providers` | Future embedding services | — (reserved) |
| `exporters` | Future export format services | — (reserved) |
| `storage_providers` | Future storage backends | — (reserved) |
| `content_providers` | Future content fetching services | — (reserved) |

### Standard Service Keys

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `event_bus` | `Arc<EventBus>` | Yes | Central pub/sub event bus |
| `capture_engine` | `Arc<CaptureEngine>` | Yes | Routes capture requests to handlers |
| `pipeline` | `Arc<ProcessingPipeline>` | Yes | Processor chain execution |
| `storage_manager` | `Arc<StorageManager>` | Yes | SQLite-based object persistence |
| `job_queue` | `Arc<JobQueue>` | No | Async background job queue |
| `worker_pool` | `Arc<WorkerPool>` | No | Tokio-based worker thread pool |
| `vault_graph` | `Arc<RwLock<VaultGraph>>` | No | Knowledge relationship graph |
| `indexer` | `Arc<Mutex<Indexer>>` | No | Tantivy full-text search index |

## Subsystem Ownership

| Subsystem | Owns | Does NOT Own |
|-----------|------|-------------|
| CaptureEngine | Handler registry, dispatch logic | KnowledgeObjects, Storage |
| ProcessingPipeline | Processor chain, processing history | Storage, Search, Graph |
| StorageManager | SQLite database, object persistence | Objects in memory, processing state |
| Indexer | Tantivy index files | Object metadata, graphs |
| VaultGraph | Petgraph graph structure | Object storage, indexes |
| JobQueue | Tokio channel, worker pool | Processing logic, storage |
| EventBus | Subscriber registry, event dispatch | Business logic, state |

## Event Flow

```
User action
    │
    ▼
CaptureEngine.ingest(request)
    │
    ▼  EVENT_ITEM_CAPTURED
IngestionPipeline
    │
    ▼  EVENT_ITEM_PROCESSED
JobQueue.enqueue() ──► WorkerPool (async)
                         │
                         ▼  ProcessingPipeline.run()
                         │
                         ├── EVENT_ITEM_PROCESSING_STARTED
                         ├── EVENT_ITEM_PROCESSING_COMPLETED
                         └── EVENT_ITEM_PROCESSED (re-published)
                              │
                              ▼
                         StorageManager.save_object()
                              │
                              ▼  EVENT_ITEM_STORED
                              ├── Indexer.index_document()
                              └── VaultGraph.update_node()
```

## Dependency Injection

All services receive dependencies through constructor injection. The `ApplicationContext` is the single point of composition:

```
build_application_context() in src-tauri/src/lib.rs
    │
    ├── EventBus::new()
    ├── ProcessingPipeline::new_no_subscribe(event_bus)
    │   ├── ContentClassifier, DuplicateDetector, ...
    │   └── Registered in CATEGORY_PROCESSORS
    ├── CaptureEngine::new(event_bus)
    │   ├── BrowserCaptureHandler, ArticleCaptureHandler, ...
    │   └── Registered in CATEGORY_CAPTURE_HANDLERS
    ├── JobQueue::new(pipeline, event_bus)
    ├── WorkerPool::new(4, job_queue)
    ├── Indexer::new(path)  →  EVENT_ITEM_STORED subscriber
    ├── VaultGraph::with_storage(path)  →  EVENT_ITEM_STORED subscriber
    └── StorageManager::new(vault_path, event_bus)  →  EVENT_ITEM_PROCESSED subscriber
```

## Plugin Infrastructure (v0)

The plugin foundation is prepared but **no third-party plugin loading is implemented**.

### Components

- **PluginManifest** — Describes a plugin's identity, version, capabilities, dependencies, permissions
- **CapabilityRegistry** — Central index of all capabilities (built-in + future plugins)
- **DependencyGraph** — Cycle detection and topological sort for dependency resolution
- **VersionNegotiation** — Semantic versioning with `^`, `~`, `>=`, `=`, `*` operators
- **LifecycleHooks** — 6-stage lifecycle: discovered → registered → initialized → started → stopped → unloaded
- **FeatureFlags** — Runtime feature toggles with observation callbacks

### Standard Capability IDs

| Capability | Description |
|------------|-------------|
| `nabu:event_bus` | Typed event communication bus |
| `nabu:storage` | File-based object storage with markdown persistence |
| `nabu:capture` | Knowledge capture from clipboard, files, bookmarks |
| `nabu:processor` | Processing pipeline |
| `nabu:graph` | Knowledge relationship graph |
| `nabu:export` | Knowledge export to various formats |
| `nabu:search` | Tantivy-based full-text search |
| `nabu:import` | Knowledge import from external sources |
| `nabu:content_provider` | URL and API content fetching |
| `nabu:theme` | UI theme management |

## Lifecycle

```
Created
    │  ApplicationContext created, no services initialized
    ▼
Initialized
    │  All required services validated and registered
    │  Core services functional
    ▼
Running
    │  Application fully operational
    │  Workers processing jobs, event bus active
    ▼
Shutdown
    │  Resources released, workers stopped
```

## Platform Readiness

### Supported Now
- ✅ Single AI provider (via EventBus subscription)
- ✅ Multiple capture handlers (6 built-in)
- ✅ Multiple processing processors (10 built-in)
- ✅ EventBus publish/subscribe
- ✅ Async job queue with worker pool
- ✅ SQLite persistence
- ✅ Tantivy search
- ✅ Petgraph relationship graph
- ✅ Diagnostics with structured tracing
- ✅ Service registry with dependency injection
- ✅ Plugin infrastructure (metadata only)
- ✅ Capability registry
- ✅ Lifecycle management

### Infrastructure Ready, Not Implemented
- 🔲 Multiple AI providers — CapabilityRegistry supports discovery
- 🔲 Multiple OCR engines — CapabilityRegistry supports `nabu:ocr`
- 🔲 Multiple embedding engines — CapabilityRegistry supports `nabu:embedding`
- 🔲 Plugin loading — PluginManifest, DependencyGraph, VersionNegotiation ready
- 🔲 Plugin sandboxing — Infrastructure prepared
- 🔲 Distributed processing — JobQueue architecture supports future remote workers
- 🔲 Export engine — ExportEngine module declared, not populated
- 🔲 Template manager — TemplateManager module declared in src-tauri

### Architectural Boundaries

```
crate boundaries:
    nabu-core          →  Core library (models, capture, processing, storage,
                           event_bus, registry, plugin, graph, indexer, job_queue)
    src-tauri           →  Tauri application shell (commands, settings, vault,
                           native_messaging, export_engine, template_manager)
    nabu-ui             →  Leptos-based UI components (tree, markdown renderer)

ownership:
    No circular crate dependencies
    nabu-core has no dependency on src-tauri or nabu-ui
    src-tauri depends on nabu-core
    nabu-ui depends on nabu-core
```
