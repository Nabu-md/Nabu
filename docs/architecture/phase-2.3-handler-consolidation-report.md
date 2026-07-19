# Phase 2.3 — Handler Consolidation & Per-Feature IPC Modules

**Status:** Complete. The monolithic `src/main/ipc.ts` (1942 lines) has been split into
feature-owned modules under `src/main/ipc/`, each exporting a single `register*IPC()`
function. A new `src/main/ipc/index.ts` bootstraps all registrations. Orphaned preload
APIs and a dead push channel were removed. Runtime behavior is unchanged.

**Scope:** IPC handler organization, registration, module layout, bootstrap, orphan
cleanup. No IPC contracts, service logic, or renderer behavior were modified.

---

## 1. Handler Consolidation Report

Every migrated handler was moved verbatim (behavior unchanged) from the monolithic
`src/main/ipc.ts` into a feature module. The previous `registerIPCHandlers()` entry
point is gone; `src/main/ipc/index.ts` now calls each `register*IPC(ctx)` in turn.

| Original location | New feature module | Owner | Registration function |
|---|---|---|---|
| `ipc.ts` vault:* (open, open-in-new-window, scan, close, switch, get-recents, get-current, create) | `vault.ts` | Vault | `registerVaultIPC` |
| `ipc.ts` file:get, file:watch | `vault.ts` | Vault | `registerVaultIPC` |
| `ipc.ts` folder:create | `vault.ts` | Vault | `registerVaultIPC` |
| `ipc.ts` favorites:get/toggle/remove | `vault.ts` | Vault | `registerVaultIPC` |
| `ipc.ts` bookmarks:get/add/remove | `vault.ts` | Vault (Bookmarks) | `registerVaultIPC` |
| `ipc.ts` task:toggle, note:toggle | `notes.ts` | Notes | `registerNotesIPC` |
| `ipc.ts` note:create/save/rename/delete/get-raw/export-html/daily/random/compose/unique | `notes.ts` | Notes | `registerNotesIPC` |
| `ipc.ts` asset:read | `notes.ts` | Notes (file asset) | `registerNotesIPC` |
| `ipc.ts` templates:list | `notes.ts` | Notes | `registerNotesIPC` |
| `ipc.ts` view-state:get-fold/set-fold | `notes.ts` | Notes | `registerNotesIPC` |
| `ipc.ts` properties:read/write | `notes.ts` | Notes (Properties) | `registerNotesIPC` |
| `ipc.ts` context:query/reindex, vector:status, search:query | `search.ts` | Search | `registerSearchIPC` |
| `ipc.ts` pdf:open/render-page/load-annotations/save-annotations | `pdf.ts` | PDF | `registerPdfIPC` |
| `ipc.ts` settings:get/set, settings:getFeatureToggles/setFeatureToggle, activity:log | `settings.ts` | Settings | `registerSettingsIPC` |
| `ipc.ts` kanban:get-data/set-status | `widgets.ts` | Widgets | `registerWidgetsIPC` |
| `widget-manager.ts` registerWidgetIPCHandlers (widget:* window controls) | `widgets.ts` | Widgets | `registerWidgetsIPC` |
| `widget-service.ts` clipboard:history-* + widget:set-shortcut | `widgets.ts` | Widgets | `registerWidgetsIPC` |
| `ipc.ts` dictation:start/stop/status/download-model | `dictation.ts` | Dictation | `registerDictationIPC` |

### Shared helpers (extracted once, reused by all modules)

`src/main/ipc/shared.ts` holds the helpers previously duplicated/defined in `ipc.ts`:
`emitActivityLog`, `formatZodError`, `getSessionForVault`, `sendToRenderer`,
`buildWatcherConfig`, frontmatter helpers (`extractFrontmatter`, `replaceFrontmatterRaw`,
`injectAutoProperty`), and the legacy-singleton bridge (`setLegacyManagers`,
`onWidgetToggle`, `getWidgetToggleCallback`, `getLegacyStateManager`).

`src/main/ipc/context.ts` defines the `IPCContext` interface (managers + feature
services) threaded into every `register*IPC(ctx)` call — replacing the previous
module-level `stateManager`/`vectorManager` closure.

---

## 2. IPC Module Summary

| Module | Responsibility | Channels | Exported registration |
|---|---|---|---|
| `vault.ts` | Vault lifecycle, file AST, folder, favorites, bookmarks | `vault:open`, `vault:open-in-new-window`, `vault:scan`, `vault:close`, `vault:switch`, `vault:get-recents`, `vault:get-current`, `vault:create`, `file:get`, `file:watch`, `folder:create`, `favorites:get`, `favorites:toggle`, `favorites:remove`, `bookmarks:get`, `bookmarks:add`, `bookmarks:remove` | `registerVaultIPC` |
| `notes.ts` | Note CRUD, task toggle, templates, view-state fold, properties | `task:toggle`, `note:toggle`, `note:create`, `note:save`, `note:rename`, `note:delete`, `note:get-raw`, `note:export-html`, `note:daily`, `note:random`, `note:compose`, `note:unique`, `asset:read`, `templates:list`, `view-state:get-fold`, `view-state:set-fold`, `properties:read`, `properties:write` | `registerNotesIPC` |
| `search.ts` | Semantic search, vector status, text search | `context:query`, `context:reindex`, `vector:status`, `search:query` | `registerSearchIPC` |
| `pdf.ts` | PDF open/render/annotations | `pdf:open`, `pdf:render-page`, `pdf:load-annotations`, `pdf:save-annotations` | `registerPdfIPC` |
| `settings.ts` | Settings get/set, feature toggles, activity log | `settings:get`, `settings:set`, `settings:getFeatureToggles`, `settings:setFeatureToggle`, `activity:log` | `registerSettingsIPC` |
| `widgets.ts` | Kanban, clipboard history, widget window controls | `kanban:get-data`, `kanban:set-status`, `clipboard:history-get`, `clipboard:history-clear`, `clipboard:history-copy`, `widget:show-clipboard`, `widget:show-dictation`, `widget:hide`, `widget:switch-mode`, `widget:get-state`, `widget:set-model`, `widget:get-model`, `widget:dictation-available`, `widget:set-mic-permission`, `widget:insert-text`, `widget:set-shortcut` | `registerWidgetsIPC` |
| `dictation.ts` | Dictation start/stop/status/download | `dictation:start`, `dictation:stop`, `dictation:status`, `dictation:download-model` | `registerDictationIPC` |
| `index.ts` | Bootstrap aggregator (no handlers) | — | `registerAllIPC`, `createIPCContext` |

---

## 3. Bootstrap Summary

`src/main/ipc/index.ts` is the single bootstrap entry point. Its responsibilities are
limited to:

1. Importing the seven feature registration functions.
2. `createIPCContext(stateManager, vectorManager, watcher)` — instantiates the shared
   `VaultService`, `SearchService`, `PdfService`, `DictationService` once (mirroring the
   previous `registerIPCHandlers` body) and packages them with the core managers into an
   `IPCContext`.
3. `registerAllIPC(ctx)` — invokes, in order: `registerVaultIPC`, `registerNotesIPC`,
   `registerSearchIPC`, `registerPdfIPC`, `registerSettingsIPC`, `registerWidgetsIPC`,
   `registerDictationIPC`.

`src/main/index.ts` now calls:
```ts
setLegacyManagers(stateManager, vectorManager, watcher)
const ipcContext = createIPCContext(stateManager, vectorManager, watcher)
registerAllIPC(ipcContext)
```
The bootstrap contains **no handler implementations** — only composition and delegation.

---

## 4. Orphan Cleanup Report

### Removed handlers / registrations

| Item | Type | Why removal was safe |
|---|---|---|
| `src/main/ipc.ts` (monolith) | File | Fully superseded by `src/main/ipc/` modules. All handlers migrated verbatim. Deleted to avoid module-resolution ambiguity with `src/main/ipc/index.ts`. |
| `registerWidgetIPCHandlers()` in `widget-manager.ts` | Dead function | Superseded by `registerWidgetsIPC()`. No remaining callers after bootstrap rewrite. |
| `WidgetManager.registerIPCHandlers()` stub | Dead method | No-op stub; never called after migration. |
| `src/main/services/widget-service.ts` | Dead file | `WidgetService` was only instantiated from `index.ts` to call `registerIPCHandlers()`. That call was removed; the class (and its orphaned `createNote`/`fetchTitle`/`openNote` methods) had no other instantiations. |
| `dictation:result` push (`event.sender.send(IPCChannel.DICTATION_RESULT, …)`) | Dead push channel | Per Phase 2.1 inventory §6.2: sent from `dictation-service.ts` but had **no preload listener and no renderer subscriber**. The widget consumes `widget:dictation-complete` instead. Removed both send sites; `appEventBus.publish('DictationFinished', …)` retained. |

### Removed stale preload APIs

The following preload exposures had **no registered `ipcMain.handle`** (Phase 2.1
inventory §6.1, flagged orphaned). Removed from both `src/preload/index.ts` and
`src/preload/index.d.ts`:

- `widget.toggle` (`widget:toggle`)
- `widget.move` (`widget:move`)
- `widget.resize` (`widget:resize`)
- `widget.createNote` (`widget:create-note`)
- `widget.fetchTitle` (`widget:fetch-title`)
- `widget.openNote` (`widget:open-note`)

### Retained (documented, not deleted)

- **`note:toggle`** (`NOTE_TOGGLE`) — registered in `notes.ts` but not exposed in the
  preload bridge and not invoked by the renderer. It is a valid internal duplicate of
  `task:toggle` (same `TaskToggleSchema`/`TaskToggleResultSchema`). Per the
  conservative rule ("if ownership is uncertain, document rather than delete"), it is
  **retained** as a registered channel. No behavior change; no duplicate pathway exists
  because the renderer reaches task toggling exclusively via `task:toggle`.

---

## 5. Files Modified / Created

### Created
- `src/main/ipc/context.ts` — `IPCContext` interface.
- `src/main/ipc/shared.ts` — shared helpers (`emitActivityLog`, `formatZodError`,
  `getSessionForVault`, `sendToRenderer`, `buildWatcherConfig`, frontmatter helpers,
  legacy-singleton bridge).
- `src/main/ipc/vault.ts` — `registerVaultIPC`.
- `src/main/ipc/notes.ts` — `registerNotesIPC`.
- `src/main/ipc/search.ts` — `registerSearchIPC`.
- `src/main/ipc/pdf.ts` — `registerPdfIPC`.
- `src/main/ipc/settings.ts` — `registerSettingsIPC`.
- `src/main/ipc/widgets.ts` — `registerWidgetsIPC`.
- `src/main/ipc/dictation.ts` — `registerDictationIPC`.
- `src/main/ipc/index.ts` — bootstrap (`registerAllIPC`, `createIPCContext`).

### Modified
- `src/main/index.ts` — replaced `registerIPCHandlers(...)` + `registerWidgetIPCHandlers()`
  + `widgetService.registerIPCHandlers()` with the new bootstrap; removed now-unused
  `WidgetService` import.
- `src/main/services/vault-service.ts` — import path `../ipc` → `../ipc/shared`.
- `src/main/services/pdf-service.ts` — import path `../ipc` → `../ipc/shared`.
- `src/main/services/search-service.ts` — import path `../ipc` → `../ipc/shared`.
- `src/main/services/dictation-service.ts` — import path `../ipc` → `../ipc/shared`;
  removed dead `dictation:result` push sends.
- `src/main/services/widget-manager.ts` — removed dead `registerWidgetIPCHandlers()`
  function and `registerIPCHandlers()` stub; dropped unused `ipcMain` import.
- `src/preload/index.ts` — removed 6 orphaned `widget.*` exposures.
- `src/preload/index.d.ts` — removed 6 orphaned `widget.*` type declarations.

### Deleted
- `src/main/ipc.ts` — monolithic handler file (superseded).
- `src/main/services/widget-service.ts` — dead service (no instantiations).

---

## 6. Verification Summary

### Build status
```
npm run typecheck   → PASS (zero errors, zero warnings)
  - typecheck:node  PASS
  - typecheck:web   PASS
npm run build (electron-vite build) → PASS
  - main, preload, renderer bundles built successfully
```
Only pre-existing npm configuration deprecation notices (`electron_mirror`,
`electron_builder_binaries_mirror`) appear — unrelated to this phase.

### Startup status
`npm run dev` launches Electron; the main, preload, and renderer bundles compile and
load. (The headless sandbox cannot fully drive the Electron runtime — `require('electron').app`
is `undefined` outside the Electron runtime — an environment limitation noted in Phase
2.2, not a regression from this phase.)

### IPC validation status
- Every renderer call → one preload API → one IPC channel → one handler → one service
  owner. No duplicate pathways remain (the previous split across `ipc.ts`,
  `widget-manager.ts`, and `widget-service.ts` is consolidated into `registerWidgetsIPC`).
- Every channel has exactly one owner and one registration.
- Orphaned preload APIs (`widget:toggle/move/resize/create-note/fetch-title/open-note`)
  and the dead `dictation:result` push channel were removed.
- `src/main/ipc/index.ts` bootstraps all registrations and contains no handler logic.

### Gate A
`npm run typecheck` passes with zero errors and zero warnings → **Gate A passes.**

---

## 7. Success Criteria

| Criterion | Met |
|---|---|
| Every channel has one owner | ✅ |
| `src/main/ipc/` contains feature modules | ✅ |
| `src/main/ipc/index.ts` bootstraps all registrations | ✅ |
| Orphaned channels and stale preload APIs removed | ✅ |
| Gate A passes | ✅ |
| Runtime behavior unchanged | ✅ (handlers migrated verbatim) |

**Phase 2.3 is complete. Do not begin Phase 2.4.**
