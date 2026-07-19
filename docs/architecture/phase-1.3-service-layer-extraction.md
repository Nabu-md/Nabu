# Phase 1.3 — Service Layer Extraction (Prompt A) — Deliverables

**Status:** Complete
**Gate A:** Passed (typecheck + build green)

This document records the service boundaries established, the blocks extracted,
the thin wrappers left behind, and the verification performed during Phase 1.3.

The work is a **pure extraction**: no behavior was redesigned, no algorithms
improved, no workflows changed, no IPC/preload/renderer contracts modified, and
no new abstractions or dependency-injection introduced.

---

## 1. Service Boundary Map

### VaultService — `src/main/services/vault-service.ts`

| Item | Value |
|------|-------|
| **Responsibility** | Vault lifecycle, vault loading, vault closing, vault path resolution, vault coordination |
| **Owned workflows** | `vault:open`, `vault:scan`, `vault:close`, `vault:create`, `vault:switch`, `vault:get-current`, `vault:get-recents`, `vault:open-in-new-window`, launch restore (`restoreVault`), E2E test-vault open (`openTestVault`) |
| **Dependencies** | `StateManager`, `VectorManager`, `VaultWatcher`, `vault-registry`, `settings`, `ipc` (`sendToRenderer`, `buildWatcherConfig`, `emitActivityLog`, `formatZodError`), `electron` (`dialog`, `BrowserWindow`, `app`) |
| **Public methods** | `openVault(opts)`, `scanVault()`, `closeVault(raw)`, `createVault(raw)`, `switchVault(raw)`, `getCurrentVault()`, `getRecents()`, `openVaultInNewWindow(raw)`, `restoreVault(mainWindow)`, `openTestVault(testVaultPath)` |

### SearchService — `src/main/services/search-service.ts`

| Item | Value |
|------|-------|
| **Responsibility** | Search orchestration, indexing coordination, search execution |
| **Owned workflows** | `search:query` |
| **Dependencies** | `StateManager` (extended index + AST), `shared/search-query` (`search`), `ipc` (`emitActivityLog`, `formatZodError`) |
| **Public methods** | `query(rawPayload)` |

### PdfService — `src/main/services/pdf-service.ts`

| Item | Value |
|------|-------|
| **Responsibility** | PDF loading, PDF processing, PDF coordination (independent from UI rendering) |
| **Owned workflows** | `pdf:open`, `pdf:render-page`, `pdf:load-annotations`, `pdf:save-annotations` |
| **Dependencies** | `pdf-viewer` (`getPDFInfo`, `renderPDFPage`, `loadPDFAnnotations`, `savePDFAnnotations`), `shared/schemas`, `ipc` (`emitActivityLog`, `formatZodError`) |
| **Public methods** | `open(raw)`, `renderPage(raw)`, `loadAnnotations(raw)`, `saveAnnotations(raw)` |

### WidgetService — `src/main/services/widget-service.ts`

| Item | Value |
|------|-------|
| **Responsibility** | Widget lifecycle, widget registration, widget coordination (does not redesign widgets) |
| **Owned workflows** | Clipboard-history start + IPC, `widget:set-shortcut`, `widget:create-note`, `widget:fetch-title`, `widget:open-note`, fn-monitor wiring delegation |
| **Dependencies** | `widget-manager` (`widgetManager`, `registerWidgetIPCHandlers`, `wireFnMonitorToWidget`), `clipboard-history`, `vault-registry`, `settings`, `electron` (`ipcMain`, `BrowserWindow`) |
| **Public methods** | `registerIPCHandlers()`, `wireFnMonitor()`, `createNote({name,content,timestamp})`, `fetchTitle({url})`, `openNote(mainWindow, {path})` |

### DictationService — `src/main/services/dictation-service.ts`

| Item | Value |
|------|-------|
| **Responsibility** | Speech workflow, transcription coordination, dictation orchestration (preserves existing behavior) |
| **Owned workflows** | `dictation:start`, `dictation:stop`, `dictation:status`, `dictation:download-model` |
| **Dependencies** | `whisper` (dynamic import: `isWhisperBinaryAvailable`, `isModelInstalled`, `downloadModel`, `startDictation`, `stopDictation`, `getModelStatus`), `shared/schemas`, `shared/channels`, `ipc` (`emitActivityLog`, `formatZodError`) |
| **Public methods** | `start(event, raw)`, `stop(raw)`, `status(raw)`, `downloadModel(event, raw)` |

---

## 2. Extraction Summary

| # | Original file | Destination service | Reason |
|---|---------------|---------------------|--------|
| 1 | `src/main/ipc.ts` — `vault:open` handler body | `VaultService.openVault` | Vault open/lifecycle logic belonged in VaultService |
| 2 | `src/main/ipc.ts` — `vault:scan` handler body | `VaultService.scanVault` | Vault re-scan logic belonged in VaultService |
| 3 | `src/main/ipc.ts` — `vault:close` handler body | `VaultService.closeVault` | Vault close logic belonged in VaultService |
| 4 | `src/main/ipc.ts` — `vault:create` handler body | `VaultService.createVault` | Vault creation logic belonged in VaultService |
| 5 | `src/main/ipc.ts` — `vault:switch` handler body | `VaultService.switchVault` | Vault switch logic belonged in VaultService |
| 6 | `src/main/ipc.ts` — `vault:get-current` handler body | `VaultService.getCurrentVault` | Current-vault read belonged in VaultService |
| 7 | `src/main/ipc.ts` — `vault:get-recents` handler body | `VaultService.getRecents` | Recents read belonged in VaultService |
| 8 | `src/main/ipc.ts` — `vault:open-in-new-window` handler body | `VaultService.openVaultInNewWindow` | Multi-window vault open belonged in VaultService |
| 9 | `src/main/ipc.ts` — `copyDefaultTemplates` helper | `VaultService.copyDefaultTemplates` | Template-copy is vault-open coordination |
| 10 | `src/main/index.ts` — `restoreVault` function | `VaultService.restoreVault` | Launch restore is vault lifecycle |
| 11 | `src/main/index.ts` — NABU_TEST_VAULT open block | `VaultService.openTestVault` | E2E vault open is vault lifecycle |
| 12 | `src/main/ipc.ts` — `search:query` handler body | `SearchService.query` | Search execution belonged in SearchService |
| 13 | `src/main/ipc.ts` — `pdf:open` handler body | `PdfService.open` | PDF load belonged in PdfService |
| 14 | `src/main/ipc.ts` — `pdf:render-page` handler body | `PdfService.renderPage` | PDF render belonged in PdfService |
| 15 | `src/main/ipc.ts` — `pdf:load-annotations` handler body | `PdfService.loadAnnotations` | PDF annotation load belonged in PdfService |
| 16 | `src/main/ipc.ts` — `pdf:save-annotations` handler body | `PdfService.saveAnnotations` | PDF annotation save belonged in PdfService |
| 17 | `src/main/index.ts` — inline clipboard-history IPC + start | `WidgetService.registerIPCHandlers` | Widget clipboard coordination belonged in WidgetService |
| 18 | `src/main/index.ts` — `widget:set-shortcut` handler | `WidgetService.registerIPCHandlers` | Widget shortcut coordination belonged in WidgetService |
| 19 | `src/main/index.ts` — `widget:create-note` handler | `WidgetService.createNote` | Widget quick-note creation belonged in WidgetService |
| 20 | `src/main/index.ts` — `widget:fetch-title` handler | `WidgetService.fetchTitle` | Widget URL-title fetch belonged in WidgetService |
| 21 | `src/main/index.ts` — `widget:open-note` handler | `WidgetService.openNote` | Widget open-note coordination belonged in WidgetService |
| 22 | `src/main/ipc.ts` — `dictation:start` handler body | `DictationService.start` | Dictation start workflow belonged in DictationService |
| 23 | `src/main/ipc.ts` — `dictation:stop` handler body | `DictationService.stop` | Dictation stop workflow belonged in DictationService |
| 24 | `src/main/ipc.ts` — `dictation:status` handler body | `DictationService.status` | Dictation status belonged in DictationService |
| 25 | `src/main/ipc.ts` — `dictation:download-model` handler body | `DictationService.downloadModel` | Dictation model download belonged in DictationService |

---

## 3. Thin Wrapper Summary

The following files were converted into thin wrappers that **initialize,
register, and delegate** (bootstrap) or **display state / collect input / call
services** (renderer). No business rules remain in them.

### `src/main/ipc.ts` (registerIPCHandlers)
Each affected handler is now a one-line delegation:

- `vault:get-current` → `vaultService.getCurrentVault()`
- `vault:open` → `vaultService.openVault(raw)` (returns `{error|canceled|vault}`)
- `vault:scan` → `vaultService.scanVault()`
- `vault:close` → `vaultService.closeVault(raw)`
- `vault:create` → `vaultService.createVault(raw)`
- `vault:switch` → `vaultService.switchVault(raw)`
- `vault:get-recents` → `vaultService.getRecents()`
- `vault:open-in-new-window` → `vaultService.openVaultInNewWindow(raw)`
- `search:query` → `searchService.query(raw)`
- `pdf:open` → `pdfService.open(raw)`
- `pdf:render-page` → `pdfService.renderPage(raw)`
- `pdf:load-annotations` → `pdfService.loadAnnotations(raw)`
- `pdf:save-annotations` → `pdfService.saveAnnotations(raw)`
- `dictation:start` → `dictationService.start(_event, raw)`
- `dictation:stop` → `dictationService.stop(raw)`
- `dictation:status` → `dictationService.status(raw)`
- `dictation:download-model` → `dictationService.downloadModel(_event, raw)`

The service instances (`vaultService`, `searchService`, `pdfService`,
`dictationService`) are constructed once at the top of `registerIPCHandlers`
and reused by all handlers.

### `src/main/index.ts` (bootstrap)
- `restoreVault(stateManager, vectorManager, watcher, mainWindow)` →
  `restoreVault(vaultService, mainWindow)` (delegates to `VaultService`).
- The inline clipboard/widget IPC handlers and `widget:create-note` /
  `widget:fetch-title` / `widget:open-note` blocks were removed and replaced
  by `new WidgetService().registerIPCHandlers()`.
- The NABU_TEST_VAULT open block now calls `vaultService.openTestVault(...)`.
- `app.whenReady` still performs bootstrap duties: instantiate managers,
  register IPC, register widget IPC, start fn-monitor, create window, register
  menu, wire feature-toggle → `widgetManager`.

### Renderer (already thin — verified, not modified)
- `src/renderer/src/features/widgets/DictationWidget.tsx` — only displays state
  and calls `window.electron.dictation.start/stop`.
- `src/renderer/src/features/pdf/PdfViewer.tsx` — UI only (page refs, scroll);
  delegates all PDF work via IPC.
- `src/renderer/src/features/search/SearchPanel.tsx` — UI only; delegates via
  IPC.

---

## 4. Files Modified

| File | Change |
|------|--------|
| `src/main/services/vault-service.ts` | **Created** — VaultService (vault lifecycle/coordination) |
| `src/main/services/search-service.ts` | **Created** — SearchService (search orchestration) |
| `src/main/services/pdf-service.ts` | **Created** — PdfService (PDF load/process/coordinate) |
| `src/main/services/widget-service.ts` | **Created** — WidgetService (widget lifecycle/registration) |
| `src/main/services/dictation-service.ts` | **Created** — DictationService (speech/transcription orchestration) |
| `src/main/ipc.ts` | Replaced 17 handler bodies with delegations; exported `emitActivityLog` + `formatZodError`; removed moved helpers/imports |
| `src/main/index.ts` | `restoreVault` + NABU_TEST_VAULT + widget IPC blocks now delegate to services; cleaned imports |

---

## 5. Verification Summary

| Check | Result |
|-------|--------|
| `npm run typecheck` (node + web) | **PASS** — no TS errors |
| `npm run build` (typecheck + electron-vite build) | **PASS** — built successfully |
| Startup unchanged | Bootstrap still instantiates managers, registers IPC, registers widget IPC, starts fn-monitor (macOS), creates window, registers menu, wires feature-toggle. Only the *implementation* of vault/widget/dictation/search/pdf logic moved into services; the *sequence* is identical. |
| Renderer unchanged | Renderer components were already thin and were not modified; they continue to call the same IPC channels with the same payloads. |
| Features behave identically | Each service preserves the exact prior handler logic (validation → business logic → schema-parse response / error contract). No control-flow, algorithm, or message-shape changes. |

### Runtime note
`npm run dev` launches the full Electron app (requires a display). In this
headless environment the build + typecheck gates were used as the authoritative
verification that the extraction compiles and preserves contracts. The runtime
behavior is unchanged because every service method is a 1:1 relocation of the
prior handler body.

---

## 6. Success Criteria — Confirmation

- [x] Service boundaries exist for all major features (Vault, Search, PDF, Widget, Dictation).
- [x] Service files exist in `src/main/services/`.
- [x] Bootstrap and UI files no longer contain business logic (delegation only).
- [x] Gate A passes (typecheck + build green).
- [x] Runtime behavior is unchanged (identical handler logic relocated).

Phase 1.3 is complete. Do not begin Phase 1.4.
