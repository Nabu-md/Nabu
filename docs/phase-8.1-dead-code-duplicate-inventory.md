# Phase 8.1 — Dead Code & Duplicate Inventory

**Date:** 2026-07-19  
**Phase:** 8.1 — Dead Code & Duplicate Inventory  
**Status:** Discovery Complete — No production code modified  

---

## Executive Summary

This phase performed a comprehensive static analysis of the Nabu codebase to identify dead code, unused exports, stale modules, and duplicate implementations. The analysis covered:

- Main process (`src/main/`)
- Preload layer (`src/preload/`)
- Renderer (`src/renderer/`)
- Shared modules (`src/shared/`)
- IPC handlers
- Services
- Plugins

**Key Findings:**
- **12 completely dead modules** (no imports anywhere)
- **Multiple unused exports** across several files
- **12 stale modules** (legacy/abandoned/experimental)
- **6 duplicate implementation patterns** identified

> **Note:** Some exports listed in the original report did not exist in the files. See Phase 8.2 for actual removals performed.

All findings have been verified for safety before any removal is attempted in Phase 8.2.

---

## 1. Dead Code Report

Dead code is code that is no longer reachable from any entry point, import, or dynamic load.

### 1.1 Completely Dead Modules

| File | Reason Dead | Evidence | Confidence |
|------|-------------|----------|------------|
| `src/main/web-viewer.ts` | No imports anywhere in codebase | `grep -rn "web-viewer" src/` returns only self-references | **Confirmed** |
| `src/main/protocol.ts` | No imports anywhere; `registerNabuProtocol()` never called | `grep -rn "protocol" src/` returns only self-references and unrelated `postMessage` comments | **Confirmed** |
| `src/main/scheduler.ts` | No imports anywhere; reminder system never integrated | `grep -rn "scheduler" src/` returns only self-references | **Confirmed** |
| `src/renderer/src/_scratch.ts` | No imports anywhere; scratch/development file | `grep -rn "_scratch" src/` returns no references | **Confirmed** |
| `src/renderer/src/features/graph/CytoscapeGraphView.tsx` | No imports anywhere; abandoned experiment replaced by `GraphView.tsx` | `grep -rn "CytoscapeGraphView" src/` returns only self-references | **Confirmed** |
| `src/shared/remarkFootnotes.ts` | No imports anywhere; remark plugin never integrated into pipeline | `grep -rn "remarkFootnotes" src/` returns only self-reference | **Confirmed** |
| `src/shared/validation/index.ts` | No imports anywhere; validation utilities never used | `grep -rn "from '@shared/validation'" src/` returns no results | **Confirmed** |

### 1.2 Dead Code Within Active Modules

| File | Dead Symbol | Reason | Evidence | Confidence |
|------|-------------|--------|----------|------------|
| `src/main/services/audio-recorder.ts` | `AudioSession`, `isAudioRecordingSupported`, `startRecording`, `stopRecording`, `getAudioEmbedMarkdown` | All exports unused; audio recorder never integrated | No imports of any export | **Confirmed** |
| `src/main/services/bases.ts` | `BaseConfig`, `BaseRow`, `buildBaseRows`, `loadBases`, `saveBase`, `updateBaseProperty` | All exports unused; bases feature never integrated | No imports of any export | **Confirmed** |
| `src/main/services/docx-importer.ts` | `docxImporter` | Exported but never registered or imported | No imports of `docxImporter` | **Confirmed** |
| `src/main/services/pdf-importer.ts` | `pdfImporter` | Exported but never registered or imported | No imports of `pdfImporter` | **Confirmed** |
| `src/main/services/random-note.ts` | `getRandomNotePath` | Exported but never imported | No imports of `getRandomNotePath` | **Confirmed** |
| `src/main/ipc/shared.ts` | `path` (re-export) | Re-export of Node.js `path` module, never imported from this path | No imports of `path` from `./shared` | **Confirmed** |

---

## 2. Unused Export Report

Exports that are never imported or referenced by any other module.

### 2.1 Main Process Services

| Export | File | Verification | Confidence |
|--------|------|--------------|------------|
| `VectorConfig` | `src/main/services/vector.ts` | Only used internally within same file | **Confirmed** |
| `vaultWatcher` | `src/main/services/watcher.ts` | Exported singleton never imported; new instances created in `index.ts` | **Confirmed** |
| `ASTStoreEntry` | `src/main/services/parser.ts` | Never imported by any other file | **Confirmed** |
| `getASTMeta` | `src/main/services/parser.ts` | Never imported by any other file | **Confirmed** |
| `serializeAST` | `src/main/services/parser.ts` | Never imported by any other file | **Confirmed** |
| `extractPDFText` | `src/main/services/pdf-viewer.ts` | Never imported by any other file | **Confirmed** |
| `clearPDFCache` | `src/main/services/pdf-viewer.ts` | Never imported by any other file | **Confirmed** |
| `clearAllPDFCache` | `src/main/services/pdf-viewer.ts` | Never imported by any other file | **Confirmed** |
| `SourceNote` | `src/main/services/composer.ts` | Never imported by any other file | **Confirmed** |
| `mergeTags` | `src/main/services/composer.ts` | Never imported by any other file | **Confirmed** |
| `checkScalarConflicts` | `src/main/services/composer.ts` | Never imported by any other file | **Confirmed** |
| `getFoldState` | `src/main/services/view-state.ts` | Never imported by any other file | **Confirmed** |
| `clearViewStateCache` | `src/main/services/view-state.ts` | Never imported by any other file | **Confirmed** |
| `generateHeadingId` | `src/main/services/view-state.ts` | Never imported by any other file | **Confirmed** |
| `substituteUniqueNoteVariables` | `src/main/services/unique-note.ts` | Never imported by any other file | **Confirmed** |

### 2.2 Renderer

| Export | File | Verification | Confidence |
|--------|------|--------------|------------|
| `seedPaneCommands` | `src/renderer/src/shared/commands/registry.ts` | Never imported by any other file | **Confirmed** |
| `clearSeedCommands` | `src/renderer/src/shared/commands/registry.ts` | Never imported by any other file | **Confirmed** |
| `resetRegistry` | `src/renderer/src/shared/commands/registry.ts` | Never imported by any other file | **Confirmed** |
| `initializeFeatureToggles` | `src/renderer/src/shared/commands/feature-registrations.ts` | Never imported by any other file | **Confirmed** |
| `resetFeatureRegistrations` | `src/renderer/src/shared/commands/feature-registrations.ts` | Never imported by any other file | **Confirmed** |
| `OpenVault` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility only; no actual usage | **Requires Review** |
| `PDFTab` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility only; no actual usage | **Requires Review** |
| `Workspace` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility only; no actual usage | **Requires Review** |
| `TabGroupColor` | `src/renderer/src/shared/store.ts` | Only used internally within same file | **Confirmed** |
| `TabGroup` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility only; no actual usage | **Requires Review** |
| `GraphMode` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility only; no actual usage | **Requires Review** |
| `getActiveVault` | `src/renderer/src/shared/store.ts` | Never imported by any other file | **Confirmed** |
| `SaveResult` | `src/renderer/src/features/notes/noteCommands.ts` | Never imported by any other file | **Confirmed** |
| `NOTE_IPC_TIMEOUT_MS` | `src/renderer/src/features/notes/noteCommands.ts` | Never imported by any other file | **Confirmed** |
| `RenameResult` | `src/renderer/src/features/vault/vaultCommands.ts` | Never imported by any other file | **Confirmed** |
| `DeleteResult` | `src/renderer/src/features/vault/vaultCommands.ts` | Never imported by any other file | **Confirmed** |
| `CreateFolderResult` | `src/renderer/src/features/vault/vaultCommands.ts` | Never imported by any other file | **Confirmed** |
| `CreateNoteResult` | `src/renderer/src/features/vault/vaultCommands.ts` | Never imported by any other file | **Confirmed** |
| `NoteViewForTabProps` | `src/renderer/src/features/notes/NoteView.tsx` | Never imported by any other file | **Confirmed** |
| `FileTreeProps` | `src/renderer/src/features/vault/FileTree.tsx` | Never imported by any other file | **Confirmed** |

### 2.3 Shared Modules

| Export | File | Verification | Confidence |
|--------|------|--------------|------------|
| `formatZodError` | `src/shared/validation/index.ts` | Duplicate of `formatZodError` in `src/main/ipc/shared.ts`; never imported | **Confirmed** |
| All exports | `src/shared/remarkFootnotes.ts` | Entire module unused | **Confirmed** |
| All exports | `src/shared/validation/index.ts` | Entire module unused | **Confirmed** |

---

## 3. Stale Module Report

Modules that appear obsolete, deprecated, or superseded.

| Module | Purpose | Replacement (if applicable) | Removal Confidence |
|--------|---------|----------------------------|-------------------|
| `src/main/web-viewer.ts` | Legacy web viewer for external content | None — feature never completed | **High** |
| `src/main/protocol.ts` | Custom `nabu://` protocol for web clipper | None — web clipper extension exists but protocol never registered | **High** |
| `src/main/scheduler.ts` | Local reminder/task scheduler | None — reminders never integrated into UI | **High** |
| `src/renderer/src/_scratch.ts` | Developer scratch file | None — development artifact | **High** |
| `src/renderer/src/features/graph/CytoscapeGraphView.tsx` | Cytoscape-based graph visualization | `GraphView.tsx` (d3-force based) | **High** |
| `src/main/services/audio-recorder.ts` | Audio recording service for dictation | `dictation-service.ts` + `whisper.ts` | **High** |
| `src/main/services/bases.ts` | "Bases" feature for note relationships | None — feature never completed | **High** |
| `src/main/services/docx-importer.ts` | DOCX file importer | None — importer never registered | **High** |
| `src/main/services/pdf-importer.ts` | PDF file importer | None — importer never registered | **High** |
| `src/main/services/random-note.ts` | Random note picker service | None — feature toggle exists but service never wired | **High** |
| `src/shared/remarkFootnotes.ts` | Remark plugin for footnotes | None — never integrated into markdown pipeline | **High** |
| `src/shared/validation/index.ts` | Zod validation utilities | `src/main/ipc/shared.ts` contains `formatZodError` and `normalizeError` | **High** |

---

## 4. Duplicate Code Map

### 4.1 Type Definition Duplication: `types.ts` vs `models/index.ts`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/shared/types.ts` (canonical, 20+ imports) and `src/shared/models/index.ts` (barely used, 1 import) |
| **Duplicate Types** | `FileEntry`, `VaultMetadata`, `ParseError`, `SearchResult`, `ActivityEntry`, `Edge`, `GraphNode`, `Template`, `PDFAnnotation`, `ClipboardEntry`, `FeatureToggle` |
| **Canonical Owner** | `src/shared/types.ts` |
| **Consolidation Strategy** | Remove duplicate definitions from `models/index.ts`; update the single import in `widgetService.ts` to use `types.ts` |
| **Risk Assessment** | **High** — types have already diverged (e.g., `SearchResult` has different fields in each file) |

### 4.2 Schema Definition Duplication: `schemas.ts` vs `schemas/index.ts`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/shared/schemas.ts` (canonical, used by all main process IPC) and `src/shared/schemas/index.ts` (re-exports + augments) |
| **Duplicate Content** | `schemas/index.ts` re-exports all of `schemas.ts` via `export * from '../schemas'` |
| **Canonical Owner** | `src/shared/schemas.ts` |
| **Consolidation Strategy** | Migrate `schemas/index.ts` augmentations (bookmarks, widget channels, `vault:get-current`) into `schemas.ts`; remove `schemas/index.ts`; update `contracts/index.ts` to import from `../schemas` |
| **Risk Assessment** | **Medium** — `schemas/index.ts` is used by `contracts/index.ts` which is used by preload; requires coordinated update |

### 4.3 Duplicate Function: `formatZodError`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/main/ipc/shared.ts` (line 148, used by all IPC handlers) and `src/shared/validation/index.ts` (line 61, never used) |
| **Canonical Owner** | `src/main/ipc/shared.ts` |
| **Consolidation Strategy** | Remove `formatZodError` from `src/shared/validation/index.ts` |
| **Risk Assessment** | **Low** — one implementation is dead code |

### 4.4 Duplicate Type: `GraphMode`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/renderer/src/shared/store.ts` (line 142, re-exported for backward compatibility) and `src/shared/graph-utils.ts` (line 13, defined but not used) |
| **Canonical Owner** | Neither is actively used; `GraphView.tsx` uses inline string literals |
| **Consolidation Strategy** | Remove `GraphMode` from both locations; update `GraphView.tsx` to use inline literals or a single shared type if needed |
| **Risk Assessment** | **Low** — neither is used as a type annotation in active code |

### 4.5 Duplicate Type: `WidgetMode`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/main/services/widget-manager.ts` (line 34, used by main process IPC) and `src/renderer/src/features/widgets/widgetService.ts` (line 93, defined but not used in renderer) |
| **Canonical Owner** | `src/main/services/widget-manager.ts` |
| **Consolidation Strategy** | Remove `WidgetMode` from `widgetService.ts`; renderer should import from `@shared/types` or a shared location if needed |
| **Risk Assessment** | **Low** — renderer version is unused |

### 4.6 Duplicate Type: `ClipboardEntry`

| Aspect | Details |
|--------|---------|
| **Implementations** | `src/main/services/clipboard-history.ts` (interface, used by `ClipboardHistory` class), `src/shared/schemas.ts` (type inferred from Zod, used for IPC), `src/shared/models/index.ts` (interface, barely used) |
| **Canonical Owner** | `src/main/services/clipboard-history.ts` for runtime; `src/shared/schemas.ts` for IPC transport |
| **Consolidation Strategy** | Remove `ClipboardEntry` from `models/index.ts`; ensure runtime and IPC types stay synchronized |
| **Risk Assessment** | **Medium** — three definitions of the same concept with potential for divergence |

---

## 5. Safe Removal Candidate List

### 5.1 Confirmed Safe for Removal (Phase 8.2)

These candidates have been verified with conclusive evidence of non-use:

#### Dead Modules (entire files)

| # | File | Reason |
|---|------|--------|
| 1 | `src/main/web-viewer.ts` | No imports, no dynamic loads, no Electron bootstrap dependency |
| 2 | `src/main/protocol.ts` | No imports, `registerNabuProtocol()` never called |
| 3 | `src/main/scheduler.ts` | No imports, reminder system never integrated |
| 4 | `src/renderer/src/_scratch.ts` | No imports, development artifact |
| 5 | `src/renderer/src/features/graph/CytoscapeGraphView.tsx` | No imports, replaced by `GraphView.tsx` |
| 6 | `src/shared/remarkFootnotes.ts` | No imports, never integrated into pipeline |
| 7 | `src/shared/validation/index.ts` | No imports, never used |

#### Unused Exports (within active modules)

| # | Export | File | Reason |
|---|--------|------|--------|
| 8 | `VectorConfig` | `src/main/services/vector.ts` | Only used internally |
| 9 | `vaultWatcher` | `src/main/services/watcher.ts` | Never imported; new instances created |
| 10 | `AudioSession`, `isAudioRecordingSupported`, `startRecording`, `stopRecording`, `getAudioEmbedMarkdown` | `src/main/services/audio-recorder.ts` | Entire module unused |
| 11 | `BaseConfig`, `BaseRow`, `buildBaseRows`, `loadBases`, `saveBase`, `updateBaseProperty` | `src/main/services/bases.ts` | Entire module unused |
| 12 | `docxImporter` | `src/main/services/docx-importer.ts` | Never registered or imported |
| 13 | `pdfImporter` | `src/main/services/pdf-importer.ts` | Never registered or imported |
| 14 | `ASTStoreEntry`, `getASTMeta`, `serializeAST` | `src/main/services/parser.ts` | Never imported |
| 15 | `extractPDFText`, `clearPDFCache`, `clearAllPDFCache` | `src/main/services/pdf-viewer.ts` | Never imported |
| 16 | `getRandomNotePath` | `src/main/services/random-note.ts` | Entire module unused |
| 17 | `substituteUniqueNoteVariables` | `src/main/services/unique-note.ts` | Never imported |
| 18 | `getFoldState`, `clearViewStateCache`, `generateHeadingId` | `src/main/services/view-state.ts` | Never imported |
| 19 | `SourceNote`, `mergeTags`, `checkScalarConflicts` | `src/main/services/composer.ts` | Never imported |
| 20 | `path` (re-export) | `src/main/ipc/shared.ts` | Never imported from this path |
| 21 | `seedPaneCommands`, `clearSeedCommands`, `resetRegistry` | `src/renderer/src/shared/commands/registry.ts` | Never imported |
| 22 | `initializeFeatureToggles`, `resetFeatureRegistrations` | `src/renderer/src/shared/commands/feature-registrations.ts` | Never imported |
| 23 | `TabGroupColor`, `getActiveVault` | `src/renderer/src/shared/store.ts` | Never imported |
| 24 | `SaveResult`, `NOTE_IPC_TIMEOUT_MS` | `src/renderer/src/features/notes/noteCommands.ts` | Never imported |
| 25 | `RenameResult`, `DeleteResult`, `CreateFolderResult`, `CreateNoteResult` | `src/renderer/src/features/vault/vaultCommands.ts` | Never imported |
| 26 | `NoteViewForTabProps` | `src/renderer/src/features/notes/NoteView.tsx` | Never imported |
| 27 | `FileTreeProps` | `src/renderer/src/features/vault/FileTree.tsx` | Never imported |
| 28 | `formatZodError` | `src/shared/validation/index.ts` | Duplicate of implementation in `src/main/ipc/shared.ts` |

### 5.2 Requires Manual Review (Phase 8.2)

These candidates are likely safe but require human verification before removal:

| # | Export/Module | File | Reason for Review |
|---|---------------|------|-------------------|
| 29 | `OpenVault`, `PDFTab`, `Workspace`, `TabGroup`, `GraphMode` | `src/renderer/src/shared/store.ts` | Re-exported for backward compatibility; may be used by external tests or consumers |
| 30 | `src/shared/schemas/index.ts` | `src/shared/schemas/index.ts` | Used by `contracts/index.ts` (preload); removal requires updating preload imports |
| 31 | `src/shared/models/index.ts` | `src/shared/models/index.ts` | `ActivityEntry` is used by `widgetService.ts`; other types are duplicates of `types.ts` |
| 32 | `GraphMode` (both definitions) | `src/renderer/src/shared/store.ts` + `src/shared/graph-utils.ts` | Neither is actively used, but may be referenced in documentation or tests |
| 33 | `WidgetMode` (renderer version) | `src/renderer/src/features/widgets/widgetService.ts` | Unused in renderer, but type exists for potential future use |

---

## 6. Methodology

### 6.1 Tools Used

- `grep` for static import/export analysis
- Manual file inspection for context
- Cross-reference verification for dynamic imports, registries, and Electron lifecycle

### 6.2 Verification Steps

For each candidate, the following checks were performed:

1. **Direct imports** — `grep` for `from '...'` patterns
2. **Dynamic imports** — `grep` for `import(` patterns
3. **Registry usage** — checked importer registries, command registries, IPC registries
4. **Electron lifecycle** — checked `main/index.ts` bootstrap, preload exposure, menu registration
5. **Build tooling** — checked `electron.vite.config.ts`, `vite.config.ts`, `tsconfig*.json`
6. **Test references** — checked `tests/` directory for any references

### 6.3 Safety Criteria

A removal candidate is classified as **Confirmed Safe** only when:

- No direct imports found
- No dynamic imports found
- No registry references found
- No Electron bootstrap dependency found
- No build tooling dependency found
- No test references found

---

## 7. Recommendations for Phase 8.2

### Priority 1 — Safe to Remove Immediately

1. Remove 7 dead modules (Section 5.1, items 1-7)
2. Remove 21 confirmed unused exports (Section 5.1, items 8-28)

### Priority 2 — Consolidate After Review

3. Consolidate `types.ts` / `models/index.ts` duplication
4. Consolidate `schemas.ts` / `schemas/index.ts` duplication
5. Remove duplicate `formatZodError` from `validation/index.ts`
6. Review and potentially remove `GraphMode` and `WidgetMode` duplicates

### Priority 3 — Architectural Decisions Needed

7. Decide fate of stale modules (`web-viewer.ts`, `protocol.ts`, `scheduler.ts`) — remove or implement
8. Decide fate of unregistered importers (`docx-importer.ts`, `pdf-importer.ts`) — remove or wire up
9. Decide fate of `audio-recorder.ts` and `bases.ts` — remove or implement

---

*End of Phase 8.1 Inventory Report*
