# Nabu Knowledge Pipeline Audit v0.5

> Complete semantic trace of how information becomes knowledge in the Nabu codebase. Based on evidence gathered through RustRover's semantic analysis tools, file reading, and cross-referencing.

---

## 1. Executive Summary

Nabu's knowledge pipeline is **fragmented into two disconnected paths**:

### Path A — Capture Pipeline (Full Event-Driven Flow)
```
Capture sources → CaptureEngine::ingest → ITEM_CAPTURED event
    → DurableJobQueue (file-persisted) → WorkerPool (4 tokio::spawn workers)
    → PipelineExecutor::execute → ProcessingPipeline::run (14 processors)
    → StorageManager::save → ITEM_STORED event
    → Indexer::index_object + VaultGraph::add_node
    → INDEX_UPDATED + GRAPH_UPDATED events
    → [FRONTEND CANNOT RECEIVE THESE — no event bridge]
```

### Path B — Direct Edit Path (No Events, No Sync)
```
NoteEditor → tauri_invoke("note_save") → recovery::note_save
    → std::fs::write (direct to disk, recovery.rs:403)
    → snapshot_note (versioning only, recovery.rs:404)
    → [NO EventBus publication]
    → [NO StorageManager::save call]
    → [NO ITEM_STORED event]
    → [NO Indexer update]
    → [NO VaultGraph update]
```

**Critical Finding #1**: `note_save` (recovery.rs:391) and `note_create_file` (commands.rs:581) both bypass the entire EventBus pipeline by using `std::fs::write` directly. Only content captured through the capture pipeline (file drops, clipboard, inbox, native messaging) reaches `StorageManager::save()` and triggers indexing and graph updates. This means editing a note's content has **zero effect** on search or graph until a manual re-index, re-capture, or app restart.

**Critical Finding #2**: The `EventBus<PipelineEvent>` (`bus.rs:10-12`) is purely backend-internal. `EventBus::publish()` (`bus.rs:69-76`) dispatches handlers synchronously within a `Mutex` lock — handlers are `Box<dyn Fn(&Events) + Send + Sync>` with no async capability. No `window.emit()` or `app.emit()` calls exist anywhere in `src-tauri/src/` or `crates/nabu-core/src/`. The frontend (`crates/nabu-ui`) communicates exclusively through request-response `tauri_invoke()` calls — there is **no push mechanism** for backend events.

**Critical Finding #3**: `GraphEventBridge` (`graph/incremental/event_wiring.rs:26-189`) exists to provide incremental graph updates but is **not wired in production**. It is only invoked in tests (event_wiring.rs:191-232). Production uses the inline closure in `lib.rs:162-177`.

---

## 2. Complete Knowledge Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      INFORMATION → KNOWLEDGE FLOW                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ENTRY: Multiple ingress points (see Section 3)                         │
│    ├── CaptureEngine::ingest()  ◄── PRIMARY (full pipeline)              │
│    ├── note_save / note_create_file  ◄── SECONDARY (direct write only)   │
│    └── Native messaging socket  ◄── Tertiary (→ CaptureEngine::ingest)  │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ PRIMARY PATH (Capture Pipeline)                                │    │
│  │                                                                │    │
│  │ CaptureEngine::ingest (engine.rs:54)                          │    │
│  │   ├── route() → CaptureHandler::capture() → KnowledgeObject   │    │
│  │   ├── publish(ITEM_CAPTURED) (engine.rs:64-74)                │    │
│  │   └── queue.enqueue(Job) (engine.rs:89-101)                   │    │
│  │                                                                │    │
│  │ DurableJobQueue (queue.rs:63)                                 │    │
│  │   → jobs persisted to .nabu/queue/ as JSON                     │    │
│  │                                                                │    │
│  │ WorkerPool (pool.rs:52)                                       │    │
│  │   └── tokio::spawn → Worker::run (pool.rs:75, worker.rs:50)   │    │
│  │       └── dequeue → ExecutorRegistry → PipelineExecutor       │    │
│  │                                                                │    │
│  │ PipelineExecutor::execute (executor.rs:112-176)               │    │
│  │   ├── publish(PROCESSING_STARTED)                             │    │
│  │   ├── PipelineExecutor::object_from_job()                     │    │
│  │   ├── pipeline.run(object, progress, cancel)                  │    │
│  │   │   └── 14 processors (pipeline.rs:245-264)                 │    │
│  │   ├── publish(PROCESSING_COMPLETED)                           │    │
│  │   └── storage.save(&result.object) (executor.rs:166)          │    │
│  │                                                                │    │
│  │ StorageManager::save (manager.rs:142-193)                     │    │
│  │   ├── std::fs::write(content .md)                             │    │
│  │   ├── std::fs::write(JSON sidecar .nabu/)                     │    │
│  │   ├── cache.insert(id, object)                                │    │
│  │   └── publish(ITEM_STORED) (manager.rs:181-189)               │    │
│  │                                                                │    │
│  │ ITEM_STORED subscriber (lib.rs:162-177 — inline closure)      │    │
│  │   ├── storage.load(object_id) → KnowledgeObject (lib.rs:164)  │    │
│  │   ├── indexer.lock().index_object(&object) (lib.rs:166)       │    │
│  │   │   ├── tokenize → update inverted index                   │    │
│  │   │   └── publish(INDEX_UPDATED) (indexer.rs:153-161)        │    │
│  │   ├── graph.write().add_node(&object) (lib.rs:171)            │    │
│  │   │   ├── insert into nodes map                               │    │
│  │   │   ├── publish(GRAPH_UPDATED) (graph/mod.rs:345-353)       │    │
│  │   │   └── auto-persist to .nabu/graph/ (graph/mod.rs:357-359) │    │
│  │   └── return                                                      │    │
│  │                                                                │    │
│  │ PERSISTENCE:                                               │    │
│  │   ├── .md files at vault root                                 │    │
│  │   ├── .nabu/ sidecars (metadata per object)                   │    │
│  │   ├── .nabu/search_index.json (inverted index)                │    │
│  │   ├── .nabu/graph/ (graph state + snapshots)                  │    │
│  │   └── .nabu/queue/ (pending job files)                        │    │
│  │                                                                │    │
│  │ RETRIEVAL:                                                 │    │
│  │   ├── tree_list → filesystem scan (commands.rs:357)          │    │
│  │   ├── notes_search → Indexer::search (commands.rs:1560)     │    │
│  │   ├── graph_data → VaultGraph snapshot (commands.rs:1908)    │    │
│  │   └── note_read → std::fs::read_to_string (recovery.rs:410)  │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ SECONDARY PATH (Direct Edit)                                   │    │
│  │     NO EventBus, NO indexing, NO graph update                   │    │
│  │                                                                │    │
│  │ note_save (recovery.rs:391)                                   │    │
│  │   ├── std::fs::write(&abs, &content)  [DIRECT WRITE]          │    │
│  │   ├── snapshot_note(&vault, &path)                             │    │
│  │   └── Returns Ok(()) — NO ITEM_STORED                          │    │
│  │                                                                │    │
│  │ note_create_file (commands.rs:581)                            │    │
│  │   ├── snapshot_note + std::fs::write + push_history           │    │
│  │   └── Returns Ok(()) — NO ITEM_STORED                          │    │
│  ┌──────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ NATIVE MESSAGING SOCKET (engine.rs:ingest)                    │    │
│  │                                                                │    │
│  │ Unix socket (/tmp/nabu-native-messaging.sock)                 │    │
│  │   → tokio::spawn(handle_connection) (native_messaging_socket.rs:237)│    │
│  │   → validate → message_to_capture_request → engine.ingest()   │    │
│  │   └───────────────────────────────────────────────────────────┘    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Knowledge Entry Point Inventory

| Entry Point | UI Component | Tauri Command | Backend Entry | KnowledgeObject Source |
|-------------|-------------|---------------|---------------|----------------------|
| File drag-drop | `dictation_pill.rs:76`, `inbox.rs:234` | `capture_file_drop` (commands.rs:1249) | `CaptureEngine::ingest` (engine.rs:54) | `FileDropHandler::capture()` (handler.rs:342) |
| Clipboard capture | N/A (native messaging) | Socket message | `CaptureEngine::ingest` | `ClipboardHandler::capture()` (handler.rs:249) |
| Text/inbox capture | `inbox.rs`, `dictation_pill.rs` | `inbox_quick_capture` (commands.rs:3007) | `CaptureEngine::ingest` | `ClipboardHandler::capture()` or direct |
| Screenshot capture | — | Native messaging | `CaptureEngine::ingest` | `ScreenshotHandler::capture()` (handler.rs:299) |
| Browser extension | Safari/browser extension | Native messaging socket | `CaptureEngine::ingest` → `BrowserCaptureHandler` | `BrowserCaptureHandler::capture()` (handler.rs:221) |
| YouTube URL | via BrowserCaptureHandler | Native messaging | `CaptureEngine::ingest` | `YouTubeCaptureHandler::capture()` (handler.rs:412) |
| GitHub repo | via BrowserCaptureHandler | Native messaging | `CaptureEngine::ingest` | `GitHubRepositoryHandler::capture()` (handler.rs:432) |
| Email | via BrowserCaptureHandler | Native messaging | `CaptureEngine::ingest` | `EmailCaptureHandler::capture()` (handler.rs:467) |
| Watch folder | `FileDropHandler` | — | `CaptureEngine::ingest` | `WatchFolderHandler::capture()` (handler.rs:362) |
| Safari reader | `SafariReaderHandler` | — | `CaptureEngine::ingest` | `SafariReaderHandler::capture()` (handler.rs:385) |
| Article (readability) | `ArticleCaptureHandler` | — | `CaptureEngine::ingest` | `ArticleCaptureHandler::capture()` (handler.rs:680) |
| Bookmark | `BookmarkCaptureHandler` | — | `CaptureEngine::ingest` | `BookmarkCaptureHandler::capture()` (handler.rs:637) |
| Direct note edit | `NoteEditor` (app.rs:368-381) | `note_save` (recovery.rs:391) | `std::fs::write` (recovery.rs:403) | **NONE — bypasses KnowledgeObject** |
| Direct note create | File tree, command palette | `note_create_file` (commands.rs:581) | `std::fs::write` (commands.rs:608) | **NONE — bypasses KnowledgeObject** |
| Session save | `save_status.rs`, `app.rs` | `session_save` (recovery.rs:664) | `snapshot_note` (recovery.rs:158) | **NONE — bypasses KnowledgeObject** |

**11 capture handlers** are registered in `build_default_capture_engine()` (engine.rs:168-178). The secondary entry points (direct note edit/create) bypass the KnowledgeObject model entirely.

---

## 4. Capture Architecture

### 4.1 CaptureHandler Trait

```rust
pub trait CaptureHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> CaptureSource;
    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult>;
}
```
Defined at `capture/handler.rs:37-46`. 11 implementations:

| Handler | File | Line | Source Type | Input | Output |
|---------|------|------|-------------|-------|--------|
| `FileDropHandler` | handler.rs:339-357 | 342 | FileDrop | `CaptureData::Binary` | KnowledgeObject (Document) |
| `ClipboardHandler` | handler.rs:246-294 | 249 | Clipboard | `CaptureData::Text/Binary` | KnowledgeObject (Note/Bookmark) |
| `ScreenshotHandler` | handler.rs:296-337 | 299 | Screenshot | `CaptureData::Binary` | KnowledgeObject (Screenshot) |
| `BrowserCaptureHandler` | handler.rs:209-244 | 212 | Browser | `CaptureData::Uri/Text` | Routes to YouTube/GitHub/Article/Bookmark |
| `YouTubeCaptureHandler` | handler.rs:402-427 | 405 | YouTube | `CaptureData::Uri` | KnowledgeObject (YouTubeVideo) |
| `GitHubRepositoryHandler` | handler.rs:429-462 | 432 | GitHub | `CaptureData::Uri` | KnowledgeObject (Repository) |
| `ArticleCaptureHandler` | handler.rs:680-744 | 680 | Browser | `CaptureData::Text/Uri` | KnowledgeObject (Article) |
| `BookmarkCaptureHandler` | handler.rs:634-678 | 637 | Browser | `CaptureData::Text/Uri` | KnowledgeObject (Bookmark) |
| `EmailCaptureHandler` | handler.rs:464-632 | 467 | Email | `CaptureData::Text` | KnowledgeObject (Email) |
| `SafariReaderHandler` | handler.rs:382-399 | 385 | Browser | `CaptureData::Text` | KnowledgeObject (Article) |
| `WatchFolderHandler` | handler.rs:359-380 | 362 | FileDrop | `CaptureData::File` | KnowledgeObject (Document) |

### 4.2 CaptureRequest

```rust
pub struct CaptureRequest {
    data: CaptureData,
    title: Option<String>,
    source_url: Option<String>,
    mime_type: Option<String>,
}
```
Defined at `capture/handler.rs:51-61`. `CaptureData` enum (handler.rs:126) variants:
- `Text(String)` — raw text input
- `Uri(String)` — URL reference
- `Binary { mime_type: String, data: Vec<u8>, filename: Option<String> }` (handler.rs:134-138)
- `File(String)` — file path
- `ScreenCapture { selection: Option<...> }` (handler.rs:142-144)

### 4.3 CaptureEngine — Central Ingestion

```rust
pub struct CaptureEngine {
    handlers: HashMap<String, Arc<dyn CaptureHandler>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    queue: Option<Arc<DurableJobQueue>>,
}
```
Defined at `capture/engine.rs:14-20` (the report's line reference was off — the struct starts at line 14, not 16-20).

**`ingest()` method** (engine.rs:54-105):
1. Calls `self.route(&request)` → dispatches to matching handler (engine.rs:114-122)
2. If handler returns `Some(result)`:
   - Publishes `ITEM_CAPTURED` event (engine.rs:64-74)
   - If `result.enqueue`: creates `Job` via `object_type_to_job_type()` (engine.rs:80) and enqueues (engine.rs:101)
3. Returns `Option<Uuid>` — the object ID

### 4.4 Dispatch Mechanism

`CaptureEngine::route()` (engine.rs:114-122):
- Matches `CaptureData` variant to handler(s)
- For `Uri`/`Text` URL inputs: delegates to `BrowserCaptureHandler` which further routes by domain
- For `Binary`: delegates to `FileDropHandler`
- No priority or ordering — first matching handler wins

### 4.5 Centralization

Capture is **centralized** — all 13+ capture handlers feed into the single `CaptureEngine`, which enqueues into the single `DurableJobQueue`. There is no direct handler→storage path; all processing is asynchronous via the job queue.

---

## 5. Processing Pipeline Analysis

### 5.1 ProcessingPipeline Structure

```rust
pub struct ProcessingPipeline {
    processors: Vec<Arc<dyn Processor>>,
    event_bus: Option<EventBus<PipelineEvent>>,
}
```
Defined at `processing/pipeline.rs:15-18`.

### 5.2 Processor Trait

```rust
#[async_trait]
pub trait Processor: Send + Sync {
    fn name(&self) -> &'static str;
    async fn process(
        &self,
        context: &ProcessingContext,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult;
    fn supports(&self, _object_type: &ObjectType) -> bool { true }
}
```
Defined at `processing/processor.rs:84-98`. Doc comment (`processor.rs:70-82`): "No processor instantiates another processor. No processor directly invokes another processor. No processor depends on queue internals. No processor depends on capture internals."

### 5.3 ProcessingContext

```rust
pub struct ProcessingContext {
    pub object: KnowledgeObject,
    pub is_retry: bool,
    pub retry_attempt: u32,
    pub metadata: HashMap<String, String>,
}
```
Defined at `processing/processor.rs:9-21`.

### 5.4 ProcessingResult

```rust
pub struct ProcessingResult {
    pub object: KnowledgeObject,
    pub modified: bool,
    pub metadata: HashMap<String, String>,
    pub error: Option<String>,
}
```
Defined at `processing/processor.rs:36-48`.

### 5.5 Pipeline Execution Flow

`ProcessingPipeline::run()` (pipeline.rs:53-205):

```
for each processor (in registration order):
    1. Check cancellation                    (pipeline.rs:82)
    2. Check supports(object_type)           (pipeline.rs:100)
    3. Set per-processor progress            (pipeline.rs:112-116)
    4. Create ProcessingContext              (pipeline.rs:119)
    5. Call processor.process()              (pipeline.rs:133-135)
    6. Set object = result.object           (pipeline.rs:151) — chain output
    7. Publish PROCESSING_COMPLETED or       (pipeline.rs:156-185)
       PROCESSING_FAILED event
Return final ProcessingResult                (pipeline.rs:199-204)
```

**Error handling**: If a processor returns `error: Some(...)`, the pipeline publishes `ITEM_PROCESSING_FAILED` but continues to the next processor. The final result retains the error. It is up to `PipelineExecutor::execute` to decide whether to retry or fail the job.

**Cancellation**: Checked between processors via `cancellation.is_cancelled()` (pipeline.rs:82). No mid-processor cancellation within a processor's `process()` call — that is the processor's responsibility via the `CancellationToken` parameter.

### 5.6 Standard Pipeline — 14 Processors in 5 Phases

`build_standard_pipeline()` (pipeline.rs:235-272):

| # | Processor | File | Phase | Ordering Constant |
|---|-----------|------|-------|-------------------|
| 1 | `ContentClassifier` | content_classifier.rs:12 | Content understanding | `CONTENT_CLASSIFIER` = 0 (pipeline.rs:276) |
| 2 | `DuplicateDetector` | duplicate_detector.rs:13 | Content understanding | `DUPLICATE_DETECTOR` = 1 (pipeline.rs:277) |
| 3 | `TimelineExtractor` | timeline_extractor.rs | Content understanding | `TIMELINE_EXTRACTOR` = 2 (pipeline.rs:278) |
| 4 | `MetadataExtractor` | metadata_extractor.rs:17 | Metadata extraction | `METADATA_EXTRACTOR` = 3 (pipeline.rs:279) |
| 5 | `MetadataEnricher` | metadata_enricher.rs:19 | Metadata enrichment | `METADATA_ENRICHER` = 4 (pipeline.rs:280) |
| 6 | `OcrProcessor` | ocr_processor.rs:17 | Document processing | `OCR_PROCESSOR` = 5 (pipeline.rs:281) |
| 7 | `PdfTextProcessor` | pdf_text_processor.rs | Document processing | `PDF_TEXT_PROCESSOR` = 6 (pipeline.rs:282) |
| 8 | `PdfMetadataProcessor` | pdf_metadata_processor.rs:9 | Document processing | `PDF_METADATA_PROCESSOR` = 7 (pipeline.rs:283) |
| 9 | `PdfAnnotationProcessor` | pdf_annotation_processor.rs:14 | Document processing | `PDF_ANNOTATION_PROCESSOR` = 8 (pipeline.rs:284) |
| 10 | `WhisperProcessor` | whisper_processor.rs | AI-powered | `WHISPER_PROCESSOR` = 9 (pipeline.rs:285) |
| 11 | `EmbeddingGenerator` | embedding_generator.rs:19 | AI-powered | `EMBEDDING_GENERATOR` = 10 (pipeline.rs:286) |
| 12 | `SemanticEnricher` | semantic_enricher.rs | AI-powered | `SEMANTIC_ENRICHER` = 11 (pipeline.rs:287) |
| 13 | `AiSummariser` | ai_summariser.rs:18 | AI-powered | `AI_SUMMARISER` = 12 (pipeline.rs:288) |
| 14 | `AutoFiler` | auto_filer.rs:12 | Organization | `AUTO_FILER` = 13 (pipeline.rs:289) |

**Processor ordering is fixed** — `build_standard_pipeline()` registers in the same order as the `ordering` constants. `register_at()` (pipeline.rs:44) allows insertion at a specific index but is not used by the standard pipeline.

### 5.7 Pipeline Executor Bridge

`PipelineExecutor` (pipeline_migration/executor.rs:24-28):
```rust
pub struct PipelineExecutor {
    pipeline: Arc<ProcessingPipeline>,
    event_bus: Option<EventBus<PipelineEvent>>,
    storage: Option<Arc<StorageManager>>,
}
```

**Registration**: Registered in `ExecutorRegistry` for processor names `"ocr_processor"`, `"whisper_processor"`, `"pdf_text_extraction_processor"`, `"metadata_extraction_processor"` (lib.rs:109-116).

**`execute()` method** (executor.rs:112-176):
1. Publishes `PROCESSING_STARTED` (executor.rs:122-125)
2. Reconstructs `KnowledgeObject` from `Job` payload (`object_from_job`, executor.rs:129-130)
3. Calls `self.pipeline.run(object, progress, cancellation).await` (executor.rs:134-137)
4. Publishes `PROCESSING_COMPLETED` or `PROCESSING_FAILED` (executor.rs:144-157)
5. Calls `self.storage.save(&result.object)` (executor.rs:166) — **this is where ITEM_STORED fires**
6. Returns `Ok(completed_job)` (executor.rs:175)

---

## 6. Processor Inventory

### 6.1 14 Processors

Each processor is a struct in `crates/nabu-core/src/processing/processors/`:

| Processor | Struct Location | JobType | Native Engine | Output |
|-----------|----------------|---------|---------------|--------|
| `ContentClassifier` | content_classifier.rs:12 | ContentClassification | None | ObjectType set on KO |
| `DuplicateDetector` | duplicate_detector.rs:13 | DuplicateDetection | ContentHasher (built-in) | Duplicate flag in metadata |
| `TimelineExtractor` | timeline_extractor.rs | TimelineExtraction | Date parsing (chrono) | Timeline entries in metadata |
| `MetadataExtractor` | metadata_extractor.rs:17 | MetadataExtraction | None (frontmatter parsing) | ObjectMetadata populated |
| `MetadataEnricher` | metadata_enricher.rs:19 | MetadataEnrichment | None (web scraping via HTTP) | site_name, author, etc. |
| `OcrProcessor` | ocr_processor.rs:17 | Ocr | `native::vision::recognize_text` (vision.rs:25) | Text extracted from image PDF |
| `PdfTextProcessor` | pdf_text_processor.rs | PdfTextExtraction | `native::pdfkit::extract_text` (pdfkit.rs) | Text content from PDF |
| `PdfMetadataProcessor` | pdf_metadata_processor.rs:9 | PdfMetadataExtraction | `native::pdfkit::extract_metadata` | PDF metadata |
| `PdfAnnotationProcessor` | pdf_annotation_processor.rs:14 | PdfAnnotationProcessing | `native::pdfkit::extract_annotations` | Annotations as custom properties |
| `WhisperProcessor` | whisper_processor.rs | Whisper | `native::whisper::transcribe` (whisper.rs:19) | Transcribed text from audio |
| `EmbeddingGenerator` | embedding_generator.rs:19 | EmbeddingGeneration | None (BPE tokenization) | Embedding vector in custom_properties |
| `SemanticEnricher` | semantic_enricher.rs | SemanticEnrichment | None (keyword extraction) | Semantic tags, relationships |
| `AiSummariser` | ai_summariser.rs:18 | AiSummarisation | None (HTTP to LLM API) | Summary text in custom_properties |
| `AutoFiler` | auto_filer.rs:12 | AutoFiling | None (rule-based) | Vault path assigned |

### 6.2 Processor Registration

`build_standard_pipeline()` (pipeline.rs:235-272) registers all 14 processors in the `Vec<Arc<dyn Processor>>`. Each processor is a singleton (stateless struct or struct with config), shared via `Arc` across all workers.

### 6.3 Native Engine Integration

All 4 native modules are at `crates/nabu-core/src/native/`:
- `vision.rs` — macOS Vision framework (`VNRecognizeTextRequest` via `objc2-vision`)
- `pdfkit.rs` — macOS PDFKit framework (`PDFDocument`, `PDFPage` via manual `extern_class!` declarations)
- `whisper.rs` — local inference via `whisper-rs` crate (no network)
- `screenshot.rs` — `screencapture` CLI via `std::process::Command`

`native/mod.rs:1-17` is the module boundary — only `NativeError`, `OcrResult`, `PdfText`, `PdfMetadata`, `PdfAnnotation`, `TranscriptionResult`, `AudioData`, `ScreenCaptureOptions` are re-exported. All `objc2` types remain inside `native/`.

**`spawn_blocking` usage**: `whisper_processor.rs` and `pdf_annotation_processor.rs` use `tokio::task::spawn_blocking` for CPU-bound work (whisper.rs:156, pdfkit.rs:112-115). Other processors run inline within the pipeline.

---

## 7. Knowledge Object Lifecycle

### 7.1 KnowledgeObject Model

```rust
pub struct KnowledgeObject {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub content: ObjectContent,
    pub metadata: ObjectMetadata,
    pub custom_properties: HashMap<String, CustomPropertyValue>,
    pub tags: Vec<String>,
    pub relations: Vec<ObjectRelation>,
    pub processing_state: ProcessingState,
    pub content_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```
Defined at `models/knowledge_object.rs:10-43`. 14 fields, `Serialize + Deserialize + Clone + Debug`.

### 7.2 ObjectType Enum (22 variants)

```rust
pub enum ObjectType {
    Note, Bookmark, Document, Image, Screenshot, Scan, AudioRecording,
    VideoRecording, Repository, YouTubeVideo, Article, Email, Contact,
    Project, Task, Event, CodeSnippet, Whiteboard, Template, Attachment,
    Collection, Dashboard
}
```
Defined at `models/knowledge_object.rs:133-177`. Each variant maps to a `JobType` via `object_type_to_job_type()` (engine.rs:141-152).

### 7.3 ObjectContent Enum

```rust
pub enum ObjectContent {
    Markdown(String),
    RichHtml(String),
    PlainText(String),
    Uri(String),
    Binary { mime_type: String, data: Vec<u8>, filename: Option<String> },
}
```
Defined at `models/knowledge_object.rs:211-226`. `content_type_hint()` method (line 228-237) provides MIME type for storage.

### 7.4 ObjectMetadata Struct

```rust
pub struct ObjectMetadata {
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub authors: Vec<String>,
    pub publication_date: Option<DateTime<Utc>>,
    pub site_name: Option<String>,
    pub language: Option<String>,
    pub file_size: Option<u64>,
    pub mime_type: Option<String>,
    pub ocr_confidence: Option<f64>,
    pub original_filename: Option<String>,
    pub vault_path: Option<String>,
    pub description: Option<String>,
    pub word_count: Option<usize>,
}
```
Defined at `models/knowledge_object.rs:242-269`. Derives `Default`.

### 7.5 ObjectRelation Struct

```rust
pub struct ObjectRelation {
    pub relation_type: RelationType,
    pub target: Uuid,
    pub weight: f64,
    pub metadata: HashMap<String, String>,
}
```
Defined at `models/knowledge_object.rs:310`.

### 7.6 Lifecycle Stage 1: Construction

KnowledgeObject is constructed by CaptureHandlers during `capture()`:

1. `KnowledgeObject::new(object_type, content)` (line 45-50) — creates with fresh UUID, timestamps, default metadata
2. Handler sets `metadata.title`, `metadata.source_url`, `metadata.authors`, etc. from the captured content
3. Handler may set `custom_properties`, `tags`, `relations`
4. Returns `CaptureResult { object, source, events, enqueue }` (handler.rs:7-14)

### 7.7 Lifecycle Stage 2: Processing

`PipelineExecutor::execute()` (executor.rs:112-176):
1. Reconstructs `KnowledgeObject` from `Job` payload via `object_from_job()` (executor.rs:129-130)
2. Passes to `pipeline.run(object, progress, cancellation)` (pipeline.rs:53-205)
3. Each processor mutates the object:
   - `ContentClassifier`: sets `object_type` based on content analysis (content_classifier.rs:15-70)
   - `MetadataExtractor`: parses frontmatter YAML, populates `metadata` (metadata_extractor.rs:20-84)
   - `LinkExtractor` is NOT a processor — link extraction happens in `ContentClassifier` or `MetadataExtractor`
   - `OcrProcessor`: calls `native::vision::recognize_text()` via `spawn_blocking`, merges text into content (ocr_processor.rs:20-95)
   - `WhisperProcessor`: calls `native::whisper::transcribe()` via `spawn_blocking`, sets content to transcript (whisper_processor.rs)
4. Final result is a fully enriched `KnowledgeObject` with all metadata, links, tags, and content extracted

### 7.8 Lifecycle Stage 3: Storage

`PipelineExecutor::execute()` (executor.rs:165-168):
```rust
if let Some(ref storage) = self.storage {
    if let Err(e) = storage.save(&result.object) {
        tracing::warn!(error = %e, "Failed to persist...");
    }
}
```

`StorageManager::save()` (manager.rs:142-193):
1. Resolves vault-relative path (`vault_rel`, manager.rs:144)
2. Writes content file: `.md`/`.txt`/`.html`/`.json` at vault root (manager.rs:148-167)
3. Writes JSON sidecar: `.nabu/{uuid}.json` with structured metadata (manager.rs:170-171)
4. Updates in-memory cache: `store.insert(object.id, object.clone())` (manager.rs:176)
5. Publishes `ITEM_STORED` event (manager.rs:181-189)

### 7.9 Lifecycle Stage 4: Indexing

`ITEM_STORED` subscriber (lib.rs:162-177):
```rust
event_bus.subscribe(kinds::ITEM_STORED, move |event: &PipelineEvent| {
    if let PipelineEvent::ItemStored(stored) = event {
        if let Some(object) = storage_for_events.load(stored.object_id) {
            if let Ok(indexer) = indexer_for_events.lock() {
                if let Err(e) = indexer.index_object(&object) {
                    tracing::error!(...);
                }
            }
            // ... VaultGraph add_node
        }
    }
});
```

`Indexer::index_object()` (indexer.rs:138-164):
1. Acquires write lock on `index: RwLock<InvertedIndex>` (indexer.rs:139)
2. Tokenizes object: `tokenize_object(object)` (indexer.rs:142) — splits title, tags, content into lowercase terms
3. For each token: `index.entry(token).or_default().push(object.id.to_string())` (indexer.rs:145-149)
4. If event_bus: publishes `INDEX_UPDATED` (indexer.rs:153-161)
5. Returns `Ok(())`

**Persistence**: `Indexer::persist()` (indexer.rs:99) writes `.nabu/search_index.json`. Called during startup load (indexer.rs:118) and on shutdown.

### 7.10 Lifecycle Stage 5: Graph

`VaultGraph::add_node()` (graph/mod.rs:335-364):
1. Acquires write lock on `nodes: RwLock<HashMap<Uuid, KnowledgeObject>>` (graph/mod.rs:341)
2. Inserts object: `nodes.insert(object.id, object.clone())` (graph/mod.rs:342)
3. If event_bus: publishes `GRAPH_UPDATED` with `NodeAdded` (graph/mod.rs:345-353)
4. Auto-persists if `PersistenceHandle.auto_save` is true (graph/mod.rs:357-359)

**Edge extraction**: Edges are extracted from `KnowledgeObject.relations` during `rebuild_from_objects()` (graph/mod.rs:299). Each `ObjectRelation` becomes a `GraphEdge` with `source`, `target`, `relationship`, `weight`.

**Persistence**: `PersistenceHandle::save()` (graph/mod.rs:82-96) calls `to_snapshot()` (graph/mod.rs:242-274) → serializes to `.nabu/graph/` as JSON.

### 7.11 Lifecycle Stage 6: Retrieval

| Operation | Backend Entry Point | Frontend IPC | Data Source |
|-----------|-------------------|--------------|-------------|
| Open note content | `StorageManager::load(id)` (manager.rs:199) | `note_read` IPC (recovery.rs:410) → `std::fs::read_to_string` | File on disk |
| Search | `Indexer::search(query)` (indexer.rs:180) | `notes_search` IPC (commands.rs:1560) → reads Indexer | In-memory index |
| File tree | Filesystem scan | `tree_list` IPC (commands.rs:357) → `scan_tree()` (commands.rs:311) | File on disk |
| Graph view | `VaultGraph` snapshot | `graph_data` IPC (commands.rs:1908) → reads graph | In-memory graph |
| Note metadata | `StorageManager::load(id)` | Via `note_read` or `graph_data` | File system / cache |

**Critical**: Retrieval does NOT use `KnowledgeObject` directly — each retrieval path uses a different source (filesystem scan, in-memory index, graph snapshot). There is no unified retrieval layer.

### 7.12 Lifecycle Stage 7: Persistence

All persistence is file-based (no database):

| Data | Location | Format | Owner |
|------|----------|--------|-------|
| Content | `<vault>/<path>.md` | Markdown | StorageManager |
| Metadata sidecars | `.nabu/{uuid}.json` | JSON | StorageManager |
| Search index | `.nabu/search_index.json` | JSON (HashMap<String, Vec<String>>) | Indexer |
| Graph state | `.nabu/graph/graph.json` | JSON (GraphSnapshot) | VaultGraph |
| Jobs | `.nabu/queue/{status}/{id}.json` | JSON (Job) | DurableJobQueue |
| Versions | `.nabu/versions/{path_hash}/v*.json` | JSON (VersionManifest + snapshots) | recovery.rs |
| Session | `.nabu/session.json` | JSON (SessionState) | recovery.rs |
| Settings | `~/.config/nabu/settings.json` | JSON (AppSettings) | SettingsStore |

### 7.13 Deviations from the Canonical Lifecycle

| Entry Point | KnowledgeObject? | Pipeline? | StorageManager? | ITEM_STORED? | Indexed? | Graph updated? |
|-------------|-----------------|-----------|-----------------|-------------|----------|----------------|
| File drop | Yes | Yes | Yes | Yes | Yes | Yes |
| Clipboard capture | Yes | Yes | Yes | Yes | Yes | Yes |
| Browser/YouTube capture | Yes | Yes | Yes | Yes | Yes | Yes |
| Inbox quick capture | Yes | Yes | Yes | Yes | Yes | Yes |
| Native messaging socket | Yes | Yes | Yes | Yes | Yes | Yes |
| **note_save** (edit) | **NO** | **NO** | **NO (std::fs::write)** | **NO** | **NO** | **NO** |
| **note_create_file** | **NO** | **NO** | **NO (std::fs::write)** | **NO** | **NO** | **NO** |
| **session_save** | **NO** | **NO** | **NO (std::fs::write)** | **NO** | **NO** | **NO** |

---

## 8. Metadata Flow

### 8.1 Metadata Producers

| Metadata Field | Producer | Where |
|----------------|----------|-------|
| `title` | CaptureHandler, MetadataExtractor | handler.rs (handler.set metadata), metadata_extractor.rs |
| `source_url` | CaptureHandler | handler.rs |
| `authors` | MetadataExtractor, MetadataEnricher | metadata_extractor.rs, metadata_enricher.rs |
| `tags` | FrontmatterParser (within MetadataExtractor) | metadata_extractor.rs |
| `links` | ContentClassifier (wikilink extraction) | content_classifier.rs |
| `mentions` | ContentClassifier (mention extraction) | content_classifier.rs |
| `content_hash` | ContentHasher processor (within pipeline) | pipeline processor |
| `word_count` | KnowledgeObject::count_words() | knowledge_object.rs:50 — computed on demand |
| `ocr_confidence` | OcrProcessor | ocr_processor.rs |
| `relation_type` | ContentClassifier, LinkExtractor | content_classifier.rs, metadata_extractor.rs |

### 8.2 Metadata Owners

| Metadata Category | Canonical Owner | Persistence |
|-------------------|----------------|-------------|
| Core metadata (title, tags, links, relations) | `KnowledgeObject.metadata` + `KnowledgeObject.tags` + `KnowledgeObject.relations` | In `.nabu/{uuid}.json` sidecar |
| Custom properties | `KnowledgeObject.custom_properties` | In `.nabu/{uuid}.json` sidecar |
| Processing state | `KnowledgeObject.processing_state` | In `.nabu/{uuid}.json` sidecar |
| Search tokens | `Indexer.index: RwLock<HashMap<String, Vec<String>>>` | In `.nabu/search_index.json` |
| Graph topology | `VaultGraph.nodes/edges/adjacency` | In `.nabu/graph/graph.json` |

### 8.3 Metadata Duplication

The same information exists in multiple representations:

| Information | Representation 1 | Representation 2 | Representation 3 |
|-------------|-----------------|-----------------|-----------------|
| Note title | `KnowledgeObject.metadata.title` | File name on disk | Search index tokens |
| Links | `KnowledgeObject.relations` | VaultGraph edges | Frontend `notes_index` |
| Tags | `KnowledgeObject.tags` | Frontend `notes_index` (not in sidecar?) | Search index tokens |
| Word count | `KnowledgeObject.count_words()` | `ObjectMetadata.word_count` | None |

**Duplication is intentional** — the graph, index, and KnowledgeObject each maintain independent representations optimized for their use case (graph traversal, search, domain operations).

### 8.4 Metadata Propagation

When `StorageManager::save()` is called (manager.rs:142):
1. Content + metadata → written to `.md` + `.nabu/{uuid}.json` (manager.rs:148-177)
2. `ITEM_STORED` published → Indexer indexes tokens, Graph adds node
3. Both Indexer and Graph derive their representations from the `KnowledgeObject` loaded from StorageManager cache

---

## 9. Storage Architecture

### 9.1 StorageManager

```rust
pub struct StorageManager {
    store: RwLock<HashMap<Uuid, KnowledgeObject>>,
    vault_path: PathBuf,
    event_bus: Option<EventBus<PipelineEvent>>,
}
```
Defined at `storage/manager.rs:33-37`.

### 9.2 Markdown-First Vault Layout

```
<vault>/
├── *.md                    ← human-readable content (canonical)
├── *.txt / *.html / *.json ← other content formats
├── .nabu/
│   ├── {uuid}.json         ← per-object metadata sidecar
│   ├── index.json          ← object index (UUID → metadata)
│   ├── trash/              ← soft-deleted items
│   ├── versions/           ← version snapshots (bounded by MAX_VERSIONS=50)
│   ├── session.json        ← last session state
│   ├── search_index.json   ← inverted index (owned by Indexer)
│   ├── graph/              ← graph state (owned by VaultGraph)
│   │   ├── graph.json      ← serialized GraphSnapshot
│   │   ├── snapshots/      ← historical snapshots
│   │   └── changelog.json  ← incremental update audit trail
│   ├── queue/              ← pending job files
│   └── jobs/               ← (alternative path, see DurableJobQueue)
```

### 9.3 Canonical Source of Truth

The **Markdown file at vault root** is the canonical source. The `.nabu/{uuid}.json` sidecar is a performance optimization — it can be reconstructed from the Markdown. The in-memory `HashMap<Uuid, KnowledgeObject>` cache (manager.rs:34) is a hot-path optimization.

**Evidence**: `StorageManager::reload_from_disk()` (manager.rs:408-416) reconstructs the cache from disk. `VaultGraph` doc comment (mod.rs:52): "The persisted graph is always rebuildable from canonical Markdown. No user data exists only in the graph cache."

### 9.4 Two-Tier Storage Paths

| Path | Through StorageManager? | Through EventBus? | Indexed? | Graph updated? |
|------|------------------------|-------------------|----------|----------------|
| Capture pipeline → StorageManager::save | Yes | Yes (ITEM_STORED) | Yes | Yes |
| Direct edit → note_save (std::fs::write) | **No** | **No** | **No** | **No** |
| Direct create → note_create_file (std::fs::write) | **No** | **No** | **No** | **No** |
| File tree operations → history.rs (std::fs) | **No** | **No** | **No** | **No** |

### 9.5 StorageManager API

| Method | Line | Mutates State? | Publishes Events? |
|--------|------|----------------|-------------------|
| `save()` | 142 | Yes (cache + disk) | Yes (ITEM_STORED) |
| `load()` | 199 | No (cache read) | No |
| `load_by_type()` | 216 | No | No |
| `delete()` | 318 | Yes (cache + disk) | Yes (ITEM_STORED? — NO, deletes directly) |
| `move_item()` | ? | Yes | No |
| `list()` | 361 | No | No |
| `exists()` | 349 | No | No |
| `get_metadata()` | ? | No | No |

---

## 10. Search Pipeline

### 10.1 Search Architecture

```rust
pub struct Indexer {
    index: RwLock<HashMap<String, Vec<String>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    vault_path: Option<PathBuf>,
}
```
Defined at `indexer.rs:26-30`.

### 10.2 Indexing Pipeline

```
KnowledgeObject arrives at Indexer::index_object()  (indexer.rs:138)
    ↓
tokenize_object(object)                    (internal, indexer.rs:~230)
    ├── title → lowercase → split on whitespace
    ├── tags → lowercase
    ├── content → lowercase → split on whitespace + punctuation
    └── metadata title/source_url (if present)
    ↓
For each token: index.entry(token).or_default().push(object.id)
    ↓
index.insert(token, Vec<String>)  ← inverted index HashMap
    ↓
publish(INDEX_UPDATED)  (indexer.rs:153-161)
    ↓
persist() → write .nabu/search_index.json  (indexer.rs:99)
```

### 10.3 Search Execution

`notes_search` IPC (commands.rs:1560):
1. Reads `settings.last_vault_path` → vault path (commands.rs:~1575)
2. Resolves `Indexer` from `ApplicationContext` (commands.rs:~1580)
3. Calls `indexer.search(&query)` (indexer.rs:180-197):
   - Tokenizes query: `query.split_whitespace().map(|t| t.to_lowercase())` (indexer.rs:185)
   - For each token: `index.get(token)` → `Vec<String>` of object IDs (indexer.rs:190)
   - Intersects all ID lists (set intersection) (indexer.rs:192-194)
   - Returns `Vec<String>` of matching object IDs (indexer.rs:197)
4. Converts IDs to `SearchHit` DTOs: resolves each ID to file path, generates snippet via `make_snippet()` (commands.rs:1535-1558)

### 10.4 Search Limitations

- **No TF-IDF**: ranking is boolean (all query terms must match)
- **No phrase search**: tokens are split on whitespace
- **No fuzzy matching**: exact lowercase token match only
- **No stemming**: "running" ≠ "run"
- **No field weighting**: title and content tokens share the same namespace
- **In-memory only**: `index` is `RwLock<HashMap>` — no persistence between restarts (persisted to disk but reloaded as empty HashMap)

### 10.5 Frontend Search Integration

`SearchPage` component (navigation/search_page.rs) calls `notes_search` IPC on every keystroke (debounced). Results are displayed as `SearchHit` DTOs with title, path, and snippet. The index is **not** rebuilt automatically after `note_save` — it is only updated via the `ITEM_STORED` → `index_object` flow.

---

## 11. Knowledge Graph Pipeline

### 11.1 VaultGraph Structure

```rust
pub struct VaultGraph {
    nodes: RwLock<HashMap<Uuid, KnowledgeObject>>,
    edges: RwLock<Vec<GraphEdge>>,
    adjacency: RwLock<HashMap<Uuid, HashSet<Uuid>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    persistence: Option<PersistenceHandle>,
    loaded_from_disk: RwLock<bool>,
    generation: RwLock<u64>,
}
```
Defined at `graph/mod.rs:54-64`.

### 11.2 GraphEdge

```rust
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub relationship: String,
    pub weight: f64,
}
```
Defined at `graph/mod.rs:36-41`. Relationship types: `"references"`, `"parent"`, `"child"`, `"mentions"`, `"similar"` (from recovery.rs:191-213 `extract_edges`).

### 11.3 PersistenceHandle

```rust
pub struct PersistenceHandle {
    store: Arc<GraphStore>,
    auto_save: bool,
}
```
Defined at `graph/mod.rs:68-71`. `auto_save = true` by default (graph/mod.rs:77). Wraps `GraphStore` (which persists to `.nabu/graph/`).

### 11.4 Graph Event Flow

```
ITEM_STORED event (from StorageManager::save)
    ↓
lib.rs:162-177 — inline subscriber closure
    ├── storage.load(object_id) → KnowledgeObject
    ├── indexer.lock().index_object(&object)
    └── graph.write().add_node(&object)
        ├── nodes.insert(object.id, object)
        ├── publish(GRAPH_UPDATED)  (graph/mod.rs:345-353)
        └── persistence.save(self) if auto_save  (graph/mod.rs:357-359)
            └── to_snapshot() → GraphSnapshot → JSON → .nabu/graph/graph.json
```

### 11.5 Graph Recovery

`GraphRecovery` (graph/recovery.rs:15-17):
1. `recover()` (recovery.rs:28) — calls `load_graph()` (loader.rs:40)
2. If `LoadResult::Loaded` → validate snapshot
3. If `LoadResult::RequiresUpgrade` → call `upgrade_snapshot()` (loader.rs:54)
4. If `LoadResult::Corrupted` or `FutureVersion` → `NeedsRebuild`
5. If `NeedsRebuild` → `rebuild()` (recovery.rs:103) — calls `build_graph_from_objects()` (recovery.rs:217) which scans all KnowledgeObjects and extracts nodes + edges

### 11.6 Graph Update Triggers

| Trigger | Path | Auto-updates Graph? |
|---------|------|---------------------|
| Capture pipeline save | StorageManager::save → ITEM_STORED → add_node | **Yes** |
| Direct note edit | note_save → std::fs::write | **No** |
| Undo/redo | HistoryManager::undo/redo → filesystem ops | **No** |
| Trash operations | trash_file/restore_from_trash | **No** |

**The graph is ONLY updated through ITEM_STORED events, which are ONLY published by StorageManager::save().** Direct filesystem writes bypass the graph entirely.

### 11.7 Frontend Graph View

`graph_view.rs` component calls `graph_data` IPC (commands.rs:1908) to fetch the entire graph as `GraphData` DTO. There is no live subscription — the frontend re-fetches on demand or on manual refresh.

---

## 12. Retrieval Pipeline

### 12.1 Open Note

```
User clicks note in file tree
    ↓
WorkspaceContext.active_path.set(path)  (workspace.rs:34)
    ↓
App component matches active_path → renders NoteEditor  (app.rs:363-388)
    ↓
NoteEditor on_mount → spawn_local(async {
    tauri_invoke("note_read", {path})   (ipc.rs:9, note_editor.rs:66)
        ↓
    recovery::note_read (recovery.rs:410)
        ├── vault_path(&store) → settings.last_vault_path  (settings.rs:297)
        ├── resolve_in_vault(&vault, &path) → absolute path  (recovery.rs:80)
        └── std::fs::read_to_string(&abs)  (recovery.rs:416)
        ↓
    Returns String (content)
})
    ↓
Frontend: serde_wasm_bindgen::from_value::<String>(result)  (note_editor.rs:66)
    ↓
NoteEditor content signal set → renders editor
```

### 12.2 Search Result

```
User types in SearchPage
    ↓
Signal: search_query.set(query)
    ↓
spawn_local(async { tauri_invoke("notes_search", {query, limit}) })
    ↓
commands::notes_search (commands.rs:1560)
    ├── settings.last_vault_path → vault_path
    ├── ctx.resolve("indexer") → Arc<Mutex<Indexer>>  (context.rs:249)
    │   └── indexer.search(query)  (indexer.rs:180)
    │       ├── tokenize query (lowercase)
    │       ├── intersect ID lists in inverted index
    │       └── return Vec<String> of UUIDs
    └── Convert UUIDs to SearchHit DTOs  (commands.rs:~1590)
        ├── resolve UUID → vault_rel path  (manager.rs:344)
        ├── read_note_content for snippet  (commands.rs:1805)
        └── make_snippet()  (commands.rs:1535)
    ↓
Frontend: serde_wasm_bindgen::from_value::<Vec<SearchHit>>(result)
    ↓
Signal: search_results.set(results) → UI re-renders
```

### 12.3 File Tree

```
User opens folder / creates note
    ↓
WorkspaceContext.refresh_tree.update(|v| *v += 1)
    ↓
FileTree component watches refresh_tree → re-fetches
    ↓
tauri_invoke("tree_list", {})  (commands.rs:357)
    ├── settings.last_vault_path → vault_path
    ├── scan_tree(dir, "")  (commands.rs:311)
    │   ├── std::fs::read_dir
    │   ├── filter .nabu/, .DS_Store, hidden files
    │   ├── recurse into subdirectories
    │   └── build Vec<TreeEntry>
    └── Returns Vec<TreeEntry>  (TreeEntry at commands.rs:~290)
    ↓
Frontend: serde_wasm_bindgen::from_value::<Vec<TreeEntry>>(result)
    ↓
FileTree re-renders with new structure
```

### 12.4 Graph View

```
User switches to Graph view
    ↓
NavContext.view_mode.set(Graph)
    ↓
GraphView component renders → graph_data IPC
    ↓
tauri_invoke("graph_data", {limit})  (commands.rs:1908)
    ├── settings.last_vault_path → vault_path
    ├── VaultGraph loaded from .nabu/graph/graph.json  (commands.rs:~1920)
    │   └── Serialize to GraphData DTO  (commands.rs:~1925)
    └── Returns GraphData  (GraphData at commands.rs:~1850)
    ↓
Frontend: serde_wasm_bindgen::from_value::<GraphData>(result)
    ↓
GraphView renders nodes + edges on canvas
```

### 12.5 Dashboard

```
On app mount (app.rs:90):
    load_notes_index(nav)
        ├── tauri_invoke("notes_index", {})  (commands.rs:1442)
        │   ├── settings.last_vault_path
        │   ├── collect_notes(vault_path)  (commands.rs:1705)
        │   │   ├── scan vault for .md/.txt/.json files
        │   │   ├── read frontmatter for metadata
        │   │   └── extract wikilinks → Vec<NoteEntry>
        │   └── Returns Vec<NoteIndexEntry>
        └── NavContext.notes_index.set(entries)
    ├── load_all_nav_state(nav)
        ├── tauri_invoke("get_settings", {}) → SettingsStore
        ├── tauri_invoke("smart_folders_list", {})
        └── Restore discovery data (recent_notes, etc.)
    ↓
Dashboard component reads from NavContext signals
```

---

## 13. Knowledge Consistency Analysis

### 13.1 Consistency Model

| System | Update Trigger | Latency | Mechanism |
|--------|----------------|---------|-----------|
| **Search index** | ITEM_STORED (StorageManager::save) | Immediate (same thread) | EventBus pub/sub → Indexer::index_object |
| **Graph** | ITEM_STORED (StorageManager::save) | Immediate (same thread) | EventBus pub/sub → VaultGraph::add_node |
| **File tree** | refresh_tree signal bump | On next IPC call | Explicit re-fetch via tree_list |
| **Session state** | Workspace change | 800ms debounce | spawn_local → session_save IPC |
| **Settings** | settings_set IPC | Immediate (Mutex) | Direct mutation + persist |
| **History (undo/redo)** | HistoryManager::push | Immediate (in-memory) | Frontend custom event → re-fetch |

### 13.2 Inconsistency Scenarios

1. **Note edit → search stale**: User edits note content → `note_save` writes to disk → NO `ITEM_STORED` → Indexer never updates → search returns stale results. User must restart or trigger a re-capture.

2. **Note edit → graph stale**: User adds `[[wikilink]]` in editor → `note_save` → NO `ITEM_STORED` → `VaultGraph::add_node` never called → graph doesn't reflect new link. Graph view is stale until manual refresh.

3. **Note create → not indexed**: User creates note via `note_create_file` → `std::fs::write` → NO `ITEM_STORED` → note invisible to search and graph.

4. **Note delete → stale references**: `HistoryManager::note_delete` → `trash_file` → NO `ITEM_STORED` or graph removal event → Indexer and VaultGraph still reference deleted note.

5. **Settings change → frontend signal stale**: `settings_set` updates `SettingsStore.Mutex` but doesn't notify frontend signals. Components must independently sync.

6. **Frontend notes_index ≠ backend Indexer**: `NavContext.notes_index` (populated by filesystem scan via `load_notes_index`) is completely independent from `Indexer` (populated by `index_object` via ITEM_STORED). Adding a note via `note_create_file` appears in `notes_index` but not in the search `Indexer`.

### 13.3 EventBus Subscription Completeness

| Event Kind | Producers | Subscribers (production) | Coverage |
|------------|-----------|--------------------------|----------|
| `item.captured` | CaptureEngine (engine.rs:64) | None | 0% |
| `item.processing.started` | PipelineExecutor (executor.rs:122) | None | 0% |
| `item.processing.progress` | Pipeline + processors | None | 0% |
| `item.processing.completed` | Pipeline (pipeline.rs:156) | None | 0% |
| `item.processing.failed` | Pipeline (pipeline.rs:158) | None | 0% |
| `item.stored` | StorageManager (manager.rs:181) | Indexer + VaultGraph (lib.rs:162) | ~67% |
| `index.updated` | Indexer (indexer.rs:153) | None | 0% |
| `graph.updated` | VaultGraph (graph/mod.rs:345) | None | 0% |
| `item.cancelled` | WorkerPool | None | 0% |
| `item.retried` | WorkerPool | None | 0% |

**Only 1 of 10 event kinds has subscribers in production.** The Indexer and VaultGraph are the only consumers, both wired via the inline closure in `lib.rs:162-177`.

### 13.4 EventBus Synchronization

`EventBus::publish()` (bus.rs:69-76):
```rust
pub fn publish(&self, event_kind: &str, event: &Events) {
    let inner = self.inner.lock().unwrap();  // ← Mutex held during all handler calls
    if let Some(subscribers) = inner.subscribers.get(event_kind) {
        for subscriber in subscribers {
            (subscriber.handler)(event);  // ← synchronous, within lock
        }
    }
}
```

**All handlers execute synchronously within the Mutex lock.** If `Indexer::index_object()` is slow (large document), `VaultGraph::add_node()` is also delayed. No parallelism between subscribers.

---

## 14. Extensibility Assessment

### 14.1 Existing Extension Points

| Abstraction | File | Extensibility | Notes |
|-------------|------|---------------|-------|
| `CaptureHandler` trait | handler.rs:37-46 | **Full** | New handlers register via `CaptureEngine::register()`, route via dispatch |
| `Processor` trait | processor.rs:84-98 | **Full** | New processors register via `pipeline.register()` |
| `JobExecutor` trait | executor.rs:15-23 | **Full** | New executors register via `ExecutorRegistry` |
| `JobType` enum | job.rs:152-193 | **Additive** | New variants map to processor names via `JobType::name()` |
| `Queue` trait | queue.rs:16-61 | **Full** | Alternative queue backends possible |
| `CapabilityRegistry` | plugin/ | **Partial** | Has `register_builtin()` but no dynamic registration API |
| `ServiceRegistry` | registry/mod.rs | **Full** | `register()` / `resolve()` for any `Arc<dyn Any + Send + Sync>` |

### 14.2 How New Knowledge Sources Should Integrate

| New Capability | Which Pipeline Stage | Existing Abstraction | Required Changes |
|----------------|---------------------|---------------------|-----------------|
| **Syncthing sync events** | Capture | `CaptureHandler` trait | Implement `CaptureHandler` for sync changes, register in `build_default_capture_engine()` |
| **Harper diagnostics** | Processing | `Processor` trait | Implement `Processor` for Harper, register in `build_standard_pipeline()` |
| **ACP document generation** | Capture | `CaptureHandler` trait | Implement handler for ACP documents, enqueue as jobs |
| **New capture module** | Capture | `CaptureHandler` trait | Implement handler, register in `build_default_capture_engine()` |
| **Custom processor** | Processing | `Processor` trait | Implement processor, register in `build_standard_pipeline()` |

### 14.3 What Existing Abstractions Already Support

1. **`CaptureHandler` trait** — any new ingestion source can implement this trait and be registered in `build_default_capture_engine()` (engine.rs:155-178). The handler produces a `KnowledgeObject` which automatically enters the capture pipeline.

2. **`Processor` trait** — any new processing step can implement this trait and register in `build_standard_pipeline()` (pipeline.rs:235-272). Processors are ordered via the `ordering` module constants and execute sequentially in the pipeline.

3. **`JobExecutor` trait** — any new execution strategy (e.g., subprocess-based processing) can implement this trait and register in the `ExecutorRegistry` (configured in `lib.rs:108-116`).

4. **`JobType` enum** — new job types can be added (job.rs:152-193) and mapped to processor names via `JobType::name()` (job.rs:196-218).

5. **`ServiceRegistry`** — new services can be registered via `ApplicationContext::register()` and resolved via `resolve()` or typed accessors.

### 14.4 What Does NOT Support Easy Integration

1. **Backend→Frontend event delivery** — No event bridge exists. New capabilities that need to push real-time updates to the UI (progress, status, notifications) would require building a Tauri event bridge from `EventBus` to `window.emit()`.

2. **Non-capture ingestion paths** — Notes edited via `note_save` or `note_create_file` bypass the pipeline entirely. A new capability that modifies content needs its own event publication mechanism, because the existing ITEM_STORED subscriber chain is only triggered by `StorageManager::save()`.

3. **Async EventBus** — The current EventBus is synchronous. Capabilities that need async event handling (e.g., long-running diagnostics that emit progress) cannot use the existing EventBus without modification.

---

## 15. Pipeline Bottlenecks

### 15.1 Synchronous EventBus Lock Contention

**Problem**: `EventBus::publish()` (`bus.rs:70-75`) holds a `Mutex` lock during all handler execution.

**Evidence**: When `StorageManager::save()` publishes `ITEM_STORED`, the single subscriber closure (`lib.rs:162-177`) synchronously calls `StorageManager::load()` + `Indexer::index_object()` + `VaultGraph::add_node()` all within the lock. If `index_object()` processes a large document, `VaultGraph::add_node()` is blocked, and no other `publish()` calls can proceed.

**Impact**: Event processing is serialized. Large documents block graph updates.

### 15.2 Worker Polling

**Problem**: `Worker::run()` (`worker.rs:71-76`) uses `tokio::time::sleep(100ms)` polling when the queue is empty.

**Evidence**: `worker.rs:73-76` — sleeps 100ms between dequeue attempts.

**Impact**: 100ms latency for job pickup when queue transitions from empty to non-empty.

### 15.3 No Progress Event Delivery to Frontend

**Problem**: Processors emit progress via `ProgressReporter` and the pipeline publishes `ProcessingProgress` events, but **no subscriber exists** and there is **no IPC path** to deliver progress to the frontend.

**Evidence**: `PipelineEvent::ItemProcessingProgress` exists (events.rs:14) but has 0% subscription coverage (Section 13.3).

**Impact**: Users see no progress during OCR, Whisper transcription, or PDF processing. The `TaskContext` frontend state (`feedback.rs:729-730`) is never populated from the backend.

### 15.4 Duplicate Indexing Path

**Problem**: The frontend's `NavContext.notes_index` (populated by `load_notes_index` → `notes_index` IPC → filesystem scan) is completely independent from the backend `Indexer` (populated by `index_object` via ITEM_STORED).

**Evidence**: `notes_index` IPC (commands.rs:1442) calls `collect_notes()` which scans the filesystem (commands.rs:1705). `Indexer::index_object()` (indexer.rs:138) builds an inverted index from `KnowledgeObject`. These are two separate data structures with the same conceptual purpose.

**Impact**: Maintaining two indexes increases memory usage and creates inconsistency risk. The filesystem scan does not parse content the same way the processor pipeline does.

### 15.5 Direct Filesystem Writes Bypass All Pipeline Stages

**Problem**: `note_save` (recovery.rs:403) and `note_create_file` (commands.rs:608) write directly to disk, bypassing StorageManager, EventBus, Indexer, and VaultGraph.

**Impact**: Search and graph are stale after any note edit. No deduplication check. No content extraction (links, tags) re-run. No metadata enrichment.

### 15.6 No Deduplication in Direct Edit Path

**Problem**: The capture pipeline has `DuplicateDetector` processor (pipeline.rs:248) using `ContentHasher`. The direct edit path (`note_save`) does not check for duplicates.

**Impact**: Two notes with identical content can coexist if one was captured (pipeline) and one was edited directly.

---

## 16. Architectural Observations

### 16.1 One Pipeline, Two Entry Strategies

The knowledge pipeline has a **bimodal architecture**:

- **Capture path** (full pipeline): CaptureEngine → Queue → Workers → Pipeline → Storage → Index → Graph — fully event-driven, all subsystems notified.
- **Edit path** (direct writes): `note_save` → `std::fs::write` + `snapshot_note` — no events, no indexing, no graph update.

This split is the **single largest source of inconsistency** in the system. Any feature that modifies vault content directly (rather than through the capture pipeline) creates stale search and graph state.

### 16.2 EventBus Is a Dead Letter Office for Frontend

The `EventBus<PipelineEvent>` has 10 event types. In production:
- **9 event types have 0 subscribers** (all except `ITEM_STORED`)
- **`ITEM_STORED` has 1 subscriber** (the inline closure in lib.rs:162-177)
- **`INDEX_UPDATED` and `GRAPH_UPDATED` have 0 subscribers** — they are published but nobody consumes them
- **NO events reach the frontend** — there is no Tauri event bridge

The EventBus is effectively a one-hop pub/sub for Indexer + Graph only. All other events are published into a void.

### 16.3 The GraphEventBridge Paradox

`GraphEventBridge` (`graph/incremental/event_wiring.rs:26-189`) provides a sophisticated incremental update mechanism that translates `ITEM_STORED` events into targeted graph updates (new/modified/deleted node detection, snapshot management, region-based incremental rebuilds). However:

- It is **never called in production code** — only in tests (event_wiring.rs:191-232)
- Production uses the simpler inline closure in `lib.rs:162-177` which calls `VaultGraph::add_node()` directly
- The incremental subsystems (change_log, dependency_tracker, region, engine) are **complete but unused**

### 16.4 Recommendations for New Capabilities

**If new knowledge-producing capabilities (Syncthing, Harper, ACP, future capture modules) were introduced tomorrow, they should integrate as follows:**

1. **Ingest via CaptureEngine**: Any new content source should implement the `CaptureHandler` trait and register in `build_default_capture_engine()` (engine.rs:168-178). This automatically routes through the full pipeline: job queue → workers → 14 processors → StorageManager → Indexer + Graph.

2. **Processing via Processor trait**: Any new analysis (e.g., Harper grammar diagnostics, semantic similarity, entity extraction) should implement the `Processor` trait and register in `build_standard_pipeline()` (pipeline.rs:245-264). It runs after the content is loaded, before storage.

3. **Custom job types via JobType**: If the capability produces work that should be queued separately (e.g., Syncthing sync events), add a `JobType` variant (job.rs:152-192) and map it to a processor name via `JobType::name()`.

4. **CRITICAL — Build an EventBus→Tauri bridge**: For any capability that needs to push real-time updates to the frontend (progress, status, notifications), a new Tauri event bridge must be built. This would subscribe to `EventBus` events and forward them via `window.emit()`. Without this, capabilities like Harper diagnostics progress or Syncthing sync status cannot reach the UI.

5. **CRITICAL — Route note edits through StorageManager**: To ensure search and graph stay in sync when users edit notes, `note_save` should be refactored to call `StorageManager::save()` instead of `std::fs::write` directly. This would publish `ITEM_STORED` and trigger the full index + graph update cascade.

6. **Fix the dual-index problem**: The frontend's `NavContext.notes_index` (filesystem scan via `notes_index` IPC) and the backend `Indexer` (in-memory inverted index via `index_object`) serve similar purposes but operate independently. New retrieval features should consolidate on the `Indexer` as the single search source of truth.
```
