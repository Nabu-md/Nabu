# Phase 7.1 — Repository Coverage Report

**Nabu Recovery Program — Storage, Search & PDF Inventory (Prompt A)**

This report confirms that every storage, search, and PDF subsystem in the repository
has been inventoried. No production code was modified during this discovery phase.

---

## 1. Inventory Scope

| Area | Files Inspected | Status |
|------|-----------------|--------|
| Storage services | `settings.ts`, `state.ts`, `vector.ts`, `view-state.ts`, `bookmarks.ts`, `favorites.ts`, `snapshots.ts`, `clipboard-history.ts`, `workspace-service.ts` | ✅ |
| Storage IPC | `notes.ts`, `vault.ts`, `shared.ts` | ✅ |
| Search shared | `indexing.ts`, `extended-indexing.ts`, `search-query.ts`, `graph.ts` | ✅ |
| Search services | `search-service.ts`, `vector.ts` | ✅ |
| Search IPC | `search.ts` | ✅ |
| Search renderer | `fuzzy.ts`, `CommandPalette.tsx` | ✅ |
| PDF services | `pdf-service.ts`, `pdf-viewer.ts`, `pdf-importer.ts` | ✅ |
| PDF IPC | `pdf.ts` | ✅ |
| PDF renderer | `PdfViewer.tsx`, `pdfCommands.ts` | ✅ |
| Preload bridge | `preload/index.ts` | ✅ |
| Bootstrap | `index.ts` | ✅ |
| Shared types | `types.ts` | ✅ |
| OCR / Whisper | `ocr-manager.ts`, `whisper.ts` | ✅ |

---

## 2. Storage Subsystems — Coverage

| Subsystem | Documented In | File |
|-----------|---------------|------|
| App settings | Storage Map §2.1 | `settings.ts` |
| Vault `.nabu/` cache | Storage Map §2.2 | `state.ts`, `vault-service.ts` |
| Vectra vector index | Storage Map §2.3 | `vector.ts` |
| BGE-micro ONNX model | Storage Map §2.4 | `vector.ts` |
| PDF annotations | Storage Map §2.5 | `pdf-viewer.ts` |
| View state (fold) | Storage Map §2.6 | `view-state.ts` |
| Bookmarks | Storage Map §2.7 | `bookmarks.ts` |
| Favorites | Storage Map §2.8 | `favorites.ts` |
| Snapshots (recovery) | Storage Map §2.9 | `snapshots.ts` |
| Clipboard history | Storage Map §2.10 | `clipboard-history.ts` |
| Note markdown files | Storage Map §2.11 | `notes.ts` IPC |
| Templates | Storage Map §2.12 | `vault-service.ts` |
| Whisper models | Storage Map §2.13 | `whisper.ts` |
| In-memory AST store | Storage Map §3 | `state.ts` |
| In-memory indexes | Storage Map §3 | `state.ts` |
| Workspace state | Storage Map §4.6 | `workspace-service.ts` |

**Coverage: 16/16 storage mechanisms documented.**

---

## 3. Search Subsystems — Coverage

| Subsystem | Documented In | File |
|-----------|---------------|------|
| Full-text index | Search Map §2.1 | `indexing.ts` |
| Tag index | Search Map §2.2 | `indexing.ts` |
| Extended search index | Search Map §2.3 | `extended-indexing.ts` |
| Knowledge graph | Search Map §2.4 | `graph.ts` |
| Vector (semantic) index | Search Map §2.5 | `vector.ts` |
| Query parser/executor | Search Map §3.1 | `search-query.ts` |
| Semantic query | Search Map §3.2 | `search.ts` IPC |
| Fuzzy matcher | Search Map §3.3 | `fuzzy.ts` |
| Tokenization | Search Map §4 | `indexing.ts`, `extended-indexing.ts` |
| Ranking & filtering | Search Map §5 | `search-query.ts`, `vector.ts`, `fuzzy.ts` |
| Incremental indexing | Search Map §6 | `state.ts` |
| Caches | Search Map §7 | `state.ts`, `vector.ts` |

**Coverage: 12/12 search subsystems documented.**

---

## 4. PDF Subsystems — Coverage

| Subsystem | Documented In | File |
|-----------|---------------|------|
| PDF Service | PDF Map §1, §7.1 | `pdf-service.ts` |
| PDF Engine | PDF Map §7.2 | `pdf-viewer.ts` |
| PDF IPC | PDF Map §6 | `pdf.ts` |
| PDF Viewer (renderer) | PDF Map §2 | `PdfViewer.tsx` |
| PDF Commands | PDF Map §3.3 | `pdfCommands.ts` |
| Annotation persistence | PDF Map §5.1 | `pdf-viewer.ts` |
| Rendering flow | PDF Map §2.2 | `pdf-viewer.ts` |
| Annotation flow | PDF Map §3 | `PdfViewer.tsx` |
| Navigation | PDF Map §4 | `PdfViewer.tsx` |
| Caching | PDF Map §2.3, §5.2 | `PdfViewer.tsx`, `pdf-viewer.ts` |
| PDF Importer | PDF Map §7.3 | `pdf-importer.ts` |
| Wiki-link PDF refs | PDF Map §4 | `types.ts` |

**Coverage: 12/12 PDF subsystems documented.**

---

## 5. Ownership Matrix (Consolidated)

| Subsystem | Feature | Service | Renderer | IPC | Persistence |
|-----------|---------|---------|----------|-----|-------------|
| Settings | Settings | `settings.ts` | Settings UI | `settings.ts` | `settings.ts` |
| Vault cache | Vault | `vault-service.ts` | — | `vault.ts` | `state.ts` |
| Vector index | Search | `vector.ts` | ContextPane | `search.ts` | `vector.ts` |
| Full-text/tag/ext | Search | `state.ts` | CommandPalette | `search.ts` | `state.ts` (mem) |
| PDF annotations | PDF | `pdf-viewer.ts` | `PdfViewer` | `pdf.ts` | `pdf-viewer.ts` |
| View state | Notes | `view-state.ts` | `NoteView` | `notes.ts` | `view-state.ts` |
| Bookmarks | Bookmarks | `bookmarks.ts` | Bookmarks UI | `notes.ts`* | `bookmarks.ts` |
| Favorites | Favorites | `favorites.ts` | Favorites UI | `notes.ts`* | `favorites.ts` |
| Snapshots | Recovery | `snapshots.ts` | Recovery UI | `notes.ts`* | `snapshots.ts` |
| Clipboard | Widget | `clipboard-history.ts` | Widget | `widgets.ts` | `clipboard-history.ts` |
| Notes | Notes | `notes.ts` IPC | `NoteView` | `notes.ts` | `notes.ts` |
| Workspace | Workspace | `workspace-service.ts` | — | — | `workspace-service.ts` |

\* Bookmarks/favorites/snapshots IPC handlers live in `notes.ts` feature module.

---

## 6. Success Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Storage persistence paths and formats fully documented | ✅ | Storage Map §2 (13 paths), §3 (8 caches) |
| Search indexing and query behavior documented | ✅ | Search Map §2 (indexing), §3 (query flow) |
| PDF rendering and annotation paths catalogued | ✅ | PDF Map §2 (render), §3 (annotations), §5 (persistence) |
| No production code modified | ✅ | Only `docs/phase-7.1-*.md` created |

---

## 7. Files Created (Deliverables)

1. `docs/phase-7.1-storage-map.md` — Storage Map
2. `docs/phase-7.1-search-map.md` — Search Map
3. `docs/phase-7.1-pdf-map.md` — PDF Map
4. `docs/phase-7.1-coverage-report.md` — This report

---

## 8. Notes for Phase 7.2+ (Optimization)

The following observations are **inventory only** (no changes made):

- **In-memory indexes are not persisted** to disk — full rebuild on every vault open
  (`StateManager.buildIndexes`). Only the Vectra vector index is on-disk.
- **PDF rasterizations are not cached on disk** — re-rendered on every zoom/navigation.
- **Snapshots use JSON-per-snapshot** files (many small files under `.nabu/snapshots/`).
- **Settings and `.nabu/*.json` are full-file rewrites** (no incremental/patch writes).
- **AST store has no size bound** — all vault ASTs held in memory for session lifetime.
- **Vector embedding is single-file serial queue** — large vaults embed sequentially.

These are candidates for optimization in subsequent phases, not part of this inventory.

---

*End of Repository Coverage Report — Phase 7.1 Prompt A. No production code modified.*
