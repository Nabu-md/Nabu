# Phase 1.4 — Shared Contracts & Typed IPC Framework

**Status:** Complete
**Gate A:** Passed (typecheck: 0 errors / 0 warnings on new files; build: success)

## Objective

Establish the project's canonical contracts layer and a strongly typed IPC
registry that will become the single source of truth for all inter-process
communication. This is an infrastructure phase: the framework was created, not
migrated. No IPC behavior, handlers, preload, renderer, or services were
modified.

---

## Shared Contracts Summary

The shared layer lives under `src/shared/` and is organized into clearly
separated responsibilities:

### `src/shared/models/` — Shared Domain Models
Centralized, reusable application types with **no runtime behavior**, **no
Electron imports**, and **no React imports**. Contains:
- Identifiers: `VaultId`, `FilePath`
- Core vault/file: `FileEntry`, `VaultMetadata`, `RecentVault`
- AST/parse: `ParseError`, `FileAST`
- Search/context: `SearchResult`, `SearchMatch`, `SearchResultItem`
- Activity/logging: `ActivityEntry`, `ActivityLevel`, `ActivityLogPayload`
- Graph: `Edge`, `GraphNode`
- Template: `Template`
- Feature toggle: `FeatureToggle`
- Clipboard: `ClipboardEntry`
- PDF: `PDFAnnotation`, `PDFMetadata`
- Dictation/whisper: `WhisperSegment`, `WhisperResult`, `DictationModelStatus`
- Kanban: `KanbanCard`
- Index: `ExtendedIndexPayload`, `IndexBuildPayload`

### `src/shared/schemas/` — Validation Schemas
Electron/React-independent Zod schemas for shared domain models and IPC
payloads. Re-exports the existing `../schemas` (the de-facto schema source) to
avoid duplicate definitions, and augments it with the few channel payloads that
were previously validated ad-hoc:
- `VaultGetCurrentSchema` (vault:get-current)
- `BookmarksGetSchema` / `BookmarksGetResultSchema`
- `BookmarksAddSchema` / `BookmarksAddResultSchema`
- `BookmarksRemoveSchema` / `BookmarksRemoveResultSchema`
- Widget channel schemas: `WidgetToggleSchemaCanonical`, `WidgetMoveSchema`,
  `WidgetResizeSchema`, `WidgetCreateNoteSchema`, `WidgetFetchTitleSchema`,
  `WidgetSetShortcutSchema`

Schemas validate runtime inputs only and contain no application behavior.

### `src/shared/validation/` — Validation Utilities
Deterministic, side-effect-free helpers:
- `ValidationError` / `ValidationResult<T>` — structured error contracts
- `zodErrorToValidationErrors(error)` — ZodError → structured errors
- `formatZodError(error)` — ZodError → readable string
- `validatePayload(schema, value)` — safe parse returning a `ValidationResult`
- `makeValidationError(code, message, path)` — construct a single error
- `isValidationSuccess(result)` — type guard

### `src/shared/contracts/` — Request/Response/Error Contracts
A typed contract object for **every** IPC channel, each declaring:
- `channel` — the `IPCChannel` identifier
- `request` — incoming (Renderer → Main) Zod schema
- `response` — successful response Zod schema
- `error` — expected failure shape Zod schema
- `metadata` — optional `direction` (`invoke` | `send` | `both`) and description

No handlers are registered; no behavior is implemented.

### `src/shared/ipc/` — Typed IPC Registry
The canonical source of truth. `IPC_REGISTRY` maps every `IPCChannel` enum
member to a `RegistryEntry` (channel + contract + direction + description).
`IPC_REGISTRY_EXTRA` captures the string-literal channels not yet in the enum
(`vault:get-current`, `widget:*`). Pure lookup helpers:
- `ALL_IPC_ENTRIES` — every registered entry
- `getIPCEntry(channel)` — lookup by channel
- `isRegisteredChannel(channel)` — membership test

---

## IPC Registry Summary

Every channel registered in `src/shared/ipc/index.ts`. For each: request /
response / error contract (schema name in `src/shared/schemas` or inline).

| Channel | Request | Response | Error |
|---|---|---|---|
| `vault:open` | `VaultOpenSchema` | `VaultScanResultSchema` | `{error}` / `{canceled:true}` |
| `vault:opened` (send) | — | `{path, files[]}` | — |
| `vault:open-in-new-window` | `{path}` | `unknown` | `{error}?` |
| `vault:scan` | `{}` | `VaultScanResultSchema` | `{error}?` |
| `vault:close` | `VaultCloseSchema` | `unknown` | `unknown` |
| `vault:switch` | `VaultSwitchSchema` | `VaultSwitchResultSchema` | `{error}?` |
| `vault:get-recents` | `VaultGetRecentsSchema` | `VaultGetRecentsResultSchema` | `unknown` |
| `vault:get-current`* | `VaultGetCurrentSchema` | `unknown` | `unknown` |
| `vault:create` | `VaultCreateSchema` | `VaultCreateResultSchema` | `{error}?` |
| `file:get` | `FileGetSchema` | `FileGetResultSchema` | `{path, ast:null, error}` |
| `file:watch` | `FileGetSchema` | `{success, path}` | `{error}` |
| `note:loaded` (send) | — | `NoteLoadedSchema` | — |
| `note:updated` (send) | — | `NoteUpdatedSchema` | — |
| `note:deleted` (send) | — | `NoteDeletedSchema` | — |
| `notes:loaded` (send) | — | `NotesLoadedSchema` | — |
| `task:toggle` | `TaskToggleSchema` | `TaskToggleResultSchema` | `TaskToggleResultSchema` |
| `note:toggle` | `TaskToggleSchema` | `TaskToggleResultSchema` | `TaskToggleResultSchema` |
| `context:query` | `ContextQuerySchema` | `ContextSearchResultSchema` | `{results, error}` / `{results, disabled, reason}` |
| `context:search` (send) | — | `ContextSearchResultSchema` | — |
| `activity:log` (both) | `ActivityLogSchema` | `{success}` / `{error}` | `{error}` |
| `view-state:get-fold` | `ViewStateGetFoldSchema` | `boolean` | `boolean` |
| `view-state:set-fold` | `ViewStateSetFoldSchema` | `void` | `void` |
| `folder:create` | `FolderCreateSchema` | `FolderCreateResultSchema` | `FolderCreateResultSchema` |
| `note:create` | `NoteCreateSchema` | `NoteCreateResultSchema` | `{path, ast:null, error}` / `{success, error}` |
| `note:save` | `NoteSaveSchema` | `NoteSaveResultSchema` | `NoteSaveResultSchema` |
| `note:rename` | `NoteRenameSchema` | `NoteRenameResultSchema` | `NoteRenameResultSchema` |
| `note:delete` | `NoteDeleteSchema` | `NoteDeleteResultSchema` | `NoteDeleteResultSchema` |
| `note:get-raw` | `NoteGetRawSchema` | `NoteGetRawResultSchema` | `{path, error}` |
| `note:export-html` | `NoteExportHtmlSchema` | `NoteExportHtmlResultSchema` | `NoteExportHtmlResultSchema` |
| `templates:list` | `TemplatesListSchema` | `TemplatesListResultSchema` | `{templates:[]}` |
| `settings:get` | `SettingsGetSchema` | `SettingsGetResultSchema` | `{success, error}` |
| `settings:set` | `SettingsSetSchema` | `SettingsSetResultSchema` | `SettingsSetResultSchema` |
| `index:build` (send) | — | `IndexBuildSchema` | — |
| `asset:read` | `AssetReadSchema` | `AssetReadResultSchema` | `{path, error}` |
| `context:reindex` | `ContextReindexSchema` | `ContextReindexResultSchema` | `{error}` |
| `vector:status` | `VectorStatusSchema` | `VectorStatusResultSchema` | `{disabled, reason, items}` |
| `search:query` | `SearchQuerySchema` | `SearchResponseSchema` | `unknown` |
| `properties:read` | `PropertiesReadSchema` | `PropertiesReadResultSchema` | `{path, properties, yaml}` |
| `properties:write` | `PropertiesWriteSchema` | `PropertiesWriteResultSchema` | `PropertiesWriteResultSchema` |
| `note:daily` | `NoteDailySchema` | `NoteDailyResultSchema` | `{path, ast:null, created, error}` |
| `favorites:get` | `FavoritesGetSchema` | `FavoritesGetResultSchema` | `{favorites:[]}` |
| `favorites:toggle` | `FavoritesToggleSchema` | `FavoritesToggleResultSchema` | `{favorites:[]}` |
| `favorites:remove` | `FavoritesRemoveSchema` | `FavoritesRemoveResultSchema` | `{favorites:[]}` |
| `bookmarks:get` | `BookmarksGetSchema` | `BookmarksGetResultSchema` | `{bookmarks:{}}` |
| `bookmarks:add` | `BookmarksAddSchema` | `BookmarksAddResultSchema` | `{bookmarks:{}}` |
| `bookmarks:remove` | `BookmarksRemoveSchema` | `BookmarksRemoveResultSchema` | `{bookmarks:{}}` |
| `note:random` | `NoteRandomSchema` | `NoteRandomResultSchema` | `{error}` |
| `note:compose` | `NoteComposeSchema` | `NoteComposeResultSchema` | `{previewMarkdown, warning}` |
| `note:unique` | `NoteUniqueSchema` | `NoteUniqueResultSchema` | `{path, error}` |
| `settings:getFeatureToggles` | `{}` | `FeatureTogglesResultSchema` | `{toggles:[]}` |
| `settings:setFeatureToggle` | `SetFeatureToggleSchema` | `SetFeatureToggleResultSchema` | `SetFeatureToggleResultSchema` |
| `kanban:get-data` | `KanbanGetDataSchema` | `KanbanGetDataResultSchema` | `{statuses:[], cards:[]}` |
| `kanban:set-status` | `KanbanSetStatusSchema` | `KanbanSetStatusResultSchema` | `KanbanSetStatusResultSchema` |
| `clipboard:history-get` | `{max?}` | `ClipboardHistoryGetResultSchema` | `unknown` |
| `clipboard:history-clear` | `{}` | `unknown` | `unknown` |
| `clipboard:history-copy` | `ClipboardHistoryCopySchema` | `ClipboardHistoryCopyResultSchema` | `ClipboardHistoryCopyResultSchema` |
| `pdf:open` | `PDFOpenSchema` | `PDFOpenResultSchema` | `{error}` |
| `pdf:render-page` | `PDFRenderPageSchema` | `PDFRenderPageResultSchema` | `{error}` |
| `pdf:load-annotations` | `PDFLoadAnnotationsSchema` | `PDFLoadAnnotationsResultSchema` | `unknown` |
| `pdf:save-annotations` | `PDFSaveAnnotationsSchema` | `PDFSaveAnnotationsResultSchema` | `PDFSaveAnnotationsResultSchema` |
| `dictation:start` | `DictationStartSchema` | `DictationStartResultSchema` | `DictationStartResultSchema` |
| `dictation:stop` | `DictationStopSchema` | `DictationStopResultSchema` | `DictationStopResultSchema` |
| `dictation:status` | `DictationStatusSchema` | `DictationStatusResultSchema` | `{error}` |
| `dictation:result` (send) | — | `WhisperResultSchema` | — |
| `dictation:download-model` | `DictationDownloadModelSchema` | `DictationDownloadModelResultSchema` | `DictationDownloadModelResultSchema` |
| `dictation:download-progress` (send) | — | `DictationDownloadProgressSchema` | — |
| `widget:toggle`* | `WidgetToggleSchemaCanonical` | `void` | `void` |
| `widget:move`* | `WidgetMoveSchema` | `void` | `void` |
| `widget:resize`* | `WidgetResizeSchema` | `void` | `void` |
| `widget:create-note`* | `WidgetCreateNoteSchema` | `unknown` | `unknown` |
| `widget:fetch-title`* | `WidgetFetchTitleSchema` | `unknown` | `unknown` |
| `widget:open-note`* | `{path}` | `void` | `void` |
| `widget:set-shortcut`* | `WidgetSetShortcutSchema` | `void` | `void` |

\* Registered in `IPC_REGISTRY_EXTRA` (string-literal channel not yet in the
`IPCChannel` enum). These will be folded into the enum during Phase 2.

---

## Files Created

All under `src/shared/`:

1. `src/shared/models/index.ts` — shared domain models
2. `src/shared/schemas/index.ts` — canonical Zod schema entry (re-exports
   `../schemas` + augments with missing channel schemas)
3. `src/shared/validation/index.ts` — reusable validation helpers
4. `src/shared/contracts/index.ts` — request/response/error contracts per channel
5. `src/shared/ipc/index.ts` — typed IPC registry (canonical source of truth)

---

## Files Modified

**None.** Per the Phase 1.4 rules, no IPC handlers, preload, renderer, services,
or existing schemas were modified. The existing `src/shared/schemas.ts` remains
the underlying schema source and is re-exported by the new
`src/shared/schemas/index.ts` to avoid duplication. No runtime behavior was
introduced or changed.

---

## Verification Summary

| Check | Command | Result |
|---|---|---|
| Typecheck (node) | `npm run typecheck:node` | ✅ 0 errors |
| Typecheck (web) | `npm run typecheck:web` | ✅ 0 errors |
| Lint (new files) | `eslint src/shared/{models,schemas,validation,contracts,ipc}/index.ts` | ✅ 0 errors, 0 warnings |
| Build | `npm run build` | ✅ success (`✓ built in 25.35s`) |
| Runtime (`npm run dev`) | Electron launch | Framework is pure type/contract definitions with no behavior; build + typecheck confirm no runtime impact. Interactive launch not executed in CI environment. |

**Gate A: PASSED.**

---

## Notes for Phase 2

- The registry (`src/shared/ipc`) is ready to back the existing `ipc.ts`
  handlers. Migration should replace ad-hoc `safeParse` calls with
  `getIPCEntry(channel).contract.request` / `.response` / `.error`.
- `IPC_REGISTRY_EXTRA` channels (`vault:get-current`, `widget:*`) should be
  promoted into the `IPCChannel` enum so the registry becomes fully exhaustive.
- The `validation/` helpers can replace inline `formatZodError` usage in
  `ipc.ts` for consistent structured errors.
