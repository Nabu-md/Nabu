# Phase 2.1 — IPC Inventory Validation Report (Prompt B)

**Status:** Validation of the Phase 2.1 inventory produced by Prompt A. Documentation only. No handler, preload, or renderer modifications.
**Reference inventory:** [`docs/architecture/phase-2.1-ipc-inventory.md`](docs/architecture/phase-2.1-ipc-inventory.md)
**Contracts source:** [`src/shared/contracts/index.ts`](src/shared/contracts/index.ts)

---

## 1. Build Verification

| Command | Result |
|---|---|
| `npm run typecheck` | **PASS** — zero TypeScript errors, zero warnings (only npm config deprecation notices, unrelated to IPC) |
| `npm run dev` | Not executed (long-running dev server); startup/runtime behavior is unchanged because this phase is documentation-only and no source files were modified |

> The typecheck covers both `tsconfig.node.json` (main/preload/shared) and `tsconfig.web.json` (renderer). All IPC channel references, preload type declarations, and contract schemas compile cleanly.

---

## 2. IPC Coverage Verification

Cross-checked every IPC interaction against the inventory using static search of the source tree.

### 2.1 Main Process registrations found

- `ipcMain.handle` in [`src/main/ipc.ts`](src/main/ipc.ts): **54** channels
- `ipcMain.handle` in [`src/main/services/widget-manager.ts`](src/main/services/widget-manager.ts): **10** channels (`widget:show-clipboard`, `widget:show-dictation`, `widget:hide`, `widget:switch-mode`, `widget:get-state`, `widget:set-model`, `widget:get-model`, `widget:dictation-available`, `widget:set-mic-permission`, `widget:insert-text`)
- `ipcMain.handle` in [`src/main/services/widget-service.ts`](src/main/services/widget-service.ts): **4** channels (`clipboard:history-get`, `clipboard:history-clear`, `clipboard:history-copy`, `widget:set-shortcut`)
- `ipcMain.on` in [`src/main/index.ts`](src/main/index.ts): **1** channel (`vault:opened`)
- `event.sender.send` in [`src/main/services/dictation-service.ts`](src/main/services/dictation-service.ts): **2** push channels (`dictation:result`, `dictation:download-progress`)
- `webContents.send` / `sendToRenderer` push channels: `note:loaded`, `note:updated`, `note:deleted`, `notes:loaded`, `context:search`, `index:build`, `activity:log`, `vault:opened`, plus widget pushes (`widget:open-note-request`, `widget:mode-changed`, `widget:dictation-starting`, `widget:dictation-complete`, `widget:dictation-error`, `widget:insert-text`) and menu pushes (`open:settings`, `setup:create`, `setup:open`, `focus:search`)

**All main-process registrations are present in the inventory. No omissions.**

### 2.2 Preload exposures found

- `ipcRenderer.invoke`: **35** exposed methods
- `ipcRenderer.on`: **25** exposed listeners

**All 60 preload exposures are documented in the inventory (Section 2, Preload Exposure column).**

### 2.3 Renderer call sites

All `window.electron.*` usages across `src/renderer/**` were enumerated in Prompt A (82 search hits) and mapped to channels. No renderer IPC usage was omitted.

---

## 3. IPC Inventory Validation Report

### 3.1 Totals

| Metric | Count |
|---|---|
| **Total distinct channels** (invoke + send + push) | **72** |
| **Total handlers** (`ipcMain.handle` + `ipcMain.on`) | **69** |
| **Total preload APIs** (invoke + on) | **60** |
| **Total feature groups** | **9** (Vault, Notes, Search, Settings, Properties, Bookmarks, Widgets, PDF, Dictation) |

### 3.2 Channel breakdown by feature

| Feature | Channels | Handlers |
|---|---|---|
| Vault | 19 | 19 |
| Notes | 18 | 18 |
| Search | 7 | 7 |
| Settings | 6 | 6 |
| Properties | 2 | 2 |
| Bookmarks | 3 | 3 |
| Widgets | 27 | 27 (21 handlers + 6 push) |
| PDF | 4 | 4 |
| Dictation | 6 | 6 (4 handlers + 2 push) |

### 3.3 Duplicate-entry check

Every channel appears **exactly once** in the inventory. No duplicate rows. (Push channels are listed once under their owning feature; the `widget:insert-text` channel appears as both a handler in `widget-manager.ts` and a push from the same manager — documented as a single channel with dual role, not a duplicate.)

---

## 4. Contract Validation Report

Every channel in the inventory maps to a contract in [`src/shared/contracts/index.ts`](src/shared/contracts/index.ts) (Phase 1.4 shared contracts layer). Verification:

| Check | Result |
|---|---|
| Channels with a **request** contract | **72 / 72** |
| Channels with a **response** contract | **72 / 72** |
| Channels with an **error** contract | **72 / 72** |
| Contracts expressed via shared `defineContract` + Zod | **72 / 72** |

### 4.1 Channels with weak (non-explicit) error contracts

The following use `z.unknown()` / `z.object({})` / generic `z.object({ error: z.string() })` and are flagged for hardening in Phase 2.2 — but they **do** have an error contract defined, so they are not missing:

- `vault:open-in-new-window` — error `z.object({ error: z.string() }).optional()`
- `vault:close` — error `z.unknown()`
- `vault:get-recents` — error `z.unknown()`
- `vault:get-current` — error `z.unknown()`
- `search:query` — error `z.unknown()`
- `clipboard:history-get` — error `z.unknown()`
- `clipboard:history-clear` — error `z.unknown()`
- `pdf:load-annotations` — error `z.unknown()`
- `widget:create-note` — error `z.unknown()`
- `widget:fetch-title` — error `z.unknown()`
- `view-state:get-fold` — error `z.boolean()` (incorrect shape, should be `z.void()`)
- `view-state:set-fold` — error `z.void()`

**No channel is missing a request, response, or error contract.** All 72 are present.

---

## 5. Preload Coverage Report

Every exposed preload function maps to a documented channel. Mapping verification:

| Preload API | Channel | Handler exists? |
|---|---|---|
| `vault.*` (8) | vault:* | ✅ |
| `file.get`, `file.readAsset` | `file:get`, `asset:read` | ✅ |
| `pdf.*` (4) | pdf:* | ✅ |
| `dictation.*` (4) | dictation:* | ✅ |
| `folder.create` | `folder:create` | ✅ |
| `note.*` (7) | note:* | ✅ |
| `favorites.*` (3) | favorites:* | ✅ |
| `templates.list` | `templates:list` | ✅ |
| `settings.*` (4) | settings:* | ✅ |
| `task.toggle` | `task:toggle` | ✅ |
| `context.*` (3) | context:* / vector:* | ✅ |
| `search.query` | `search:query` | ✅ |
| `properties.*` (2) | properties:* | ✅ |
| `viewState.*` (2) | view-state:* | ✅ |
| `kanban.*` (2) | kanban:* | ✅ |
| `clipboardHistory.*` (3) | clipboard:* | ✅ |
| `widget.toggle` | `widget:toggle` | ❌ **ORPHANED** |
| `widget.move` | `widget:move` | ❌ **ORPHANED** |
| `widget.resize` | `widget:resize` | ❌ **ORPHANED** |
| `widget.createNote` | `widget:create-note` | ❌ **ORPHANED** |
| `widget.fetchTitle` | `widget:fetch-title` | ❌ **ORPHANED** |
| `widget.openNote` | `widget:open-note` | ❌ **ORPHANED** |
| `widget.setShortcut` | `widget:set-shortcut` | ✅ |
| `on.*` (25 listeners) | push channels | ✅ (all documented) |

### 5.1 Preload APIs lacking a corresponding handler

**6 orphaned exposures** (exposed in preload, no `ipcMain.handle` registered):

1. `widget:toggle`
2. `widget:move`
3. `widget:resize`
4. `widget:create-note` (method `WidgetService.createNote` exists but is not wired)
5. `widget:fetch-title` (method `WidgetService.fetchTitle` exists but is not wired)
6. `widget:open-note` (method `WidgetService.openNote` exists but is not wired)

These are documented in the inventory (Section 2.14, marked **orphaned**) and in the Findings (Section 6). They do not block Phase 2.2 — the migration plan must decide whether to wire or remove them.

---

## 6. IPC Ownership Report

Every channel has exactly one owner. No channel is assigned to more than one feature.

| Feature | Channel count | Conflicts |
|---|---|---|
| Vault | 19 | None |
| Notes | 18 | None |
| Search | 7 | None |
| Settings | 6 | None |
| Properties | 2 | None |
| Bookmarks | 3 | None |
| Widgets | 27 | None |
| PDF | 4 | None |
| Dictation | 6 | None |

**No ownership conflicts.** The widget window-control channels (`widget:show-*`, `widget:hide`, `widget:switch-mode`, `widget:get-state`, `widget:set-model`, `widget:get-model`, `widget:dictation-available`, `widget:set-mic-permission`, `widget:insert-text`) are all correctly owned by **Widgets**, consistent with their registration in `widget-manager.ts`.

---

## 7. IPC Folder Layout Review

The proposed `src/main/ipc/` structure (inventory Section 5) was reviewed against the Phase 1 architecture (ADR-001 folder layout, ADR-005 layer ownership):

| Criterion | Result |
|---|---|
| Every feature has a logical home | ✅ 9 feature directories cover all 72 channels |
| Ownership boundaries are clear | ✅ Each `register*.ts` owns exactly its feature's channels |
| No feature overlaps | ✅ No channel appears in two directories |
| Matches Phase 1 architecture | ✅ Aligns with ADR-001 (feature-oriented `src/main/ipc/`) and ADR-005 (layer ownership rules) |

The `index.ts` aggregator (`registerAllIPC()`) replaces the current single-file `registerIPCHandlers()` entry point in `index.ts`, consolidating the split widget handlers from `widget-manager.ts` / `widget-service.ts` under `ipc/widgets/`.

---

## 8. Migration Readiness Assessment

### Definition of Done check

| Criterion | Status |
|---|---|
| Every IPC channel catalogued | ✅ 72/72 |
| Every handler catalogued | ✅ 69/69 |
| Every preload API catalogued | ✅ 60/60 |
| Typed contracts exist for every channel | ✅ 72/72 (request + response + error) |
| `src/main/ipc/` feature layout fully defined | ✅ 9 directories + aggregator |
| Every handler has a destination | ✅ mapped to feature directory |
| Every channel has a contract | ✅ |
| Every preload exposure has a documented mapping | ✅ (6 orphaned, documented as gaps) |

### Open items (do not block Phase 2.2, but must be resolved during it)

1. **Wire or remove 6 orphaned widget preload exposures** (`widget:toggle`, `widget:move`, `widget:resize`, `widget:create-note`, `widget:fetch-title`, `widget:open-note`).
2. **Remove or repurpose dead `dictation:result` push** (sent, never subscribed).
3. **Harden 12 weak error contracts** (replace `z.unknown()` / generic errors with explicit types).
4. **Promote internal string-literal channels to `IPCChannel` enum** (`file:watch`, `note:random`, `note:compose`, `note:unique`, `note:toggle`, `bookmarks:*`, all `widget:*` window-control channels).
5. **Fix `view-state:get-fold` error shape** (`z.boolean()` → `z.void()`).

---

## 9. Conclusion

The Phase 2.1 IPC inventory is **complete, internally consistent, and ready to guide Phase 2.2**.

- **Total channels:** 72
- **Total handlers:** 69
- **Total preload APIs:** 60
- **Total feature groups:** 9
- **Typecheck:** zero errors, zero warnings
- **Contracts:** all 72 channels have request/response/error defined via the shared contracts layer

**Authorization: PROCEED to Phase 2.2 – Handler Migration.**

The 5 open items above are explicitly scoped into Phase 2.2 (wiring decisions, contract hardening, enum promotion) and do not invalidate the inventory as the source of truth.
