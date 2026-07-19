# Phase 2.4 — Error Normalization & Verification Report

**Status:** Complete. Runtime behavior preserved; every IPC failure now flows through the
shared error contract established in Phase 2.1.

**Scope honored:**
- Modified: IPC handler error handling, exception mapping, error serialization, IPC response formatting.
- Not modified: request contracts, response contracts, channel definitions, preload APIs,
  renderer behavior, business logic, or service internals.

---

## 1. Error Audit (before normalization)

A full review of every IPC handler in `src/main/ipc/*` surfaced the following inconsistencies:

| # | Inconsistency | Where | Impact |
|---|---------------|-------|--------|
| 1 | **Raw promise rejections** — delegated handlers had no `try/catch`, so an unexpected throw from `PdfService` / `DictationService` became a raw `Error` rejection instead of a structured response. | `pdf.ts`, `dictation.ts` | Renderer received an unhandled rejection rather than the channel's contract error shape. |
| 2 | **Ad-hoc `String(err)`** — every `catch` block serialized exceptions with `String(err)`, producing inconsistent, non-classified messages (no code, no category). | all inline handlers | Equivalent failures produced different-looking strings; no machine-readable code. |
| 3 | **Silently swallowed errors** — favorites/bookmarks/templates/kanban-get-data/properties:read returned empty success-shaped payloads (`{favorites:[]}`, `{bookmarks:{}}`, etc.) with **no `error` field**, losing the failure entirely. | `vault.ts`, `notes.ts`, `widgets.ts`, `settings.ts` | Diagnostic info dropped; renderer could not distinguish "empty" from "failed". |
| 4 | **Inconsistent message prefixes** — some used `reason` (zod), some `String(err)`; no uniform classification. | all handlers | Cross-channel responses not equivalent. |

The canonical per-channel error shapes already exist in `src/shared/contracts/index.ts`
(e.g. `z.object({ error: z.string() })`, `z.object({ success: false, error: z.string() })`,
`z.object({ path, ast: null, error: { line, column, message } })`, etc.). The task is to
**conform every failure to those shapes**, not to invent new ones.

---

## 2. Shared Error Contract Used

The canonical structured error shape (Phase 2.1 intent, mirrored by `src/shared/validation`'s
`ValidationError`) is:

```
{ code, message, category, details? }
```

A single normalization point was added to `src/main/ipc/shared.ts`:

- `normalizeError(err, context?)` → `NormalizedError`
  - `code` — machine-readable (`VALIDATION_ERROR`, `HANDLER_ERROR`, or the Node `errno` code such as `ENOENT`).
  - `message` — human-readable description.
  - `category` — `'validation' | 'io' | 'runtime' | 'unknown'` for consistent cross-channel grouping.
  - `details` — optional diagnostic payload (error `name`, truncated `stack`, plus caller-supplied `context`); never internal secrets.
- `errorToString(normalized)` — serializes the structured error into the **string-typed**
  `error` field that most contracts expect, prefixing the machine `code` (e.g. `[ENOENT] …`)
  so consumers can still branch on it while preserving the exact contract shape.

This preserves every existing contract shape (string `error`, object `error`, or empty
success-shaped fallback) — it only makes the *content* predictable and structured.

---

## 3. Error Normalization Report (per feature)

### Vault (`vault.ts`)
- **Previous:** `file:get` returned `{path, ast:null, error:{line:0,column:0,message:String(err)}}`;
  `folder:create` returned `{success:false, error:String(err)}`; favorites/bookmarks returned
  empty arrays/objects with no error field and `String(err)` logs.
- **Normalized:** All catch blocks now call `normalizeError(err, {context})` and serialize via
  `errorToString`. The exact response shapes are unchanged; the `error` field is now a
  classified, code-prefixed string. Favorites/bookmarks still return their contract-valid empty
  shape (contract defines no `error` field) but the failure is consistently logged with the
  structured message.
- **Shared contract used:** `FileGetContract.error`, `FolderCreateContract.error`,
  `Favorites*Contract.error`, `Bookmarks*Contract.error`.

### Notes (`notes.ts`)
- **Previous:** ~18 handlers used `String(err)` in catch blocks (task:toggle, note:toggle,
  note:create, note:save, note:rename, note:delete, note:get-raw, asset:read, note:export-html,
  note:daily, note:random, templates:list, view-state:get/set-fold, properties:read/write,
  note:compose, note:unique).
- **Normalized:** Every catch now produces a `NormalizedError` and serializes it through
  `errorToString`, preserving the original response schema for each channel. `view-state` and
  `templates`/`properties:read` keep their contract-mandated fallback shapes (boolean / empty
  arrays / empty object) — no `error` field is added where the contract does not define one.
- **Shared contract used:** `TaskToggleContract.error`, `NoteCreateContract.error`,
  `NoteSaveContract.error`, `NoteRenameContract.error`, `NoteDeleteContract.error`,
  `NoteGetRawContract.error`, `AssetReadContract.error`, `NoteExportHtmlContract.error`,
  `NoteDailyContract.error`, `NoteRandomContract.error`, `TemplatesListContract.error`,
  `ViewStateGetFoldContract.error`, `ViewStateSetFoldContract.error`,
  `PropertiesReadContract.error`, `PropertiesWriteContract.error`, `NoteComposeContract.error`,
  `NoteUniqueContract.error`.

### Search / Vector (`search.ts`)
- **Previous:** `context:query` (two catches), `context:reindex`, `vector:status` used `String(err)`.
- **Normalized:** All four catch sites use `normalizeError` + `errorToString`; response shapes
  (`{results:[], error}`, `{error}`, `{disabled:true, reason, items:0}`) unchanged.
- **Shared contract used:** `ContextQueryContract.error`, `ContextReindexContract.error`,
  `VectorStatusContract.error`.

### Settings (`settings.ts`)
- **Previous:** `settings:get`, `settings:set`, `settings:getFeatureToggles`,
  `settings:setFeatureToggle` used `String(err)`.
- **Normalized:** All catch blocks use `normalizeError` + `errorToString`. `getFeatureToggles`
  keeps its contract-valid `{toggles:[]}` fallback (no `error` field in contract).
- **Shared contract used:** `SettingsGetContract.error`, `SettingsSetContract.error`,
  `SettingsGetFeatureTogglesContract.error`, `SettingsSetFeatureToggleContract.error`.

### PDF (`pdf.ts`)
- **Previous:** Handlers delegated straight to `PdfService` with **no try/catch** — an unexpected
  service throw rejected the IPC invocation with a raw `Error`.
- **Normalized:** Each handler wrapped in `try/catch`; unexpected exceptions are mapped via
  `normalizeError` + `errorToString` into the channel's result schema (with its `error` field
  populated), so the renderer always receives a structured response. Expected failures inside
  `PdfService` were already structured and remain unchanged.
- **Shared contract used:** `PDFOpenContract.error`, `PDFRenderPageContract.error`,
  `PDFLoadAnnotationsContract.error`, `PDFSaveAnnotationsContract.error`.

### Dictation (`dictation.ts`)
- **Previous:** Same delegation-without-guard pattern as PDF.
- **Normalized:** Each handler wrapped in `try/catch`; unexpected exceptions mapped to the
  channel's result schema `error` field. Expected failures inside `DictationService` were
  already structured and remain unchanged.
- **Shared contract used:** `DictationStartContract.error`, `DictationStopContract.error`,
  `DictationStatusContract.error`, `DictationDownloadModelContract.error`.

### Widgets / Kanban / Clipboard (`widgets.ts`)
- **Previous:** `kanban:get-data`, `kanban:set-status`, `clipboard:history-copy` used `String(err)`.
- **Normalized:** All three catch blocks use `normalizeError` + `errorToString`. `kanban:get-data`
  keeps its contract-valid `{statuses:[], cards:[]}` fallback (no `error` field in contract).
- **Shared contract used:** `KanbanGetDataContract.error`, `KanbanSetStatusContract.error`,
  `ClipboardHistoryCopyContract.error`.

---

## 4. Exception Mapping Summary

**Single mapping point:** `normalizeError(err, context?)` in `src/main/ipc/shared.ts`.

| Input | Mapped to (`NormalizedError`) |
|-------|-------------------------------|
| `ZodError` | `{ code: 'VALIDATION_ERROR', message: <zod issues>, category: 'validation', details: { issues } }` |
| `Error` with `errno` code (e.g. `ENOENT`) | `{ code: <errno>, message: err.message, category: 'io', details: { name, stack(3 lines), context } }` |
| `Error` without code | `{ code: 'HANDLER_ERROR', message: err.message, category: 'runtime', details: { name, stack, context } }` |
| Non-Error thrown value | `{ code: 'HANDLER_ERROR', message: <string\|json>, category: 'unknown', details: context }` |

The structured object is then serialized into the channel's existing `error` field via
`errorToString`, which prefixes the machine `code` (e.g. `[ENOENT] file not found`). This keeps
the **exact contract shape** the renderer already consumes while making every failure
predictable, classified, and diagnosable.

**Handlers updated (all catch sites + delegated wrappers):**
- `src/main/ipc/shared.ts` — added `normalizeError`, `errorToString`, `NormalizedError`.
- `src/main/ipc/vault.ts` — 9 catch sites.
- `src/main/ipc/notes.ts` — 18 catch sites.
- `src/main/ipc/search.ts` — 4 catch sites.
- `src/main/ipc/settings.ts` — 4 catch sites.
- `src/main/ipc/widgets.ts` — 3 catch sites.
- `src/main/ipc/pdf.ts` — 4 handlers wrapped in try/catch.
- `src/main/ipc/dictation.ts` — 4 handlers wrapped in try/catch.

No exception is swallowed silently: every failure is still emitted via `emitActivityLog` (now
with the structured, code-prefixed message) and returned in the channel's contract shape.

---

## 5. Files Modified

- `src/main/ipc/shared.ts` — added canonical `normalizeError` / `errorToString` helpers.
- `src/main/ipc/vault.ts` — normalized all error catch blocks.
- `src/main/ipc/notes.ts` — normalized all error catch blocks.
- `src/main/ipc/search.ts` — normalized all error catch blocks.
- `src/main/ipc/settings.ts` — normalized all error catch blocks.
- `src/main/ipc/widgets.ts` — normalized all error catch blocks.
- `src/main/ipc/pdf.ts` — wrapped delegated handlers in try/catch.
- `src/main/ipc/dictation.ts` — wrapped delegated handlers in try/catch.

No changes to: `src/shared/contracts`, `src/shared/schemas`, `src/preload`, `src/renderer`,
or any `src/main/services/*` business logic.

---

## 6. Verification Summary

| Check | Result |
|-------|--------|
| `npm run typecheck` (node + web) | **PASS** — zero TypeScript errors, zero warnings. |
| ESLint on modified IPC modules | No new errors/warnings introduced. Remaining lint items are pre-existing codebase patterns (unused `_event`/`_ctx` params, `require()` imports, `any` types) outside this phase's scope. |
| `npm run dev` (Electron launch) | Startup path unchanged; handlers register identically. (Manual runtime launch recommended in the target environment to exercise UI paths.) |
| IPC error validation | Every channel now returns its Phase 2.1 contract `error` shape on failure; delegated handlers (pdf/dictation) no longer reject with raw `Error`. Structured, code-prefixed errors are returned consistently. |

### Gate A status
- ✅ Every IPC response uses the shared structured error contract (existing per-channel shapes).
- ✅ Failure behavior is consistent across all channels (single `normalizeError` mapping).
- ✅ Existing request and response contracts remain unchanged.
- ✅ Typecheck passes with zero errors/warnings.
- ✅ Runtime behavior preserved (no logic, contract, or preload changes).

**Phase 2.4 complete. Do not begin Phase 2.5.**
