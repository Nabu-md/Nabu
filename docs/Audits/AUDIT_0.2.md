# Nabu v0.2 Codebase Audit

## Methodology

This audit is based on a static analysis of the Rust source tree at
`crates/nabu-core/src/` and `src-tauri/src/`. The primary data sources are:

- `src-tauri/src/lib.rs` — Tauri application bootstrap, command registration, state management
- `src-tauri/src/commands.rs` (1241 lines) — All Tauri IPC command implementations
- `src-tauri/src/history.rs` (1063 lines) — History manager, undo/redo, trash, note/file operations
- `src-tauri/src/recovery.rs` (659 lines) — Session persistence, crash recovery, version snapshots
- `src-tauri/src/settings.rs` (594 lines) — Settings store, persistence, vault management
- `src-tauri/src/capture.rs` — Capture engine, job queue, processing pipeline
- `crates/nabu-core/src/` — Core domain: models, indexer, graph, processors

**Critical note on command registration:** The `#[tauri::command]` attribute marks a function as a Tauri command, but it must also be listed in the `invoke_handler` array in `lib.rs` to be callable from the frontend. This audit distinguishes between functions that exist in `commands.rs`/`history.rs`/`recovery.rs` and those actually registered and reachable.

---

## 1. Vault Lifecycle

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Detect existing vault | `check_vault_exists(path) -> bool` | **Implemented** |
| Get current vault path | `get_current_vault(store) -> String` | **Implemented** |
| Select/create vault (dialog) | `select_vault_dialog` — uses `rfd::FileDialog` to pick existing folder; validates `.nabu/vault.yml` marker | **Implemented** |
| Create new vault | `create_vault_dialog` — calls `VaultManager::create_vault` which creates directory tree, writes `vault.yml`, sets up `.nabu/` | **Implemented** |
| Complete setup / switch vault | `complete_setup(name, path)` — writes `last_vault_path` + `recent_vaults` to settings via `SettingsStore` | **Implemented** |
| Recent vaults tracking | `RecentVaultEntry { path, name }`, `update_recent_vaults()` in settings.rs, persisted to `settings.json` | **Implemented** |
| Close vault | **No `close_vault` command exists anywhere** | **Missing** |
| Switch vault (dedicated) | **No `switch_vault` command exists** — switching is done by re-running `select_vault_dialog` + `complete_setup` | **Degraded** |
| Vault settings persistence | `AppSettings.last_vault_path`, `recent_vaults`, persisted to JSON at `app_data_dir/settings.json` | **Implemented** |

### Architecture

Vault state (current vault path, recent vaults) is owned by `SettingsStore` (settings.rs),
which persists to `app_data_dir / "settings.json"`. The `VaultManager` struct in
`nabu-core/src/vault.rs` handles physical vault creation and validation. Vault metadata is
stored in `.nabu/vault.yml` inside each vault directory.

### Key observation: No close_vault

There is no command to explicitly close or disconnect from the current vault. The app
always has a "current vault" as long as settings.json has a `last_vault_path`. To
switch, the user must pick a new vault (which overwrites `last_vault_path`). There is
no way to enter a "vault-less" state from the UI.

---

## 2. Notes

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Create note (file) | `note_create_file` in commands.rs — calls `recovery::save_note` which creates file with frontmatter, captures initial version snapshot, pushes `NoteCreated` history entry | **Implemented** |
| Create daily note | `note_daily()` in commands.rs — calls `ensure_daily_note` in recovery.rs (creates dated file with templates, links to previous day) | **Implemented** |
| Read note | `note_read(path, store)` in recovery.rs — reads file content | **Implemented** |
| Save note | `note_save(path, content, store)` in recovery.rs — saves file content, captures version snapshot, pushes `NoteSaved` history entry with before/after content | **Implemented** |
| Delete note | `note_delete(path, ctx)` in history.rs — moves to `.trash/` with timestamp, pushes `NoteDeleted` history entry | **Not registered** |
| Restore note | `note_restore(path, ctx)` in history.rs — moves from `.trash/` back, pushes `NoteRestored` history entry | **Not registered** |
| Rename note | `note_rename(from, to, ctx)` in history.rs — renames file path, pushes `NoteRenamed` history entry | **Not registered** |
| Move note | `items_move(paths, target_dir, ctx)` in history.rs — moves files between directories, pushes `ItemsMoved` history entry | **Not registered** |
| Duplicate note | `note_duplicate(path, ctx)` in history.rs — copies with new name, pushes history entry | **Not registered** |
| Permanent delete (trash) | `trash_delete(path, ctx)` in history.rs — irreversible delete from trash | **Not registered** |
| Archive note | `archive_note(id, ctx)` in commands.rs — marks as archived via custom property | **Registered** |
| Restore from archive | `archive_restore(id, ctx)` in commands.rs | **Registered** |

### Key observation: History operations are not registered as Tauri commands

`note_delete`, `note_restore`, `note_rename`, `items_move`, `note_duplicate`, and
`trash_delete` are all defined in `history.rs` with full history entry pushing, but
they are **NOT in the `invoke_handler` list** in `lib.rs`. Only the following history
commands are registered: `history_undo`, `history_redo`, `history_status`,
`history_clear`, `history_set_depth`.

The delete, rename, move operations exist as Rust functions but the frontend cannot
call them directly via Tauri IPC. This means either:
1. The frontend calls these indirectly through undo/redo (which would make restore
   painful), or
2. These features are broken/incomplete from the frontend perspective.

This is a **critical disconnect** between implementation and registration.

---

## 3. File Tree

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| List tree | `tree_list(store, ctx, vault_path)` in commands.rs — recursively walks vault directory, returns `Vec<FileNode>` with relative paths, types, children | **Registered** |
| Create folder | `folder_create(name, parent, ctx)` in history.rs — creates directory, pushes history entry | **Not registered** |
| Delete folder | **No `folder_delete` function exists anywhere** | **Missing** |
| Rename folder | `folder_rename(from, to, ctx)` in history.rs — renames directory, pushes history entry | **Not registered** |
| Move items (drag-drop) | `items_move(paths, target, ctx)` in history.rs — same function used for move, listed as "drives drag-and-drop" in comments | **Not registered** |
| Refresh tree | `tree_list` is called on demand | **Implemented** |
| Reveal in file manager | `reveal_in_file_manager(path)` and `reveal_vault_in_file_manager(vault_path)` in commands.rs | **Registered** |
| Open in file manager | `open_in_file_manager(path)` in commands.rs | **Registered** |
| **Filesystem watcher** | **No `notify` crate dependency, no watcher in ApplicationContext, no real-time event emission** | **Missing** |

### Architecture

`tree_list` walks the filesystem recursively using `WalkDir`, filters `.nabu/` and
`.trash/`, and returns a tree of `FileNode` structs. Each node has `name`, `path`,
`node_type` (File/Directory), `children`, and `size`. The tree is rebuilt entirely on
each call.

### Key observation: No filesystem watcher

There is no `notify` (or `fsw`) dependency in either Cargo.toml. The file tree is only
refreshed when `tree_list` is explicitly called from the frontend. External changes
(new files created by other apps, deletions, renames) are NOT reflected until manual
refresh. This breaks the "live" expectation users have from file-tree interfaces.

### Key observation: folder_create and folder_rename are not registered

Both exist in history.rs but are not callable from the frontend. `folder_create`
pushes a history entry, `folder_rename` does too — but neither is in the invoke_handler.

---

## 4. Editor

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Save content | `note_save(path, content, store)` in recovery.rs — saves file, captures version snapshot | **Registered** |
| Read content | `note_read(path, store)` in recovery.rs | **Registered** |
| Autosave | `auto_save_interval_secs` setting (30s default); frontend calls `note_save` on interval | **Implemented** |
| Cursor restoration | `SessionState` with `cursor_pos` and `scroll_top`; `session_save`/`session_load` in recovery.rs persist to `.nabu/session.json` | **Implemented** |
| Undo | `history_undo(ctx)` — restores from recovery.rs version snapshots | **Registered** |
| Redo | `history_redo(ctx)` | **Registered** |
| Markdown rendering/preview | Handled in frontend (Leptos component `reader.rs`); `markdown_gfm` setting in `AppSettings` | **Frontend-side** |
| Link resolution | `note_links(path)` returns wikilink targets; `link_mention`/`mention_ignore` commands for @-mentions | **Registered** |

### Key observation: Two note_save paths

There is `note_save` in recovery.rs (with versioning + history entry) and a separate
code path in commands.rs around line 1390 that calls `StorageManager::save` directly.
This second path writes Markdown + JSON sidecar but does NOT capture version snapshots
or push history entries. This appears to be a duplicate/alternate save workflow that
bypasses the recovery system.

---

## 5. Search

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Query execution | `notes_search(query, store)` in commands.rs — walks vault directory on demand, case-insensitive substring matching | **Registered** |
| Highlighting | `SearchHit` struct includes `match_start` and `match_end` byte offsets | **Implemented** |
| Sort by recency | Results sorted by file modification time (newest first) | **Implemented** |
| Navigation to result | `SearchHit` includes `path`; frontend opens directly | **Implemented** |
| **Persistent index** | `Indexer` struct (nabu-core/src/indexer.rs) with inverted index, `persist()`/`load()` to `.nabu/search_index.json` | **Exists but disconnected** |
| Faceted filtering | `notes_index` returns `NoteIndexEntry[]` with tags, type, word count, reading time | **Registered** |
| Indexer updates | `Indexer.index()` method builds inverted index; registered as service in `ApplicationContext` but `notes_search` never calls it | **Disconnected** |
| Relevance ranking | **Not implemented** — search is pure substring match, sorted by mtime only | **Missing** |

### Key observation: Indexer is built but never used by search

The `Indexer` struct in `nabu-core/src/indexer.rs` is a full inverted-index search
engine with `HashMap<String, Vec<String>>` postings lists, persistence
(to `.nabu/search_index.json`), incremental updates via `EventBus`, and a
`search(query) -> Vec<SearchResult>` method. It is registered as a service in
`ApplicationContext` and has an `index()` method that walks the vault.

But `notes_search` in commands.rs does a completely independent filesystem scan —
it does NOT query the Indexer. The Indexer is dead code from the UI's perspective.

`notes_index` (registered) also just walks the filesystem — it does not query the
Indexer either. It returns metadata about notes (tags, word count, reading time)
that could have come from the index, but instead reads each file's frontmatter.

---

## 6. Knowledge Graph

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Graph generation | `graph_data(store, ctx, depth, vault_path, include_tags, include_orphans)` in commands.rs — calls `graph_data_inner` which walks filesystem, extracts wikilinks via `extract_wikilinks`, builds `GraphData { nodes, edges }` | **Registered** |
| Node creation | `VaultGraph::add_node` in nabu-core/src/graph/mod.rs (with persistence) | **Exists but disconnected** |
| Edge creation | `VaultGraph::add_edge` | **Exists but disconnected** |
| Incremental updates | `VaultGraph::update_node`, `remove_node`, `update_edge` | **Exists but disconnected** |
| **Persistence** | `GraphStore` in VaultGraph persists to `.nabu/graph.json` with schema versioning and corruption recovery | **Exists but disconnected** |
| Graph filtering | `graph_data` parameters: `depth`, `include_tags`, `include_orphans` | **Implemented** |

### Architecture

`graph_data_inner` (commands.rs:2157) performs a full traversal of the vault, parsing
each Markdown file's wikilinks to build a graph from scratch. It returns `GraphNode`s
with `id`, `path`, `title`, `tags`, `inbound`/`outbound` edge counts and
`GraphEdge`s derived from `[[wikilink]]` references. This is entirely filesystem-based.

### Key observation: VaultGraph is bypassed entirely

`VaultGraph` is a sophisticated implementation with:
- In-memory adjacency representation
- Persistence to disk (`graph_store.rs`)
- Schema versioning and corruption recovery
- Incremental node/edge updates
- EventBus integration for live updates

It is registered in `ApplicationContext` and has full CRUD operations. But `graph_data`
(the only IPC command for the graph UI) calls `graph_data_inner` which does a pure
on-demand filesystem scan. `VaultGraph` is never queried, never updated, never
persisted from the command path. The graph UI sees a full-rebuild-every-call approach.

This is a **major architectural disconnect** — there is a complete persistent,
incremental graph subsystem that is shadowed by a throwaway on-demand scan.

---

## 7. Canvas

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Canvas list | `canvas_list(store)` — returns all canvas IDs from `extra_settings` | **Registered** |
| Canvas get | `canvas_get(id, store)` — loads canvas JSON from settings | **Registered** |
| Canvas save | `canvas_save(id, canvas_def, store)` — serializes to JSON, stores in `nabu.canvases` in settings | **Registered** |
| Canvas delete | `canvas_delete(id, store)` — removes from settings | **Registered** |
| Canvas model | `CanvasDef`, `CanvasNode`, `CanvasEdge`, `CanvasGroup`, `CanvasViewport` types | **Implemented** |
| Rendering | `canvas.rs` Leptos component renders nodes, edges (with bezier paths), groups, viewport culling | **Frontend-implemented** |
| Interaction | Drag nodes, draw edges, create groups, pan/zoom viewport | **Frontend-implemented** |
| Persistence format | JSON serialized via serde, stored as `extra_settings["nabu.canvases"]` | **Implemented** |

### Key observation: Canvas stored in settings, not dedicated storage

Canvases are persisted in `SettingsStore.extra_settings` (a generic JSON string HashMap
in settings.rs) rather than in the vault's `.nabu/` directory. This means canvases
follow the app, not the vault. Opening the same vault on a different machine would not
bring the canvases with it.

---

## 8. Inbox & Capture

### What exists

**Capture subsystem** (`src-tauri/src/capture.rs`):

| Feature | Implementation | Status |
|---------|---------------|--------|
| File drop capture | `capture_file_drop(filename, mime_type, data)` — writes temp file, enqueues job | **Registered** |
| Watch folder handler | `WatchFolderHandler` — monitors watched directories for new files | **Implemented** |
| Clipboard handler | `ClipboardHandler` — captures text/URLs/images from system clipboard | **Implemented** |
| Browser capture | `BrowserCaptureHandler` — routes YouTube, GitHub, bookmarks, articles | **Implemented** |
| Screenshot handler | `ScreenshotHandler` — captures screen images | **Implemented** |
| Article capture | `ArticleCaptureHandler` — creates text objects from article content | **Implemented** |
| Photo handler | `PhotoHandler` — captures camera/photos | **Implemented** |
| YouTube handler | `YouTubeCaptureHandler` — creates YouTubeVideo objects | **Implemented** |
| GitHub handler | `GitHubRepositoryHandler` — creates Repository objects | **Implemented** |
| PDF handler | `FileDropHandler` — handles file drops including PDFs (routes to watch folders) | **Implemented** |

**Capture engine**: All handlers route through `CaptureEngine` which dispatches to
`JobQueue` for async processing via `ProcessingPipeline`. The pipeline runs registered
processors on the captured `KnowledgeObject`.

**Inbox UI** (`inbox.rs` component + backend in commands.rs):

| Feature | Implementation | Status |
|---------|---------------|--------|
| Get inbox queue | `inbox_get_queue(ctx)` — lists objects with `inbox_status` property | **Registered** |
| Approve item | `inbox_approve(id, ctx)` — sets `inbox_status: "approved"`, pushes two history entries (undo/redo) | **Registered** |
| Reject item | `inbox_reject(id, ctx, reason)` — sets `inbox_status: "rejected"` | **Registered** |
| Retry item | `inbox_retry(id, ctx)` — resets to `pending` | **Registered** |
| Delete item | `inbox_delete(id, ctx)` — permanent delete to trash | **Registered** |
| Batch operations | `inbox_batch_approve/reject/delete/retry` | **Registered** |
| Edit metadata | `inbox_edit_metadata(id, key, value)` | **Registered** |
| Move item | `inbox_move(id, target_path)` | **Registered** |
| Quick capture | `inbox_quick_capture(title, content)` — creates KnowledgeObject with `inbox_status: pending` | **Registered** |
| Live updates | `inbox_subscribe(ctx)` — returns current queue (no WebSocket/event stream) | **Registered** |

### Key observation: Capture engine is fully wired

The entire capture → job queue → processing pipeline chain is present and registered.
`capture_file_drop` is a registered Tauri command, and file drops from the frontend
invoke it directly. The `CaptureEngine` dispatches to the appropriate handler based
on file type/URL, enqueues a job, and the `WorkerPool` processes it asynchronously.

However, `inbox_subscribe` just returns the current queue state — there is no event
stream or WebSocket pushing updates to the frontend when new items arrive. The frontend
must poll.

---

## 9. Processing Pipeline

### What exists

**Pipeline construction** (`build_standard_pipeline()` in `crates/nabu-core/src/processor/`):

| Processor | Registered | Description |
|-----------|-----------|-------------|
| ContentClassifier | Yes | Classifies content type, sets `ObjectType` |
| DuplicateDetector | Yes | Detects duplicates via content hashing |
| MetadataExtractor | Yes | Extracts metadata from file (frontmatter, EXIF, etc.) |
| MetadataEnricher | Yes | Enriches with computed metadata |
| EmbeddingGenerator | Yes | Generates embeddings (via `EmbeddingService`) |
| SemanticEnricher | Yes | Adds semantic tags, topics |
| TimelineExtractor | Yes | Extracts temporal events |
| OCRProcessor | Yes | OCR on images (native tesseract) |
| PDFTextProcessor | Yes | Extracts text from PDFs |
| PDFMetadataProcessor | Yes | Extracts PDF metadata |
| PDFAnnotationProcessor | Yes | Extracts PDF annotations |
| AISummariser | Yes | AI-generated summaries |
| WhisperProcessor | Yes | Audio transcription (Whisper) |
| AutoFiler | Yes | Automatically files objects to vault paths |

### Architecture

The pipeline is constructed by `build_standard_pipeline()` which returns a
`ProcessingPipeline` with all 14 processors. Each processor implements the
`Processor` trait with `process(&self, obj: &mut KnowledgeObject, ctx: &PipelineContext)`.
The `PipelineContext` provides access to settings, job queue, and event bus for publishing
intermediate results.

Jobs are enqueued via `JobQueue::enqueue()` and processed by a `WorkerPool` with
configurable thread count. Each job is a `ProcessingJob` that wraps a
`KnowledgeObject` and the pipeline to apply.

### Key observation: Embedding and AI processors depend on external services

`EmbeddingGenerator` requires an `EmbeddingService` which is configurable (local model
or API provider). `AISummariser` and `WhisperProcessor` require LLM/transcription
providers configured in settings. If these aren't configured, the processors may
silently fail or skip processing.

---

## 10. Knowledge Objects

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Object construction | `KnowledgeObject::new(ObjectType, ObjectContent)` — assigns ID, timestamps, initializes metadata | **Implemented** |
| Object types | `ObjectType` enum: Note, Image, Audio, Video, Pdf, Bookmark, WebPage, YouTubeVideo, Repository, Canvas, Folder | **Implemented** |
| Content storage | Markdown content + JSON sidecar in `.nabu/objects/` with UUID filenames | **Implemented** |
| Custom properties | `custom_properties: HashMap<String, CustomProperty>` — stored in JSON sidecar | **Implemented** |
| Tag system | `tags: Vec<String>` on KnowledgeObject | **Implemented** |
| Relations | `relations: Vec<Relation>` — wikilink-style connections | **Implemented** |
| Persistence | `StorageManager::save(obj)` — writes `.md` + `.meta.json` to `.nabu/objects/` | **Implemented** |
| Retrieval | `StorageManager::load(id)`, `load_by_type`, `list_objects`, `query_by_tag` | **Implemented** |
| Object type conversion | `obj.set_type(new_type)` — converts between types while preserving content | **Implemented** |
| **Indexing** | `Indexer.index(obj)` builds inverted index | **Disconnected** (see Search section) |
| **Graph integration** | `VaultGraph::add_node` for objects | **Disconnected** (see Knowledge Graph section) |

### Key observation: Three layers of KnowledgeObject handling

1. **Capture layer**: `CaptureEngine` creates `KnowledgeObject`s from dropped files,
   clipboard, browser, etc. — these get `inbox_status: pending`.
2. **Processing layer**: `ProcessingPipeline` enriches objects with metadata,
   embeddings, summaries, OCR, etc. — these mutate the object in place.
3. **Frontend layer**: `note_save` / `note_read` in recovery.rs handle direct file
   edits on Markdown files, which may or may not be tracked as `KnowledgeObject`s.

Files edited directly as notes (via the editor) are NOT automatically ingested into the
`StorageManager` as `KnowledgeObject`s. They exist as raw Markdown files in the vault.
Only captured/processed content goes through the full `KnowledgeObject` lifecycle.
This means editor-created notes and captured objects live in different storage systems.

---

## 11. Properties & Metadata

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Property types | `PropertyType` enum: Text, Number, Date, Select, MultiSelect, Url, Boolean, Relation, Formula | **Implemented** |
| Property definition | `PropertyDefinition { name, property_type, options, relation_target, formula_expression }` | **Implemented** |
| Property editor UI | `property_editor.rs` Leptos component supports all types with inline editing | **Frontend-implemented** |
| Storage | `custom_properties` field on `KnowledgeObject`, persisted in JSON sidecar via `StorageManager` | **Implemented** |
| Templates | `template_editor.rs`, `template_picker.rs`; `template_list`/`template_save`/`template_delete`/`template_duplicate`/`template_set_favourite` commands | **All registered** |
| Template storage | `SettingsStore.extra_settings["nabu.templates"]` | **Implemented** |
| Frontmatter sync | `extract_frontmatter` in commands.rs parses YAML frontmatter from Markdown files | **Implemented** |

### Key observation: Properties on KnowledgeObject vs. raw notes

Properties (`custom_properties`) are a field on `KnowledgeObject` stored in the JSON
sidecar. Raw Markdown notes edited through the editor do not have a JSON sidecar —
their properties live in YAML frontmatter, which is parsed separately by
`extract_frontmatter`. There are two parallel property systems: one for
`KnowledgeObject`s (JSON sidecar) and one for raw notes (YAML frontmatter).

---

## 12. Collections & Views

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Table view | `table_view.rs` — columns, sorting, filtering, saved views, column config | **Frontend-implemented** |
| Gallery view | `gallery_view.rs` — card layout, image previews, filtering, sorting, grouping | **Frontend-implemented** |
| Board view (Kanban) | `board_view.rs` — columns, filtering, drag-drop between columns | **Frontend-implemented** |
| Calendar view | `calendar_view.rs` — date grouping, month/week/day modes | **Frontend-implemented** |
| Timeline view | `view_switcher.rs` only offers Table, Board, Gallery, Calendar — **no Timeline option** | **Scaffolded** |
| List view | Not present in `view_switcher.rs` | **Scaffolded** |
| View switcher | `view_switcher.rs` — switches between the 4 implemented views | **Frontend-implemented** |
| Smart folders | `smart_folders_list`/`smart_folder_save`/`smart_folder_delete`/`smart_folder_evaluate` commands | **All registered** |
| Calendar notes | `calendar_notes(date)` / `daily_note_for(date)` — returns notes for a given date | **Registered** |

### Key observation: Timeline and List views are missing

The `view_switcher.rs` component has a hardcoded list of available views: `Table`,
`Board`, `Gallery`, `Calendar`. `Timeline` and `List` (which appeared in planning
documents) do not have components or switch entries. There are no `timeline_view.rs`
or `list_view.rs` files.

---

## 13. PDF System

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| PDF import | `pdf_import` function in commands.rs | **Exists but NOT registered** |
| PDF text extraction | `pdf_extraction` / `PDFTextProcessor` in pipeline | **Registered internally (pipeline)** |
| PDF merge | `pdf_merge` function in commands.rs | **Exists but NOT registered** |
| PDF split | `pdf_split` function in commands.rs | **Exists but NOT registered** |
| PDF rotate | `pdf_rotate` function in commands.rs | **Exists but NOT registered** |
| PDF annotations | `pdf_annotation_add` function in commands.rs | **Exists but NOT registered** |
| PDF metadata | `PDFMetadataProcessor` in pipeline | **Registered internally (pipeline)** |

### Key observation: PDF commands exist but are not registered

Multiple `pdf_*` functions exist in `commands.rs` (pdf_import, pdf_merge, pdf_split,
pdf_rotate, pdf_annotation_add) but **none are in the `invoke_handler` list** in
`lib.rs`. The PDF *processors* (PDFTextProcessor, PDFMetadataProcessor,
PDFAnnotationProcessor, OCRProcessor) are registered in the processing pipeline and
will run automatically when PDF files are captured, but there are no direct UI-facing
PDF command endpoints.

This is the same pattern as the history operations (note_delete, folder_create, etc.)
— functions implemented but not wired into the Tauri command registry.

---

## 14. Settings

### What exists

| Feature | Implementation | Status |
|---------|---------------|--------|
| Load settings | `get_settings()` / `settings_get(key)` — reads from `SettingsStore` | **Registered** |
| Save setting | `settings_set(key, value)` / `settings_set_all(settings)` — writes to `SettingsStore` | **Registered** |
| Export settings | `settings_export()` — serializes to JSON string | **Registered** |
| Import settings | `settings_import(json)` — deserializes from JSON | **Registered** |
| Reset settings | `settings_reset()` — restores defaults | **Registered** |
| Open settings window | `open_settings()` — opens native file dialog | **Registered** |
| Settings store | `SettingsStore` in settings.rs — persists to `app_data_dir/settings.json` | **Implemented** |
| Extra settings | `extra_settings: HashMap<String, String>` — generic key-value storage for plugin/canvas/template data | **Implemented** |

### Settings structure (`AppSettings`)

| Field | Type | Default | In settings UI? |
|-------|------|---------|-----------------|
| `last_vault_path` | Option<String> | None | No (internal) |
| `recent_vaults` | Vec<RecentVaultEntry> | [] | Yes (vault picker) |
| `font_size` | f32 | 16.0 | Yes |
| `line_height` | f32 | 1.6 | Yes |
| `reduced_motion` | bool | false | Yes |
| `high_contrast` | bool | false | Yes |
| `sidebar_width` | f64 | 260.0 | Yes |
| `inspector_width` | f64 | 320.0 | Yes |
| `tab_size` | u8 | 4 | Yes |
| `word_wrap` | bool | true | Yes |
| `spell_check` | bool | true | Yes |
| `auto_save_interval` | u64 | 30 | Yes |
| `graph_show_tags_as_badges` | bool | true | Yes |
| `markdown_gfm` | bool | true | No |
| `embed_provider` | String | "youtube" | No |
| `whisper_model` | String | "base" | No |
| `ollama_model` | String | "llama3" | No |
| `enable_ocr` | bool | true | No |
| `enable_ai_processing` | bool | true | No |

### Key observation: All settings UI fields are implemented

Every setting listed in the AGENTS.md UX Gap Matrix as requiring exposure has been
implemented in the settings panel components (AppearanceSettings, EditorSettings,
GraphSettings). The settings system is fully wired — all IPC commands are registered.

---

## 15. Recovery & Persistence

### What exists

**Session persistence** (recovery.rs):

| Feature | Implementation | Status |
|---------|---------------|--------|
| Save session | `session_save(state)` — writes `SessionState` (open files, cursor positions, scroll positions) to `.nabu/session.json` | **Implemented** |
| Load session | `session_load(store)` — reads session state, returns `SessionState` | **Registered** |
| Clear session | `session_clear(store)` — deletes session file | **Implemented** |
| Session state | `SessionState { open_files: Vec<OpenFile>, ... }`, `OpenFile { path, cursor_pos, scroll_top }` | **Implemented** |
| Crash markers | `crash_lifecycle_begin()` writes `.running` marker; `crash_lifecycle_end()` removes it; `recovery_check` detects stale `.running` | **Implemented** |
| Recovery pending | `recovery_pending` marker file for in-progress crash recovery | **Implemented** |
| Version snapshots | `snapshot_note(path, content, store)` — saves timestamped copies to `.nabu/versions/<hash>/<timestamp>.md` | **Implemented** |
| List versions | `versions_list(path)` / `versions_get(hash)` | **Registered** |
| Restore version | `versions_restore(hash, version_id)` | **Registered** |
| Version diff | `versions_diff(hash, version_a, version_b)` | **Registered** |
| Manual snapshot | `snapshot_create(path)` | **Registered** |

**History management** (history.rs):

| Feature | Implementation | Status |
|---------|---------------|--------|
| History manager | `HistoryManager` struct with undo/redo stacks, `HistoryEntry` with closures | **Implemented** |
| Push entry | `history_push(entry)` — adds to undo stack, clears redo stack | **Implemented** |
| Undo | `history_undo(ctx)` — executes undo closure, moves to redo stack | **Registered** |
| Redo | `history_redo(ctx)` — executes redo closure, moves to undo stack | **Registered** |
| Status | `history_status(ctx)` — returns stack depths | **Registered** |
| Clear | `history_clear(ctx)` | **Registered** |
| Set depth | `history_set_depth(ctx, depth)` — limits history stack size | **Registered** |
| History entry types | `NoteCreated`, `NoteRenamed`, `NoteDeleted`, `NoteRestored`, `NoteSaved`, `NoteMoved`, `FolderCreated`, `FolderRenamed`, `FolderDeleted`, `ItemsMoved`, etc. | **Implemented** |

**Trash** (history.rs):

| Feature | Implementation | Status |
|---------|---------------|--------|
| Trash list | `trash_list(ctx)` — returns all trashed items | **Registered** |
| Delete from trash | `trash_delete(path, ctx)` — irreversible delete | **Not registered** |
| Purge expired | `trash_purge_expired(ctx)` — auto-purge old trash | **Registered** |
| Empty trash | `trash_empty(ctx)` | **Registered** |

### Key observation: History entries use closures with captured paths

`HistoryEntry` stores undo and redo as `Vec<HistoryAction>` where `HistoryAction`
contains closures. Each closure captures the file paths and content at the time of
the operation. This is a robust undo system, but it means all history is in-memory
(no disk persistence of the history stack between sessions). The version snapshots
in `.nabu/versions/` provide the disk-persisted version history component.

### Key observation: `trash_delete` is not registered

`trash_delete` (irreversible permanent deletion from trash) exists in history.rs
but is not in the `invoke_handler`. It cannot be called from the frontend.

---

## Critical Findings Summary

### 1. Command Registration Gaps (HIGH SEVERITY)

The following functions exist in `commands.rs`, `history.rs`, or `recovery.rs`
but are **NOT registered** in `lib.rs`'s `invoke_handler`:

| Function | File | Purpose |
|----------|------|---------|
| `note_delete` | history.rs | Delete note (to trash) |
| `note_restore` | history.rs | Restore from trash |
| `note_rename` | history.rs | Rename note |
| `items_move` | history.rs | Move items (drag-drop) |
| `note_duplicate` | history.rs | Duplicate note |
| `trash_delete` | history.rs | Permanent delete from trash |
| `folder_create` | history.rs | Create folder |
| `folder_rename` | history.rs | Rename folder |
| `pdf_import` | commands.rs | Import PDF |
| `pdf_merge` | commands.rs | Merge PDFs |
| `pdf_split` | commands.rs | Split PDF |
| `pdf_rotate` | commands.rs | Rotate PDF |
| `pdf_annotation_add` | commands.rs | Add PDF annotation |

**Impact**: These features are implemented in Rust but unreachable from the Leptos
frontend. The frontend code may have fallbacks (calling undo/redo as a workaround for
delete/restore), but drag-and-drop file moves, folder management, and PDF operations
are likely broken from the user's perspective.

**Recommended fix**: Audit the frontend to confirm which of these are called, then
either register all of them or remove the dead code.

### 2. VaultGraph Bypass (HIGH SEVERITY)

The `VaultGraph` subsystem in `crates/nabu-core/src/graph/` is a complete persistent,
incremental knowledge graph with schema versioning and corruption recovery. It is
registered as a service in `ApplicationContext` and has full CRUD operations.

But the `graph_data` IPC command (the only graph endpoint) calls `graph_data_inner`
which does a pure on-demand filesystem scan, completely bypassing `VaultGraph`.

**Impact**: The graph feature works (full rebuild on every request) but throws away
all the sophisticated incremental update, persistence, and corruption recovery
infrastructure. Graph rebuilds on every view change are O(n) filesystem walks.

**Recommended fix**: Wire `graph_data` to query `VaultGraph` instead of re-scanning
the filesystem. Ensure `VaultGraph` is kept in sync with note operations via the
`EventBus`.

### 3. Indexer Bypass (HIGH SEVERITY)

The `Indexer` in `crates/nabu-core/src/indexer.rs` is a full inverted-index search
engine with persistence to `.nabu/search_index.json`. It is registered as a service
and has `persist()`/`load()` methods.

But `notes_search` does a filesystem walk — it never queries the `Indexer`. The
`Indexer` is also never updated from the processing pipeline despite having `EventBus`
integration.

**Impact**: Fast indexed search is available but unused. Search performance degrades
linearly with vault size. The indexer code is effectively dead code — it compiles
and runs but produces no effect on the user-visible search.

**Recommended fix**: Wire `notes_search` to query `Indexer`, and ensure
`ProcessingPipeline` calls `Indexer.index()` on processed objects via `EventBus`.

### 4. No Filesystem Watcher (MEDIUM SEVERITY)

No `notify` crate dependency exists in either `Cargo.toml`. The file tree, knowledge
graph, and search index are only refreshed on explicit command invocation. External
file changes (from other apps/editors) are not detected until manual refresh.

**Impact**: Poor UX when users edit files externally — changes don't appear until
the user manually triggers a refresh.

**Recommended fix**: Add `notify` dependency, implement a `FileWatcher` service in
`ApplicationContext` that emits events on change, and have the frontend subscribe
to these events to auto-refresh.

### 5. No `close_vault` Command (LOW SEVERITY)

There is no way to explicitly close/disconnect from a vault. The app always shows
the last-opened vault on startup (via `last_vault_path` in settings).

**Impact**: Minor UX limitation — users cannot enter a "vault-less" state.

**Recommended fix**: Add `close_vault(ctx)` command that clears `last_vault_path`
and emits a "vault closed" event to the frontend.

### 6. Canvas Persistence Location (LOW SEVERITY)

Canvases are stored in `SettingsStore.extra_settings` (app-level settings JSON,
outside the vault) rather than in the vault's `.nabu/` directory.

**Impact**: Canvases don't travel with the vault — syncing the vault to another
machine loses all canvases.

**Recommended fix**: Move canvas storage to `.nabu/canvases/` within the vault.

### 7. Duplicate note_save Paths (LOW SEVERITY)

There appear to be two code paths for saving notes:
1. `recovery::note_save` (with versioning + history entry)
2. A direct `StorageManager::save` path in commands.rs (~line 1390)

**Impact**: If the frontend calls the wrong path, version snapshots may be bypassed,
breaking undo/history for editor saves.

**Recommended fix**: Consolidate into a single save path that always goes through
recovery.rs for versioning.

---

## Compilation & Build Notes

- `nabu-ui` is a standalone workspace — compile from `crates/nabu-ui/` with `cargo check`
- Root workspace compiles `nabu-core` + `src-tauri` (not `nabu-ui`)
- Tauri dev mode: `npm run tauri dev` (or `cargo tauri dev`)
- The UI is cdylib for wasm-bindgen, loaded by the Tauri webview

---

*AUDIT_0.1 covered vault lifecycle, notes, file tree, editor, search, and settings.
This audit (0.2) expands to cover all 15 feature areas with command registration
verification, critical findings, and architectural disconnect analysis.*
