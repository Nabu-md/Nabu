# Universal Knowledge Capture Expansion — Master Programme

> **Programme Name:** Universal Knowledge Capture Expansion
> **Version:** 1.1.0
> **Status:** Proposed
> **Date:** 2026-07-27
> **Document Classification:** Internal — Architecture Authority
> **Authority:** Nabu Architecture Board
> **Supersedes:** None (this is the foundational expansion programme)

---

## Revision History

| Version | Date | Author | Change |
|---------|------|--------|--------|
| 1.0.0 | 2026-07-27 | Architecture Authority | Initial release |
| 1.1.0 | 2026-07-27 | Architecture Authority | Refinement pass: strengthened product vision, added Knowledge Inbox, expanded Universal Knowledge Objects, clarified AI philosophy, added product capability mapping, expanded principles, improved dependency graph and critical path |

---

## 1. Executive Summary

### 1.1 Programme Vision

> Every piece of knowledge enters Nabu exactly once, is understood exactly once, and becomes permanently discoverable.

Nabu is becoming **The Personal Knowledge Vault** — a local-first, Rust-powered platform that serves as the canonical home for every piece of personal knowledge. The objective is not to replace individual applications, but to become the unified system into which all knowledge eventually flows.

Every piece of personal knowledge belongs in one place:

- **Notes** — fleeting thoughts, meeting notes, daily journals
- **Documents** — reports, letters, contracts, proposals
- **PDFs** — research papers, manuals, invoices, receipts
- **Bookmarks** — web pages, articles, resources
- **Research** — literature reviews, annotated sources, bibliographies
- **Screenshots** — visual captures, error states, design references
- **Recordings** — voice memos, meetings, lectures, podcasts
- **Repositories** — code, configuration, documentation
- **Scans** — physical documents, whiteboards, business cards
- **Future knowledge formats** — the system must be extensible to knowledge types not yet imagined

The programme defines the architectural blueprint for this transformation without compromising Nabu's core philosophy: local-first, markdown-first where appropriate, graph-first, AI-assisted never AI-dependent, Rust-first backend, thin frontend, modular, extensible, self-hostable, and private by design.

### 1.2 Strategic Objectives

| Objective | Description | Success Indicator |
|-----------|-------------|-------------------|
| **Capture Everything** | Ingest knowledge from every supported channel — markdown, PDF, images, audio, video, email, browser, drag-and-drop, share sheets, watch folders, office docs, Git repos, ZIP archives, APIs | Every knowledge type has a registered ingestion path |
| **Understand Everything** | Automatically process every captured item with OCR, metadata extraction, entity extraction, semantic tagging, AI summaries, embeddings, duplicate detection, relationship discovery, timeline extraction, language detection, classification | Every ingested item receives a full enrichment pipeline pass |
| **Connect Everything** | Every knowledge object becomes a node in the graph with typed relationships to notes, documents, projects, people, organisations, books, research papers, code, bookmarks, media, tasks, and future object types | The graph contains typed edges between all knowledge objects |
| **Retrieve Everything** | Universal search across all knowledge types by semantic meaning, not file type | Search returns mixed-type results ranked by relevance |

### 1.3 Why This Programme Exists

Nabu currently excels at Markdown note management with a growing graph engine, search index, and PDF viewer. However, the knowledge capture surface is narrow — users must manually create or import files. The processing pipeline is limited to tag extraction and wiki-link parsing. The graph model tracks file-level relationships but lacks semantic understanding. Search is full-text only, with vector search in early stages.

The market has proven that users need a single destination for personal knowledge. Projects like Paperless-ngx (document ingestion/OCR), Karakeep (universal capture), Anytype (knowledge objects/relationships), and Stirling PDF (document tooling) have demonstrated demand for these capabilities. Nabu will not clone any of them. Instead, it will absorb the underlying architectural capabilities that made them successful and integrate them into a unified, cohesive product.

### 1.4 Long-Term Product Positioning

Nabu will remain:

- **Local-first** — all data lives on the user's machine; no cloud dependency
- **Markdown-first where appropriate** — Markdown remains the primary authoring format, but not the only storage format
- **Graph-first** — the knowledge graph is the primary data model, not an afterthought
- **AI-assisted, never AI-dependent** — AI enhances workflows but the product is fully usable without it
- **Rust-first backend** — all business logic, indexing, and processing lives in Rust
- **Thin frontend** — the renderer is a presentation layer; all logic lives in the main process or Rust crates
- **Modular and extensible** — new knowledge types, processors, and integrations are added via plugins
- **Self-hostable** — no external services required for core functionality
- **Privacy by design** — no telemetry, no cloud sync, no data exfiltration

Nabu will NOT become:

- A Paperless clone (document management is a subset, not the whole)
- An Anytype clone (object types are a capability, not the entire model)
- A Karakeep clone (capture is one pillar, not the only function)
- A Stirling PDF clone (PDF tooling is a capability, not the product)
- A password manager (out of scope)
- A Notion clone (not a cloud-first, all-in-one workspace)
- Unrelated productivity software (stays focused on knowledge)

---

### 1.5 Knowledge Inbox

The **Knowledge Inbox** is the primary review surface for newly ingested knowledge. It is a first-class product capability built on top of the Universal Capture Pipeline, serving as the user-facing orchestration layer between raw capture and permanent storage.

The Inbox is not merely an implementation detail of the Capture Engine. It is the operational hub where users review, validate, correct, and approve incoming knowledge before it becomes part of their vault.

#### 1.5.1 Inbox Responsibilities

| Responsibility | Description |
|---------------|-------------|
| **Drag-and-drop ingestion** | Accept files, URLs, and text dropped directly into the Inbox |
| **Import queue** | Maintain a visible queue of pending items awaiting review |
| **Processing status** | Show real-time progress for each item (captured → processing → ready → stored) |
| **Thumbnails** | Generate and display preview thumbnails for visual content (images, PDFs, videos) |
| **Metadata preview** | Show extracted metadata (title, author, date, source, MIME type, size) |
| **Suggested destination** | Recommend the most appropriate collection, folder, or tag based on content analysis |
| **Confidence indicators** | Display confidence scores for AI-generated suggestions (destination, tags, entities) |
| **Capture confidence** | Show overall ingestion confidence with reasons (matched existing project, detected invoice, known sender, OCR quality high) |
| **Accept suggestion** | One-click approval of the suggested destination and metadata |
| **Choose another destination** | Override the suggestion with manual selection |
| **Bulk approval** | Approve multiple items at once with shared settings |
| **Retry failed imports** | Re-process items that failed during capture or processing |
| **Watch-folder activity** | Show real-time activity from configured watch folders |
| **Processing history** | Log all processing steps, errors, and corrections for each item |

#### 1.5.2 Inbox Architecture

The Inbox is a renderer-side component that communicates with the main process via IPC. It subscribes to events from the Capture Engine and Processing Pipeline:

```
CaptureEngine → publish(ItemCaptured) → KnowledgeInbox subscribes
ProcessingPipeline → publish(ItemProcessed) → KnowledgeInbox subscribes
ProcessingPipeline → publish(ProcessingFailed) → KnowledgeInbox subscribes
User action (accept/reject/retry) → IPC → CaptureEngine / ProcessingPipeline
```

#### 1.5.3 Inbox States

| State | Description | User Action Available |
|-------|-------------|----------------------|
| **Pending** | Item captured, awaiting processing | Retry, Cancel |
| **Processing** | Item is being processed by the pipeline | Cancel |
| **Ready** | Processing complete, awaiting user review | Accept, Reject, Edit, Retry |
| **Stored** | Item approved and stored in vault | Open, Move, Delete |
| **Failed** | Processing failed after max retries | Retry, Delete, View Error |
| **Rejected** | User rejected the item | Restore, Delete Permanently |

#### 1.5.5 Capture Confidence

Every ingestion produces a confidence score that drives the Inbox UI, automation rules, and user trust. Confidence is calculated from multiple signals:

| Signal | Description | Weight |
|--------|-------------|--------|
| **Source match** | Item matches a known source (known sender, watched folder, trusted browser clip) | High |
| **Content match** | Item content matches existing knowledge (duplicate detection, similar documents) | Medium |
| **Type detection** | MIME type and content sniffing agree on object type | Medium |
| **OCR quality** | OCR confidence score for scanned documents and images | Medium |
| **Metadata completeness** | Percentage of metadata fields successfully extracted | Low |
| **Sender reputation** | Known sender domain or previous interactions | High |
| **Format familiarity** | Item matches a known format template (invoice, receipt, meeting note) | Medium |

**Confidence display in Inbox:**

```
┌─────────────────────────────────────────────────────────────┐
│  Confidence: 97%                                            │
│  ████████████████████████████████████████████████████░░░░  │
│                                                             │
│  Reasons:                                                   │
│  • Matched existing project "Thesis"                        │
│  • Detected invoice format                                  │
│  • Known sender: supervisor@university.edu                  │
│  • OCR quality: high (98% character accuracy)               │
│                                                             │
│  Suggested Collection: Research → Thesis                    │
│  Suggested Tags: #invoice #thesis #supervisor               │
└─────────────────────────────────────────────────────────────┘
```

**Confidence-driven automation:**

- **High confidence (>90%)** — Auto-approve and store; minimal user intervention required
- **Medium confidence (70-90%)** — Show in Inbox with suggestions; user approves or corrects
- **Low confidence (<70%)** — Flag for manual review; show all available metadata for user decision

#### 1.5.6 Inbox UI Requirements

- **Split-pane layout** — queue on the left, detail preview on the right
- **Keyboard shortcuts** — approve (Enter), reject (Delete), retry (Cmd+R), open (Cmd+O)
- **Batch operations** — select multiple items, apply bulk actions
- **Filtering** — filter by source type, status, date, confidence score
- **Sorting** — sort by date, confidence, source, status
- **Search** — search within Inbox items by content, metadata, or source
- **Drag-and-drop reordering** — reorder queue priority by dragging items

---

### 1.6 Programme Scope

This programme defines the boundaries of Nabu's evolution into The Personal Knowledge Vault.

#### 1.6.1 In Scope

| Area | Description |
|------|-------------|
| **Knowledge capture** | Universal ingestion from all supported channels |
| **Knowledge processing** | OCR, metadata extraction, entity extraction, tagging, embeddings, duplicate detection |
| **Knowledge graph** | Typed entities, semantic relationships, entity resolution, graph queries |
| **Universal search** | Hybrid search (full-text + vector + graph), faceted filtering, mixed-type results |
| **AI enrichment** | Local embeddings, summarisation, entity extraction, relationship suggestion |
| **Automation** | Rule-based processing, on-ingest workflows, scheduled tasks |
| **Integrations** | Email, Git, APIs, browser, mobile, watch folders |
| **Security** | Vault encryption, access control, audit logging |
| **Platform** | Cross-platform desktop (macOS, Linux, Windows), mobile capture |
| **Extensibility** | Plugin architecture, custom object types, custom processors |

#### 1.6.2 Out of Scope

| Area | Description |
|------|-------------|
| **Password management** | Not a knowledge management concern |
| **Office productivity suites** | Word processors, spreadsheets, and presentation software are creation tools, not knowledge destinations |
| **Specialised creative software** | Image editors, video editors, DAWs are creation tools |
| **Proprietary AI chatbot** | Nabu will not build a conversational assistant; it will remain compatible with external AI tools |
| **Cloud-first workflows** | Nabu is local-first; cloud sync is optional and user-managed |
| **Real-time collaboration** | Multi-user editing is not a programme objective |
| **Social features** | Sharing, following, or social graph are out of scope |
| **Enterprise features** | SSO, team management, admin consoles are out of scope |

#### 1.6.3 Programme Boundaries

The programme is bounded by:

- **Local-first** — all core functionality works without network access
- **Privacy** — no telemetry, no data exfiltration, no external API calls by default
- **Extensibility** — new capabilities are added via plugins, not core modification
- **Knowledge management** — the focus is on capturing, organising, and retrieving knowledge
- **Unified capture** — all knowledge flows through a single ingestion pipeline
- **Modular architecture** — bounded contexts communicate via events, not direct coupling

## 2. Maturity Assessment

### 2.1 Current State vs Target State

| Category | Current State | Target State | Gap | Priority |
|----------|--------------|--------------|-----|----------|
| **Knowledge Capture** | Manual file creation; watch folder for `.md` files; basic drag-and-drop into vault | Universal ingestion engine supporting 15+ input types with automated pipeline | Capture surface is 1 type (Markdown files); target is 15+ | **Critical** |
| **Knowledge Processing** | Tag extraction from frontmatter and inline `#tags`; wiki-link parsing; basic markdown-to-HTML | Full processing pipeline: OCR, metadata extraction, entity extraction, semantic tagging, AI summaries, embeddings, duplicate detection, relationship discovery, timeline extraction, language detection, classification | Processing is limited to 2 extractors (tags, wiki-links); target is 10+ processors | **Critical** |
| **Knowledge Graph** | File-level graph with wiki-link edges; `petgraph`-based `VaultGraph` and `GraphEngine`; backlink resolution | Typed knowledge graph with entity nodes (Person, Org, Project, Book, Paper, Code, Bookmark, Media, Task) and semantic edges | Graph is file-centric, not entity-centric; no typed relationships | **Critical** |
| **AI** | Whisper.cpp dictation (macOS only); no embeddings; no LLM integration; no AI summaries | AI enrichment pipeline: embeddings (BGE-micro), summarisation, entity extraction, semantic search, relationship suggestion | AI is limited to speech-to-text; no NLP/LLM capabilities | **High** |
| **Search** | Tantivy full-text search across content, title, tags, path; vector search in early stages | Universal semantic search across all knowledge types with hybrid ranking (full-text + vector + graph signals) | Search is single-index, single-type; no semantic ranking or graph signals | **High** |
| **Documents** | PDF viewer via `pdfjs-dist`; basic PDF rendering; no OCR; no annotation extraction | Full document processing: PDF text extraction, OCR on scanned PDFs, annotation parsing, metadata extraction, format conversion | PDF is view-only; no extraction or processing | **High** |
| **Browser Capture** | No browser integration | Browser extension / native messaging for clipping, bookmark import, tab capture | No browser capture capability exists | **Medium** |
| **Mobile Capture** | No mobile support | Share sheet integration (iOS/Android); camera capture; voice memo capture | No mobile presence | **Medium** |
| **Object Modelling** | 6 domain models (Note, Vault, Workspace, Tag, GraphNode, Attachment) | Universal knowledge object model with 12+ typed objects and extensible schema | Model is note-centric; no support for documents, people, projects, etc. | **Critical** |
| **Automation** | No automation engine | Rule-based automation: on-ingest processing, auto-tagging, duplicate detection, relationship inference, scheduled tasks | No automation exists | **Medium** |
| **Security** | Local-only; no encryption at rest; no access control | Encryption at rest (vault-level); optional biometric unlock; audit logging; permission model | No encryption or access control | **High** |
| **Integrations** | None | Email import; Git repo ingestion; API webhooks; calendar integration; task manager sync; cloud storage connectors | Zero integrations | **Medium** |
| **User Experience** | Functional but note-centric; no unified knowledge browsing | Knowledge dashboard; unified browse across all types; contextual sidebar; smart collections; timeline view | UX is file-tree centric; no knowledge-centric navigation | **High** |
| **Platform Services** | Single-platform desktop (macOS primary); no Linux/Windows parity | Cross-platform desktop (macOS, Linux, Windows); mobile companion; optional cloud relay | Platform coverage is 1 of 3 desktop targets | **Medium** |

### 2.2 Detailed Gap Analysis

#### 2.2.1 Knowledge Capture Gap

**Current:** Users create Markdown files in a vault directory. The file watcher (`notify`-backed) detects changes and triggers re-indexing. Drag-and-drop into the vault folder works at the OS level but is not handled by Nabu. The `VaultService.scan()` method enumerates files but only returns `FileEntry` structs with path, name, and mtime.

**Gap:** There is no unified ingestion pipeline. No support for PDFs (import, not just view), images (OCR), audio (transcription), video (frame extraction), email (MIME parsing), browser clipping (extension/protocol), share sheet (mobile), watch folders beyond the vault root, office documents (DOCX, XLSX, PPTX), Git repositories (commit history as knowledge), ZIP archives (extract and index), or APIs (webhook ingestion).

**Required Capability:** A `CaptureEngine` that accepts any input type, normalises it to an internal representation, and feeds it into the processing pipeline. Each input type is a `CaptureSource` with a `CaptureHandler` implementation.

#### 2.2.2 Knowledge Processing Gap

**Current:** `parser.rs` provides `parse_markdown_to_html()`, `extract_tags()`, and `extract_block_refs()`. The `indexer.rs` in `nabu-core` builds a Tantivy index with path, content, tags, and mtime fields. The `graph.rs` in `nabu-core` extracts wiki-links and builds a file-level graph.

**Gap:** No OCR, no metadata extraction beyond file system metadata, no entity extraction, no semantic tagging, no AI summaries, no embeddings, no duplicate detection, no relationship discovery beyond wiki-links, no timeline extraction, no language detection, no classification.

**Required Capability:** A `ProcessingPipeline` that applies a configurable chain of processors to each ingested item. Each processor is a Rust trait object that can be enabled/disabled per vault or globally.

#### 2.2.3 Knowledge Graph Gap

**Current:** The graph is built from wiki-links between Markdown files. `GraphEngine` in `src-tauri/src/graph.rs` uses `petgraph::StableGraph<String, EdgeKind>` with `NodeKind::File` and `NodeKind::Tag`. Edges are `EdgeKind::Wikilink`, `EdgeKind::Embed`, `EdgeKind::Backlink`.

**Gap:** The graph is file-centric, not entity-centric. There are no typed nodes (Person, Organisation, Project, Book, etc.). There are no semantic edges (mentions, cites, collaborates_with, belongs_to). There is no graph traversal API beyond neighbours and connected components.

**Required Capability:** A `KnowledgeGraph` that supports typed nodes and edges, entity resolution (merging duplicate entities), graph queries (Cypher-like or pattern matching), and incremental updates.

#### 2.2.4 AI Gap

**Current:** Whisper.cpp integration for macOS dictation (`whisper-rs` crate, optional `native` feature). No embeddings. No LLM integration. No AI-powered features beyond speech-to-text.

**Gap:** No semantic search (embeddings), no summarisation, no entity extraction, no relationship suggestion, no auto-tagging, no duplicate detection via similarity.

**Required Capability:** An `AIEnrichmentPipeline` that runs locally (on-device models) and optionally connects to local LLM servers. Embeddings use `sentence-transformers`-equivalent Rust crates. All AI processing is optional and configurable per vault.

#### 2.2.5 Search Gap

**Current:** Tantivy full-text search with fields for vault_id, path, title, content, tags, mtime. Vector search exists in `src/main/services/vector.ts` (TypeScript) using BGE-micro embeddings. Search is limited to notes.

**Gap:** No hybrid search (full-text + vector + graph signals). No search across non-note types (PDFs, images, bookmarks, etc.). No faceted search. No search result ranking beyond TF-IDF. No search suggestions or auto-complete.

**Required Capability:** A `UniversalSearchEngine` that indexes all knowledge object types, supports hybrid ranking, faceted filtering, and returns mixed-type results.

---

## 2.3 Product Capability Mapping

This section maps external inspirations to the architectural capabilities Nabu will absorb. These are not competitors to copy; they represent proven patterns that Nabu will integrate into a unified, cohesive product.

### 2.3.1 Paperless-ngx

| Capability Users Value | How Nabu Implements It | How Nabu Extends It |
|------------------------|------------------------|---------------------|
| Document ingestion with OCR | `CaptureEngine` with `FileDropHandler` and `WatchFolderHandler`; OCR via `tesseract-rs` or platform-native OCR | Universal capture — not just documents, but all knowledge types; OCR is one processor in a configurable pipeline |
| Tag-based organisation | `SemanticTagger` processor; tag extraction from frontmatter and inline `#tags` | Typed graph relationships — tags become `EntityType::Tag` nodes with typed edges to any object |
| Full-text search | `UniversalSearchEngine` with Tantivy | Hybrid search — full-text + vector + graph signals across all object types |
| Metadata extraction | `MetadataExtractor` processor | Metadata is first-class — every object carries a `MetadataEnvelope` with processing history |
| Duplicate detection | `DuplicateDetector` processor | Content hash + semantic similarity + graph-based duplicate detection |

### 2.3.2 Karakeep

| Capability Users Value | How Nabu Implements It | How Nabu Extends It |
|------------------------|------------------------|---------------------|
| Universal capture (browser, mobile, API) | `CaptureEngine` with handlers for `BrowserClip`, `ShareSheet`, `ApiWebhook`, `Screenshot` | Knowledge Inbox — the primary review surface for all captured items with confidence indicators and suggested destinations |
| Bookmark management | `Bookmark` object type; `BrowserClipHandler` | Bookmarks are first-class knowledge objects with typed relationships to notes, documents, and people |
| Tagging and organisation | `SemanticTagger` processor; `AutomationEngine` for auto-tagging | Graph-first organisation — objects are connected by typed relationships, not just tags |
| Search across captured items | `UniversalSearchEngine` | Hybrid search with faceted filtering and mixed-type results |
| Bulk operations | `KnowledgeInbox` with batch approval, retry, and bulk actions | Processing history and audit trail for all bulk operations |

### 2.3.3 Anytype

| Capability Users Value | How Nabu Implements It | How Nabu Extends It |
|------------------------|------------------------|---------------------|
| Knowledge objects with typed schemas | `KnowledgeObject` model with `ObjectType` enum and extensible `Custom(String)` variant | Universal objects — every knowledge type is a `KnowledgeObject`; schemas are extensible via plugins |
| Relations between objects | `Relation` model with `RelationType` enum; `KnowledgeGraph` with typed edges | Semantic edges — `Mentions`, `Cites`, `CollaboratesWith`, `BelongsTo`, `PartOf`, `SimilarTo`, `DuplicateOf`, `DerivedFrom` |
| Graph-based navigation | `KnowledgeGraph` with traversal API; graph visualisation in renderer | Graph-first navigation — the graph is the primary data model, not an afterthought |
| Local-first, self-hostable | All data stored locally; no cloud dependency | Rust-first backend — all business logic in Rust for performance and safety |
| Extensible object types | `ObjectType::Custom(String)`; plugin interface for new types | Plugin architecture — new object types added via dynamic libraries or WASM modules |

### 2.3.4 Stirling PDF

| Capability Users Value | How Nabu Implements It | How Nabu Extends It |
|------------------------|------------------------|---------------------|
| PDF text extraction | `PdfEngine` in `nabu-core`; `pdfjs-dist` for rendering | PDF is a knowledge object — extracted text becomes searchable content; PDFs are connected to related objects in the graph |
| PDF OCR | `OcrEngine` processor; `tesseract-rs` or platform-native OCR | OCR is a processor in the pipeline — applies to images, scanned PDFs, screenshots, and any visual content |
| PDF metadata extraction | `MetadataExtractor` processor | Metadata is first-class — extracted metadata populates the `MetadataEnvelope` |
| Document format conversion | `PdfEngine` with conversion capabilities | Documents are universal objects — conversion is a processor that transforms `ObjectContent` |
| Annotation extraction | `PdfEngine` with annotation parsing | Annotations become graph edges — highlights and comments link to the source document and related objects |

---

## 3. Architecture / Foundation

### 3.1 System Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│                        Nabu Application                         │
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌───────────────────┐  │
│  │  Renderer    │    │  Preload    │    │  Main Process     │  │
│  │  (Leptos     │    │  (context   │    │  (Electron/Tauri) │  │
│  │   WASM)      │    │   bridge)   │    │                   │  │
│  │              │    │             │    │  ┌───────────────┐ │  │
│  │  Features:   │    │  Typed      │    │  │  Services     │ │  │
│  │  - notes     │    │  electronAPI│    │  │  - Vault      │ │  │
│  │  - search    │    │             │    │  │  - Capture    │ │  │
│  │  - graph     │    │             │    │  │  - Processing │ │  │
│  │  - pdf       │    │             │    │  │  - Search     │ │  │
│  │  - settings  │    │             │    │  │  - AI         │ │  │
│  │  - widgets   │    │             │    │  │  - Graph      │ │  │
│  │  - vault     │    │             │    │  │  - Automation │ │  │
│  │              │    │             │    │  └───────────────┘ │  │
│  └─────────────┘    └─────────────┘    │  ┌───────────────┐ │  │
│                                        │  │  IPC Handlers │ │  │
│                                        │  └───────────────┘ │  │
│                                        │  ┌───────────────┐ │  │
│                                        │  │  Adapters     │ │  │
│                                        │  │  - FileSystem │ │  │
│                                        │  │  - Clipboard  │ │  │
│                                        │  │  - Database   │ │  │
│                                        │  │  - AI         │ │  │
│                                        │  │  - Network    │ │  │
│                                        │  └───────────────┘ │  │
│                                        └───────────────────┘  │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │              nabu-core (Rust Crate)                       │ │
│  │  ┌─────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐ │ │
│  │  │ Parser  │ │ Indexer  │ │ Graph    │ │ Vault       │ │ │
│  │  │         │ │          │ │ Engine   │ │ Config      │ │ │
│  │  └─────────┘ └──────────┘ └──────────┘ └─────────────┘ │ │
│  │  ┌─────────┐ ┌──────────┐ ┌──────────┐ ┌─────────────┐ │ │
│  │  │ OCR     │ │ PDF      │ │ Whisper  │ │ Template    │ │ │
│  │  │ Engine  │ │ Engine   │ │ (native) │ │ Manager     │ │ │
│  │  └─────────┘ └──────────┘ └──────────┘ └─────────────┘ │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │              Data Storage Layer                           │ │
│  │  ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌────────────┐ │ │
│  │  │ Tantivy  │ │ SQLite    │ │ File     │ │ Vector     │ │ │
│  │  │ (Search) │ │ (Metadata)│ │ System   │ │ Index      │ │ │
│  │  └──────────┘ └───────────┘ └──────────┘ └────────────┘ │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Core Domains

| Domain | Responsibility | Owner | Key Types |
|--------|---------------|-------|-----------|
| **Capture** | Ingest knowledge from any source | `CaptureEngine` (main) | `CaptureSource`, `CaptureHandler`, `IngestionRequest` |
| **Processing** | Transform raw items into enriched knowledge objects | `ProcessingPipeline` (main) | `KnowledgeObject`, `Processor`, `ProcessingResult` |
| **Graph** | Manage typed knowledge graph with entities and relationships | `KnowledgeGraph` (nabu-core + main) | `GraphNode`, `GraphEdge`, `EntityType`, `RelationType` |
| **Search** | Universal hybrid search across all knowledge types | `UniversalSearchEngine` (main) | `SearchQuery`, `SearchResult`, `SearchFacet` |
| **AI** | Local AI enrichment: embeddings, summarisation, extraction | `AIEnrichmentPipeline` (main) | `EmbeddingModel`, `Summariser`, `Extractor` |
| **Storage** | Persist knowledge objects, metadata, and indexes | `StorageManager` (main) | `VaultStore`, `ObjectStore`, `IndexStore` |
| **Security** | Encryption, access control, audit | `SecurityManager` (main) | `VaultKey`, `Permission`, `AuditEntry` |
| **Automation** | Rule-based processing and workflows | `AutomationEngine` (main) | `Rule`, `Trigger`, `Action` |
| **Integration** | External connectors (email, Git, API, etc.) | `IntegrationHub` (main) | `Connector`, `SyncProfile`, `Webhook` |
| **Collections** | Dynamic, user-defined groupings of knowledge objects | `CollectionManager` (main) | `Collection`, `CollectionQuery`, `CollectionView` |

### 3.3 Domain-Driven Architecture

The programme adopts a **Domain-Driven Design** approach with the following bounded contexts:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Nabu Bounded Contexts                       │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  Capture      │  │  Processing  │  │  Knowledge Graph   │  │
│  │  Context      │  │  Context     │  │  Context           │  │
│  │               │  │              │  │                    │  │
│  │  CaptureSource│  │  Knowledge   │  │  Entity            │  │
│  │  CaptureHandler│  │  Object     │  │  Relation          │  │
│  │  Ingestion    │  │  Processor   │  │  GraphTraversal    │  │
│  │  Pipeline     │  │  Pipeline    │  │  Query             │  │
│  └──────────────┘  └──────────────┘  └────────────────────┘  │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  Search       │  │  AI          │  │  Storage           │  │
│  │  Context      │  │  Context     │  │  Context           │  │
│  │               │  │              │  │                    │  │
│  │  SearchQuery  │  │  Embedding   │  │  VaultStore        │  │
│  │  SearchResult │  │  Model       │  │  ObjectStore       │  │
│  │  SearchFacet  │  │  Summariser  │  │  IndexStore        │  │
│  │  HybridRanker │  │  Extractor   │  │  MetadataStore     │  │
│  └──────────────┘  └──────────────┘  └────────────────────┘  │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │  Security     │  │  Automation  │  │  Integration       │  │
│  │  Context      │  │  Context     │  │  Context           │  │
│  │               │  │              │  │                    │  │
│  │  VaultKey     │  │  Rule        │  │  Connector         │  │
│  │  Permission   │  │  Trigger     │  │  SyncProfile       │  │
│  │  AuditEntry   │  │  Action      │  │  Webhook           │  │
│  └──────────────┘  └──────────────┘  └────────────────────┘  │
│                                                                 │
│  ┌──────────────┐                                             │
│  │  Collections │                                             │
│  │  Context      │                                             │
│  │               │                                             │
│  │  Collection  │                                             │
│  │  CollectionQuery│                                           │
│  │  CollectionView│                                            │
│  └──────────────┘                                             │
└─────────────────────────────────────────────────────────────────┘
```

### 3.4 Service Boundaries

Each service owns exactly one business capability. Services communicate via the typed event bus (`appEventBus`) and IPC. No service imports another service's internal types directly — all cross-service communication goes through events or shared schemas.

| Service | Domain | Responsibilities | Events Published |
|---------|--------|-----------------|------------------|
| `CaptureEngine` | Capture | Register handlers, accept ingestion requests, normalise inputs, produce `IngestionResult` | `ItemCaptured`, `CaptureFailed` |
| `ProcessingPipeline` | Processing | Apply processor chain to captured items, produce enriched `KnowledgeObject` | `ItemProcessed`, `ProcessingFailed` |
| `KnowledgeGraph` | Graph | Manage typed nodes/edges, entity resolution, graph queries, traversal | `GraphUpdated`, `EntityResolved` |
| `UniversalSearchEngine` | Search | Index all object types, hybrid ranking, faceted search, suggestions | `IndexUpdated`, `SearchCompleted` |
| `AIEnrichmentPipeline` | AI | Generate embeddings, summarise, extract entities, detect duplicates | `EmbeddingsGenerated`, `SummaryCreated` |
| `StorageManager` | Storage | Vault CRUD, object persistence, index management, backup | `VaultOpened`, `VaultClosed`, `ItemStored` |
| `SecurityManager` | Security | Encryption, decryption, key management, permission checks | `VaultLocked`, `VaultUnlocked` |
| `AutomationEngine` | Automation | Evaluate rules, trigger actions, schedule tasks | `RuleTriggered`, `ActionExecuted` |
| `IntegrationHub` | Integration | Manage connectors, sync profiles, webhook endpoints | `SyncCompleted`, `WebhookReceived` |
| `CollectionManager` | Collections | Manage dynamic collections, evaluate collection queries, render collection views | `CollectionCreated`, `CollectionUpdated`, `CollectionViewed` |

### 3.5 Core Data Models

#### 3.5.1 Knowledge Object Model

The `KnowledgeObject` is the fundamental abstraction of Nabu's knowledge system. Every piece of personal knowledge — regardless of source, format, or type — becomes a specialised Knowledge Object. This is not a file-centric model; it is a knowledge-centric model where the unit of organisation is the *thing you know*, not the *file that contains it*.

**Universal Knowledge Objects** share a common architectural model while allowing specialised schemas. Every object type inherits the core `KnowledgeObject` structure (id, vault_id, timestamps, source, metadata, tags, relations) and extends it with type-specific content and properties.

The following knowledge types are first-class citizens in the Nabu object model:

| Object Type | Description | Example Content |
|-------------|-------------|-----------------|
| **Note** | Free-form text, thoughts, ideas | Meeting notes, daily journal, brainstorming |
| **Document** | Structured text documents | Reports, letters, proposals, contracts |
| **PDF** | Portable document format | Research papers, manuals, invoices, receipts |
| **Image** | Raster graphics | Screenshots, photos, diagrams, scans |
| **Receipt** | Financial transaction records | Store receipts, invoices, expense records |
| **Invoice** | Billing documents | Client invoices, vendor bills, purchase orders |
| **Meeting** | Scheduled gatherings | Meeting notes, agendas, action items, attendees |
| **Person** | Individuals | Contacts, authors, colleagues, family members |
| **Organisation** | Companies, institutions, groups | Employers, vendors, clubs, government agencies |
| **Project** | Coordinated endeavours | Work projects, personal projects, research initiatives |
| **Research Paper** | Academic publications | Journal articles, conference papers, theses |
| **Book** | Published works | Fiction, non-fiction, textbooks, reference books |
| **Course** | Educational programmes | Online courses, workshops, training programmes |
| **Website** | Web resources | Articles, blogs, documentation sites |
| **Bookmark** | Saved web references | URLs, web pages, online resources |
| **Repository** | Code and configuration | Git repos, code snippets, configuration files |
| **Audio Recording** | Sound recordings | Voice memos, meetings, lectures, podcasts |
| **Video** | Moving image recordings | Screen recordings, tutorials, presentations |
| **Scan** | Digitised physical documents | Scanned papers, whiteboards, business cards |
| **Screenshot** | Screen captures | Error states, design references, visual notes |
| **Custom** | User-defined types | Any future knowledge format not yet imagined |

**Key architectural principles of Universal Knowledge Objects:**

1. **One model, many types** — All objects share the `KnowledgeObject` struct; type-specific behaviour is encoded in `ObjectType` and `ObjectContent` variants
2. **Extensible by design** — New object types are added via the `Custom(String)` variant and plugin system; no core code changes required
3. **Relations are universal** — Any object can relate to any other object via typed `Relation` edges; the graph is the primary data model
4. **Metadata is first-class** — Every object carries a `MetadataEnvelope` with source, timestamps, processing history, and custom properties
5. **Content is polymorphic** — `ObjectContent` supports Markdown, PlainText, Html, Binary, and Structured variants; new variants are added via plugins
6. **Storage is separate from organisation** — Objects are stored in the file system or blob store; organisation happens through the graph and metadata

```rust
// crates/nabu-core/src/models/knowledge_object.rs

use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeObject {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub vault_id: String,
    pub created_at: i64,
    pub modified_at: i64,
    pub source: Option<CaptureSource>,
    pub content: ObjectContent,
    pub metadata: ObjectMetadata,
    pub embeddings: Option<EmbeddingVector>,
    pub tags: Vec<String>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectType {
    Note,
    Document,
    Image,
    Audio,
    Video,
    Bookmark,
    Email,
    ResearchPaper,
    CodeSnippet,
    Task,
    Person,
    Organisation,
    Project,
    Book,
    Scan,
    Screenshot,
    Attachment,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectContent {
    Markdown { body: String, blocks: Vec<Block> },
    PlainText { body: String },
    Html { body: String },
    Binary { mime_type: String, size_bytes: u64 },
    Structured { json: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub source_url: Option<String>,
    pub source_file: Option<String>,
    pub mime_type: Option<String>,
    pub page_count: Option<u32>,
    pub word_count: Option<u32>,
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub target_id: Uuid,
    pub relation_type: RelationType,
    pub confidence: f32,
    pub source: RelationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationType {
    LinksTo,
    Embeds,
    References,
    Cites,
    BelongsTo,
    PartOf,
    Mentions,
    SimilarTo,
    DuplicateOf,
    DerivedFrom,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationSource {
    Manual,
    AutoDetected,
    AiSuggested,
    Inferred,
}
```

#### 3.5.2 Capture Pipeline Model

```rust
// crates/nabu-core/src/models/capture.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSource {
    pub source_type: SourceType,
    pub identifier: String,
    pub timestamp: i64,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    FileDrop,
    ShareSheet,
    BrowserClip,
    WatchFolder,
    EmailImport,
    GitSync,
    ApiWebhook,
    Dictation,
    Screenshot,
    Clipboard,
    Manual,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionRequest {
    pub source: CaptureSource,
    pub raw_data: Vec<u8>,
    pub mime_type: Option<String>,
    pub vault_id: String,
    pub options: IngestionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionOptions {
    pub auto_process: bool,
    pub auto_tag: bool,
    pub auto_embed: bool,
    pub deduplicate: bool,
    pub priority: IngestionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestionPriority {
    Low,
    Normal,
    High,
    Urgent,
}
```

#### 3.5.3 Metadata Model

```rust
// crates/nabu-core/src/models/metadata.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEnvelope {
    pub object_id: Uuid,
    pub vault_id: String,
    pub created_at: i64,
    pub modified_at: i64,
    pub created_by: Option<String>,
    pub source_system: Option<String>,
    pub processing_history: Vec<ProcessingStep>,
    pub checksum: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStep {
    pub processor: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub success: bool,
    pub output_summary: Option<String>,
    pub error: Option<String>,
}
```

#### 3.5.4 Graph Model

The graph model extends the existing `petgraph`-based implementation to support typed entities and semantic relationships.

```rust
// crates/nabu-core/src/models/graph.rs

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub graph: DiGraph<GraphEntity, GraphEdge>,
    pub entity_index: HashMap<Uuid, NodeIndex>,
    pub relation_index: HashMap<RelationType, Vec<GraphEdge>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub id: Uuid,
    pub entity_type: EntityType,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub vault_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Note,
    Document,
    Person,
    Organisation,
    Project,
    Book,
    ResearchPaper,
    CodeRepository,
    Bookmark,
    Media,
    Task,
    Tag,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub relation_type: RelationType,
    pub confidence: f32,
    pub source: RelationSource,
    pub metadata: Option<serde_json::Value>,
}
```

#### 3.5.5 Search Model

```rust
// crates/nabu-core/src/models/search.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub vector: Option<Vec<f32>>,
    pub filters: Vec<SearchFilter>,
    pub facets: Vec<String>,
    pub limit: usize,
    pub offset: usize,
    pub ranking: RankingStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RankingStrategy {
    TfIdf,
    Semantic,
    Hybrid { alpha: f32 },
    GraphBoosted { boost_factor: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilter {
    pub field: String,
    pub operator: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    In,
    Between,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query_id: Uuid,
    pub results: Vec<SearchHit>,
    pub facets: HashMap<String, Vec<FacetValue>>,
    pub total_count: usize,
    pub took_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub object_id: Uuid,
    pub object_type: ObjectType,
    pub score: f32,
    pub highlights: Vec<TextHighlight>,
    pub metadata: ObjectMetadata,
}
```

#### 3.5.6 AI Enrichment Model

```rust
// crates/nabu-core/src/models/ai.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVector {
    pub model: String,
    pub dimensions: usize,
    pub values: Vec<f32>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSummary {
    pub object_id: Uuid,
    pub summary: String,
    pub key_entities: Vec<String>,
    pub topics: Vec<String>,
    pub sentiment: Option<f32>,
    pub language: Option<String>,
    pub model_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub text: String,
    pub entity_type: String,
    pub confidence: f32,
    pub start_offset: usize,
    pub end_offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    pub object_id_a: Uuid,
    pub object_id_b: Uuid,
    pub similarity_score: f32,
    pub similarity_type: SimilarityType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SimilarityType {
    Semantic,
    ContentHash,
    MetadataMatch,
    Hybrid,
}
```

### 3.6 Capture Pipeline

The capture pipeline is the entry point for all knowledge into Nabu. It follows a staged architecture:

```
Input Source → CaptureHandler → Normaliser → IngestionRequest → ProcessingPipeline
```

**Stage 1: Capture** — A `CaptureHandler` accepts raw input from any source. Each handler is responsible for one source type (file drop, share sheet, browser clip, etc.). The handler extracts raw bytes, MIME type, and source metadata.

**Stage 2: Normalisation** — The `Normaliser` converts raw input into a canonical `IngestionRequest`. This includes MIME type detection, charset normalisation, and metadata extraction from the source.

**Stage 3: Ingestion** — The `IngestionPipeline` receives the normalised request, deduplicates (if enabled), and passes the item to the processing pipeline.

**Stage 4: Processing** — The `ProcessingPipeline` applies a configurable chain of `Processor` trait objects. Each processor transforms the item and adds metadata. Processors include: OCR, metadata extraction, entity extraction, semantic tagging, AI summarisation, embedding generation, duplicate detection, relationship discovery, timeline extraction, language detection, classification.

**Stage 5: Storage** — The enriched `KnowledgeObject` is persisted by the `StorageManager` to the object store, indexed by the `UniversalSearchEngine`, and added to the `KnowledgeGraph`.

#### 3.6.1 Capture Sources

Every piece of knowledge enters Nabu through a `CaptureSource`. The following sources are first-class citizens in the capture architecture:

| Source | Description | Handler | Platform |
|--------|-------------|---------|----------|
| **Browser** | Web clipping, bookmark import, tab capture | `BrowserClipHandler` | Desktop extension, native messaging |
| **Clipboard** | Paste from system clipboard | `ClipboardHandler` | All platforms |
| **Filesystem** | Drag-and-drop files into vault or Inbox | `FileDropHandler` | All platforms |
| **Watch Folder** | Monitor directory for new files | `WatchFolderHandler` | All platforms |
| **Scanner** | Scan physical documents via TWAIN/SANE | `ScannerHandler` | Desktop only |
| **Camera** | Capture photos via device camera | `CameraHandler` | Mobile, desktop |
| **Email** | Import emails via IMAP/POP3 | `EmailImportHandler` | All platforms |
| **API** | Webhook ingestion, REST API | `ApiWebhookHandler` | All platforms |
| **CLI** | Command-line ingestion | `CliHandler` | All platforms |
| **Drag & Drop** | Drag files from other apps into Nabu | `FileDropHandler` | All platforms |
| **Import** | Bulk import from ZIP, Git, etc. | `ImportHandler` | All platforms |
| **Mobile Share** | iOS/Android share sheet | `ShareSheetHandler` | Mobile only |
| **Voice** | Dictation, voice memos | `DictationHandler` | All platforms |
| **Future Plugins** | Extensible via plugin system | `PluginCaptureHandler` | All platforms |

**Key principle:** New capture sources are added by implementing the `CaptureHandler` trait and registering with the `CaptureEngine`. No core code changes are required.

### 3.7 Storage Strategy

Nabu uses a multi-storage strategy optimised for local-first operation:

| Storage | Purpose | Technology | Location |
|---------|---------|------------|----------|
| **File System** | Primary storage for Markdown notes and raw files | OS filesystem | Vault directory |
| **Tantivy** | Full-text search index | Tantivy (Rust) | `.nabu/index/search/` |
| **SQLite** | Metadata store, object registry, graph edges | rusqlite | `.nabu/db/metadata.db` |
| **Vector Index** | Embedding storage for semantic search | FAISS-compatible or custom | `.nabu/index/vectors/` |
| **Blob Store** | Binary attachments (images, PDFs, audio) | File system with DB tracking | `.nabu/objects/` |
| **Cache** | Processed results, AI outputs, thumbnails | SQLite + filesystem | `.nabu/cache/` |

All storage is local. No cloud sync is built into the core product. Users may optionally use their own sync solution (Git, rsync, cloud storage) at the vault directory level.

### 3.8 Security Model

#### 3.8.1 Threat Model

| Threat | Likelihood | Impact | Mitigation |
|--------|-----------|--------|------------|
| Unauthorised access to vault files | Medium | High | Vault-level encryption (AES-256-GCM) with user-derived key |
| Malicious file upload (code execution) | Low | Critical | Sandboxed processing; no arbitrary code execution from ingested files |
| Data leakage via AI processing | Medium | Medium | All AI processing is local; no data leaves the machine |
| Supply chain attack via dependencies | Medium | High | `deny.toml` audit; `cargo audit` in CI; minimal dependency surface |
| Insider threat (shared vault) | Low | Medium | Optional per-vault access control with permission levels |
| Physical theft of device | Medium | High | Full-disk encryption (OS-level); optional vault-level encryption |
| Metadata leakage | Low | Medium | Metadata is stored locally; no telemetry |

#### 3.8.2 Security Principles

1. **Zero trust by default** — every operation requires explicit permission
2. **Encryption at rest** — vault data is encrypted with AES-256-GCM
3. **No telemetry** — no data leaves the user's machine without explicit consent
4. **Principle of least privilege** — each component has only the permissions it needs
5. **Audit trail** — all security-relevant operations are logged locally
6. **Secure defaults** — encryption is enabled by default; sharing is opt-in

### 3.9 Extensibility Model

Nabu's extensibility is built on a plugin architecture that allows new knowledge types, processors, capture handlers, and integrations to be added without modifying core code.

#### 3.9.1 Plugin Interface

```rust
// crates/nabu-core/src/plugin.rs

pub trait CaptureHandler: Send + Sync {
    fn source_type(&self) -> SourceType;
    fn can_handle(&self, mime_type: &str, data: &[u8]) -> bool;
    fn capture(&self, request: &IngestionRequest) -> Result<IngestionResult>;
}

pub trait Processor: Send + Sync {
    fn name(&self) -> &str;
    fn object_types(&self) -> &[ObjectType];
    fn process(&self, object: &mut KnowledgeObject) -> Result<ProcessingResult>;
}

pub trait Connector: Send + Sync {
    fn name(&self) -> &str;
    fn sync(&self, profile: &SyncProfile) -> Result<SyncResult>;
}

pub trait KnowledgeObjectType: Send + Sync {
    fn type_name(&self) -> &str;
    fn validate(&self, object: &KnowledgeObject) -> Result<()>;
    fn default_processors(&self) -> Vec<String>;
}
```

#### 3.9.2 Plugin Discovery

Plugins are discovered via:
1. **Built-in** — core plugins compiled into `nabu-core`
2. **Dynamic library** — `.so`/`.dylib`/`.dll` files in `.nabu/plugins/`
3. **WASM module** — WebAssembly modules for sandboxed extensions
4. **Configuration** — declarative plugin registration in vault config

### 3.10 Event Architecture

The event bus (`appEventBus`) is the backbone for decoupled communication between services. Events are typed and follow the pattern established in Phase 1.5.

```
Event Flow:

CaptureHandler → publish(ItemCaptured) → ProcessingPipeline subscribes
ProcessingPipeline → publish(ItemProcessed) → SearchEngine subscribes
ProcessingPipeline → publish(ItemProcessed) → GraphEngine subscribes
ProcessingPipeline → publish(ItemProcessed) → AIEnrichment subscribes
AIEnrichment → publish(EmbeddingsGenerated) → SearchEngine subscribes
GraphEngine → publish(GraphUpdated) → SearchEngine subscribes
StorageManager → publish(ItemStored) → AutomationEngine subscribes
AutomationEngine → publish(RuleTriggered) → ProcessingPipeline subscribes
```

### 3.11 Module Inventory

| Module | Location | Language | Responsibility |
|--------|----------|----------|----------------|
| `nabu-core` | `crates/nabu-core/` | Rust | Core engine: parsing, indexing, graph, vault config, templates, export, themes |
| `nabu-ui` | `crates/nabu-ui/` | Rust (Leptos WASM) | UI components: app shell, file tree, note editor, graph view, PDF viewer, settings |
| `src-tauri` | `src-tauri/` | Rust | Tauri shell: IPC commands, app lifecycle, window management |
| `main/services` | `src/main/services/` | TypeScript | Application services: vault, notes, search, capture, processing, AI, graph, automation |
| `main/ipc` | `src/main/ipc/` | TypeScript | IPC handler registration |
| `shared/models` | `src/shared/models
### 3.12 Data Flow Diagrams

#### 3.12.1 Ingestion Flow

```
User Action (drag-drop, share sheet, browser clip, API call)
    │
    ▼
CaptureHandler (source-type-specific)
    │
    ▼
Normaliser (MIME detection, charset normalisation, metadata extraction)
    │
    ▼
IngestionPipeline (deduplication check, priority assignment)
    │
    ▼
ProcessingPipeline (configurable chain of processors)
    ├── OCR Processor
    ├── Metadata Extractor
    ├── Entity Extractor
    ├── Semantic Tagger
    ├── AI Summariser
    ├── Embedding Generator
    ├── Duplicate Detector
    ├── Relationship Discoverer
    ├── Timeline Extractor
    ├── Language Detector
    └── Classifier
    │
    ▼
StorageManager (persist to object store, update indexes)
    │
    ▼
UniversalSearchEngine (index for search)
    │
    ▼
KnowledgeGraph (add nodes and edges)
    │
    ▼
Event Bus (publish ItemStored, GraphUpdated, IndexUpdated)
```

#### 3.12.2 Search Flow

```
User Query (text, voice, or structured)
    │
    ▼
Query Parser (parse filters, facets, ranking strategy)
    │
    ▼
UniversalSearchEngine
    ├── Tantivy (full-text search)
    ├── Vector Index (semantic search)
    └── Graph Index (relationship-boosted ranking)
    │
    ▼
Hybrid Ranker (combine scores with configurable weights)
    │
    ▼
Result Formatter (unified result shape across types)
    │
    ▼
Renderer (display mixed-type results)
```

#### 3.12.3 AI Enrichment Flow

```
KnowledgeObject (new or updated)
    │
    ▼
AIEnrichmentPipeline
    ├── Embedding Generator (local model, e.g. BGE-micro)
    │   └── Stores vector in Vector Index
    ├── Summariser (local LLM or on-device model)
    │   └── Stores summary in KnowledgeObject
    ├── Entity Extractor (NER model)
    │   └── Creates GraphEntity nodes
    ├── Duplicate Detector (similarity check)
    │   └── Flags potential duplicates
    └── Relationship Discoverer (graph-based inference)
        └── Creates GraphEdge relations
```

### 3.13 Capability Map

| Capability | Current | Target | Gap |
|-----------|---------|--------|-----|
| Markdown authoring | ✅ Full | ✅ Full | None |
| File system watch | ✅ Full | ✅ Full | None |
| Full-text search | ✅ Full | ✅ Full | None |
| Vector search | 🟡 Partial | ✅ Full | Expand to all object types |
| PDF viewing | ✅ Full | ✅ Full | None |
| PDF text extraction | ❌ None | ✅ Full | Add PDF text extraction |
| PDF OCR | ❌ None | ✅ Full | Add OCR for scanned PDFs |
| Image OCR | 🟡 macOS only | ✅ Cross-platform | Add cross-platform OCR |
| Audio transcription | 🟡 macOS only | ✅ Cross-platform | Add cross-platform transcription |
| Wiki-link graph | ✅ Full | ✅ Full | None |
| Typed entity graph | ❌ None | ✅ Full | Add entity nodes and semantic edges |
| Tag extraction | ✅ Full | ✅ Full | None |
| Semantic tagging | ❌ None | ✅ Full | Add AI-powered tagging |
| Duplicate detection | ❌ None | ✅ Full | Add content hash + semantic similarity |
| Relationship discovery | 🟡 Wiki-links only | ✅ Full | Add entity-level relationship extraction |
| AI summarisation | ❌ None | ✅ Full | Add local LLM summarisation |
| Browser capture | ❌ None | ✅ Full | Add browser extension/protocol handler |
| Mobile capture | ❌ None | ✅ Full | Add share sheet and camera integration |
| Email import | ❌ None | ✅ Full | Add MIME parsing and import |
| Git repo ingestion | ❌ None | ✅ Full | Add commit history as knowledge |
| ZIP archive extraction | ❌ None | ✅ Full | Add archive extraction and indexing |
| API webhook ingestion | ❌ None | ✅ Full | Add webhook receiver |
| Automation rules | ❌ None | ✅ Full | Add rule engine |
| Vault encryption | ❌ None | ✅ Full | Add AES-256-GCM encryption |
| Cross-platform desktop | 🟡 macOS primary | ✅ macOS/Linux/Windows | Add Linux/Windows support |
| Plugin system | ❌ None | ✅ Full | Add plugin architecture |

### 3.14 Ownership Matrix

| Concern | Owner | Notes |
|---------|-------|-------|
| Knowledge capture | **Main** (`CaptureEngine`) | All input sources handled through capture handlers |
| Knowledge processing | **Main** (`ProcessingPipeline`) | All processors run in the main process |
| Knowledge graph | **Main** (`KnowledgeGraph`) + **nabu-core** | Graph operations in Rust; queries via IPC |
| Search indexing | **Main** (`UniversalSearchEngine`) | Tantivy + vector index in main process |
| AI enrichment | **Main** (`AIEnrichmentPipeline`) | Local models only; no cloud dependency |
| Storage | **Main** (`StorageManager`) | File system + SQLite + Tantivy |
| Security | **Main** (`SecurityManager`) | Encryption, key management, permissions |
| Automation | **Main** (`AutomationEngine`) | Rule evaluation and action execution |
| Integrations | **Main** (`IntegrationHub`) | External connectors |
| UI rendering | **Renderer** | Presentation only; no business logic |
| IPC | **Main** (`ipc/`) | Typed handlers; thin delegation |
| Domain models | **Shared** (`models/`) | Pure types; no runtime dependencies |
| Schemas | **Shared** (`schemas/`) | Zod validation for IPC contracts |
| Event bus | **Shared** (`events/`) | Typed events; main-process only |
| Plugin contracts | **Shared** (`contracts/`) | Plugin interface definitions |

---
### 3.15 Data Flow Diagrams

#### 3.15.1 Ingestion Flow

```
User Action (drag-drop, share sheet, browser clip, API call)
    │
    ▼
CaptureHandler (source-type-specific)
    │
    ▼
Normaliser (MIME detection, charset normalisation, metadata extraction)
    │
    ▼
IngestionPipeline (deduplication check, priority assignment)
    │
    ▼
ProcessingPipeline (configurable chain of processors)
    ├── OCR Processor
    ├── Metadata Extractor
    ├── Entity Extractor
    ├── Semantic Tagger
    ├── AI Summariser
    ├── Embedding Generator
    ├── Duplicate Detector
    ├── Relationship Discoverer
    ├── Timeline Extractor
    ├── Language Detector
    └── Classifier
    │
    ▼
StorageManager (persist to object store, update indexes)
    │
    ▼
UniversalSearchEngine (index for search)
    │
    ▼
KnowledgeGraph (add nodes and edges)
    │
    ▼
Event Bus (publish ItemStored, GraphUpdated, IndexUpdated)
```

#### 3.15.2 Search Flow

```
User Query (text, voice, or structured)
    │
    ▼
Query Parser (parse filters, facets, ranking strategy)
    │
    ▼
UniversalSearchEngine
    ├── Tantivy (full-text search)
    ├── Vector Index (semantic search)
    └── Graph Index (relationship-boosted ranking)
    │
    ▼
Hybrid Ranker (combine scores with configurable weights)
    │
    ▼
Result Formatter (unified result shape across types)
    │
    ▼
Renderer (display mixed-type results)
```

### 3.16 Data Flow Diagrams (continued)

#### 3.16.1 AI Enrichment Flow

```
KnowledgeObject (new or updated)
    │
    ▼
AIEnrichmentPipeline
    ├── Embedding Generator (local model, e.g. BGE-micro)
    │   └── Stores vector in Vector Index
    ├── Summariser (local LLM or on-device model)
    │   └── Stores summary in KnowledgeObject
    ├── Entity Extractor (NER model)
    │   └── Creates GraphEntity nodes
    ├── Duplicate Detector (similarity check)
    │   └── Flags potential duplicates
    └── Relationship Discoverer (graph-based inference)
        └── Creates GraphEdge relations
```

### 3.17 AI Philosophy

AI is an enhancement layer, not a dependency. The system must function completely without AI. This principle is non-negotiable and governs every AI-related decision in the programme.

**Core tenets:**

1. **AI is optional** — Every feature must work without AI. AI features are progressive enhancements that improve the experience but are never required for core functionality.
2. **AI improves organisation, it does not own it** — AI suggests tags, entities, relationships, and destinations. The user retains final authority. AI never modifies or deletes user data without explicit consent.
3. **Core capabilities are AI-independent** — OCR, metadata extraction, search, graph relationships, and document management remain core platform capabilities independent of AI models. These work whether AI is enabled or not.
4. **No proprietary chatbot** — Nabu will not build an integrated conversational assistant. The product is a knowledge vault, not a chatbot interface.
5. **External AI compatibility** — Nabu remains compatible with external AI tools such as terminal coding agents, local LLM servers, and future integrations. Users can bring their own AI workflows without Nabu requiring its own chatbot.
6. **Local-first AI** — All AI processing runs on-device. No data leaves the user's machine for AI processing. Users may optionally connect to local LLM servers, but cloud AI APIs are not built into the product.
7. **Transparent AI** — All AI-generated content is attributed and distinguishable from user-created content. Users can see what AI suggested and what they accepted or rejected.

**AI in the architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI as Enhancement Layer                       │
│                                                                   │
│  Core Platform (always available)                                 │
│  ├── OCR (tesseract-rs / platform-native)                        │
│  ├── Metadata Extraction                                          │
│  ├── Full-text Search (Tantivy)                                   │
│  ├── Graph (petgraph)                                             │
│  ├── Document Management                                          │
│  └── Storage (SQLite, file system)                                │
│                                                                   │
│  AI Enhancement Layer (optional, configurable)                    │
│  ├── Embedding Generator (BGE-micro)                              │
│  ├── Summariser (local LLM)                                       │
│  ├── Entity Extractor (NER)                                       │
│  ├── Duplicate Detector (semantic similarity)                     │
│  └── Relationship Discoverer (graph inference)                    │
│                                                                   │
│  External AI Tools (user-managed)                                 │
│  ├── Terminal coding agents                                       │
│  ├── Local LLM servers (Ollama, LM Studio)                        │
│  └── Future integrations                                          │
└─────────────────────────────────────────────────────────────────┘
```

### 3.18 Existing Foundations

This programme extends existing Nabu capabilities rather than introducing them from scratch. The following foundations are already in place and will be expanded:

| Foundation | Current State | Programme Expansion |
|------------|--------------|---------------------|
| **OCR support** | macOS-only via `whisper-rs` native feature | Cross-platform OCR via `tesseract-rs` or platform-native APIs; applied to images, PDFs, screenshots, and scans |
| **PDF parsing** | `pdfjs-dist` for rendering; basic text extraction | Full PDF processing pipeline: text extraction, OCR for scanned PDFs, metadata extraction, annotation parsing, format conversion |
| **Annotation infrastructure** | Basic PDF viewer with no annotation support | Annotation extraction from PDFs; annotations become graph edges linking to source documents and related objects |
| **Rust backend** | `nabu-core` with parser, indexer, graph engine, vault config, templates, export, themes | Expanded with `KnowledgeObject` model, `CaptureEngine`, `ProcessingPipeline`, `AIEnrichmentPipeline`, `KnowledgeGraph`, `StorageManager`, `SecurityManager`, `AutomationEngine`, `IntegrationHub` |
| **Search infrastructure** | Tantivy full-text search with fields for vault_id, path, title, content, tags, mtime | Expanded to `UniversalSearchEngine` with hybrid ranking (full-text + vector + graph), faceted filtering, and mixed-type results across all knowledge objects |
| **Graph work** | `petgraph`-based `VaultGraph` and `GraphEngine` with file-level nodes and wiki-link edges | Expanded to `KnowledgeGraph` with typed entities (Person, Organisation, Project, Book, etc.), semantic edges (Mentions, Cites, BelongsTo, etc.), entity resolution, and graph queries |
| **Markdown foundation** | Primary authoring format; `parser.rs` with `parse_markdown_to_html()`, `extract_tags()`, `extract_block_refs()` | Expanded to support all knowledge types equally; Markdown remains the primary authoring format but is one of many `ObjectContent` variants |

**Key principle:** Universal Knowledge Capture does not replace these systems. It extends them. The existing Rust backend, search infrastructure, and graph engine are the foundation upon which the new capabilities are built.

---

### 3.19 Knowledge Lifecycle

Every knowledge object in Nabu follows a defined lifecycle. This lifecycle is the backbone of the architecture — every subsystem, automation rule, and UI component maps to a stage in this lifecycle.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Knowledge Lifecycle                              │
│                                                                           │
│  Capture ──▶ Review ──▶ Understand ──▶ Connect ──▶ Organise ──▶ Retrieve │
│     │           │          │            │           │           │        │
│     │           │          │            │           │           │        │
│     ▼           ▼          ▼            ▼           ▼           ▼        │
│  Ingested   Validated   Enriched    Graph-      Tagged     Found         │
│  from       by user     with        connected    and         by search    │
│  source     in Inbox    metadata,   to related   filed       or graph     │
│                        OCR,        objects      in          traversal    │
│                        entities,                 Collections          │
│                        embeddings                                                │
│                                                                           │
│  Retrieve ──▶ Use ──▶ Evolve ──▶ Archive                                 │
│     │           │         │         │                                    │
│     ▼           ▼         ▼         ▼                                    │
│  Displayed   Modified   Updated   Moved to                               │
│  in UI       by user    via        archive                               │
│              or AI      sync,      store                                 │
│                          import,                                          │
│                          processing                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

#### 3.19.1 Lifecycle Stages

| Stage | Description | Owner | Key Operations |
|-------|-------------|-------|----------------|
| **Capture** | Knowledge enters the system from any source | `CaptureEngine` | Ingestion, normalisation, deduplication, priority assignment |
| **Review** | User validates, corrects, and approves captured items | `KnowledgeInbox` | Accept, reject, edit, retry, bulk approve, choose destination |
| **Understand** | System enriches the object with metadata, OCR, entities, embeddings | `ProcessingPipeline` + `AIEnrichmentPipeline` | OCR, metadata extraction, entity extraction, semantic tagging, embedding generation, summarisation |
| **Connect** | Object is linked to related objects in the graph | `KnowledgeGraph` + `AIEnrichmentPipeline` | Entity resolution, relationship discovery, graph insertion |
| **Organise** | Object is tagged, filed, and placed in collections | `AutomationEngine` + `KnowledgeInbox` | Auto-tagging, collection assignment, folder organisation |
| **Retrieve** | Object is found via search, graph traversal, or collection browsing | `UniversalSearchEngine` + `KnowledgeGraph` | Hybrid search, faceted filtering, graph traversal, collection browsing |
| **Use** | Object is consumed by the user | `nabu-ui` | View, edit, annotate, export, share |
| **Evolve** | Object is updated, versioned, or linked to new knowledge | `StorageManager` + `AutomationEngine` | Versioning, sync, import, relationship updates |
| **Archive** | Object is moved to long-term storage | `StorageManager` | Compression, migration, retention policies |

#### 3.19.2 Lifecycle Hooks

Automation rules can hook into any stage of the lifecycle:

| Hook | Trigger | Example Rule |
|------|---------|--------------|
| `on_capture` | Item captured | Auto-tag invoices, detect duplicates |
| `on_review` | Item ready for review | Suggest collection based on content |
| `on_understand` | Processing complete | Create graph edges for detected entities |
| `on_connect` | Graph updated | Notify user of new relationships |
| `on_organise` | Item filed | Apply user-defined tags and collections |
| `on_retrieve` | Search performed | Boost results from trusted sources |
| `on_use` | Item opened | Log usage patterns for recommendations |
| `on_evolve` | Item updated | Re-process embeddings, update graph |
| `on_archive` | Item archived | Compress, move to archive store |

#### 3.19.3 Lifecycle States

Each knowledge object carries a `lifecycle_state` field:

| State | Description |
|-------|-------------|
| `captured` | Ingested, awaiting review |
| `reviewed` | Reviewed and approved by user |
| `processing` | Being processed by the pipeline |
| `processed` | Processing complete |
| `connected` | Linked to graph |
| `organised` | Tagged and filed |
| `active` | In regular use |
| `evolved` | Updated since initial capture |
| `archived` | Moved to long-term storage |
| `deleted` | Marked for deletion |

---

### 3.20 Knowledge Destinations

Every knowledge object eventually ends up somewhere. **Knowledge Destinations** are the endpoints of the knowledge lifecycle — the places where knowledge lives, is accessed, and is used. Unlike physical folders, destinations are dynamic views that can include objects from multiple sources.

| Destination | Description | Type | Example |
|-------------|-------------|------|---------|
| **Collections** | Dynamic, user-defined groupings of knowledge objects | View | Medical, Tax, Travel, Reading, Programming, AI, Invoices, Recipes |
| **Projects** | Knowledge related to a specific project | Entity | Project Alpha, Home Renovation, Thesis |
| **People** | Knowledge related to a specific person | Entity | Alice Smith, Dr. Johnson, Client X |
| **Organisations** | Knowledge related to a specific organisation | Entity | Acme Corp, University, Government Agency |
| **Research** | Knowledge related to a specific research topic | View | Machine Learning, Climate Change, History of Rome |
| **Timeline** | Knowledge ordered by creation or event date | View | Today, This Week, 2024, 1990s |
| **Recent** | Recently captured or modified knowledge | View | Last 24 hours, Last 7 days |
| **Archive** | Long-term stored knowledge | View | 2023 Archive, Completed Projects |
| **Graph** | Knowledge discovered through graph traversal | View | Connected to current note, Related to person X |
| **Search** | Knowledge found through search queries | View | Results for "machine learning", Tagged with #important |
| **Widgets** | Knowledge surfaced in dashboard widgets | View | Daily note, Random note, Upcoming events |
| **Automation** | Knowledge moved or tagged by automation rules | View | Auto-filed invoices, Duplicate candidates |

**Key principle:** Collections are not folders. They are dynamic views that can include objects from any source, any type, and any location. A single knowledge object can belong to multiple collections simultaneously. Collections are defined by queries, not by physical location.

---

## 4. Specialized Sub-System

### 4.1 Knowledge Capture Engine

The Knowledge Capture Engine is the single most critical subsystem. It is the entry point for all knowledge into Nabu and determines whether the vision of "Capture Everything" is achievable. Without a robust capture engine, all downstream processing, graph, search, and AI capabilities are irrelevant because there is no knowledge to process.

#### 4.1.1 Architecture

The Capture Engine follows a **handler-registry pattern** where each supported input type has a dedicated `CaptureHandler` implementation. Handlers are registered at startup and discovered dynamically.

```
┌─────────────────────────────────────────────────────────┐
│                  Capture Engine                           │
│                                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐│
│  │ Handler      │  │ Handler      │  │ Handler          ││
│  │ Registry     │  │ Registry     │  │ Registry         ││
│  │              │  │              │  │                  ││
│  │ FileDrop     │  │ ShareSheet   │  │ BrowserClip      ││
│  │ WatchFolder  │  │ EmailImport  │  │ GitSync          ││
│  │ ApiWebhook   │  │ Dictation    │  │ Clipboard        ││
│  │ Screenshot   │  │ ZipArchive   │  │ OfficeDoc        ││
│  │ Custom       │  │ ...          │  │ ...              ││
│  └─────────────┘  └─────────────┘  └─────────────────┘│
│                                                           │
│  ┌─────────────────────────────────────────────────────┐│
│  │              Normaliser                               ││
│  │  MIME detection │ charset normalisation │ metadata   ││
│  └─────────────────────────────────────────────────────┘│
│                                                           │
│  ┌─────────────────────────────────────────────────────┐│
│  │              IngestionPipeline                        ││
│  │  Deduplication │ priority queue │ batch processing  ││
│  └─────────────────────────────────────────────────────┘│
│                                                           │
│  ┌─────────────────────────────────────────────────────┐│
│  │              Output: IngestionResult                  ││
│  │  KnowledgeObject │ CaptureSource │ ProcessingHints   ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

#### 4.1.2 Responsibilities

1. **Handler Registration** — Accept and validate `CaptureHandler` implementations at startup
2. **Source Detection** — Automatically detect the source type of incoming data
3. **Normalisation** — Convert raw input into a canonical `IngestionRequest`
4. **Deduplication** — Detect and prevent duplicate ingestion (content hash + semantic similarity)
5. **Priority Management** — Queue items by priority (Urgent > High > Normal > Low)
6. **Batch Processing** — Support batch ingestion for bulk operations (e.g., Git repo sync)
7. **Error Handling** — Graceful degradation when a handler fails; retry with backoff
8. **Progress Reporting** — Emit progress events for long-running operations (e.g., ZIP extraction)
9. **Audit Logging** — Log all capture operations for security and debugging

#### 4.1.3 Interfaces

```rust
// crates/nabu-core/src/capture/handler.rs

pub trait CaptureHandler: Send + Sync {
    fn source_type(&self) -> SourceType;
    fn can_handle(&self, mime_type: &str, data: &[u8]) -> bool;
    fn capture(&self, request: &IngestionRequest) -> Result<IngestionResult>;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn requires_network(&self) -> bool;
    fn estimated_duration(&self) -> Duration;
}

pub trait CaptureHandlerFactory: Send + Sync {
    fn create(&self, config: &CaptureHandlerConfig) -> Box<dyn CaptureHandler>;
}

pub struct CaptureEngine {
    handlers: HashMap<SourceType, Box<dyn CaptureHandler>>,
    normaliser: Normaliser,
    pipeline: IngestionPipeline,
    config: CaptureEngineConfig,
}

impl CaptureEngine {
    pub fn register_handler(&mut self, handler: Box<dyn CaptureHandler>);
    pub fn capture(&mut self, request: IngestionRequest) -> Result<IngestionResult>;
    pub fn capture_batch(&mut self, requests: Vec<IngestionRequest>) -> Result<BatchResult>;
    pub fn get_handler(&self, source_type: &SourceType) -> Option<&dyn CaptureHandler>;
    pub fn list_handlers(&self) -> Vec<HandlerInfo>;
}
```

#### 4.1.4 Failure Modes

| Failure Mode | Probability | Impact | Detection | Recovery |
|-------------|------------|--------|-----------|----------|
| Handler panic during capture | Low | High | Panic catch in handler wrapper | Restart handler; retry with next handler |
| Invalid MIME type detection | Medium | Medium | MIME detection failure rate metric | Fall back to binary/octet-stream; use content sniffing |
| Deduplication false positive | Low | Medium | Duplicate detection accuracy metric | Manual review queue; confidence threshold |
| Normalisation failure | Medium | High | Normalisation error rate metric | Return descriptive error; log raw input for debugging |
| Storage full during capture | Medium | High | Disk space monitoring | Pause ingestion; alert user; evict cache |
| Handler timeout | Low | Medium | Per-handler timeout metric | Kill handler; retry with reduced priority |
| Corrupted input data | Medium | Medium | Checksum validation failure | Skip item; log error; continue batch |
| Network failure (for network handlers) | Medium | Medium | Network error rate metric | Retry with exponential backoff; queue for later |

#### 4.1.5 Recovery Strategy

1. **Handler-level recovery** — Each handler runs in isolation; a panic in one handler does not affect others
2. **Retry with backoff** — Failed captures are retried with exponential backoff (1s, 2s, 4s, 8s, max 60s)
3. **Dead letter queue** — Items that fail after max retries are moved to a dead letter queue for manual review
4. **Idempotent ingestion** — Each ingestion request has a unique ID; duplicate requests are detected and skipped
5. **Checkpoint-based batch processing** — Batch operations save progress checkpoints; failures resume from last checkpoint
6. **Vault-level rollback** — If a vault-level operation fails, all changes within that batch are rolled back atomically

#### 4.1.6 Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Single file capture latency | < 100ms (excluding processing) | End-to-end from handler input to IngestionResult |
| Batch capture throughput | > 1000 files/minute (watch folder) | Files processed per minute |
| Deduplication check latency | < 10ms per item | Hash computation + lookup |
| Memory usage per capture | < 50MB peak | RSS during capture |
| Handler startup time | < 50ms | Time from registration to ready |
| Concurrent handler capacity | 10+ simultaneous captures | Parallel capture operations |

#### 4.1.7 Security

1. **Input validation** — All captured data is validated before processing; malicious files are quarantined
2. **Sandboxed processing** — Handlers that execute external processes (e.g., PDF text extraction) run in sandboxed environments
3. **No arbitrary code execution** — Ingested files are never executed; only parsed and extracted
4. **Path traversal prevention** — All file paths are validated to be within the vault directory
5. **Size limits** — Configurable per-vault size limits for individual files and batch operations
6. **Type allowlisting** — Only registered handler MIME types are accepted; unknown types are rejected

#### 4.1.8 Scaling

- **Horizontal** — Handlers are stateless and can be replicated; the CaptureEngine distributes work across handler instances
- **Vertical** — Batch processing uses parallel worker threads (configurable, default = CPU core count)
- **Queue-based** — High-priority items are processed first; low-priority items are queued
- **Backpressure** — When the processing pipeline is saturated, the capture engine applies backpressure to handlers

#### 4.1.9 Testing

| Test Type | Scope | Frequency |
|-----------|-------|-----------|
| Unit tests | Individual handler logic | Every build |
| Integration tests | Full capture pipeline (handler → normaliser → pipeline) | Every build |
| Property tests | Handler contract invariants (idempotency, error handling) | Every build |
| Performance tests | Capture latency, throughput, memory usage | Nightly |
| Fuzz tests | Malformed input handling | Weekly |
| Security tests | Path traversal, MIME spoofing, size limit bypass | Per release |

#### 4.1.10 Future Evolution

1. **Streaming capture** — Support for continuous capture from live sources (e.g., screen recording, real-time email)
2. **Predictive capture** — AI-powered suggestion of what to capture based on user behaviour
3. **Collaborative capture** — Multi-user capture with conflict resolution
4. **Mobile capture SDK** — Native SDK for mobile apps to integrate Nabu capture
5. **Browser extension** — Chrome/Firefox/Safari extension for web clipping
6. **OS-level integration** — macOS Share Sheet, Android Share Sheet, Windows Share Sheet
7. **IoT capture** — Support for IoT device data ingestion (sensors, cameras, etc.)

---

## 5. Principles

### 5.1 Non-Negotiable Engineering Principles

| # | Principle | Description |
|---|-----------|-------------|
| 1 | **Capture Once** | Every piece of knowledge enters Nabu through a single, unified ingestion pipeline. No duplicate ingestion paths. |
| 2 | **Store Forever** | Once captured, knowledge is never deleted without explicit user action. All versions are preserved. |
| 3 | **Everything is Searchable** | Every knowledge object, regardless of type, is indexed and searchable through the universal search engine. |
| 4 | **Everything Can Be Linked** | Every knowledge object can have typed relationships to any other object. The graph is the primary data model. |
| 5 | **Everything Has Metadata** | Every knowledge object carries a metadata envelope with source, timestamps, processing history, and custom properties. |
| 6 | **AI Enhances** | AI features enhance workflows (summarisation, tagging, search) but are never required for core functionality. |
| 7 | **AI Never Owns** | AI-generated content is always attributed and can be distinguished from user-created content. AI never modifies original data without user consent. |
| 8 | **Local First** | All core functionality works offline. No cloud dependency for any feature. |
| 9 | **Privacy First** | No telemetry, no data exfiltration, no external API calls by default. All processing is on-device. |
| 10 | **Thin UI** | The renderer is a presentation layer only. All business logic lives in the main process or Rust crates. |
| 11 | **Rust Owns Business Logic** | All performance-critical and correctness-critical logic lives in Rust (nabu-core). The TypeScript layer is thin. |
| 12 | **Universal Objects** | All knowledge is represented as `KnowledgeObject` instances with typed `ObjectType` and `ObjectContent`. |
| 13 | **Progressive Enhancement** | Core functionality works without AI. AI features are progressive enhancements that improve with available resources. |
| 14 | **No Duplicate Knowledge** | The system actively detects and prevents duplicate knowledge objects. Users are notified of potential duplicates. |
| 15 | **Extensibility Before Complexity** | New features are added via the plugin architecture before modifying core code. Complexity is contained within bounded contexts. |
| 16 | **Markdown-First, Not Markdown-Only** | Markdown is the primary authoring format, but the system supports and enriches all knowledge types equally. |
| 17 | **Graph-First Navigation** | The knowledge graph is the primary navigation model. File-tree navigation is a secondary view. |
| 18 | **Self-Hostable** | The product requires no external services. All AI models run locally. All storage is local. |
| 19 | **Open Format** | All data is stored in open, standard formats (Markdown, JSON, SQLite, Tantivy). No proprietary lock-in. |
| 20 | **Auditability** | Every operation that modifies knowledge is logged with timestamp, actor, and change description. |

### 5.2 Additional Principles

| # | Principle | Description |
|---|-----------|-------------|
| 21 | **Fail Gracefully** | When a processor or handler fails, the system continues processing other items. Failures are logged and reported, never silently swallowed. |
| 22 | **Observe Everything** | All subsystems emit structured telemetry. No black boxes. Every operation is observable. |
| 23 | **Test the Boundaries** | Integration tests cover all cross-subsystem boundaries. Unit tests cover all internal logic. |
| 24 | **Version Everything** | Knowledge objects, schemas, and indexes carry version numbers. Migrations are automated and reversible. |
| 25 | **Zero Trust Configuration** | All configuration changes require explicit user action. No silent configuration changes. |
| 26 | **Everything Enters Through Capture** | There is a single, unified ingestion pipeline for all knowledge. No knowledge enters the system outside the Capture Engine. |
| 27 | **Knowledge Before Files** | The unit of organisation is the knowledge object, not the file. Files are storage artefacts; knowledge objects are the meaningful units. |
| 28 | **Universal Objects** | Every knowledge type is a `KnowledgeObject` with a typed `ObjectType`. The same architectural model applies to notes, PDFs, images, people, and future types. |
| 29 | **Metadata Is First-Class** | Every knowledge object carries a complete metadata envelope: source, timestamps, processing history, checksums, and custom properties. Metadata is never an afterthought. |
| 30 | **Storage Is Separate From Organisation** | Objects are stored in the file system or blob store. Organisation happens through the graph and metadata, not through folder hierarchies. |
| 31 | **Progressive Intelligence** | AI features are progressive enhancements. Core functionality works without AI. AI improves with available resources but never blocks the user. |
| 32 | **Automation Before Manual Organisation** | The system automates tagging, relationship discovery, duplicate detection, and destination suggestion. Manual organisation is the exception, not the rule. |
| 33 | **No Vendor Lock-In** | All data is stored in open, standard formats. Users can export, migrate, or replace Nabu without losing their knowledge. |
| 34 | **User Owns Their Knowledge** | The user has complete control over their knowledge. No cloud dependency, no telemetry, no data exfiltration. The vault is the user's property. |
| 35 | **Knowledge Is Durable** | Once captured, knowledge is never deleted without explicit user action. All versions are preserved. The system is designed for decades of use. |
| 36 | **Knowledge Should Be Understandable** | The system explains its decisions: why an item was tagged, why a relationship was suggested, why a destination was recommended. Users can audit all automated actions. |
| 37 | **The Rust Backend Owns Platform Intelligence** | All performance-critical and correctness-critical logic lives in Rust (`nabu-core`). The TypeScript layer is thin. The renderer is a presentation layer only. |

---

## 6. Phased Roadmap

### Phase 1: Foundation (Months 1-3)

| Field | Detail |
|-------|--------|
| **Phase Number** | 1 |
| **Phase Name** | Foundation |
| **Purpose** | Establish the architectural foundation for the Universal Knowledge Capture Expansion: knowledge object model, capture pipeline skeleton, storage layer, and event architecture |
| **Expected Outcome** | A working knowledge object model in `nabu-core`, a skeleton `CaptureEngine` with one handler (file drop), a `StorageManager` with SQLite metadata store, and the event bus wired to all new services |
| **Major Deliverables** | 1. `KnowledgeObject` model in `nabu-core` 2. `CaptureEngine` with `FileDropHandler` 3. `StorageManager` with SQLite metadata store 4. `ProcessingPipeline` skeleton with pass-through processor 5. Event bus integration for new services 6. Updated domain models in `shared/models/` 7. ADR for capture architecture |
| **Dependencies** | Phase 1.6 (architecture gate) complete; existing Tantivy search index; existing `petgraph` graph engine |
| **Complexity** | High — new Rust crate modules, new TypeScript services, cross-layer integration |
| **Estimated Prompt Count** | 8-12 |
| **Exit Criteria** | 1. `KnowledgeObject` model compiles and serialises correctly 2. `CaptureEngine` accepts file drops and produces `IngestionResult` 3. `StorageManager` persists and retrieves `KnowledgeObject` instances 4. Event bus publishes `ItemCaptured` and `ItemProcessed` events 5. All existing tests pass 6. New integration tests pass |
| **Suggested Architecture Reviews** | Review `KnowledgeObject` model against domain model principles; review capture pipeline design against DDD bounded contexts |
| **Potential Risks** | 1. `KnowledgeObject` model may be too large for initial implementation 2. SQLite schema migration may conflict with existing data 3. Event bus integration may introduce circular dependencies |
| **Potential Future Extensions** | 1. Additional `CaptureHandler` implementations (share sheet, browser clip) 2. `Processor` implementations (OCR, metadata extraction) 3. `KnowledgeGraph` integration with typed entities |

### Phase 2: Processing Pipeline (Months 3-6)

| Field | Detail |
|-------|--------|
| **Phase Number** | 2 |
| **Phase Name** | Processing Pipeline |
| **Purpose** | Implement the full processing pipeline with OCR, metadata extraction, entity extraction, semantic tagging, AI summarisation, embedding generation, duplicate detection, relationship discovery, timeline extraction, language detection, and classification |
| **Expected Outcome** | A fully functional `ProcessingPipeline` with all 11 processors, each configurable per vault, producing enriched `KnowledgeObject` instances |
| **Major Deliverables** | 1. OCR Processor (tesseract-rs or platform-native) 2. Metadata Extractor (EXIF, PDF metadata, document properties) 3. Entity Extractor (NER model) 4. Semantic Tagger (AI-powered tag generation) 5. AI Summariser (local LLM) 6. Embedding Generator (BGE-micro or equivalent) 7. Duplicate Detector (content hash + semantic similarity) 8. Relationship Discoverer (entity-level relationship extraction) 9. Timeline Extractor (date extraction from documents) 10. Language Detector (langdetect-rs) 11. Classifier (topic classification) 12. Processor configuration UI 13. Processing history tracking |
| **Dependencies** | Phase 1 complete; AI model infrastructure; embedding model weights |
| **Complexity** | Very High — 11 processors, each with its own model dependencies and failure modes |
| **Estimated Prompt Count** | 15-20 |
| **Exit Criteria** | 1. All 11 processors compile and run independently 2. Each processor produces correct output on test fixtures 3. Processor chain is configurable per vault 4. Processing history is tracked in metadata envelope 5. Failed processors are isolated and don't block the pipeline 6. Performance benchmarks meet targets |
| **Suggested Architecture Reviews** | Review processor chain design against pipeline pattern; review AI model integration against local-first principle |
| **Potential Risks** | 1. AI model size may exceed mobile/embedded constraints 2. Processor chain may become a bottleneck for large batches 3. NER model accuracy may be insufficient for domain-specific entities |
| **Potential Future Extensions** | 1. Custom processor plugins 2. Processor chain optimisation (parallel execution) 3. Processor result caching |

### Phase 3: Knowledge Graph Expansion (Months 6-9)

| Field | Detail |
|-------|--------|
| **Phase Number** | 3 |
| **Phase Name** | Knowledge Graph Expansion |
| **Purpose** | Extend the existing file-level graph to a typed entity graph with 12+ entity types, semantic relationships, entity resolution, and graph query capabilities |
| **Expected Outcome** | A `KnowledgeGraph` that supports typed nodes (Person, Organisation, Project, Book, etc.), typed edges (mentions, cites, collaborates_with, etc.), entity resolution (merging duplicates), and Cypher-like pattern queries |
| **Major Deliverables** | 1. Typed `GraphEntity` and `EntityType` model 2. Typed `GraphEdge` and `RelationType` model 3. Entity resolution engine (merge duplicates) 4. Graph query API (pattern matching, traversal) 5. Incremental graph updates 6. Graph visualisation enhancements (typed nodes, coloured edges) 7. Graph migration from file-level to entity-level 8. ADR for graph architecture |
| **Dependencies** | Phase 2 complete; entity extraction processors operational |
| **Complexity** | High — graph migration is irreversible; entity resolution requires careful design |
| **Estimated Prompt Count** | 10-14 |
| **Exit Criteria** | 1. Typed graph model compiles and serialises correctly 2. Entity resolution correctly merges duplicate entities 3. Graph query API supports pattern matching 4. Incremental updates work correctly 5. Graph visualisation shows typed nodes and coloured edges 6. Migration from file-level graph preserves all existing data |
| **Suggested Architecture Reviews** | Review entity resolution strategy against data integrity requirements; review graph query API against performance requirements |
| **Potential Risks** | 1. Graph migration may corrupt existing data 2. Entity resolution may incorrectly merge distinct entities 3. Graph query performance may degrade with large datasets |
| **Potential Future Extensions** | 1. Graph-based recommendation engine 2. Temporal graph analysis (timeline of relationships) 3. Graph export (GraphML, GEXF) |

### Phase 4: Universal Search (Months 9-12)

| Field | Detail |
|-------|--------|
| **Phase Number** | 4 |
| **Phase Name** | Universal Search |
| **Purpose** | Implement universal hybrid search across all knowledge types with full-text, vector, and graph-signal ranking, faceted filtering, and mixed-type results |
| **Expected Outcome** | A `UniversalSearchEngine` that indexes all knowledge object types, supports hybrid ranking (TF-IDF + semantic + graph signals), faceted filtering, and returns mixed-type results ranked by relevance |
| **Major Deliverables** | 1. Universal search index (all object types) 2. Hybrid ranker (TF-IDF + vector + graph signals) 3. Faceted search UI 4. Search result ranking with explanation 5. Search suggestions and auto-complete 6. Search history and saved searches 7. Search API (typed IPC) 8. ADR for search architecture |
| **Dependencies** | Phase 3 complete; embedding generation operational; all object types indexed |
| **Complexity** | High — hybrid ranking requires careful weight tuning; faceted search requires UI investment |
| **Estimated Prompt Count** | 10-14 |
| **Exit Criteria** | 1. Search index contains all object types 2. Hybrid ranking produces relevant results across types 3. Faceted filtering works correctly 4. Search suggestions are accurate 5. Search performance meets targets (< 100ms for typical queries) 6. All existing search functionality is preserved |
| **Suggested Architecture Reviews** | Review hybrid ranking weights against relevance benchmarks; search index schema against performance requirements |
| **Potential Risks** | 1. Hybrid ranking may produce unexpected results for edge cases 2. Search index size may grow unbounded 3. Faceted search UI may be complex for users |
| **Potential Future Extensions** | 1. Natural language query parsing 2. Search result clustering 3. Personalised ranking (learn from user behaviour) |

### Phase 5: AI Enrichment (Months 12-15)

| Field | Detail |
|-------|--------|
| **Phase Number** | 5 |
| **Phase Name** | AI Enrichment |
| **Purpose** | Implement the full AI enrichment pipeline with local embeddings, summarisation, entity extraction, duplicate detection, and relationship suggestion |
| **Expected Outcome** | An `AIEnrichmentPipeline` that runs entirely on-device, generating embeddings, summarising content, extracting entities, detecting duplicates, and suggesting relationships |
| **Major Deliverables** | 1. Embedding model integration (BGE-micro or equivalent) 2. Summarisation model integration (local LLM) 3. Entity extraction model (NER) 4. Duplicate detection (semantic similarity) 5. Relationship suggestion (graph-based inference) 6. AI processing configuration UI 7. AI processing history and audit log 8. ADR for AI architecture |
| **Dependencies** | Phase 2 complete; embedding infrastructure operational; local LLM model available |
| **Complexity** | Very High — AI models are large and require careful integration; on-device inference has performance constraints |
| **Estimated Prompt Count** | 12-18 |
| **Exit Criteria** | 1. Embeddings are generated for all object types 2. Summaries are accurate and useful 3. Entity extraction identifies relevant entities 4. Duplicate detection correctly identifies near-duplicates 5. Relationship suggestions are relevant 6. AI processing is configurable per vault 7. AI processing does not block the main thread 8. All AI processing is local (no cloud dependency) |
| **Suggested Architecture Reviews** | Review AI model selection against local-first principle; review on-device inference performance against UX requirements |
| **Potential Risks** | 1. AI models may be too large for some devices 2. On-device inference may be too slow for large documents 3. AI-generated content may be inaccurate or misleading |
| **Potential Future Extensions** | 1. Custom AI model plugins 2. AI-powered automation rules 3. AI-assisted knowledge graph exploration |

### Phase 6: Integrations & Automation (Months 15-18)

| Field | Detail |
|-------|--------|
| **Phase Number** | 6 |
| **Phase Name** | Integrations & Automation |
| **Purpose** | Implement external integrations (email, Git, API webhooks, calendar, task manager) and the automation engine for rule-based processing |
| **Expected Outcome** | An `IntegrationHub` with connectors for email, Git, APIs, and an `AutomationEngine` for rule-based processing workflows |
| **Major Deliverables** | 1. Email connector (IMAP/POP3, MIME parsing) 2. Git connector (commit history as knowledge) 3. API webhook receiver 4. Calendar connector (ICS import) 5. Task manager connector 6. Automation engine (rules, triggers, actions) 7. Automation UI (rule builder) 8. Integration configuration UI 9. ADR for integration architecture |
| **Dependencies** | Phase 4 complete; capture pipeline operational; processing pipeline operational |
| **Complexity** | High — each connector has its own protocol and error handling; automation engine requires careful rule design |
| **Estimated Prompt Count** | 12-16 |
| **Exit Criteria** | 1. All connectors authenticate and sync correctly 2. Automation rules evaluate and trigger actions correctly 3. Integration errors are handled gracefully 4. Automation UI is intuitive and complete 5. All integrations are configurable per vault 6. Integration data is stored as KnowledgeObjects |
| **Suggested Architecture Reviews** | Review connector error handling against resilience requirements; review automation rule engine against security requirements |
| **Potential Risks** | 1. External service APIs may change without notice 2. Automation rules may have unintended side effects 3. Integration data may contain sensitive information |
| **Potential Future Extensions** | 1. Marketplace for community connectors 2. Conditional automation (if-then-else chains) 3. Integration with cloud storage providers |

### Phase 7: Security & Platform Expansion (Months 18-21)

| Field | Detail |
|-------|--------|
| **Phase Number** | 7 |
| **Phase Name** | Security & Platform Expansion |
| **Purpose** | Implement vault-level encryption, cross-platform desktop support (Linux, Windows), and mobile capture (share sheet, camera) |
| **Expected Outcome** | Encrypted vaults, cross-platform desktop app, and mobile capture capabilities |
| **Major Deliverables** | 1. Vault-level encryption (AES-256-GCM) 2. Linux support (CI, testing, packaging) 3. Windows support (CI, testing, packaging) 4. macOS share sheet integration 5. iOS share sheet integration 6. Android share sheet integration 7. Camera capture (mobile) 8. Voice memo capture (mobile) 9. Security audit and penetration testing 10. ADR for security architecture |
| **Dependencies** | Phase 6 complete; all core subsystems operational |
| **Complexity** | Very High — cross-platform support requires significant testing and packaging work; encryption must be carefully implemented |
| **Estimated Prompt Count** | 15-20 |
| **Exit Criteria** | 1. Vault encryption works correctly on all platforms 2. App builds and runs on macOS, Linux, and Windows 3. Mobile share sheet integration works on iOS and Android 4. Security audit passes with no critical findings 5. All platform-specific tests pass |
| **Suggested Architecture Reviews** | Review encryption implementation against security best practices; review cross-platform testing strategy |
| **Potential Risks** | 1. Encryption key management may be complex on mobile 2. Cross-platform testing may reveal platform-specific bugs 3. Mobile capture may have performance issues on low-end devices |
| **Potential Future Extensions** | 1. Biometric unlock 2. Multi-vault encryption with different keys 3. Secure sharing between vaults |

### Phase 8: Plugin System & Extensibility (Months 21-24)

| Field | Detail |
|-------|--------|
| **Phase Number** | 8 |
| **Phase Name** | Plugin System & Extensibility |
| **Purpose** | Implement the plugin architecture allowing new knowledge types, processors, capture handlers, and integrations to be added without modifying core code |
| **Expected Outcome** | A fully functional plugin system with dynamic library loading, WASM module support, and a plugin marketplace/registry |
| **Major Deliverables** | 1. Plugin interface definitions (CaptureHandler, Processor, Connector, KnowledgeObjectType) 2. Dynamic library loader (.so/.dylib/.dll) 3. WASM module loader 4. Plugin registry and discovery 5. Plugin sandboxing and security 6. Plugin UI (install, configure, enable/disable) 7. Plugin marketplace/registry 8. Plugin development SDK and documentation 9. ADR for plugin architecture |
| **Dependencies** | Phase 7 complete; all core subsystems operational and stable |
| **Complexity** | High — plugin sandboxing requires careful security design; plugin marketplace requires infrastructure |
| **Estimated Prompt Count** | 10-14 |
| **Exit Criteria** | 1. Plugins can be loaded dynamically at runtime 2. WASM modules execute in sandboxed environment 3. Plugin security model prevents malicious plugins 4. Plugin UI is intuitive and complete 5. Plugin SDK is documented and usable 6. Sample plugins demonstrate all interface types |
| **Suggested Architecture Reviews** | Review plugin sandboxing against security requirements; review plugin API stability against versioning strategy |
| **Potential Risks** | 1. Plugin security vulnerabilities may compromise the vault 2. Plugin API may change between versions 3. Plugin marketplace may become a support burden |
| **Potential Future Extensions** | 1. Community plugin marketplace 2. Plugin versioning and compatibility matrix 3. Plugin signing and verification |

---

## 7. Compliance / Evidence Mapping

### 7.1 Security

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Vault-level encryption (AES-256-GCM) | Implementation in `SecurityManager`; key derivation using user password + salt; encrypted files in vault directory | Planned (Phase 7) |
| No telemetry | No telemetry code in codebase; `deny.toml` blocks telemetry dependencies; CI checks for telemetry imports | Planned |
| Input validation | `CaptureHandler.can_handle()` validates MIME type; `IngestionRequest` validated by Zod schemas; size limits enforced | Planned (Phase 1) |
| Path traversal prevention | All file paths validated against vault root directory; `Path::canonicalise()` used for all file operations | Planned (Phase 1) |
| Audit logging | `AuditEntry` model; all security-relevant operations logged to `.nabu/audit.log` | Planned (Phase 7) |
| Dependency audit | `deny.toml` for license auditing; `cargo audit` in CI; `npm audit` in CI | Existing |
| Secure defaults | Encryption enabled by default; sharing is opt-in; no external connections by default | Planned (Phase 7) |

### 7.2 Privacy

| Requirement | Evidence | Status |
|-------------|----------|--------|
| No data exfiltration | All processing is on-device; no network calls by default; AI models are local | Planned (Phase 5) |
| User data ownership | All data stored in user's vault directory; no proprietary formats; open formats only | Existing |
| GDPR compliance | No personal data collection; no third-party data sharing; data export/delete supported | Planned |
| Data minimisation | Only necessary metadata is stored; processing history is configurable | Planned (Phase 1) |
| Right to erasure | Users can delete any knowledge object; deletion is permanent and irreversible | Existing (file deletion) |
| Right to portability | All data in open formats (Markdown, JSON, SQLite); export to Markdown, HTML, PDF | Existing |

### 7.3 Accessibility

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Keyboard navigation | All UI components support keyboard navigation; command palette for quick actions | Existing |
| Screen reader support | Semantic HTML in renderer; ARIA labels on interactive elements | Planned (Phase 4) |
| High contrast mode | Theme engine supports custom CSS; high contrast theme available | Existing |
| Font size adjustment | Theme engine supports font size configuration | Existing |
| Focus management | Focus is managed correctly in modals and dialogs | Existing |

### 7.4 Performance

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Search latency < 100ms | Tantivy full-text search; vector search with FAISS; hybrid ranking with caching | Planned (Phase 4) |
| Capture latency < 100ms | Handler-level capture is synchronous and fast; processing is async | Planned (Phase 1) |
| Memory usage < 500MB peak | Rust memory safety; streaming processing for large files; memory-mapped indexes | Planned |
| Startup time < 2s | Lazy loading of services; incremental index loading; warm cache on startup | Planned |
| Indexing throughput > 1000 files/min | Tantivy batch indexing; parallel processing; incremental updates | Planned (Phase 1) |

### 7.5 Maintainability

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Code coverage > 80% | Unit tests for all new modules; integration tests for cross-subsystem boundaries | Planned |
| Documentation | ADRs for all major decisions; inline documentation for all public APIs | Planned |
| Type safety | TypeScript strict mode; Rust type system; Zod schemas for IPC contracts | Existing |
| Linting | ESLint with architecture enforcement rules; `cargo clippy` in CI | Existing |
| Formatting | Prettier for TypeScript; `rustfmt` for Rust | Existing |

### 7.6 Scalability

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Vault size > 100GB | Tantivy handles large indexes; SQLite scales to GBs; file system storage is unlimited | Planned |
| Object count > 1M | Tantivy supports millions of documents; graph engine uses efficient data structures | Planned |
| Concurrent users > 1 | Single-user model; no concurrent access support (by design) | N/A |
| Processing pipeline throughput | Configurable worker threads; backpressure mechanism; batch processing | Planned (Phase 1) |

### 7.7 Reliability

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Crash recovery | Checkpoint-based batch processing; transaction-based storage operations; automatic recovery on restart | Planned |
| Data integrity | Checksums for all stored objects; SQLite WAL mode; Tantivy durability guarantees | Planned |
| Error handling | All errors are typed and propagated; no swallowed errors; dead letter queue for failed items | Planned |
| Graceful degradation | AI features degrade gracefully when models are unavailable; search works without embeddings | Planned |

### 7.8 Self-Hostability

| Requirement | Evidence | Status |
|-------------|----------|--------|
| No external services required | All AI models run locally; all storage is local; no cloud dependency | Planned |
| Single-command setup | `cargo tauri dev` for development; packaged installer for production | Existing |
| Configuration via files | Vault config in `.nabu/config.json`; settings in `~/.config/nabu/settings.json` | Existing |
| Open source | AGPL-3.0 license; all code publicly available | Existing |


### 7.9 Offline-First Behaviour

| Requirement | Evidence | Status |
|-------------|----------|--------|
| Core functionality works offline | Capture, processing, search, graph, and AI all work without network | Planned |
| No network fallback | No graceful degradation to cloud; offline is the only mode | Planned |
| Sync is optional | Users may use their own sync solution at the vault directory level | Planned |

### 7.10 Local-First Philosophy

| Requirement | Evidence | Status |
|-------------|----------|--------|
| All data stored locally | Vault directory contains all data; no cloud storage | Existing |
| No cloud sync built in | No sync service in codebase; users bring their own sync | Existing |
| AI runs on-device | All AI models are local; no API calls to external services | Planned |
| No telemetry | No telemetry code; no data collection | Planned |

### 7.11 Open-Source Sustainability

| Requirement | Evidence | Status |
|-------------|----------|--------|
| AGPL-3.0 license | Existing; all contributions must be AGPL-3.0 compatible | Existing |
| Clear contribution guidelines | `CONTRIBUTING.md` exists | Existing |
| Issue tracking | GitHub Issues used for bug reports and feature requests | Existing |
| Release process | `scripts/build-release.sh` exists; CI builds releases | Existing |
| Documentation | Comprehensive architecture docs; ADRs; inline docs | Planned |
---

## 8. Observability

### 8.1 Metrics

| Metric | Type | Collection | Target |
|--------|------|-----------|--------|
| Capture latency | Histogram | Per-capture timing | p99 < 100ms |
| Processing pipeline duration | Histogram | Per-item processing time | p99 < 5s |
| Search latency | Histogram | Per-query time | p99 < 100ms |
| Indexing throughput | Counter | Documents indexed per second | > 1000/sec |
| Graph node count | Gauge | Total nodes in knowledge graph | N/A |
| Graph edge count | Gauge | Total edges in knowledge graph | N/A |
| AI model load time | Histogram | Time to load AI model | p99 < 10s |
| AI inference latency | Histogram | Per-inference time | p99 < 5s |
| Duplicate detection rate | Gauge | Percentage of items flagged as duplicates | N/A |
| Entity extraction accuracy | Gauge | Precision/recall of NER | > 0.85 |
| Vault size | Gauge | Total vault directory size | N/A |
| Object count | Gauge | Total KnowledgeObjects stored | N/A |
| Error rate | Gauge | Percentage of failed operations | < 1% |
| Memory usage | Gauge | RSS of main process | < 500MB |
| CPU usage | Gauge | CPU utilisation during processing | < 80% |
| Disk I/O | Gauge | Read/write bytes per second | N/A |
| Network usage | Gauge | Bytes sent/received (should be 0)

### 8.2 Logging

| Log Level | Usage | Format |
|-----------|-------|--------|
| ERROR | Failed operations, security violations, data corruption | Structured JSON with error code, stack trace, context |
| WARN | Degraded functionality, recoverable errors, performance warnings | Structured JSON with warning code, context |
| INFO | Service lifecycle events, configuration changes, user actions | Structured JSON with event type, relevant IDs |
| DEBUG | Detailed processing steps, handler decisions, pipeline state | Structured JSON with debug code, verbose context |
| TRACE | Per-item processing details, AI model inputs/outputs | Structured JSON with trace code, full context |

### 8.3 Tracing

| Trace Type | Purpose | Sampling |
|-----------|---------|----------|
| Capture trace | End-to-end capture pipeline timing | 100% for errors; 1% for success |
| Processing trace | Per-processor timing and output | 100% for errors; 1% for success |
| Search trace | Query parsing, index lookup, ranking | 100% for errors; 5% for success |
| AI trace | Model loading, inference, output | 100% for errors; 0.1% for success |
| Graph trace | Traversal, entity resolution, query | 100% for errors; 1% for success |

### 8.4 Health Checks

| Check | Type | Frequency | Alert Threshold |
|-------|------|-----------|-----------------|
| Vault accessibility | Liveness | 30s | Vault directory not accessible |
| Search index health | Liveness | 60s | Index read errors > 0 |
| Processing pipeline health | Liveness | 30s | Pipeline stalled > 5min |
| AI model availability | Readiness | 60s | Model not loaded |
| Storage capacity | Readiness | 5min | Disk usage > 90% |
| Memory usage | Readiness | 30s | RSS > 80% of limit |
| Event bus health | Liveness | 30s | Event backlog > 1000 |

### 8.5 Dashboards

| Dashboard | Metrics | Audience |
|-----------|---------|----------|
| **Capture Dashboard** | Capture rate, handler success rate, normalisation errors, ingestion queue depth | Engineering |
| **Processing Dashboard** | Pipeline throughput, processor success rate, processing latency, error rate | Engineering |
| **Search Dashboard** | Query latency, index size, hit rate, facet usage | Engineering + Product |
| **AI Dashboard** | Model load status, inference latency, embedding dimensions, duplicate rate | Engineering |
| **Graph Dashboard** | Node count, edge count, entity resolution rate, traversal latency | Engineering + Product |
| **Storage Dashboard** | Vault size, object count, index size, cache hit rate | Engineering |
| **System Dashboard** | CPU, memory, disk I/O, network, uptime | Engineering |

### 8.6 Alerts

| Alert | Condition | Severity | Notification |
|-------|-----------|----------|-------------|
| Capture handler failure | Any handler fails > 5 times in 5min | Critical | PagerDuty + Slack |
| Processing pipeline stalled | No items processed for > 10min | Critical | PagerDuty + Slack |
| Search index corruption | Index read errors > 0 | Critical | PagerDuty + Slack |
| AI model unavailable | Model not loaded for > 5min | Warning | Slack |
| Storage capacity low | Disk usage > 90% | Warning | Slack + Email |
| Memory pressure | RSS > 80% of limit | Warning | Slack |
| Event bus backlog | Backlog > 10000 | Warning | Slack |
| Vault encryption key rotation due | Key age > 90 days | Info | Email |

### 8.7 Performance Monitoring

| Metric | Collection Method | Storage | Retention |
|--------|-------------------|---------|-----------|
| Capture latency | Instrumented handlers | Prometheus | 30 days |
| Processing latency | Instrumented pipeline | Prometheus | 30 days |
| Search latency | Instrumented search engine | Prometheus | 30 days |
| AI inference latency | Instrumented AI pipeline | Prometheus | 30 days |
| Graph traversal latency | Instrumented graph engine | Prometheus | 30 days |
| Memory usage | Process metrics | Prometheus | 7 days |
| CPU usage | Process metrics | Prometheus | 7 days |
| Disk I/O | System metrics | Prometheus | 7 days |

### 8.8 Knowledge Processing Telemetry

| Metric | Description | Collection |
|--------|-------------|-----------|
| Items captured per hour | Throughput of the capture pipeline | Prometheus counter |
| Items processed per hour | Throughput of the processing pipeline | Prometheus counter |
| Processor success rate | Percentage of processors that succeed per item | Prometheus histogram |
| Processor latency | Per-processor processing time | Prometheus histogram |
| Duplicate detection rate | Percentage of items flagged as duplicates | Prometheus gauge |
| Entity extraction count | Number of entities extracted per item | Prometheus histogram |
| Tag generation accuracy | Precision/recall of auto-generated tags | Prometheus gauge |
| Embedding generation rate | Items embedded per hour | Prometheus counter |

### 8.9 AI Pipeline Telemetry

| Metric | Description | Collection |
|--------|-------------|-----------|
| Model load time | Time to load AI model into memory | Prometheus histogram |
| Inference latency | Time per AI inference call | Prometheus histogram |
| Model memory usage | Memory used by AI models | Prometheus gauge |
| GPU utilisation | GPU usage for AI inference | Prometheus gauge |
| Token usage | Tokens processed by AI models | Prometheus counter |
| Summarisation quality | Human-rated summary quality (sampled) | Prometheus gauge |
| Entity extraction precision | Precision of NER model | Prometheus gauge |
| Entity extraction recall | Recall of NER model | Prometheus gauge |

### 8.10 Search Metrics

| Metric | Description | Collection |
|--------|-------------|-----------|
| Query latency | Time from query to results | Prometheus histogram |
| Query throughput | Queries per second | Prometheus counter |
| Result count | Number of results per query | Prometheus histogram |
| Result relevance | User click-through on results | Prometheus counter |
| Facet usage | Most used facets | Prometheus counter |
| Index size | Size of search index | Prometheus gauge |
| Index update latency | Time to update index after ingestion | Prometheus histogram |

### 8.11 Graph Metrics

| Metric | Description | Collection |
|--------|-------------|-----------|
| Node count | Total nodes in knowledge graph | Prometheus gauge |
| Edge count | Total edges in knowledge graph | Prometheus gauge |
| Entity type distribution | Count of nodes per entity type | Prometheus gauge |
| Relation type distribution | Count of edges per relation type | Prometheus gauge |
| Traversal latency | Time for graph traversal queries | Prometheus histogram |
| Entity resolution rate | Rate of entity resolution operations | Prometheus counter |
| Connected components | Number of disconnected graph components | Prometheus gauge |
| Average degree | Average number of edges per node | Prometheus gauge |

### 8.12 Operational KPIs

| KPI | Description | Target |
|-----|-------------|--------|
| Capture success rate | Percentage of capture attempts that succeed | > 99% |
| Processing success rate | Percentage of items that complete processing | > 95% |
| Search success rate | Percentage of search queries that return results | > 99% |
| AI availability | Percentage of time AI models are loaded and ready | > 95% |
| Vault accessibility | Percentage of time vault is accessible | > 99.9% |
| Mean time to recovery | Average time to recover from failures | < 5min |
| Mean time between failures | Average time between failures | > 720h |
| User satisfaction | User-reported satisfaction with capture/processing/search | > 4.0/5.0 |

---

## 9. Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Universal Knowledge Capture Expansion                  │
│                         Dependency Graph                                  │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                    Programme Level                                │  │
│  │                                                                   │  │
│  │  Universal Knowledge Capture Expansion                           │  │
│  │  ├── Phase 1: Foundation                                         │  │
│  │  │   ├── Knowledge Object Model (nabu-core)                      │  │
│  │  │   ├── Capture Engine (CaptureEngine)                          │  │
│  │  │   ├── Knowledge Inbox (NEW)                                   │  │
│  │  │   ├── Storage Manager (StorageManager)                        │  │
│  │  │   └── Event Bus Integration                                   │  │
│  │  ├── Phase 2: Processing Pipeline                                │  │
│  │  │   ├── OCR Processor                                           │  │
│  │  │   ├── Metadata Extractor                                      │  │
│  │  │   ├── Entity Extractor                                        │  │
│  │  │   ├── Semantic Tagger                                         │  │
│  │  │   ├── AI Summariser                                           │  │
│  │  │   ├── Embedding Generator                                     │  │
│  │  │   ├── Duplicate Detector                                      │  │
│  │  │   ├── Relationship Discoverer                                 │  │
│  │  │   ├── Timeline Extractor                                      │  │
│  │  │   ├── Language Detector                                       │  │
│  │  │   └── Classifier                                              │  │
│  │  ├── Phase 3: Knowledge Graph Expansion                          │  │
│  │  │   ├── Typed Entity Model                                      │  │
│  │  │   ├── Entity Resolution Engine                                │  │
│  │  │   ├── Graph Query API                                         │  │
│  │  │   └── Graph Migration                                         │  │
│  │  ├── Phase 4: Universal Search                                   │  │
│  │  │   ├── Universal Search Index                                  │  │
│  │  │   ├── Hybrid Ranker                                           │  │
│  │  │   ├── Faceted Search UI                                       │  │
│  │  │   └── Search API                                              │  │
│  │  ├── Phase 5: AI Enrichment                                      │  │
│  │  │   ├── Embedding Model Integration                             │  │
│  │  │   ├── Summarisation Model                                     │  │
│  │  │   ├── NER Model                                               │  │
│  │  │   ├── Duplicate Detection                                     │  │
│  │  │   └── Relationship Suggestion                                 │  │
│  │  ├── Phase 6: Integrations & Automation                          │  │
│  │  │   ├── Email Connector                                         │  │
│  │  │   ├── Git Connector                                           │  │
│  │  │   ├── API Webhook Receiver                                    │  │
│  │  │   ├── Calendar Connector                                      │  │
│  │  │   ├── Task Manager Connector                                  │  │
│  │  │   └── Automation Engine                                       │  │
│  │  ├── Phase 7: Security & Platform Expansion                      │  │
│  │  │   ├── Vault Encryption                                        │  │
│  │  │   ├── Linux Support                                           │  │
│  │  │   ├── Windows Support                                         │  │
│  │  │   ├── Mobile Capture                                          │  │
│  │  │   └── Security Audit                                          │  │
│  │  └── Phase 8: Plugin System                                      │  │
│  │       ├── Plugin Interface                                      │  │
│  │       ├── Dynamic Library Loader                                │  │
│  │       ├── WASM Module Loader                                    │  │
│  │       ├── Plugin Registry                                       │  │
│  │       └── Plugin Marketplace                                    │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                    Subsystem Dependencies                         │  │
│  │                                                                   │  │
│  │  CaptureEngine ──depends on──▶ StorageManager                    │  │
│  │  CaptureEngine ──depends on──▶ EventBus                          │  │
│  │  KnowledgeInbox ──depends on──▶ CaptureEngine                    │  │
│  │  KnowledgeInbox ──depends on──▶ ProcessingPipeline               │  │
│  │  KnowledgeInbox ──depends on──▶ EventBus                         │  │
│  │  ProcessingPipeline ──depends on──▶ CaptureEngine               │  │
│  │  ProcessingPipeline ──depends on──▶ AIEnrichmentPipeline        │  │
│  │  ProcessingPipeline ──depends on──▶ StorageManager               │  │
│  │  KnowledgeGraph ──depends on──▶ StorageManager                   │  │
│  │  KnowledgeGraph ──depends on──▶ EventBus                         │  │
│  │  UniversalSearchEngine ──depends on──▶ StorageManager           │  │
│  │  UniversalSearchEngine ──depends on──▶ AIEnrichmentPipeline     │  │
│  │  UniversalSearchEngine ──depends on──▶ KnowledgeGraph           │  │
│  │  AIEnrichmentPipeline ──depends on──▶ StorageManager             │  │
│  │  AIEnrichmentPipeline ──depends on──▶ KnowledgeGraph             │  │
│  │  AutomationEngine ──depends on──▶ EventBus                       │  │
│  │  AutomationEngine ──depends on──▶ ProcessingPipeline             │  │
│  │  IntegrationHub ──depends on──▶ CaptureEngine                    │  │
│  │  IntegrationHub ──depends on──▶ StorageManager                   │  │
│  │  SecurityManager ──depends on──▶ StorageManager                   │  │
│  │  SecurityManager ──depends on──▶ EventBus                        │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                    Knowledge Flow Dependencies                    │  │
│  │                                                                   │  │
│  │  Capture Sources ──▶ Universal Capture Pipeline ──▶ Knowledge Inbox │
│  │                                                          │        │
│  │  Knowledge Inbox ──▶ Knowledge Objects ──▶ Metadata            │  │
│  │                                                          │        │
│  │  Knowledge Objects ──▶ Graph ──▶ Search                        │  │
│  │  Knowledge Objects ──▶ Collections ──▶ Automation              │  │
│  │  Knowledge Objects ──▶ Future Platform Features                │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                    Critical Architectural Dependencies            │  │
│  │                                                                   │  │
│  │  nabu-core ──┬── Parser (markdown, PDF, OCR)                    │  │
│  │              ├── Indexer (Tantivy)                               │  │
│  │              ├── Graph Engine (petgraph)                         │  │
│  │              ├── Vault Config                                    │  │
│  │              ├── Template Manager                                │  │
│  │              ├── Export Engine                                   │  │
│  │              ├── Theme Manager                                   │  │
│  │              ├── Knowledge Object Model (NEW)                   │  │
│  │              ├── Capture Pipeline (NEW)                         │  │
│  │              ├── AI Enrichment Models (NEW)                     │  │
│  │              └── Plugin Interface (NEW)                         │  │
│  │                                                                   │  │
│  │  src-tauri ──┬── Commands (IPC)                                  │  │
│  │              ├── Vault Service                                   │  │
│  │              ├── Search Service                                  │  │
│  │              ├── Graph Engine                                    │  │
│  │              ├── Settings Service                                │  │
│  │              ├── Capture Engine (NEW)                            │  │
│  │              ├── Knowledge Inbox (NEW)                           │  │
│  │              ├── Processing Pipeline (NEW)                       │  │
│  │              ├── AI Enrichment Pipeline (NEW)                    │  │
│  │              ├── Knowledge Graph Service (NEW)                   │  │
│  │              ├── Universal Search Engine (NEW)                   │  │
│  │              ├── Storage Manager (NEW)                           │  │
│  │              ├── Security Manager (NEW)                          │  │
│  │              ├── Automation Engine (NEW)                         │  │
│  │              └── Integration Hub (NEW)                           │  │
│  │                                                                   │  │
│  │  nabu-ui ────┬── App Shell                                       │  │
│  │              ├── File Tree                                       │  │
│  │              ├── Note Editor                                     │  │
│  │              ├── Graph View                                      │  │
│  │              ├── PDF Viewer                                      │  │
│  │              ├── Search Panel                                    │  │
│  │              ├── Settings Panel                                  │  │
│  │              ├── Knowledge Inbox UI (NEW)                        │  │
│  │              ├── Capture UI (NEW)                                │  │
│  │              ├── Processing Status UI (NEW)                      │  │
│  │              ├── Graph Explorer (NEW)                            │  │
│  │              └── Search Results UI (NEW)                         │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 10. Critical Path

### 10.1 Minimum Viable Implementation Order

The critical path identifies the minimum sequence of work required to deliver a functional knowledge capture and processing system. Work not on the critical path can proceed in parallel.

```
Critical Path:

1. Knowledge Object Model (nabu-core) ──────────────────────────────┐
   │                                                                  │
   ▼                                                                  │
2. CaptureEngine + FileDropHandler ──────────────────────────────────┤
   │                                                                  │
   ▼                                                                  │
3. Knowledge Inbox ──────────────────────────────────────────────────┤
   │                                                                  │
   ▼                                                                  │
4. StorageManager (SQLite metadata store) ───────────────────────────┤
   │                                                                  │
   ▼                                                                  │
5. ProcessingPipeline (skeleton, pass-through) ──────────────────────┤
   │                                                                  │
   ▼                                                                  │
6. Event Bus Integration (new services → appEventBus) ───────────────┤
   │                                                                  │
   ▼                                                                  │
7. UniversalSearchEngine (index all object types) ───────────────────┤
   │                                                                  │
   ▼                                                                  │
8. AIEnrichmentPipeline (embeddings + summarisation) ────────────────┤
   │                                                                  │
   ▼                                                                  │
9. KnowledgeGraph (typed entities + semantic edges) ─────────────────┤
   │                                                                  │
   ▼                                                                  │
10. Universal Search (hybrid ranking + faceted filtering) ───────────┤
    │                                                                  │
    ▼                                                                  │
11. Automation Engine + Integration Hub ──────────────────────────────┘
    (parallel with 8-10)
```

### 10.2 Knowledge Inbox in the Critical Path

The Knowledge Inbox is required before advanced automation because it provides:

- **Review** — users can inspect captured items before they become permanent knowledge
- **Confidence validation** — users can verify AI-generated suggestions (destination, tags, entities)
- **Correction** — users can correct misclassified items before they propagate through the system
- **Import management** — users control the flow of knowledge into their vault
- **Processing transparency** — users can see exactly what happened to each captured item

Without the Inbox, automation would operate on unvalidated data, leading to incorrect tags, wrong destinations, and polluted knowledge graphs.

### 10.3 Dependency Justification

| Dependency | Why It Exists |
|-----------|---------------|
| Knowledge Object Model → CaptureEngine | The capture engine produces `KnowledgeObject` instances; the model must exist first |
| Knowledge Object Model → KnowledgeInbox | The inbox displays `KnowledgeObject` instances; the model must exist first |
| Knowledge Object Model → StorageManager | The storage manager persists `KnowledgeObject` instances; the model must exist first |
| Knowledge Object Model → ProcessingPipeline | The processing pipeline transforms `KnowledgeObject` instances; the model must exist first |
| Knowledge Object Model → UniversalSearchEngine | The search engine indexes `KnowledgeObject` instances; the model must exist first |
| Knowledge Object Model → KnowledgeGraph | The graph engine stores `KnowledgeObject` nodes; the model must exist first |
| Knowledge Object Model → AIEnrichmentPipeline | The AI pipeline enriches `KnowledgeObject` instances; the model must exist first |
| CaptureEngine → KnowledgeInbox | The inbox receives captured items from the capture engine |
| CaptureEngine → StorageManager | Captured items must be persisted; storage must be available |
| CaptureEngine → EventBus | Capture events must be published for downstream processing |
| KnowledgeInbox → ProcessingPipeline | The inbox triggers processing on approved items |
| KnowledgeInbox → EventBus | Inbox events must be published for downstream processing |
| ProcessingPipeline → CaptureEngine | The pipeline processes items captured by the engine |
| ProcessingPipeline → AIEnrichmentPipeline | The pipeline triggers AI enrichment on captured items |
| ProcessingPipeline → StorageManager | Processed items must be persisted |
| StorageManager → EventBus | Storage events must be published for search indexing and graph updates |
| EventBus → UniversalSearchEngine | Search engine subscribes to storage events for indexing |
| EventBus → KnowledgeGraph | Graph engine subscribes to storage events for graph updates |
| EventBus → AIEnrichmentPipeline | AI pipeline subscribes to capture events for enrichment |
| UniversalSearchEngine → StorageManager | Search engine reads from and writes to the storage manager |
| UniversalSearchEngine → AIEnrichmentPipeline | Search engine uses embeddings from AI enrichment for semantic search |
| UniversalSearchEngine → KnowledgeGraph | Search engine uses graph signals for boosted ranking |
| AIEnrichmentPipeline → StorageManager | AI pipeline reads from and writes to the storage manager |
| AIEnrichmentPipeline → KnowledgeGraph | AI pipeline creates graph entities and edges |
| KnowledgeGraph → StorageManager | Graph engine reads from and writes to the storage manager |
| KnowledgeGraph → EventBus | Graph updates are published as events |
| AutomationEngine → EventBus | Automation engine subscribes to all events for rule evaluation |
| AutomationEngine → ProcessingPipeline | Automation engine triggers processing actions |
| IntegrationHub → CaptureEngine | Integration hub feeds external data into the capture engine |
| IntegrationHub → StorageManager | Integration hub persists synced data |

### 10.3 Parallel Work

The following work can proceed in parallel with the critical path:

| Work Stream | Parallel With | Rationale |
|------------|---------------|-----------|
| UI components for capture | Phase 1 (CaptureEngine) | UI can be built against the capture engine interface before the engine is complete |
| AI model integration | Phase 1 (StorageManager) | AI model selection and integration can proceed independently of storage |
| Graph visualisation enhancements | Phase 3 (KnowledgeGraph) | UI work for graph visualisation can proceed in parallel with graph model development |
| Search UI enhancements | Phase 4 (UniversalSearch) | Search UI can be built against the search engine interface before the engine is complete |
| Security audit | Phase 7 (Security) | Security audit can begin before encryption is implemented |
| Plugin SDK design | Phase 8 (Plugin System) | Plugin interface design can proceed before the plugin system is implemented |
| Documentation | All phases | Documentation can be written in parallel with implementation |
| Testing infrastructure | All phases | Test fixtures, test harnesses, and CI configuration can be set up in parallel |

### 10.4 Sequential Work

The following work must proceed sequentially:

| Sequence | Work | Reason |
|----------|------|--------|
| 1 → 2 | Knowledge Object Model → CaptureEngine | Capture engine produces KnowledgeObject instances |
| 2 → 3 | CaptureEngine → StorageManager | Captured items must be persisted |
| 3 → 4 | StorageManager → ProcessingPipeline | Pipeline processes persisted items |
| 4 → 5 | ProcessingPipeline → EventBus Integration | Events must flow from pipeline to subscribers |
| 5 → 6 | EventBus → UniversalSearchEngine | Search engine subscribes to events |
| 6 → 7 | UniversalSearchEngine → AIEnrichmentPipeline | AI enrichment provides embeddings for search |
| 7 → 8 | AIEnrichmentPipeline → KnowledgeGraph | AI enrichment creates graph entities |
| 8 → 9 | KnowledgeGraph → Universal Search | Graph signals boost search ranking |
| 9 → 10 | Universal Search → Automation Engine | Automation uses search for rule conditions |

### 10.5 Decision Gates

| Gate | After Phase | Decision | Criteria |
|------|------------|----------|----------|
| Gate A | Phase 1 | Proceed to Phase 2? | Knowledge Object Model is stable; CaptureEngine works with FileDropHandler; StorageManager persists and retrieves objects correctly |
| Gate B | Phase 2 | Proceed to Phase 3? | All 11 processors work correctly; processing pipeline is configurable; processing history is tracked |
| Gate C | Phase 3 | Proceed to Phase 4? | Typed graph model is stable; entity resolution works; graph query API is functional; migration preserves data |
| Gate D | Phase 4 | Proceed to Phase 5? | Universal search works across all object types; hybrid ranking produces relevant results; faceted search is functional |
| Gate E | Phase 5 | Proceed to Phase 6? | AI enrichment works on-device; embeddings are accurate; summarisation is useful; duplicate detection is reliable |
| Gate F | Phase 6 | Proceed to Phase 7? | All connectors work; automation rules evaluate correctly; integration data is stored as KnowledgeObjects |
| Gate G | Phase 7 | Proceed to Phase 8? | Encryption works on all platforms; cross-platform support is verified; mobile capture works |
| Gate H | Phase 8 | Programme Complete? | Plugins load dynamically; sandboxing is secure; plugin marketplace is functional |

### 10.6 Milestones

| Milestone | Phase | Date (Target) | Deliverable |
|-----------|-------|---------------|-------------|
| M1: Knowledge Object Model | Phase 1 | Month 1 | `KnowledgeObject` model in `nabu-core` |
| M2: Capture Engine MVP | Phase 1 | Month 2 | `CaptureEngine` with `FileDropHandler` |
| M3: Storage Manager MVP | Phase 1 | Month 3 | `StorageManager` with SQLite metadata store |
| M4: Processing Pipeline MVP | Phase 2 | Month 4 | `ProcessingPipeline` with all 11 processors |
| M5: Typed Graph MVP | Phase 3 | Month 6 | `KnowledgeGraph` with typed entities |
| M6: Universal Search MVP | Phase 4 | Month 9 | `UniversalSearchEngine` with hybrid ranking |
| M7: AI Enrichment MVP | Phase 5 | Month 12 | `AIEnrichmentPipeline` with embeddings and summarisation |
| M8: Integrations MVP | Phase 6 | Month 15 | `IntegrationHub` with email and Git connectors |
| M9: Security MVP | Phase 7 | Month 18 | Vault encryption + cross-platform support |
| M10: Plugin System MVP | Phase 8 | Month 21 | Plugin architecture with dynamic loading |
| M11: Programme Complete | Phase 8 | Month 24 | All phases complete; programme delivered |

---

## 11. Metrics

### 11.1 Before vs After Scoring

| Metric | Before | After | Target | Measurement Method |
|--------|--------|-------|--------|-------------------|
| **Capture Speed** | Manual file creation only | Universal ingestion with 15+ source types | < 100ms per file capture | Capture latency histogram |
| **Search Quality** | Full-text only; no semantic ranking | Hybrid search (full-text + vector + graph) with faceted filtering | > 0.85 relevance score (user-rated) | Search relevance A/B testing |
| **Knowledge Connectivity** | Wiki-link graph (file-level) | Typed entity graph with 12+ entity types and 10+ relation types | > 1000 entities per vault; > 5000 edges per vault | Graph node/edge count |
| **Automation** | None | Rule-based automation with 5+ trigger types and 10+ action types | > 10 automation rules per vault | Automation rule count |
| **User Effort** | Manual file management; no auto-processing | Automated capture, processing, and enrichment | < 5 clicks to capture and process an item | User task completion time |
| **Performance** | Tantivy search; no vector index | Tantivy + vector index + graph signals | p99 search latency < 100ms | Search latency histogram |
| **Memory** | ~200MB baseline | ~400MB with AI models loaded | < 500MB peak RSS | Process memory gauge |
| **Startup** | ~1s | ~2s (with AI models lazy-loaded) | < 3s cold start | Startup time measurement |
| **Reliability** | 99.5% uptime | 99.9% uptime | < 0.1% error rate | Error rate gauge |
| **Developer Productivity** | 1 codebase; 1 language (TypeScript + Rust) | 3 codebases; 3 languages (TypeScript + Rust + WASM) | < 2 weeks to add new processor | Time-to-add-new-processor |
| **Platform Maturity** | macOS only | macOS, Linux, Windows, iOS, Android | 5 platforms supported | Platform count |
| **Overall Programme Maturity** | Phase 1 (design) | Phase 8 (complete) | All 8 phases complete | Phase completion count |

### 11.2 Maturity Model

| Level | Description | Criteria |
|-------|-------------|----------|
| **Level 0** | Inception | Programme documented; no implementation |
| **Level 1** | Initial | Phase 1 complete; knowledge object model and capture engine working |
| **Level 2** | Repeatable | Phase 2 complete; processing pipeline with all processors working |
| **Level 3** | Defined | Phase 3 complete; typed knowledge graph with entity resolution |
| **Level 4** | Managed | Phase 4 complete; universal search with hybrid ranking |
| **Level 5** | Optimising | Phase 5 complete; AI enrichment pipeline operational |
| **Level 6** | Integrated | Phase 6 complete; integrations and automation working |
| **Level 7** | Secure | Phase 7 complete; encryption and cross-platform support |
| **Level 8** | Extensible | Phase 8 complete; plugin system operational |
| **Level 9** | Complete | All phases complete; programme delivered |

---

## 12. Risk Register

| ID | Risk | Probability | Impact | Severity | Mitigation | Contingency | Owner | Status | Review Cadence |
|----|------|------------|--------|----------|-----------|-------------|-------|--------|---------------|
| R001 | Knowledge Object Model too large for initial implementation | Medium | High | High | Start with minimal model; extend incrementally; use feature flags for optional fields | Strip model to core fields only; defer optional fields to Phase 2 | Architecture Authority | Monitoring | Monthly |
| R002 | AI model size exceeds device constraints | High | High | Critical | Provide model size options; quantise models; allow users to disable AI features | Graceful degradation; AI features become no-ops when models unavailable | AI Lead | Monitoring | Bi-weekly |
| R003 | Processing pipeline becomes bottleneck for large batches | Medium | High | High | Implement backpressure; use worker threads; batch processing with checkpoints | Reduce batch size; add queue depth monitoring | Engineering Lead | Monitoring | Weekly |
| R004 | Graph migration corrupts existing data | Medium | Critical | Critical | Migration is additive only; existing file-level graph preserved; migration is reversible | Rollback migration; restore from backup | Graph Lead | Monitoring | Per migration |
| R005 | Entity resolution incorrectly merges distinct entities | Medium | Medium | Medium | Confidence threshold for merging; user review queue; undo support | Disable auto-merge; require manual confirmation | Graph Lead | Monitoring | Bi-weekly |
| R006 | Hybrid search ranking produces unexpected results | Medium | Medium | Medium | A/B testing with user feedback; configurable ranking weights; explainable ranking | Fall back to TF-IDF only; disable graph boosting | Search Lead | Monitoring | Monthly |
| R007 | Plugin system introduces security vulnerabilities | Low | Critical | Critical | Sandboxed execution; permission model; plugin signing; code review | Disable plugin system; revoke plugin permissions | Security Lead | Monitoring | Monthly |
| R008 | Cross-platform support reveals platform-specific bugs | High | Medium | Medium | Comprehensive CI matrix; platform-specific test suites; early alpha releases | Delay platform release; fix bugs in priority order | Platform Lead | Monitoring | Weekly |
| R009 | On-device AI inference too slow for large documents | Medium | Medium | Medium | Streaming processing; model quantisation; GPU acceleration; user-configurable quality | Reduce model size; disable AI features for large documents | AI Lead | Monitoring | Bi-weekly |
| R010 | Vault encryption key management complexity | Medium | High | High | Use OS keychain for key storage; provide recovery key export; clear documentation | Disable encryption; use OS-level encryption only | Security Lead | Monitoring | Monthly |
| R011 | External API changes break connectors | Medium | Medium | Medium | Connector abstraction layer; version pinning; fallback mechanisms | Disable affected connector; manual workaround | Integration Lead | Monitoring | Monthly |
| R012 | Automation rules have unintended side effects | Low | High | High | Dry-run mode for rules; audit log for all automation actions; undo support | Disable automation; review and fix rules | Automation Lead | Monitoring | Bi-weekly |
| R013 | Scope creep from programme expansion | Medium | Medium | Medium | Strict phase boundaries; change control board; programme authority approval | Defer new scope to future programme | Programme Manager | Monitoring | Monthly |
| R014 | Insufficient test coverage for new subsystems | Medium | High | High | Mandatory test coverage targets; CI enforcement; code review requirements | Delay release; allocate additional testing resources | QA Lead | Monitoring | Weekly |
| R015 | Dependency on external AI model providers | Medium | High | High | All AI models are local; no external API dependency; open-source model weights | Switch to alternative local model; disable AI features | AI Lead | Monitoring | Quarterly |
| R016 | Performance degradation with large vaults (>100GB) | Medium | Medium | Medium | Incremental indexing; lazy loading; memory-mapped files; configurable cache sizes | Reduce vault size; archive old items; increase hardware resources | Performance Lead | Monitoring | Monthly |
| R017 | Mobile capture has poor UX on low-end devices | Medium | Medium | Medium | Progressive enhancement; configurable quality settings; offline-first design | Disable mobile capture on low-end devices; provide web-based alternative | Mobile Lead | Monitoring | Quarterly |
| R018 | Documentation lag behind implementation | Medium | Medium | Medium | Documentation as part of definition of done; ADRs for all major decisions | Allocate dedicated documentation sprint | Documentation Lead | Monitoring | Monthly |

---

## 13. Operations Playbook

### 13.1 Incident Response

| Incident Type | Severity | Response Time | Resolution Time | Owner |
|--------------|----------|---------------|-----------------|-------|
| Capture handler crash | Critical | < 5min | < 30min | On-call engineer |
| Processing pipeline stall | Critical | < 5min | < 1hr | On-call engineer |
| Search index corruption | Critical | < 15min | < 4hr | On-call engineer |
| AI model unavailable | Warning | < 30min | < 4hr | AI lead |
| Storage capacity critical | Warning | < 1hr | < 24hr | Platform lead |
| Vault encryption key lost | Critical | < 1hr | < 24hr | Security lead |
| Cross-platform build failure | Warning | < 4hr | < 24hr | Platform lead |
| Plugin security violation | Critical | < 15min | < 2hr | Security lead |

### 13.2 Recovery

| Scenario | Recovery Procedure | RTO | RPO |
|----------|-------------------|-----|-----|
| Corrupted search index | Rebuild index from vault files; Tantivy supports full reindex | 1hr | 0 (full rebuild) |
| Corrupted metadata DB | Restore from backup; rebuild indexes from vault files | 30min | 1hr |
| Corrupted vault encryption key | Use recovery key; re-encrypt vault | 2hr | 0 (no data loss) |
| Failed AI model update | Rollback to previous model version; restart AI pipeline | 15min | 0 |
| Failed plugin update | Disable plugin; revert to previous version; restart | 15min | 0 |
| Failed automation rule | Disable rule; review audit log; re-enable after fix | 5min | 0 |

### 13.3 Runbooks

#### 13.3.1 Rebuild Search Index

1. Stop the application
2. Navigate to vault directory
3. Delete `.nabu/index/search/`
4. Restart the application
5. Wait for reindexing to complete (monitor progress via dashboard)
6. Verify search results are correct

#### 13.3.2 Migrate Graph from File-Level to Entity-Level

1. Stop the application
2. Run migration tool: `nabu graph migrate --vault <vault-path>`
3. Verify migration log for errors
4. Restart the application
5. Verify graph visualisation shows typed entities
6. Rollback if migration fails: restore from backup

#### 13.3.3 Rotate Vault Encryption Key

1. Unlock vault with current key
2. Navigate to Settings → Security → Encryption
3. Click "Rotate Key"
4. Enter current key and new key
5. Wait for re-encryption to complete
6. Verify all files are accessible with new key
7. Export recovery key to secure location

#### 13.3.4 Disable AI Features

1. Navigate to Settings → AI
2. Toggle off all AI features
3. Restart the application
4. Verify AI features are disabled (no model loading, no inference)

### 13.4 Backups

| Backup Type | Frequency | Retention | Location |
|------------|-----------|-----------|----------|
| Vault files | Real-time (file system) | Indefinite | Vault directory |
| Metadata DB | Hourly | 30 days | `.nabu/backup/` |
| Search Index | On change | 30 days | `.nabu/backup/` |
| Configuration | On change | 90 days | `.nabu/backup/` |
| Encryption Key | On rotation | Indefinite | User-managed (export) |
| Full Vault Backup | Daily | 30 days | User-specified location |

### 13.5 Migration

| Migration | From | To | Strategy | Rollback |
|-----------|------|----|----------|----------|
| File-level → Entity-level graph | File-level graph | Typed entity graph | Additive migration; preserve old graph; build new graph alongside | Restore old graph from backup |
| Single-index → Multi-index search | Single Tantivy index | Separate indexes per object type | Create new indexes; migrate data incrementally; switch at cutover | Revert to single index |
| No encryption → Encrypted vault | Unencrypted vault | AES-256-GCM encrypted vault | Encrypt files in-place; update metadata; key stored in OS keychain | Decrypt files; remove key from keychain |

### 13.6 Feature Flags

| Flag | Default | Description | Owner |
|------|---------|-------------|-------|
| `capture.enabled` | `true` | Enable knowledge capture pipeline | Capture Lead |
| `capture.ocr.enabled` | `true` | Enable OCR processing | Capture Lead |
| `capture.deduplication.enabled` | `true` | Enable duplicate detection | Capture Lead |
| `ai.embeddings.enabled` | `true` | Enable embedding generation | AI Lead |
| `ai.summarisation.enabled` | `true` | Enable AI summarisation | AI Lead |
| `ai.entity-extraction.enabled` | `true` | Enable entity extraction | AI Lead |
| `graph.typed.enabled` | `true` | Enable typed entity graph | Graph Lead |
| `search.hybrid.enabled` | `true` | Enable hybrid search ranking | Search Lead |
| `search.facets.enabled` | `true` | Enable faceted search | Search Lead |
| `encryption.vault.enabled` | `true` | Enable vault encryption | Security Lead |
| `automation.enabled` | `true` | Enable automation engine | Automation Lead |
| `plugins.enabled` | `true` | Enable plugin system | Platform Lead |
| `mobile.capture.enabled` | `true` | Enable mobile capture | Mobile Lead |
| `integrations.enabled` | `true` | Enable external integrations | Integration Lead |

### 13.7 Rollbacks

| Rollback Scenario | Procedure | Time |
|-------------------|-----------|------|
| Failed Phase deployment | Revert to previous version; restore metadata DB from backup; restart | < 30min |
| Failed AI model update | Rollback to previous model version; restart AI pipeline | < 15min |
| Failed plugin installation | Disable plugin; revert to previous version; restart | < 15min |
| Failed graph migration | Restore old graph from backup; restart | < 30min |
| Failed encryption migration | Decrypt files; remove key from keychain; restart | < 1hr |
| Failed search index rebuild | Restore old index from backup; restart | < 15min |

### 13.8 Monitoring

| Monitoring Type | Tool | Frequency | Alert |
|----------------|------|-----------|-------|
| Application health | Custom health checks | 30s | PagerDuty |
| Search index health | Tantivy metrics | 60s | Slack |
| AI model health | Model availability check | 60s | Slack |
| Storage capacity | Disk usage check | 5min | Slack + Email |
| Memory usage | Process metrics | 30s | Slack |
| CPU usage | Process metrics | 30s | Slack |
| Event bus health | Backlog size check | 30s | Slack |
| Capture pipeline health | Throughput check | 30s | PagerDuty |
| Processing pipeline health | Throughput check | 30s | PagerDuty |

### 13.9 Maintenance

| Task | Frequency | Owner |
|------|-----------|-------|
| Search index optimisation | Weekly | Search Lead |
| Vector index optimisation | Weekly | AI Lead |
| Graph index optimisation | Weekly | Graph Lead |
| Metadata DB vacuum | Monthly | Storage Lead |
| Cache cleanup | Daily | Storage Lead |
| Log rotation | Daily | Platform Lead |
| Dependency audit | Monthly | Security Lead |
| Security patch application | As needed | Security Lead |
| Model weight updates | Quarterly | AI Lead |
| Plugin compatibility check | Quarterly | Platform Lead |

### 13.10 Upgrade Strategy

| Upgrade Type | Strategy | Downtime | Data Migration |
|-------------|----------|----------|----------------|
| Patch release | In-place update; no migration needed | None | None |
| Minor release | In-place update; schema migration if needed | < 1min | Automatic |
| Major release | In-place update; full migration |