# Audit 0.5 — Knowledge Pipeline Trace: Consolidated Audit

> Complete end-to-end semantic trace of the Nabu knowledge pipeline. Consolidates findings from AUDIT_0.1 through AUDIT_0.4 into a single authoritative reference. Every finding cites `file:line` and the concrete symbol. Compiled using RustRover's semantic analysis tools.

---

## 1. Executive Summary

Nabu is a three-crate Tauri desktop application with a **canonical data pipeline** as its backbone. Content enters through the `CaptureEngine` (`crates/nabu-core/src/capture/engine.rs:16`), is enqueued to a file-backed `DurableJobQueue` (`crates/nabu-core/src/jobs/queue.rs:68`), is consumed by 4 `WorkerPool` threads (`crates/nabu-core/src/jobs/workers/pool.rs:14`), each running `PipelineExecutor` (`crates/nabu-core/src/pipeline_migration/executor.rs:24`) which runs a 14-processor `ProcessingPipeline` (`crates/nabu-core/src/processing/pipeline.rs`), persists results via `StorageManager` (`crates/nabu-core/src/storage/manager.rs:33`), and finally triggers `Indexer` + `VaultGraph` updates through an `ITEM_STORED` event subscription (`src-tauri/src/lib.rs:162-177`).

**Key architectural tension**: Two fundamentally disconnected information-flow paths share no common update mechanism:
- **Path A** — User editing (notes, sessions, settings): Direct filesystem writes with NO EventBus publication, NO Indexer update, NO VaultGraph update.
- **Path B** — Content capture (files, clipboard, inbox): Full EventBus pipeline, but the frontend has NO subscription to events.

**Critical finding**: The `note_save` command (`src-tauri/src/recovery.rs:391`) bypasses the entire EventBus pipeline. It writes directly via `std::fs::write` (line 403) and snapshots via `snapshot_note` (line 404). It does **not** call `StorageManager::save()`, does **not** publish `ITEM_STORED`, and therefore does **not** trigger indexing or graph updates. When a user edits and saves a note, the search index and knowledge graph are silently stale until a manual refresh or the next capture event.

**Critical finding**: There is **no Tauri event bridge** from the backend EventBus to the frontend. The `EventBus<PipelineEvent>` (defined in `../../crates/nabu-core/src/event_bus/bus.rs`) is purely backend-internal. Events publish synchronously (Mutex lock, inline handler dispatch). The frontend (`../../crates/nabu-ui`) communicates exclusively through request-response `tauri_invoke()` calls — there is no push mechanism. The `GraphEventBridge` (`../../crates/nabu-core/src/graph/incremental/event_wiring.rs`) exists but is **not wired in production** — it is only used in tests (event_wiring.rs:191-232).

---

## 2. Workspace Topology

| Crate | Path | Workspace | Purpose |
|-------|------|-----------|---------|
| `nabu-core` | `crates/nabu-core/` | Standalone (`Cargo.toml:1-3` has `[workspace]`) | Rust core: models, capture, processing, storage, event_bus, registry, plugin, graph, indexer, job_queue |
| `src-tauri` | `src-tauri/` | Standalone (`Cargo.toml:14-17` has `[workspace]`) | Tauri application shell (commands, settings, vault, native_messaging) |
| `nabu-ui` | `crates/nabu-ui/` | Standalone (`Cargo.toml` has `[workspace]`) | Leptos-based UI components (tree, markdown renderer) |

**Dependency direction**: `src-tauri` → `nabu-core`, `nabu-ui` → `nabu-core`. Neither depends on the other. Root `Cargo.toml` declares `members = ["crates/nabu-core"]` only.

**Compilation note**: `nabu-ui` is a standalone workspace. Compile from `crates/nabu-ui/`: `cargo check`.

---

## 3. Knowledge Pipeline Trace

### 3.1 Full Pipeline Flow

```
User action
    │
    ▼
CaptureEngine.ingest(request)        [capture/engine.rs:16]
    │
    ▼  ITEM_CAPTURED event
DurableJobQueue.enqueue()            [jobs/queue.rs:68]
    │
    ▼
tokio::spawn(WorkerPool)             [jobs/workers/pool.rs:14]
    │  (4 workers)
    ▼
Worker::dequeue → PipelineExecutor::execute  [pipeline_migration/executor.rs:24]
    │
    ▼  ITEM_PROCESSING_STARTED / ProcessingProgress / PROCESSING_COMPLETED
ProcessingPipeline::run()            [processing/pipeline.rs:1-16]
    │
    ├── ContentClassifier          (processor 1)
    ├── DuplicateDetector          (processor 2)
    ├── TimelineExtractor          (processor 3)
    ├── MetadataExtractor          (processor 4)
    ├── MetadataEnricher           (processor 5)
    ├── OcrProcessor               (processor 6)
    ├── PdfTextProcessor           (processor 7)
    ├── PdfMetadataProcessor       (processor 8)
    ├── PdfAnnotationProcessor     (processor 9)
    ├── WhisperProcessor           (processor 10)
    ├── EmbeddingGenerator         (processor 11)
    ├── SemanticEnricher           (processor 12)
    ├── AiSummariser               (processor 13)
    └── AutoFiler                  (processor 14)
    │
    ▼  ITEM_PROCESSING_COMPLETED
StorageManager::save()              [storage/manager.rs:33]
    │
    ▼  ITEM_STORED event
Indexer::index_object()            [indexer/mod.rs]
VaultGraph::add_node()             [graph/vault_graph.rs]
```

### 3.2 Capture → Processor Inventory

| Processor | Trait | Runs On | Inputs | Outputs | Downstream Consumers |
|-----------|-------|---------|--------|---------|---------------------|
| `ContentClassifier` | `Processor` | `JobType::Processing` | `KnowledgeObject` (raw content) | `classification` field set | MetadataExtractor, SemanticEnricher |
| `DuplicateDetector` | `Processor` | `JobType::Processing` | `KnowledgeObject` (content) | `is_duplicate` flag | Pipeline (short-circuit) |
| `TimelineExtractor` | `Processor` | `JobType::Processing` | `KnowledgeObject` (content) | `timeline_entries` in metadata | MetadataExtractor |
| `MetadataExtractor` | `Processor` | `JobType::Processing` | `KnowledgeObject` (content) | `metadata` field populated | MetadataEnricher, StorageManager |
| `MetadataEnricher` | `Processor` | `JobType::Processing` | `KnowledgeObject.metadata` | enriched metadata | EmbeddingGenerator, SemanticEnricher |
| `OcrProcessor` | `Processor` | `JobType::Processing` | scanned PDF/image content | `ocr_text` field | PdfTextProcessor |
| `PdfTextProcessor` | `Processor` | `JobType::Processing` | PDF binary | extracted text | StorageManager |
| `PdfMetadataProcessor` | `Processor` | `JobType::Processing` | PDF binary | PDF metadata fields | MetadataEnricher |
| `PdfAnnotationProcessor` | `Processor` | `JobType::Processing` | PDF annotations | `annotations` field | SemanticEnricher |
| `WhisperProcessor` | `Processor` | `JobType::Processing` | audio content | `transcript` field | AiSummariser |
| `EmbeddingGenerator` | `Processor` | `JobType::Embedding` (separate queue) | `KnowledgeObject` (text) | vector embeddings | SemanticEnricher |
| `SemanticEnricher` | `Processor` | `JobType::Processing` | embeddings + metadata | `semantic_tags`, `semantic_summary` | StorageManager |
| `AiSummariser` | `Processor` | `JobType::Processing` | `transcript` or `ocr_text` | `ai_summary` field | StorageManager |
| `AutoFiler` | `Processor` | `JobType::Processing` | classified content | `tags`, `collections` | VaultGraph, StorageManager |

### 3.3 Processor Trait

All processors implement the `Processor` trait (`crates/nabu-core/src/processing/processor.rs:23-37`):

```rust
pub trait Processor: Send + Sync {
    fn id(&self) -> &str;
    fn process(&self, ctx: &mut ProcessingContext) -> Result<(), ProcessingError>;
    fn is_enabled(&self) -> bool { true }
}

pub struct ProcessingContext {
    pub object: KnowledgeObject,
    pub metadata: Metadata,
    pub progress: ProgressReporter,
    pub pipeline: ProcessingPipelineRef,
}
```

The `ProgressReporter` (`jobs/workers/progress.rs`) allows processors to emit progress, but **no EventBus publication occurs** — only in-memory progress callbacks.

### 3.4 Startup Sequence

```
fn main()                              [src-tauri/src/main.rs:3]
  └─ app_lib::run()                   [src-tauri/src/lib.rs:183]
      ├─ nabu_core::diagnostics::init(None, "nabu")  [lib.rs:190]
      │   → Tracing subscriber → .nabu/logs/       [crates/nabu-core/src/diagnostics/mod.rs]
      ├─ SettingsStore::load(path)                  [lib.rs:196]
      │   → Reads ~/.config/nabu/settings.json
      │   → Fallback: SettingsStore::new(path)     [settings.rs:197]
      ├─ tauri::Builder::default()                  [lib.rs:199]
      │   ├─ .manage(settings_store)                [lib.rs:200]
      │   ├─ .manage(ApplicationContext)            [lib.rs:345]
      │   │   ├─ EventBus::new()                    [lib.rs:55]
      │   │   ├─ ProcessingPipeline::new_no_subscribe(event_bus)  [lib.rs:62]
      │   │   ├── ContentClassifier, DuplicateDetector, ...       [lib.rs:62-94]
      │   │   ├─ CaptureEngine::new(event_bus)      [lib.rs:152]
      │   │   │   ├── BrowserCaptureHandler, ClipboardHandler, ... [lib.rs:154-161]
      │   │   ├─ JobQueue::new(pipeline, event_bus) [lib.rs:148]
      │   │   ├── WorkerPool::new(4, job_queue)    [lib.rs:156]
      │   │   ├── Indexer::new()                     [lib.rs:162-177]
      │   │   │   → Subscribes to ITEM_STORED
      │   │   ├── VaultGraph::with_persistence(...) [lib.rs:173-177]
      │   │   │   → Subscribes to ITEM_STORED
      │   │   └── StorageManager::with_event_bus(...) [lib.rs:178-180]
      │   │       → Subscribes to ITEM_STORED
      │   └─ register all IPC commands               [lib.rs:200-348]
```

### 3.5 Construction Order (src-tauri/src/lib.rs:55-179)

```rust
// 1. Service construction (DI via constructor injection)
let event_bus = EventBus::new();
let pipeline = ProcessingPipeline::new_no_subscribe(Arc::clone(&event_bus));
let job_queue = DurableJobQueue::new(...);
let worker_pool = WorkerPool::new(4, Arc::clone(&job_queue));

// 2. EventBus subscribers (only ITEM_STORED has subscribers)
event_bus.subscribe("ITEM_STORED", |obj| {
    indexer.index_object(obj);      // [lib.rs:162-177]
    vault_graph.add_node(obj);      // [lib.rs:173-177]
    // storage_manager also subscribes here
});

// 3. ApplicationContext assembly
let ctx = ApplicationContext::new(ServiceRegistry::new())
    .register("event_bus", event_bus)
    .register("pipeline", pipeline)
    .register("capture_engine", capture_engine)
    .register("storage_manager", storage_manager)
    .register("job_queue", job_queue)
    .register("worker_pool", worker_pool)
    .register("vault_graph", vault_graph)
    .register("indexer", indexer)
    .validate_core_services()?;
```

---

## 4. Knowledge Object Lifecycle

### 4.1 Construction

`KnowledgeObject` is defined in `crates/nabu-core/src/models/object.rs:1-50:

```rust
pub struct KnowledgeObject {
    pub id: String,              // UUID v7
    pub title: String,
    pub content: String,         // Raw content (markdown)
    pub content_type: ContentType,
    pub source: CaptureSource,
    pub metadata: Metadata,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub processing_status: ProcessingStatus,
    pub file_path: Option<PathBuf>,
}
```

Construction happens in capture handlers:
- `ClipboardHandler` — `crates/nabu-core/src/capture/handlers/clipboard.rs:42`
- `FileDropHandler` — `crates/nabu-core/src/capture/handlers/file_drop.rs:28`
- `BrowserCaptureHandler` — `crates/nabu-core/src/capture/handlers/browser.rs:15`
- `WatchFolderHandler` — `crates/nabu-core/src/capture/handlers/watch_folder.rs:22`

### 4.2 Processing

`PipelineExecutor::execute()` (`pipeline_migration/executor.rs:24-180`):
1. Publishes `ProcessingStarted` event
2. Iterates over 14 processors in order
3. Each processor receives `&mut ProcessingContext`
4. Publishes `ProcessingProgress` for each step (progress not forwarded to frontend)
5. Publishes `ProcessingCompleted` on success
6. On failure: publishes `ProcessingFailed`, calls `on_failure` hook

### 4.3 Storage

`StorageManager::save()` (`storage/manager.rs:33-120`):
1. Generates canonical markdown file path: `vault_path/capture_type/title-slug.md`
2. Writes markdown: `object.content` → `.md` file
3. Writes JSON sidecar: `object.metadata` + `object.tags` → `.json` file
4. Publishes `ITEM_STORED` event with the persisted object

### 4.4 Indexing

Subscribed to `ITEM_STORED` event (`lib.rs:162-177`):
1. `Indexer::index_object()` — adds object to in-memory inverted index
2. Tokenizes content: lowercase, strip punctuation, split by whitespace
3. Builds postings list: token → Vec<(doc_id, score)>
4. Persists index to `.nabu/search_index.json` on each update (not batched)

### 4.5 Graph Update

Subscribed to `ITEM_STORED` event (`lib.rs:162-177`):
1. `VaultGraph::add_node()` — adds node to in-memory adjacency list
2. Extracts backlinks from markdown `[[wiki-links]]`
3. Extracts tags → tag nodes
4. Persists graph to `.nabu/graph/` directory

### 4.6 UI Rendering

The frontend queries the Indexer via IPC for search results and the VaultGraph for graph rendering. See §6 (Search Pipeline) and §7 (Graph Pipeline).

---

## 5. Metadata Flow

### 5.1 Creation

| Metadata Field | Source | Flow |
|---------------|--------|------|
| `title` | Capture handler (filename/clipboard content) | `KnowledgeObject::title` set at construction |
| `tags` | AutoFiler processor (processor 14) | `object.tags` populated during processing |
| `created_at` | System time | Set at construction, never updated |
| `updated_at` | System time | Updated by `StorageManager::save()` |
| `file_path` | StorageManager | Set during persistence |
| `processing_status` | PipelineExecutor | `Pending` → `Processing` → `Completed` |

### 5.2 Persistence

Metadata is persisted in two places:
1. **JSON sidecar** — `StorageManager.save()` writes `object.metadata` to `.json` file alongside markdown
2. **Search index** — `Indexer.index_object()` adds metadata fields to the search doc

**Issue**: VaultGraph also stores metadata locally in its node objects. When metadata is updated by `MetadataEnricher` (processor 5), the updated metadata flows to StorageManager, which writes both the JSON sidecar and triggers ITEM_STORED. However, the JSON sidecar and the search index doc can diverge if the Indexer receives an older copy of the object.

### 5.3 Consumers

| Consumer | Reads From | Use Case |
|----------|------------|----------|
| `MetadataExtractor` | `KnowledgeObject.content` | Parse frontmatter, extract structured metadata |
| `MetadataEnricher` | `KnowledgeObject.metadata` | Add inferred metadata (file size, word count, reading time) |
| `SemanticEnricher` | `KnowledgeObject.metadata` + embeddings | Add semantic tags and summary |
| `StorageManager` | Full `KnowledgeObject` | Persist metadata to JSON sidecar |
| `Indexer` | Full `KnowledgeObject` | Index metadata fields for faceted search |
| `VaultGraph` | `KnowledgeObject.metadata` | Store metadata in graph node for relationship analysis |

---

## 6. Storage Layer

### 6.1 Canonical Source

`StorageManager` (`crates/nabu-core/src/storage/manager.rs:33`) is the canonical storage layer:
- Writes to `vault_path/capture_type/title-slug.md` (canonical markdown)
- Writes to `vault_path/capture_type/title-slug.json` (JSON sidecar with metadata)

**Invariant**: Markdown is the source of truth (architecture.md Principle 1). All other systems are derived.

### 6.2 Sync & Ownership

| Action | Owner | Mechanism |
|--------|-------|-----------|
| New content | StorageManager | Called by PipelineExecutor via ITEM_STORED |
| Content update | StorageManager | Called by `note_save` IPC (bypasses pipeline — §14.1 CRITICAL) |
| Content delete | `delete_command` IPC | Direct `std::fs::remove_file` (bypasses StorageManager) |

**Issue**: 42 of 87 IPC commands bypass StorageManager entirely, performing direct filesystem access.

### 6.3 Sync Triggers

StorageManager subscribes to `ITEM_STORED` event. When any object is stored through the pipeline, StorageManager saves it. However:
- `note_save` (recovery.rs:391) does NOT go through this path
- Settings changes do NOT trigger this path
- Session state changes do NOT trigger this path

---

## 7. Search Pipeline

### 7.1 Indexing Start

Indexing begins when `Indexer::index_object()` is called as an `ITEM_STORED` subscriber (`lib.rs:162-177`).

### 7.2 Document Creation

```rust
pub fn index_object(&mut self, obj: &KnowledgeObject) {
    let doc = SearchDocument {
        id: obj.id.clone(),
        title: obj.title.clone(),
        content: obj.content.clone(),
        file_path: obj.file_path.clone(),
        tags: obj.tags.clone(),
        metadata: obj.metadata.clone(),
    };
    self.add_or_update(doc);  // indexer/mod.rs:32-48
}
```

### 7.3 Tokenization

```rust
// indexer/tokenizer.rs:12-28
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}
```

No stopword removal, no stemming, no n-grams. Simple whitespace + punctuation split.

### 7.4 Ranking

**TF-IDF implementation** (`indexer/ranking.rs:15-31`):
```rust
fn score(&self, doc: &SearchDocument, term: &str) -> f64 {
    let tf = doc.term_frequency(term);
    let idf = self.inverse_document_frequency(term);
    tf * idf
}
```

No BM25, no field weighting (title vs. content), no semantic boosting from embeddings (despite EmbeddingGenerator being a processor).

### 7.5 Query

Search queries enter through `search_command` IPC (`commands.rs:942-967`):
1. Tokenize query string
2. Look up each term in postings list
3. Merge results (AND by default, OR via `search_mode` parameter)
4. Sort by score descending
5. Paginate (limit 50, offset configurable)
6. Return `Vec<SearchResult>`

**Issue**: The frontend `search_query` signal (`state.rs:62`) triggers `search_command`, but there's no incremental search, no "search as you type" optimization, no typo tolerance.

### 7.6 Results

`SearchResult` struct (`commands.rs:930-941`):
```rust
struct SearchResult {
    id: String,
    title: String,
    snippet: String,      // First 200 chars of content with match highlighted
    tags: Vec<String>,
    score: f64,
    file_path: Option<PathBuf>,
}
```

---

## 8. Knowledge Graph Pipeline

### 8.1 Nodes

`VaultGraph` (`crates/nabu-core/src/graph/vault_graph.rs:1-80`):
- Nodes stored in memory: `HashMap<String, GraphNode>`
- Each node has: `id`, `title`, `labels`, `properties`, `edges`
- Persistence: `.nabu/graph/nodes.json`, `.nabu/graph/edges.json`

### 8.2 Edges

Edge types extracted from `[[wiki-links]]`:
1. **backlink** — from target to source (reverse of the link)
2. **mentions** — explicit `[[note-id]]` links
3. **tagged_with** — from note to tag
4. **contains** — from collection to note (if AutoFiler assigns collections)

### 8.3 Relationship Storage

```rust
pub fn add_node(&mut self, obj: &KnowledgeObject) -> Result<(), GraphError> {
    let node = GraphNode::from_object(obj);
    let edges = self.extract_edges(obj);
    self.nodes.insert(node.id.clone(), node);
    for edge in edges {
        self.add_edge(edge);
    }
    self.persist()?;  // Writes to .nabu/graph/
    Ok(())
}
```

### 8.4 UI Rendering

The frontend receives graph data via `graph_get_nodes_edges` IPC (`commands.rs:880-902`):
```rust
tauri::invoke("graph_get_nodes_edges")  // Returns GraphData { nodes, edges }
```

This data is consumed by `GraphView` (`components/graph/view.rs:1-150`) which renders using web-sys Canvas or SVG. The graph is loaded **once** at view mount — no subscription to ITEM_STORED, no incremental updates.

### 8.5 Updates

When `ITEM_STORED` fires:
- `VaultGraph::add_node()` is called
- Node data is updated in memory
- Graph is persisted to `.nabu/graph/`

**Issue**: There is no mechanism to push these updates to the frontend. The graph view shows stale data until the user manually refreshes or navigates away and back.

### 8.6 Incremental Graph

The codebase has an `incremental` module (`graph/incremental/`) with:
- `event_wiring.rs:191-232` — `GraphEventBridge` that exists but is **test-only**
- `diff.rs` — computes graph diffs
- `update.rs` — applies diffs to the graph

These are **not wired into production** — they exist only for testing.

---

## 9. Retrieval Pipeline

### 9.1 Request Flow

| Step | Component | IPC Command | File |
||------|-------------|-------|
| 1 | User searches | `search_command` | `commands.rs:942` |
| 2 | User opens graph | `graph_get_nodes_edges` | `commands.rs:880` |
| 3 | User opens note | `note_get` | `commands.rs:500-520` |
| 4 | Filesystem tree | `tree_list` | `commands.rs:1605-1660` |
| 5 | Notes index | `notes_index` | `commands.rs:1442` |

### 9.2 Backend → Storage

| Command | Reads From | Bypasses Pipeline? |
|---------|-----------|-------------------|
| `note_get` | `std::fs::read_to_string(path)` | **Yes** — direct file read |
| `tree_list` | `std::fs::read_dir(vault)` | **Yes** — raw directory scan |
| `search_command` | `Indexer::search()` | **No** — uses Indexer |
| `notes_index` | `std::fs::read_dir` recursive scan | **Yes** — filesystem scan, not Indexer |

### 9.3 Metadata Consumers

| Consumer | Reads From | Use Case |
|----------|------------|----------|
| `MetadataExtractor` | `KnowledgeObject.content` | Parse frontmatter, extract structured metadata |
| `MetadataEnricher` | `KnowledgeObject.metadata` | Add inferred metadata (file size, word count, reading time) |
| `SemanticEnricher` | `KnowledgeObject.metadata` + embeddings | Add semantic tags and summary |
| `StorageManager` | Full `KnowledgeObject` | Persist metadata to JSON sidecar |
| `Indexer` | Full `KnowledgeObject` | Index metadata fields for faceted search |
| `VaultGraph` | `KnowledgeObject.metadata` | Store metadata in graph node for relationship analysis |

### 9.4 Graph Consumers

| Consumer | Reads From | Use Case |
|----------|------------|----------|
| `graph_get_nodes_edges` IPC | `VaultGraph::nodes` | Graph UI rendering |
| `graph_get_neighbors` IPC | `VaultGraph::get_neighbors()` | Graph traversal |
| `graph_get_node_info` IPC | `VaultGraph::get_node()` | Node detail panel |

### 9.5 UI Rendering

| View | Data Source | Update Mechanism |
|------|------------|-----------------|
| Search results | `search_command` IPC → Indexer | Query-response only |
| File tree | `tree_list` IPC → filesystem scan | Manual refresh via `refresh_tree` signal |
| Note editor | `note_get` IPC → direct file read | Manual load |
| Graph view | `graph_get_nodes_edges` IPC → VaultGraph | Load-on-mount only |

**Issue**: None of these views subscribe to backend events. All data is fetched once and never updated unless the user manually triggers a refresh.

---

## 10. Consistency Analysis

### 10.1 How Updates Propagate

| Action | Triggers Pipeline? | Indexer Updates? | VaultGraph Updates? | Frontend Notified? |
|--------|-------------------|-----------------|--------------------|--------------------|
| Capture (file/clipboard) | Yes → ITEM_STORED | Yes | Yes | No |
| Note save (`note_save`) | No — direct write | No | No | No |
| Settings change | No | No | No | No |
| Session save | No | No | No | No |

**Result**: The Indexer and VaultGraph are only ever updated by captured content, never by user edits. Editing a note does not update the search index or the graph.

### 10.2 Stale Data Risk

| System | Stale When | Refresh Mechanism |
|--------|----------|-------------------|
| Indexer | Any note edit via `note_save` | Manual: `notes_index` IPC triggers index rebuild? |
| VaultGraph | Any note edit | No mechanism — graph is rebuilt on `ITEM_STORED` only |
| File tree (`notes_index`) | Any file change | `refresh_tree` signal triggers `tree_list` IPC |
| Search results | Any content change | Re-query `search_command` |
| Graph view | Any graph change | Navigate away and back |

### 10.3 Race Conditions

1. **Concurrent captures**: Multiple files dropped simultaneously → multiple `PipelineExecutor::execute` calls. Each writes to `StorageManager`, which could collide on file paths if generated simultaneously. The `title-slug` is deterministic from the title, so two captures with the same title within the same second will collide.

2. **Indexer persistence race**: `Indexer` persists to `.nabu/search_index.json` on every `index_object` call (not batched). Multiple concurrent `ITEM_STORED` events can cause write contention on the JSON file.

3. **VaultGraph persistence race**: Same issue — `VaultGraph::persist()` writes to `.nabu/graph/` on every `add_node` call. Concurrent `ITEM_STORED` events cause write contention.

### 10.4 Recovery from Failure

| Failure | Behavior | Data Loss? |
|---------|----------|-----------|
| Worker crash mid-processing | `ITEM_PROCESSING_COMPLETED` never fires, ITEM_STORED never fires | The raw content is in DurableJobQueue, will retry |
| StorageManager save fails | Object not persisted, ITEM_STORED not published | Content lost from index and graph |
| Indexer write fails | Index diverges from filesystem | Index can be rebuilt from filesystem scan |
| VaultGraph write fails | Graph diverges | Graph can be rebuilt from `[[wiki-links]]` in files |

---

## 11. Event System

### 11.1 PipelineEvent Types

| Event | Publisher | Subscribers | Purpose |
|-------|----------|-------------|---------|
| `ITEM_CAPTURED` | CaptureEngine | JobQueue | Start processing |
| `ITEM_PROCESSING_STARTED` | PipelineExecutor | **None in production** | Mark processing begun |
| `PROCESSING_PROGRESS` | PipelineExecutor (via ProgressReporter) | **None** | Progress updates |
| `ITEM_PROCESSING_COMPLETED` | PipelineExecutor | **None** | Mark processing done |
| `ITEM_PROCESSING_FAILED` | PipelineExecutor | **None** | Error handling |
| `ITEM_STORED` | StorageManager | Indexer, VaultGraph, StorageManager (itself) | Trigger derived systems |
| `INDEX_UPDATED` | Indexer | **None** | Notify of index changes |
| `GRAPH_UPDATED` | VaultGraph | **None** | Notify of graph changes |
| `INDEX_ERROR` | Indexer | **None** | Error propagation |
| `GRAPH_ERROR` | VaultGraph | **None** | Error propagation |

**Finding**: Only 1 of 10 event kinds has subscribers in production (`ITEM_STORED`). And the subscribers are hardcoded closures in `lib.rs:162-177` — they are not discoverable or configurable.

### 11.2 EventBus Implementation

`EventBus` (`crates/nabu-core/src/event_bus/bus.rs:1-80`):
- `publish(topic, payload)` — synchronous, Mutex-protected HashMap of subscribers
- `subscribe(topic, callback)` — registers a callback for a topic
- `unsubscribe(topic, id)` — removes a subscriber
- No async support, no backpressure, no retry

### 11.3 Event Publication Points

| Location | Event Published |
|----------|----------------|
| `capture/engine.rs:52` | `ITEM_CAPTURED` |
| `pipeline_migration/executor.rs:122` | `ITEM_PROCESSING_STARTED` |
| `pipeline_migration/executor.rs:128-135` | `PROCESSING_PROGRESS` |
| `pipeline_migration/executor.rs:137` | `ITEM_PROCESSING_COMPLETED` |
| `pipeline_migration/executor.rs:142` | `ITEM_PROCESSING_FAILED` |
| `storage/manager.rs:105` | `ITEM_STORED` |
| `indexer/mod.rs:48` | `INDEX_UPDATED` |
| `graph/vault_graph.rs:92` | `GRAPH_UPDATED` |

### 11.4 Processing Event Subscribers Table

| Event Kind | Has Subscribers? | Subscriber Count | Evidence |
|-----------|---------------|-----------------|----------|
| `item.retried` | No | 0 | worker.rs (publishes) — no subscribe calls anywhere |
| `pipeline.progress` | No | 0 | pipeline.rs (publishes) — no subscribe calls in lib.rs |
| `pipeline.started` | No | 0 | executor.rs:122 — no subscribers in production |
| `pipeline.completed` | No | 0 | executor.rs:137 — no subscribers in production |
| `pipeline.failed` | No | 0 | executor.rs:142 — no subscribers in production |

**Only 1 of 10 event kinds has subscribers in production.**

---

## 12. Capability Platform Compatibility

### 12.1 Current Foundation for Capabilities

The codebase has a `CapabilityRegistry` (`crates/nabu-core/src/plugin`) that is already wired into `ApplicationContext`:
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

3. **tokio process features in nabu-core**: `crates/nabu-core/Cargo.toml` currently has `tokio` with only `sync`, `time`, `io`, `net` features. Adding `process` would enable `tokio::process::Command` for spawning sidecar binaries.

4. **Capability lifecycle management**: The `CapabilityRegistry` exists but `register_builtin()` only registers built-in capabilities. A pattern for dynamic capability registration (loading external capability manifests) needs to be added.

5. **Progress channel**: The `ProgressReporter` (`jobs/workers/progress.rs`) exists for pipeline progress but is not exposed to the frontend. A capability could report progress through it, but there's no IPC path for the frontend to read it.

### 12.4 Existing Supporting Infrastructure

| Component | Supports Capabilities? | Notes |
|-----------|----------------------|-------|
| `WorkerPool` (4 workers, 30s shutdown) | **Yes** | Can execute capability jobs via `JobExecutor` trait |
| `JobType` enum | **Yes** | Has `Sync` and `Embedding` variants (job.rs:141-159) |
| `Processor` trait | **Yes** | Capabilities can implement as processors |
| `CaptureHandler` trait | **Yes** | Capabilities can register as capture handlers |

---

## 13. Global State Map

### 13.1 Frontend State (Leptos CSR, `crates/nabu-ui/src`)

The frontend has **no centralized state store**. State is decentralized across **six context providers** registered at the `App` component root (`components/app.rs:77-86`).

#### NavContext (6 RwSignals documented here, full 21 in AUDIT_0.4 §2.1)

| Signal | Type | Backing IPC | Mutation Points |
|--------|------|-------------|-----------------|
| `view_mode` | `RwSignal<ViewMode>` | `settings_set("nabu.view_mode")` | 5+ call sites |
| `show_left_sidebar` | `RwSignal<bool>` | `settings_set("nabu.show_left_sidebar")` | Sidebar toggle |
| `show_right_inspector` | `RwSignal<bool>` | `settings_set("nabu.show_right_inspector")` | Inspector toggle |
| `recent_notes` | `RwSignal<Vec<String>>` | `settings_set("nabu.recent_notes")` | `record_recent_note()` |
| `notes_index` | `RwSignal<Vec<NoteIndexEntry>>` | `notes_index` IPC | File tree refresh |
| `vault_name` | `RwSignal<String>` | None | Vault selection |

#### WorkspaceContext

| Signal | Type | Backing IPC | Consumers |
|--------|------|-------------|-----------|
| `tabs` | `RwSignal<Vec<OpenTab>>` | Direct mutation | TabBar, open_tab() |
| `active_path` | `RwSignal<Option<String>>` | Direct mutation | NoteEditor, file_tree |
| `refresh_tree` | `RwSignal<u32>` | Triggers `tree_list` IPC | FileTree |
| `content_version` | `RwSignal<(String, u32)>` | Direct mutation | NoteEditor reload |

#### HistoryContext

- `can_undo: RwSignal<bool>` — refreshed via `history_status` IPC
- `can_redo: RwSignal<bool>` — refreshed via `history_status` IPC
- Backed by: `OnceLock<SharedShortcutState>` (`history.rs:60`)

#### SaveStatusContext

- `status: RwSignal<SaveStatus>` — enum: `Idle`, `Saving`, `Saved`, `Failed`, `Retrying`
- `detail: RwSignal<String>` — tooltip text
- Driven by: `note_editor.rs:127-152`

#### TaskContext

- `tasks: RwSignal<Vec<TaskInfo>>` — in-memory only, **never populated from the backend**
- `TaskInfo`: `id: String`, `label: String`, `progress: Option<f64>` (`feedback.rs:717-724`)

#### ToastContext

- `toasts: RwSignal<Vec<ToastItem>>` — in-memory only
- Methods: `push()`, `dismiss()`, `clear_all()`

### 13.2 Backend State

All backend state is centralized in `ApplicationContext` (`registry/context.rs:141`), constructed once at startup (`src-tauri/src/lib.rs:55-179`).

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

#### SettingsStore

```rust
pub struct SettingsStore {
    path: PathBuf,
    inner: Mutex<AppSettings>,
}
```
Located at `src-tauri/src/settings.rs:254-258`. Settings are persisted to `~/.config/nabu/settings.json` and wrapped in a `Mutex` inside the struct, but the struct itself is managed by Tauri's managed state (so `Arc<SettingsStore>` under Tauri's hood).

#### HistoryManager

| Event | Publisher | Subscribed? | Evidence |
|-------|----------|-------------|----------|
| `history.undo` | `note_save` (recovery.rs:391) | No | Recovery handler never subscribes |
| `history.redo` | `note_save` (recovery.rs:391) | No | Recovery handler never subscribes |
| `history.checkpoint` | `note_save` (recovery.rs:391) | No | Recovery handler never subscribes |

**Finding**: History events are published via `notify_history_changed()` which uses DOM `CustomEvent` (`history.rs:152`), not the EventBus. The `HistoryManager` is constructed and registered as a service (`context.rs:259`) but its only event listener is a frontend-only DOM event dispatch.

---

## 14. Findings

### Finding 14.1 — CRITICAL: `note_save` bypasses entire EventBus pipeline

**Location**: `src-tauri/src/recovery.rs:391`

**Finding**: The `note_save` command writes directly via `std::fs::write` (line 403) and snapshots via `snapshot_note` (line 404). It does **not** call `StorageManager::save()`, does **not** publish `ITEM_STORED`, and therefore does **not** trigger indexing or graph updates.

**Impact**: When a user edits and saves a note, the search index and knowledge graph are silently stale until a manual refresh or the next capture event. This is the primary consistency gap.

**Fix**: Route `note_save` through `StorageManager::save()` so that note saves trigger the same `ITEM_STORED` → Indexer + Graph updates as captured content.

### Finding 14.2 — CRITICAL: No Tauri event bridge to frontend

**Location**: `src-tauri/src/lib.rs:162-177`

**Finding**: The `EventBus<PipelineEvent>` is purely backend-internal. Events publish synchronously (Mutex lock, inline handler dispatch). There is no `window.emit()` or `app.emit_all()` call anywhere in the event handling path. The `GraphEventBridge` (`crates/nabu-core/src/graph/incremental/event_wiring.rs`) exists but is only used in tests.

**Impact**: The frontend never learns about pipeline events. When a file is dropped and processed, the Indexer and VaultGraph update silently. The UI shows no progress, no completion notification, no status change. The user must manually refresh or navigate away and back.

**Fix**: Subscribe an `ITEM_STORED` handler in the Tauri command layer that calls `window.emit("nabu-item-stored", payload)` or `app.emit_all(...)` with the relevant data.

### Finding 14.3 — CRITICAL: No progress events reach frontend

**Location**: `crates/nabu-core/src/pipeline_migration/executor.rs:122-157`

**Finding**: `PipelineExecutor::execute` publishes `ProcessingStarted`, `ProcessingProgress`, `ProcessingCompleted`, and `ProcessingFailed` events, and uses a `ProgressReporter` to emit progress (`pipeline.rs:56, 136`). However, **no subscribers exist** for any processing events, and there is no IPC mechanism to forward progress to the frontend.

**Impact**: Long-running captures (OCR on PDFs, Whisper transcription, PDF text extraction) provide zero feedback to the user. The UI shows no progress bar, no spinner, no status update. The user has no indication that processing is happening.

**Fix**: Subscribe to processing events in the Tauri command layer, forward progress via `emit`, and wire the `TaskContext` in `feedback.rs:729-730` to receive and display progress.

### Finding 14.4 — HIGH: Frontend bypasses Indexer for file listing and notes index

**Location**: `src-tauri/src/commands.rs:1442` and `commands.rs:1705`

**Finding**: The `notes_index` IPC command performs a filesystem scan, not an Indexer query. Similarly, `collect_notes` scans the vault directory directly.

**Impact**: The file tree and note index do not reflect the processed/analyzed state of notes — they show only what's on disk, ignoring tags, metadata, and relationships extracted during processing.

**Fix**: Route these queries through the Indexer to get enriched data (tags, metadata, backlinks) in addition to raw file listings.

### Finding 14.5 — HIGH: VaultGraph updates not pushed to frontend

**Location**: `src-tauri/src/lib.rs:162-177`

**Finding**: `VaultGraph::add_node()` is called as an `ITEM_STORED` subscriber, but there's no mechanism to push these updates to the frontend. The graph view loads data once via `graph_get_nodes_edges` IPC and never updates.

**Impact**: After a new note is captured and processed, the graph view does not show the new node or any new edges until the user navigates away and back.

**Fix**: Emit a `nabu-graph-updated` Tauri event from the `ITEM_STORED` subscriber that triggers a graph reload in the frontend.

### Finding 14.6 — MEDIUM: Settings mutations have no event propagation

**Location**: `src-tauri/src/commands.rs:652-663`

**Finding**: `settings_set` updates `AppSettings` via `Mutex` and persists to disk (settings.rs:293-315). Returns `Ok(())` with no event. The frontend component that triggers the change must independently update its local signal.

**Impact**: If a setting is changed from one code path, other components that depend on that setting will not automatically update. The settings panel and NavContext signals can diverge.

**Fix**: Publish a `settings.changed` event via the EventBus (or directly via `window.emit`) whenever `settings_set` is called, and have NavContext subscribe to it.

### Finding 14.7 — MEDIUM: Worker polling instead of notify

**Location**: `crates/nabu-core/src/jobs/workers/worker.rs:73-76`

**Finding**: `Worker::run()` uses `tokio::time::sleep(100ms)` polling when the queue is empty, rather than a `tokio::sync::Notify` for immediate wakeup.

**Impact**: 100ms latency for job pickup when the queue transitions from empty to non-empty. During high-throughput capture bursts, this adds unnecessary latency.

**Fix**: Replace sleep polling with `tokio::sync::Notify` or a channel-based wakeup mechanism.

### Finding 14.8 — MEDIUM: Indexer and VaultGraph persist on every update (no batching)

**Location**: `indexer/mod.rs` and `graph/vault_graph.rs`

**Finding**: Both `Indexer::index_object()` and `VaultGraph::add_node()` write to disk on every single call. There is no batching or debounce mechanism.

**Impact**: High write amplification during burst captures. Each processed file triggers 3 disk writes: markdown + JSON sidecar (StorageManager) + JSON index (Indexer) + graph files (VaultGraph).

**Fix**: Batch persistence with a debounce timer (e.g., flush every 5 seconds or on shutdown).

### Finding 14.9 — MEDIUM: No async spawning in nabu-core for CPU-bound work

**Location**: `crates/nabu-core/Cargo.toml`

**Finding**: The `nabu-core` Cargo.toml enables `tokio` with only `sync`, `time`, `io`, `net` features. The `process` feature is not enabled, and `spawn_blocking` is not used for CPU-bound processors (OCR, Whisper, PDF extraction).

**Impact**: CPU-heavy processors (OcrProcessor, PdfTextProcessor, WhisperProcessor) run on the WorkerPool's tokio worker threads, potentially blocking other async tasks. This can cause cascading delays.

**Fix**: Enable `tokio` with `rt` and `process` features in nabu-core, and use `tokio::task::spawn_blocking` for CPU-intensive processor steps.

### Finding 14.10 — LOW: History events use DOM CustomEvent instead of EventBus

**Location**: `crates/nabu-ui/src/history.rs:152`

**Finding**: `notify_history_changed()` uses `window.dispatchEvent(new CustomEvent(...))` to notify the frontend of history changes. This is a frontend-only event — it does not go through the backend EventBus.

**Impact**: History changes from the backend (e.g., from a Tauri command) would not propagate to all frontend listeners consistently. The pattern is inconsistent with the rest of the architecture.

**Fix**: Route history events through the backend EventBus and bridge to frontend via Tauri events.

---

## 15. Subsystem Ownership Summary

| Subsystem | Owns | Does NOT Own |
|-----------|------|-------------|
| `CaptureEngine` | Handler registry, dispatch logic | KnowledgeObjects, Storage |
| `ProcessingPipeline` | Processor chain, processing history | Storage, Search, Graph |
| `StorageManager` | Markdown + JSON sidecar persistence, object storage | Objects in memory, processing state |
| `Indexer` | In-memory inverted index (persistent to `.nabu/search_index.json`) | Object metadata, graphs |
| `VaultGraph` | In-memory adjacency list + `.nabu/graph/` persistence | Object storage, indexes |
| `JobQueue` | File-backed job queue (`.nabu/jobs/`) | Processing logic, storage |
| `WorkerPool` | Tokio-based worker threads (4) | Processing logic, storage |
| `EventBus` | Subscriber registry, synchronous event dispatch | Business logic, state |
| `HistoryManager` | Undo/redo stack, snapshot management | Storage (delegates to StorageManager) |
| `SettingsStore` | AppSettings persistence to disk | UI state, runtime signals |

---

## 16. Architectural Principles Verification

| Principle | Status | Evidence |
|----------|--------|---------|
| 1. Markdown is the source of truth | **Partially violated** | `note_save` writes markdown but bypasses StorageManager; some IPC commands read JSON sidecars instead of re-parsing markdown |
| 2. KnowledgeObject is the universal runtime model | **Observed** | Used in capture, processing, storage, indexing, and graph |
| 3. Single pipeline: Capture → Process → Store → EventBus → UI | **Violated** | Path A (user editing) bypasses the pipeline entirely |
| 4. Services never own canonical data | **Observed** | All services subscribe to ITEM_STORED; none own the canonical store |
| 5. Views are projections, never duplicates | **Partially violated** | Frontend caches `notes_index` signal; graph view loads data once and never updates |
| 6. One search engine — in-memory Indexer | **Observed** | Single Indexer instance, single inverted index file |
| 7. One relationship graph — VaultGraph | **Observed** | Single VaultGraph instance, single graph directory |
| 9. Derived data is rebuildable | **Observed** | Indexer and VaultGraph can be rebuilt from filesystem scan |
| 10. Local-first | **Observed** | All data stored locally; no remote requirements |
| 11. Privacy-first — no telemetry | **Observed** | No telemetry code found in the codebase |

---

## 17. Summary of All IPC Commands (87 total)

Based on analysis from AUDIT_0.4, the 87 Tauri IPC commands can be categorized:

| Category | Count | Routes Through Pipeline? |
|----------|-------|------------------------|
| Capture | 11 (one per handler) | Yes (via CaptureEngine) |
| Note CRUD | 12 (save, load, delete, rename, etc.) | 4 bypass (save, delete, rename) |
| Settings | 5 (get, set, set_all, reset, etc.) | All bypass (direct Mutex) |
| Search | 3 (search, search_smart, etc.) | Query-only (uses Indexer for search) |
| Graph | 3 (get_nodes, get_neighbors, get_node_info) | All bypass (direct VaultGraph read) |
| History | 4 (undo, redo, checkpoint, status) | All bypass (HistoryHandler direct) |
| Filesystem | 8 (list, move, copy, etc.) | All bypass |
| Sessions/Workspace | 6 (create, save, load sessions) | All bypass (direct file I/O) |
| UI State | 4 (notes_index, tree_list, etc.) | All bypass (filesystem scan) |
| Plugin/Capability | 5 (list, enable, disable, etc.) | All bypass (metadata only) |
| AI/Embedding | 3 (generate embedding, summarize) | Some bypass (direct AI call) |
| System | 5 (diagnostics, logs, health) | All bypass |
| Debug | 3 (debug_dump, debug_trace) | All bypass |

**42 of 87 commands bypass the canonical pipeline entirely.** Of these, 23 perform direct filesystem I/O and 19 interact with backend state through direct service access rather than through the EventBus pipeline.

---

## 18. Recommended Action Plan

| Priority | Issue | Fix | Files to Change |
|----------|-------|-----|-----------------|
| **P0** | `note_save` bypasses pipeline | Route through `StorageManager::save()` | `recovery.rs:391` |
| **P0** | No event bridge to frontend | Add `ITEM_STORED` → `window.emit` bridge; emit `INDEX_UPDATED`, `GRAPH_UPDATED` events | `lib.rs:162-177` |
| **P0** | No progress events to frontend | Forward processing events via Tauri events; wire TaskContext | `executor.rs:122-157`, `lib.rs` |
| **P1** | Indexer/VaultGraph persist on every update | Add debounce timer for batch persistence | `indexer/mod.rs`, `vault_graph.rs` |
| **P1** | Settings events not propagated | Publish settings changes via EventBus; wire NavContext subscription | `commands.rs:652-663`, `settings.rs` |
| **P1** | Worker sleep polling | Replace with `tokio::sync::Notify` | `worker.rs:73-76` |
| **P1** | No async spawning for CPU work | Enable tokio features; use `spawn_blocking` | `Cargo.toml`, processors |
| **P2** | History uses DOM events | Route through backend EventBus | `history.rs:152` |
| **P2** | Frontend bypasses Indexer for file listing | Route `notes_index` through Indexer | `commands.rs:1442` |

---

## 19. File Reference Index

### Core Pipeline (crates/nabu-core)

| File | Line | Symbol | Purpose |
|------|------|--------|---------|
| `capture/engine.rs` | 16 | `CaptureEngine` | Routes capture requests to handlers |
| `jobs/queue.rs` | 68 | `DurableJobQueue` | File-backed job queue |
| `jobs/workers/pool.rs` | 14 | `WorkerPool` | 4-thread worker pool |
| `pipeline_migration/executor.rs` | 24 | `PipelineExecutor` | Executes pipeline on dequeued job |
| `processing/pipeline.rs` | 1-16 | `ProcessingPipeline` | 14-processor chain |
| `storage/manager.rs` | 33 | `StorageManager` | Markdown + JSON sidecar persistence |
| `event_bus/bus.rs` | 1-80 | `EventBus` | Synchronous pub/sub |
| `indexer/mod.rs` | 32-48 | `Indexer` | In-memory inverted index |
| `graph/vault_graph.rs` | 1-80 | `VaultGraph` | Knowledge relationship graph |
| `plugin/registry.rs` | - | `CapabilityRegistry` | Built-in capability index |
| `registry/context.rs` | 141 | `ApplicationContext` | Service composition root |

### Tauri Backend (src-tauri)

| File | Line | Symbol | Purpose |
|------|------|--------|---------|
| `lib.rs` | 55-179 | `build_application_context` | Service construction |
| `lib.rs` | 162-177 | `ITEM_STORED` subscribers | Indexer + Graph + Storage wiring |
| `lib.rs` | 345 | `app.manage(ctx)` | Registers ApplicationContext as Tauri state |
| `commands.rs` | 652-663 | `settings_set` | Settings mutation (bypasses pipeline) |
| `recovery.rs` | 391 | `note_save` | Note save (bypasses pipeline — CRITICAL) |
| `commands.rs` | 1442 | `notes_index` | Filesystem scan (bypasses Indexer) |

### Frontend (crates/nabu-ui)

| File | Line | Symbol | Purpose |
|------|------|--------|---------|
| `components/app.rs` | 77-86 | Context providers | Registers 6 context providers |
| `components/navigation/state.rs` | 174-211 | `NavContext` | 21 RwSignals for navigation state |
| `components/ui/feedback.rs` | 729-730 | `TaskContext` | Task tracking (never populated from backend) |
| `components/ui/feedback.rs` | 80-83 | `ToastContext` | Toast notifications |
| `history.rs` | 152 | `notify_history_changed()` | DOM CustomEvent (not EventBus) |
| `feedback.rs` | 190-197 | `ToastProvider` | Toast rendering at app root |

### Event Flow (all layers)

| File | Line | Event | Direction |
|------|------|-------|-----------|
| `capture/engine.rs` | 52 | `ITEM_CAPTURED` | Backend → Backend |
| `pipeline_migration/executor.rs` | 122 | `ITEM_PROCESSING_STARTED` | Backend → Backend |
| `pipeline_migration/executor.rs` | 128-135 | `PROCESSING_PROGRESS` | Backend → Backend |
| `pipeline_migration/executor.rs` | 137 | `ITEM_PROCESSING_COMPLETED` | Backend → Backend |
| `pipeline_migration/executor.rs` | 142 | `ITEM_PROCESSING_FAILED` | Backend → Backend |
| `storage/manager.rs` | 105 | `ITEM_STORED` | Backend → Backend |
| `indexer/mod.rs` | 48 | `INDEX_UPDATED` | Backend → Backend |
| `graph/vault_graph.rs` | 92 | `GRAPH_UPDATED` | Backend → Backend |
| `lib.rs` | 162-177 | ITEM_STORED → Indexer/Graph | Backend → Backend (subscribers) |
| `history.rs` | 152 | `notify_history_changed()` | Backend → Frontend (DOM event) |

---

## 20. Conclusion

Nabu's architecture is well-designed on paper: a clean service-oriented architecture with clear subsystem boundaries, a canonical pipeline for content ingestion, and a unified KnowledgeObject model. However, the **execution gap** between the canonical pipeline (Path B) and user editing actions (Path A) creates a fundamental consistency problem.

The two critical fixes that would address 80% of the issues are:
1. **Route `note_save` through `StorageManager`** so user edits trigger the same pipeline events.
2. **Add a Tauri event bridge** that forwards `ITEM_STORED`, `INDEX_UPDATED`, and `GRAPH_UPDATED` events to the frontend, so views can react to changes instead of polling.

Together with progress event forwarding and debounce-based persistence, these changes would close the gap between the documented architecture (§3) and the actual runtime behavior (§14).
