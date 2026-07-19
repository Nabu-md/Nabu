# Nabu Storage Architecture

**Phase:** 7.4 — Verification & Documentation  
**Program:** Nabu Recovery Program  
**Date:** 2026-07-19  

---

## 1. System Overview

Nabu is an Electron-based note-taking application with a multi-vault architecture. The system is organized into four primary subsystems:

### 1.1 Storage Architecture

Storage is file-system-based. Each vault is a directory containing Markdown (`.md`) files. The application reads and writes these files directly using Node.js `fs/promises`. No database is used for note content.

**Key components:**
- **VaultService** — owns vault lifecycle (open, close, create, switch, scan)
- **StateManager** — owns in-memory vault state, AST cache, and indexes
- **VaultWatcher** — monitors the vault directory for external changes via chokidar
- **VaultRegistry** — manages multiple open vault sessions

### 1.2 Indexing Architecture

Indexing is in-memory with optional persistence. Three indexes are maintained:

- **Full-text index** — inverted index mapping words to file paths
- **Tag index** — maps tags to file paths
- **Extended index** — combines full-text, tag, alias, and property indexes for search

Indexes are rebuilt incrementally when files change and rebuilt fully on vault open.

**Key components:**
- **StateManager** — owns all in-memory indexes
- **VectorManager** — owns the Vectra vector index for semantic search
- **Watcher integration** — triggers incremental index updates on file changes

### 1.3 Search Architecture

Search operates against the extended index maintained by StateManager. Two search modes are supported:

- **Full-text search** — matches against parsed Markdown content
- **Semantic search** — uses BGE-micro embeddings via Vectra

**Key components:**
- **SearchService** — orchestrates search execution
- **StateManager** — provides the extended index and AST cache
- **VectorManager** — provides semantic search results

### 1.4 PDF Architecture

PDF handling is isolated to the main process. PDFs are rendered to base64 PNG using pdfjs-dist and the `canvas` package.

**Key components:**
- **PdfService** — owns PDF business logic (open, render, annotations)
- **pdf-viewer.ts** — low-level PDF engine functions
- **PDFAnnotationStore** — manages annotation persistence per PDF

---

## 2. Storage

### 2.1 Persistence Locations

| Data | Location | Format |
|------|----------|--------|
| Note content | `{vault}/*.md` | Markdown with YAML frontmatter |
| Settings | `{userData}/settings.json` | JSON |
| View state | `{vault}/.nabu/view-state.json` | JSON |
| Bookmarks | `{vault}/.nabu/bookmarks.json` | JSON |
| Favorites | `{vault}/.nabu/favorites.json` | JSON |
| Snapshots | `{vault}/.nabu/snapshots/` | Markdown files |
| PDF annotations | `{vault}/.nabu/pdf-annotations/` | JSON files |
| Vector index | `{vault}/.nabu/vectra/` | Vectra binary format |
| AST cache | In-memory only | MDAST (unist) |

### 2.2 Ownership

| Data | Owner | Module |
|------|-------|--------|
| Note files | VaultService | `vault-service.ts` |
| Settings | Settings module | `settings.ts` |
| View state | ViewState module | `view-state.ts` |
| Bookmarks | Bookmark module | `bookmarks.ts` |
| Favorites | Favorite module | `favorites.ts` |
| Snapshots | Snapshot module | `snapshots.ts` |
| PDF annotations | PDFAnnotationStore | `pdf-viewer.ts` |
| Vector index | VectorManager | `vector.ts` |
| AST cache | StateManager | `state.ts` |

### 2.3 Serialization Flow

```
User Edit
  → IPC (note:save)
  → VaultService / StateManager
  → fs.writeFile (Markdown)
  → clearPendingWrite (after indexing)
  → Watcher detects change (if external)
  → Incremental index update
```

### 2.4 Recovery Flow

```
Vault Open
  → VaultService.openVault()
  → StateManager.openVault()
  → scanVault() → FileEntry[]
  → buildIndexes() → full-text, tag, extended
  → VectorManager.reindexAll()
  → Watcher start
  → Renderer notification
```

---

## 3. Indexing

### 3.1 Trigger Architecture

| Event | Trigger Source | Index Update |
|-------|---------------|--------------|
| `note:create` | IPC handler | Full-text, tag, extended, vector |
| `note:save` | IPC handler | Full-text, tag, extended, vector |
| `note:rename` | IPC handler | Full-text, tag, extended, vector (rename) |
| `note:delete` | IPC handler | Full-text, tag, extended, vector (remove) |
| `note:daily` | IPC handler | Full-text, tag, extended, vector |
| `note:unique` | IPC handler | Full-text, tag, extended, vector |
| `task:toggle` | IPC handler | Extended (task status) |
| `note:toggle` | IPC handler | Extended (fold state) |
| `properties:write` | IPC handler | Extended (frontmatter) |
| Watcher `onFileChanged` | VaultWatcher | Full-text, tag, extended, vector |
| Watcher `onFileAdded` | VaultWatcher | Full-text, tag, extended, vector |
| Watcher `onFileDeleted` | VaultWatcher | Full-text, tag, extended, vector (remove) |
| `vault:open` | VaultService | Full rebuild of all indexes |
| `vault:scan` | VaultService | FileEntry list update |

### 3.2 Synchronization

The expected flow for every storage change:

```
Storage Change
  ↓
Single Index Trigger (IPC or Watcher)
  ↓
Index Update (incremental or full)
  ↓
Search Availability
```

**Determinism guarantees:**
- Every IPC write path calls `clearPendingWrite` AFTER indexing completes
- Watcher checks `hasPendingWrite` before triggering re-index
- `note:delete` uses incremental removal, not full rebuild
- `note:rename` updates both old and new paths atomically

### 3.3 Ownership

| Responsibility | Owner | Module |
|----------------|-------|--------|
| Full-text index | StateManager | `state.ts` |
| Tag index | StateManager | `state.ts` |
| Extended index | StateManager | `state.ts` |
| Vector index | VectorManager | `vector.ts` |
| Watcher integration | VaultWatcher + IPC | `watcher.ts`, `shared.ts` |
| Index build orchestration | StateManager | `state.ts` |

### 3.4 Update Lifecycle

1. **Vault Open:** `buildIndexes()` scans all files, builds all indexes, pushes to renderer
2. **File Write (app-initiated):** `setPendingWrite` → `fs.writeFile` → `updateIndexesForFile` → `clearPendingWrite`
3. **File Change (external):** Watcher detects → checks `hasPendingWrite` → `updateIndexesForFile` if not pending
4. **File Delete:** `removeFileFromIndexes` + `vectorManager.removeFile`
5. **File Rename:** `renameFileInVault` + `stateManager.renameFile` + `vectorManager.renameFile`

---

## 4. Search

### 4.1 Indexing

Search uses the extended index maintained by StateManager. The extended index combines:
- Full-text inverted index
- Tag index
- Alias index (wiki-link aliases)
- Property index (frontmatter fields)

Vector search uses the Vectra index maintained by VectorManager with BGE-micro embeddings.

### 4.2 Query Lifecycle

```
search:query IPC
  → SearchService.query()
  → Validate payload
  → Get current vault
  → search() from @shared/search-query
  → Returns ranked results
  → Send to renderer
```

### 4.3 Ranking

Full-text search uses BM25-like ranking with term frequency and document frequency. Vector search uses cosine similarity. Results are merged and deduplicated.

### 4.4 Filtering

Search filters by:
- Vault scope (current vault only)
- File type (Markdown only)
- Excluded paths (dot-prefixed, `.nabu/`)

### 4.5 Ownership

| Responsibility | Owner | Module |
|----------------|-------|--------|
| Search orchestration | SearchService | `search-service.ts` |
| Extended index | StateManager | `state.ts` |
| Vector index | VectorManager | `vector.ts` |
| Query execution | `@shared/search-query` | `search-query.ts` |

---

## 5. PDF

### 5.1 Rendering Pipeline

```
pdf:open IPC
  → PdfService.open()
  → getPDFInfo() from pdf-viewer.ts
  → pdfjs-dist getDocument()
  → Return metadata (pages, title, author, etc.)

pdf:render-page IPC
  → PdfService.renderPage()
  → renderPDFPage() from pdf-viewer.ts
  → pdfjs-dist page.render()
  → canvas package → base64 PNG
  → Return data URI
```

### 5.2 Annotation Flow

```
pdf:load-annotations IPC
  → PdfService.loadAnnotations()
  → loadPDFAnnotations() from pdf-viewer.ts
  → Read from .nabu/pdf-annotations/{hash}.json
  → Return annotations

pdf:save-annotations IPC
  → PdfService.saveAnnotations()
  → savePDFAnnotations() from pdf-viewer.ts
  → Write to .nabu/pdf-annotations/{hash}.json
```

### 5.3 Persistence

PDF annotations are stored per-PDF in `.nabu/pdf-annotations/` using a hash of the file path as the filename. Annotations are loaded on PDF open and saved on modification.

### 5.4 Ownership

| Responsibility | Owner | Module |
|----------------|-------|--------|
| PDF business logic | PdfService | `pdf-service.ts` |
| PDF rendering engine | pdf-viewer.ts | `pdf-viewer.ts` |
| Annotation persistence | PDFAnnotationStore | `pdf-viewer.ts` |

---

## 6. Metadata

### 6.1 Ownership

| Metadata | Owner | Persistence |
|----------|-------|-------------|
| AppSettings | Settings module | `userData/settings.json` |
| VaultMetadata | StateManager | In-memory |
| FileEntry | StateManager | In-memory (from scan) |
| WorkspaceState | WorkspaceService | In-memory (hydrated from AppSettings) |
| ViewState | ViewState module | `.nabu/view-state.json` |
| PDFAnnotation | PDFAnnotationStore | `.nabu/pdf-annotations/` |
| ClipboardEntry | ClipboardHistory | In-memory (session) |
| BookmarksCollection | Bookmark module | `.nabu/bookmarks.json` |
| FavoritesList | Favorite module | `.nabu/favorites.json` |
| Snapshot | Snapshot module | `.nabu/snapshots/` |
| AST (MDAST) | StateManager | In-memory |
| VectorMetadata | VectorManager | Vectra index file |

### 6.2 Synchronization

Metadata synchronization follows these rules:

1. **Single write path:** Each metadata type has exactly one module that writes it.
2. **Cleanup on delete/rename:** When a note is deleted or renamed, all associated metadata (view state, snapshots, bookmarks, favorites, indexes, vector) is cleaned up atomically in the IPC handler.
3. **Watcher reconciliation:** External changes detected by the watcher update indexes but do not modify persistent metadata directly.

### 6.3 Update Lifecycle

| Event | Metadata Updated |
|-------|-----------------|
| `note:create` | View state, vector index |
| `note:save` | View state (fold state), vector index |
| `note:rename` | View state (clear old), snapshots (rename), bookmarks (rename), favorites (rename), vector index (rename), indexes (rename) |
| `note:delete` | View state (clear), snapshots (remove), bookmarks (remove), favorites (remove), vector index (remove), indexes (remove) |
| `vault:scan` | FileEntry list, FileEntry.mtime |
| `vault:open` | VaultMetadata, WorkspaceState |

---

## 7. Maintenance Guidelines

### 7.1 Adding Storage Types

1. Define the type in `src/shared/types.ts`
2. Create a dedicated module in `src/main/services/` or `src/main/`
3. Implement read/write functions with error handling
4. Register cleanup in `note:delete` and `note:rename` IPC handlers
5. Add unit tests

### 7.2 Extending Search

1. Add new index fields to `ExtendedSearchIndex` in `src/shared/extended-indexing.ts`
2. Update `buildExtendedIndex` and `updateExtendedIndexForFile`
3. Update `search` function in `src/shared/search-query.ts`
4. Add tests for new search behavior

### 7.3 Extending Indexing

1. Add new index to `StateManager` class
2. Implement `build*Index()` and `update*IndexForFile()` methods
3. Call from `buildIndexes()` and `updateIndexesForFile()`
4. Add incremental removal in `removeFileFromIndexes()`
5. Add rename support in `renameFileInVault()`
6. Wire into watcher callbacks in `src/main/ipc/shared.ts`

### 7.4 Extending PDF Functionality

1. Add new functions to `pdf-viewer.ts`
2. Add new IPC handlers to `PdfService` in `pdf-service.ts`
3. Register handlers in `src/main/ipc/pdf.ts`
4. Add Zod schemas to `src/shared/schemas.ts`
5. Add tests

### 7.5 Metadata Ownership Rules

1. **One owner per metadata type:** Each persistent or in-memory metadata type has exactly one module responsible for reading and writing it.
2. **No direct cross-module writes:** Modules must not write to another module's metadata directly. Use the owner's public API.
3. **Cleanup on lifecycle events:** Delete and rename operations must clean up all associated metadata through the owner's API.
4. **In-memory is derived:** In-memory state should be derived from persisted state on load, not maintained independently.

---

*End of Storage Architecture Documentation*
