# Universal Knowledge Expansion — Evolution Roadmap

> **Programme Name:** Universal Knowledge Expansion & Evolution
> **Version:** 5.0.0
> **Status:** Active — Architecture Alignment Complete
> **Date:** 2026-07-27
> **Authority:** Nabu Architecture Board

---

## The Rule

Every implementation in this programme must satisfy **three conditions**:

1. **Gap-filling** — addresses a capability gap identified from Paperless-ngx, Anytype, Karakeep or Stirling PDF.
2. **Rust-native** — builds on Nabu's existing architecture in `crates/nabu-core/` or `src-tauri/`. Never replaces working systems.
3. **Markdown-first** — the vault remains a normal folder of Markdown files and assets. No proprietary formats, no cloud dependency, no lock-in.

---

## The Objective

Close the four capability gaps between Nabu and the best open-source knowledge tools while remaining 100% Markdown-native and local-first.

Nothing else.

---

## Existing Foundation (Do Not Rebuild)

| System | Location | What It Does |
|--------|----------|-------------|
| KnowledgeObject (22 types, 5 content variants) | `crates/nabu-core/src/models/knowledge_object.rs` | Universal typed object model with serialization |
| CaptureEngine + CaptureHandler trait | `crates/nabu-core/src/capture/engine.rs` | Handler registry, dispatch, event-driven ingest |
| StorageManager + SQLite storage | `crates/nabu-core/src/storage/manager.rs` | Object persistence with event-driven save |
| EventBus (typed pub/sub) | `crates/nabu-core/src/event_bus/bus.rs` | Decoupled service communication |
| IngestionPipeline (MIME→type routing) | `crates/nabu-core/src/capture/pipeline.rs` | Creates KnowledgeObject from raw input |
| AudioEngine (Whisper.cpp) | `crates/nabu-core/src/native/audio.rs` | Real audio transcription |
| PdfAnnotator (JSON annotations) | `crates/nabu-core/src/native/pdf.rs` | PDF annotation persistence |
| ExportEngine (Tera HTML export) | `crates/nabu-core/src/export_engine.rs` | Template-based HTML export |
| VaultGraph (petgraph, wiki-links) | `crates/nabu-core/src/graph.rs` | File-level graph with backlinks |
| Tantivy full-text index | `crates/nabu-core/src/indexer.rs` + `src-tauri/src/search.rs` | Search with path/title/content/tags/mtime |
| Dictation pill UI + Tauri commands | `dictation-pill.html` + `src-tauri/src/commands.rs` | Floating dictation widget |
| Markdown parser (GFM) | `crates/nabu-core/src/parser.rs` + `src-tauri/src/markdown/` | Tables, tasks, strikethrough |
| Template manager | `crates/nabu-core/src/template_manager.rs` | Template rendering for export |
| macOS Vision OCR placeholder | `crates/nabu-core/src/native/ocr.rs` | OCR stub — needs real implementation |
| File watcher | `crates/nabu-core/src/watcher.rs` | notify-based vault change detection |

---

## Programme 1: Paperless Capability Gap

**What Nabu already has:** OCR, PDF support, search, watchers.

**What's missing:** Knowledge Inbox, import folders, automatic filing, metadata review, duplicate detection, document timeline, batch processing.

**Estimated prompts:** 5

### 1.1 — Knowledge Inbox + Document Workflow (3 prompts)

**Prompt 1** — Inbox backend: Create `ProcessingPipeline` with `Processor` trait. Wire IngestedItem → ProcessingPipeline → Store flow. Implement `WatchFolderHandler` for import folders.

- `crates/nabu-core/src/processing/` — `Processor` trait, `ProcessingPipeline`, processing history
- `crates/nabu-core/src/capture/` — `WatchFolderHandler` reusing existing file watcher infra
- Existing `CaptureEngine` dispatches to pipeline → pipeline runs processors → `ItemProcessed` event → `StorageManager` persists

**Prompt 2** — Inbox UI: Split-pane component with queue + preview. Status indicators, keyboard shortcuts, filtering/sorting.

- `crates/nabu-ui/src/components/inbox.rs` — Queue list, detail preview, batch actions, filters
- Events from pipeline drive UI state (ItemCaptured → pending, ItemProcessed → ready)

**Prompt 3** — Document workflow: Auto-filing (detect invoice/receipt/meeting from content patterns), editable metadata review, batch approval/reject/retry wired to backend.

- `crates/nabu-core/src/processing/` — `ContentClassifier` processor (pattern-based, no ML)
- Inbox UI gains suggested destination, metadata editor, batch actions calling Tauri commands

### 1.2 — Document Intelligence (2 prompts)

**Prompt 4** — Duplicate detection + timeline extraction: Content hash (SHA-256), filename similarity check, date extraction from metadata/content.

- `crates/nabu-core/src/processing/` — `DuplicateDetector`, `TimelineExtractor` processors
- Duplicate flags appear in Inbox before storage

**Prompt 5** — OCR integration: Replace placeholder `OcrEngine` with real macOS Vision `VNRecognizeTextRequest`. Wire as processor that runs on Image, Scan, Screenshot, scanned PDF.

- `crates/nabu-core/src/native/ocr.rs` — Direct Vision framework calls. No abstraction layer.
- Confidence scores stored in metadata custom fields

---

## Programme 2: Karakeep Capability Gap

**What Nabu already has:** CaptureEngine.

**What's missing:** Browser extension, one-click capture, clipboard monitoring, screenshot capture, metadata extraction, reading queue.

**Estimated prompts:** 5

### 2.1 — Browser Capture (2 prompts)

**Prompt 6** — Safari extension: Extension manifest, popup UI, native messaging host. One-click capture of page URL + title + selected text.

- `extensions/safari/` — Safari web extension
- `src-tauri/src/` — Native messaging host, passes `CaptureRequest` to existing `CaptureEngine`
- Capture types: current page (Bookmark), selected text (Note), full HTML (Document)

**Prompt 7** — Rich capture: Save article (readability-extracted content), save YouTube (video URL + metadata), save GitHub repository (repo URL + description + stars).

- Extend Safari extension with content extraction (Reader mode)
- Handlers in `crates/nabu-core/src/capture/` registered with CaptureEngine
- Object types: AudioRecording for YouTube, Repository for GitHub repos

### 2.2 — Universal Capture (2 prompts)

**Prompt 8** — Clipboard handler + screenshot handler: Monitor NSPasteboard for URLs, images, text. Capture screen region via macOS screen capture API.

- `crates/nabu-core/src/capture/` — `ClipboardHandler`, `ScreenshotHandler`
- Both registered with existing CaptureEngine
- Clipboards triggers (configurable): manual shortcut, auto on copy

**Prompt 9** — Metadata extraction: Title, author, publication date, site name, language detection from captured web/URL content.

- `crates/nabu-core/src/processing/` — `MetadataExtractor` processor
- Extracted metadata populates KnowledgeObject metadata before Inbox

### 2.3 — Reading Queue (1 prompt)

**Prompt 10** — Reading list view: Articles, videos, repos marked as "read later". Progress tracking (unread/reading/read). Priority sorting. Archive.

- `crates/nabu-ui/src/components/reading_queue.rs` — List view with status badges
- `crates/nabu-core/src/` — ReadingQueue model (wraps KnowledgeObject query with status field)
- Depends on Inbox UI (Programme 1.1) for the component pattern

---

## Programme 3: Anytype Capability Gap

**What Nabu already has:** Graph, templates, notes, backlinks, search.

**What's missing:** Custom properties, collection views (table/board/gallery/calendar), object templates, relation editor, rich metadata panels.

**Constraint:** Not databases. Not replacing Markdown. Just richer ways to interact with Markdown files.

**Estimated prompts:** 4

### 3.1 — Custom Properties + Typed Relations (2 prompts)

**Prompt 11** — Custom properties: Property definitions stored in vault config. Property types: text, number, date, select, multi-select, URL. Property editor in note sidebar. Properties indexed in Tantivy.

- `crates/nabu-core/src/models/knowledge_object.rs` — Extend `ObjectMetadata.custom` schema
- `crates/nabu-core/src/vault_config.rs` — Property definitions
- `crates/nabu-ui/src/components/property_editor.rs` — Sidebar editor

**Prompt 12** — Typed graph entities + relation editor: Extend existing `VaultGraph` with `GraphEntity` (typed nodes) and `GraphEdge` (semantic edges). Relation picker UI for linking notes to people, projects, tags.

- `crates/nabu-core/src/graph.rs` — Add EntityType, RelationType enums. Add typed nodes alongside existing file nodes.
- `crates/nabu-ui/src/components/relation_editor.rs` — Autocomplete search, create new entity, typed relationship picker

### 3.2 — Collection Views + Object Templates (2 prompts)

**Prompt 13** — Collection views: Table view (notes as rows, properties as columns), Board view (Kanban grouped by select property), Gallery view (card layout), Calendar view (grouped by date property). All filterable/sortable.

- `crates/nabu-ui/src/components/collections/` — View switcher, table, board, gallery, calendar components
- Backed by Tantivy queries with property filters — no separate database

**Prompt 14** — Object templates: Template editor (frontmatter defaults + content skeleton), template picker on note creation, per-folder templates. Extends existing `TemplateManager`.

- `crates/nabu-core/src/template_manager.rs` — Extend with property presets
- `crates/nabu-ui/src/components/template_picker.rs` — Template chooser dialog
- Existing Bug Report, Meeting Note, Project Brief remain

---

## Programme 4: Stirling PDF Capability Gap

**What Nabu already has:** PDF viewing (pdfjs-dist), PDF annotation (PdfAnnotator), OCR placeholder.

**What's missing:** Merge, split, compress, rotate, convert, extract pages, extract images, fill forms, text extraction, scanned PDF OCR.

**Estimated prompts:** 2

### 4.1 — Complete PDF Toolkit (2 prompts)

**Prompt 15** — PDF manipulation + text extraction: Merge, split, extract pages, extract images, rotate, compress, convert, fill forms all via PDFKit (macOS native). Text extraction for born-digital PDFs. Scanned PDF OCR (reuses Programme 1.2 OCR).

- `crates/nabu-core/src/native/pdf.rs` — Extend `PdfAnnotator` with all operations
- PDFKit for macOS-native operations (no third-party PDF libraries)
- Annotation extraction stored as graph edges
- Text extraction indexed in Tantivy for full-text search

---

## Summary

| Programme | Phases | Prompts | Total |
|-----------|--------|---------|-------|
| 1. Paperless | 1.1 Inbox + Workflow, 1.2 Intelligence | 3 + 2 | 5 |
| 2. Karakeep | 2.1 Browser, 2.2 Universal, 2.3 Reading Queue | 2 + 2 + 1 | 5 |
| 3. Anytype | 3.1 Properties + Relations, 3.2 Views + Templates | 2 + 2 | 4 |
| 4. Stirling PDF | 4.1 Complete PDF Toolkit | 2 | 2 |
| **Total** | | | **16** |

**Parallel execution:** Programmes 3 (Anytype) and 4 (Stirling PDF) depend only on existing foundation — can run alongside Programmes 1 and 2. Safari extension (Programme 2.1) and PDF toolkit (Programme 4) have no cross-dependencies.

