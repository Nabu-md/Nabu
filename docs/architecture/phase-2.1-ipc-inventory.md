# Phase 2.1 — IPC Inventory & Channel Contracts

**Status:** Analysis / Documentation only. No runtime behavior modified.
**Scope:** Inventory every IPC interface (Main, Renderer, Preload) and define canonical request/response/error contracts for every channel.
**Source of truth for:** Phase 2.2+ migration.

---

## 1. Methodology

The inventory was produced by static analysis of:

- `src/main/ipc.ts` — `registerIPCHandlers()` (all `ipcMain.handle` registrations)
- `src/main/index.ts` — `ipcMain.on('vault:opened')`, menu-driven `webContents.send(...)`, `registerVaultPersistence()`
- `src/main/services/widget-manager.ts` — `registerWidgetIPCHandlers()`
- `src/main/services/widget-service.ts` — `registerIPCHandlers()` (clipboard history + widget shortcut)
- `src/main/services/dictation-service.ts` — `event.sender.send(DICTATION_RESULT / DICTATION_DOWNLOAD_PROGRESS)`
- `src/preload/index.ts` + `src/preload/index.d.ts` — exposed `electron` API surface
- `src/renderer/**` — all `window.electron.*` call sites
- `src/shared/channels.ts` — `IPCChannel` enum (canonical channel names)
- `src/shared/contracts/index.ts` — Phase 1.4 typed contracts

**No handlers, preload APIs, renderer usage, or channel names were modified.**

---

## 2. IPC Channel Inventory

Every channel appears exactly once. Columns:

- **Channel** — canonical name (`IPCChannel` enum value or raw string)
- **Owner** — single feature area
- **Handler Location** — where `ipcMain.handle`/`ipcMain.on` is registered (or "—" if none)
- **Preload Exposure** — `window.electron.*` path
- **Renderer Call Sites** — files using the channel
- **Service Dependency** — backing service
- **Status** — `active` | `orphaned` (exposed but no handler) | `send-only` (Main→Renderer push)

### 2.1 Vault

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `vault:open` | Vault | `ipc.ts:575` | `electron.vault.open` | `SetupWizard.tsx:30,43` | `VaultService.openVault` | active |
| `vault:opened` | Vault | `index.ts:310` (`ipcMain.on`) | `electron.on.vaultOpened` | `App.tsx:807` | — (event) | send-only |
| `vault:open-in-new-window` | Vault | `ipc.ts:816` | `electron.vault.openInNewWindow` | — (menu only) | `VaultService.openVaultInNewWindow` | active |
| `vault:scan` | Vault | `ipc.ts:585` | `electron.vault.scan` | `FileTree.tsx:498,527`, `SettingsPanel.tsx:91` | `VaultService.scanVault` | active |
| `vault:close` | Vault | `ipc.ts:595` | `electron.vault.close` | — | `VaultService.closeVault` | active |
| `vault:switch` | Vault | `ipc.ts:1720` | `electron.vault.switch` | — | `VaultService.switchVault` | active |
| `vault:get-recents` | Vault | `ipc.ts:1727` | `electron.vault.getRecents` | — | `VaultService.getRecents` | active |
| `vault:get-current` | Vault | `ipc.ts:567` | `electron.vault.getCurrent` | `App.tsx:957` | `VaultService.getCurrentVault` | active |
| `vault:create` | Vault | `ipc.ts:937` | `electron.vault.create` | `SetupWizard.tsx:55` | `VaultService.createVault` | active |

### 2.2 File / Note AST

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `file:get` | Vault | `ipc.ts:602` | `electron.file.get` | `QuickSwitcher.tsx:166`, `EmbedBlock.tsx:120,158`, `PagePreview.tsx:37`, `NoteView.tsx:798,883,967,1071,1257`, `FavoritesPanel.tsx:42`, `GraphView.tsx:474`, `FileTree.tsx:446,587`, `SearchPanel.tsx:165`, `OCRTextPanel.tsx:30`, `App.tsx:769` | `StateManager.getAST` | active |
| `file:watch` | Vault | `ipc.ts:636` | — (not exposed) | — | `VaultWatcher` | active (internal) |
| `asset:read` | Vault | `ipc.ts:1177` | `electron.file.readAsset` | `EmbedBlock.tsx:120`, `SandboxedHtml.tsx:195` | `fs` | active |
| `note:create` | Notes | `ipc.ts:970` | `electron.note.create` | `FileTree.tsx:521`, `PdfViewer.tsx:250` | `StateManager` + `substituteVariables` | active |
| `note:save` | Notes | `ipc.ts:1039` | `electron.note.save` | `NoteView.tsx:1036,1089,1384` | `StateManager` + `VectorManager` | active |
| `note:rename` | Notes | `ipc.ts:1090` | `electron.note.rename` | `FileTree.tsx:438` | `fs` | active |
| `note:delete` | Notes | `ipc.ts:1118` | `electron.note.delete` | `FileTree.tsx:465` | `fs` + `StateManager` | active |
| `note:get-raw` | Notes | `ipc.ts:1153` | `electron.note.getRaw` | `NoteView.tsx:1056`, `FileTree.tsx:530` | `fs` | active |
| `note:export-html` | Notes | `ipc.ts:1220` | `electron.note.exportHtml` | `NoteView.tsx:1240` | `dialog` + `fs` | active |
| `note:daily` | Notes | `ipc.ts:1327` | `electron.note.daily` | `App.tsx:985` | `StateManager` + `substituteVariables` | active |
| `note:random` | Notes | `ipc.ts:1422` | — (not exposed) | — | `StateManager` | active (internal) |
| `note:compose` | Notes | `ipc.ts:1871` | — (not exposed) | — | `composer.mergeNotes` | active (internal) |
| `note:unique` | Notes | `ipc.ts:1900` | — (not exposed) | — | `unique-note` | active (internal) |
| `note:toggle` | Notes | `ipc.ts:677` | — (not exposed) | — | `StateManager.toggleTask` | active (internal) |
| `task:toggle` | Notes | `ipc.ts:652` | `electron.task.toggle` | `NoteView.tsx:1152` | `StateManager.toggleTask` | active |
| `folder:create` | Vault | `ipc.ts:946` | `electron.folder.create` | `FileTree.tsx:491` | `fs` | active |

### 2.3 Context / Vector (Search feature)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `context:query` | Search | `ipc.ts:701` | `electron.context.query` | `ContextPane.tsx:81` | `VectorManager.search` | active |
| `context:search` | Search | `ipc.ts` (via `sendToRenderer`) | `electron.on.contextSearch` | `App.tsx:780` | `VectorManager` | send-only |
| `context:reindex` | Search | `ipc.ts:749` | `electron.context.reindex` | — | `VectorManager.reindexAll` | active |
| `vector:status` | Search | `ipc.ts:785` | `electron.context.status` | `App.tsx:972` | `VectorManager.getStatus` | active |
| `search:query` | Search | `ipc.ts:808` | `electron.search.query` | `SearchPanel.tsx:130` | `SearchService.query` | active |

### 2.4 Settings

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `settings:get` | Settings | `ipc.ts:886` | `electron.settings.get` | `App.tsx:905`, `SettingsPanel.tsx:36,139,191` | `loadSettings` | active |
| `settings:set` | Settings | `ipc.ts:911` | `electron.settings.set` | `SettingsPanel.tsx:102,182` | `saveSettings` | active |
| `settings:getFeatureToggles` | Settings | `ipc.ts:1512` | `electron.settings.getFeatureToggles` | `SettingsPanel.tsx:36` | `feature-toggles` | active |
| `settings:setFeatureToggle` | Settings | `ipc.ts:1532` | `electron.settings.setFeatureToggle` | `SettingsPanel.tsx:114` | `feature-toggles` + `widgetToggleCallback` | active |

### 2.5 Properties

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `properties:read` | Properties | `ipc.ts:1781` | `electron.properties.read` | — (used via PropertiesView) | `extractFrontmatter` | active |
| `properties:write` | Properties | `ipc.ts:825` | `electron.properties.write` | `NoteView.tsx:1192` | `fs` + `StateManager` | active |

### 2.6 Favorites

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `favorites:get` | Vault | `ipc.ts:1261` | `electron.favorites.get` | `FavoriteToggle.tsx:27`, `FavoritesPanel.tsx:27` | `readFavorites` | active |
| `favorites:toggle` | Vault | `ipc.ts:1283` | `electron.favorites.toggle` | `FavoriteToggle.tsx:41` | `toggleFavorite` | active |
| `favorites:remove` | Vault | `ipc.ts:1305` | `electron.favorites.remove` | — | `removeFavorite` | active |

### 2.7 Bookmarks

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `bookmarks:get` | Bookmarks | `ipc.ts:1810` | — (not exposed) | — | `readBookmarks` | active (internal) |
| `bookmarks:add` | Bookmarks | `ipc.ts:1831` | — (not exposed) | — | `addBookmark` | active (internal) |
| `bookmarks:remove` | Bookmarks | `ipc.ts:1851` | — (not exposed) | — | `removeBookmark` | active (internal) |

### 2.8 Templates

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `templates:list` | Notes | `ipc.ts:1468` | `electron.templates.list` | `FileTree.tsx:654` | `fs` | active |

### 2.9 View State

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `view-state:get-fold` | Notes | `ipc.ts:1734` | `electron.viewState.getFold` | `NoteView.tsx` (via OutlinePanel) | `loadViewState` | active |
| `view-state:set-fold` | Notes | `ipc.ts:1759` | `electron.viewState.setFold` | `NoteView.tsx:1282` | `setFoldState` | active |

### 2.10 Kanban

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `kanban:get-data` | Widgets | `ipc.ts:1563` | `electron.kanban.getData` | `KanbanBlock.tsx:150` | `fs` + `extractFrontmatter` | active |
| `kanban:set-status` | Widgets | `ipc.ts:1661` | `electron.kanban.setStatus` | `KanbanBlock.tsx:198` | `fs` + `StateManager` | active |

### 2.11 Clipboard History (Widgets)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `clipboard:history-get` | Widgets | `widget-service.ts:54` | `electron.clipboardHistory.get` | — (widget window) | `ClipboardHistory` | active |
| `clipboard:history-clear` | Widgets | `widget-service.ts:58` | `electron.clipboardHistory.clear` | — | `ClipboardHistory` | active |
| `clipboard:history-copy` | Widgets | `widget-service.ts:61` | `electron.clipboardHistory.copy` | — | `ClipboardHistory` | active |

### 2.12 PDF

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `pdf:open` | PDF | `ipc.ts:1626` | `electron.pdf.open` | `PdfViewer.tsx:69` | `PdfService.open` | active |
| `pdf:render-page` | PDF | `ipc.ts:1633` | `electron.pdf.renderPage` | `PdfViewer.tsx:150` | `PdfService.renderPage` | active |
| `pdf:load-annotations` | PDF | `ipc.ts:1640` | `electron.pdf.loadAnnotations` | `PdfViewer.tsx:110` | `PdfService.loadAnnotations` | active |
| `pdf:save-annotations` | PDF | `ipc.ts:1647` | `electron.pdf.saveAnnotations` | `PdfViewer.tsx:133` | `PdfService.saveAnnotations` | active |

### 2.13 Dictation

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `dictation:start` | Dictation | `ipc.ts:1654` | `electron.dictation.start` | `DictationWidget.tsx:132` | `DictationService.start` | active |
| `dictation:stop` | Dictation | `ipc.ts:1699` | `electron.dictation.stop` | `DictationWidget.tsx:121` | `DictationService.stop` | active |
| `dictation:status` | Dictation | `ipc.ts:1706` | `electron.dictation.status` | `SettingsPanel.tsx:377` | `DictationService.status` | active |
| `dictation:download-model` | Dictation | `ipc.ts:1713` | `electron.dictation.downloadModel` | `SettingsPanel.tsx:446` | `DictationService.downloadModel` | active |
| `dictation:result` | Dictation | `dictation-service.ts:90,103` (`event.sender.send`) | `electron.on.*` (none — see note) | — | `DictationService` | send-only |
| `dictation:download-progress` | Dictation | `dictation-service.ts:199` (`event.sender.send`) | `electron.on.dictationDownloadProgress` | `SettingsPanel.tsx:435` | `DictationService` | send-only |

> **Note:** `dictation:result` is sent from main but has **no preload `on.*` listener** and **no renderer subscription**. It is effectively dead — the widget receives `widget:dictation-complete` instead. Flagged for Phase 2.2 review.

### 2.14 Widget Window (Widgets)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `widget:show-clipboard` | Widgets | `widget-manager.ts:507` | — (not exposed) | — | `WidgetManager.show` | active (internal) |
| `widget:show-dictation` | Widgets | `widget-manager.ts:512` | — (not exposed) | — | `WidgetManager.show` | active (internal) |
| `widget:hide` | Widgets | `widget-manager.ts:517` | — (not exposed) | — | `WidgetManager.hide` | active (internal) |
| `widget:switch-mode` | Widgets | `widget-manager.ts:522` | — (not exposed) | — | `WidgetManager.switchMode` | active (internal) |
| `widget:get-state` | Widgets | `widget-manager.ts:527` | — (not exposed) | — | `WidgetManager.getState` | active (internal) |
| `widget:set-model` | Widgets | `widget-manager.ts:532` | — (not exposed) | — | `WidgetManager.setModel` | active (internal) |
| `widget:get-model` | Widgets | `widget-manager.ts:537` | — (not exposed) | — | `WidgetManager.getModel` | active (internal) |
| `widget:dictation-available` | Widgets | `widget-manager.ts:542` | — (not exposed) | — | `WidgetManager.isDictationAvailable` | active (internal) |
| `widget:set-mic-permission` | Widgets | `widget-manager.ts:547` | — (not exposed) | — | `WidgetManager.setMicPermission` | active (internal) |
| `widget:insert-text` | Widgets | `widget-manager.ts:552` | — (not exposed) | — | `WidgetManager.insertTextAtCursor` | active (internal) |
| `widget:set-shortcut` | Widgets | `widget-service.ts:81` | `electron.widget.setShortcut` | `SettingsPanel.tsx:184,192` | `WidgetManager.setShortcut` | active |
| `widget:toggle` | Widgets | — **NONE** | `electron.widget.toggle` | — | — | **orphaned** |
| `widget:move` | Widgets | — **NONE** | `electron.widget.move` | — | — | **orphaned** |
| `widget:resize` | Widgets | — **NONE** | `electron.widget.resize` | — | — | **orphaned** |
| `widget:create-note` | Widgets | — **NONE** (method exists in `widget-service.ts:102` but not wired) | `electron.widget.createNote` | — | `WidgetService.createNote` | **orphaned** |
| `widget:fetch-title` | Widgets | — **NONE** (method exists in `widget-service.ts:124` but not wired) | `electron.widget.fetchTitle` | — | `WidgetService.fetchTitle` | **orphaned** |
| `widget:open-note` | Widgets | — **NONE** (method exists in `widget-service.ts:142` but not wired) | `electron.widget.openNote` | — | `WidgetService.openNote` | **orphaned** |

#### Widget → Renderer push channels (Main→Renderer, via `webContents.send`)

| Channel | Owner | Sender Location | Preload Exposure | Renderer Call Sites | Status |
|---|---|---|---|---|---|
| `widget:open-note-request` | Widgets | `widget-service.ts:143` | `electron.on.noteOpenRequested` | `App.tsx:768` | send-only |
| `widget:mode-changed` | Widgets | `widget-manager.ts:150,195` | `electron.on.widgetModeChanged` | `DictationWidget.tsx:72` | send-only |
| `widget:dictation-starting` | Widgets | `widget-manager.ts:240` | `electron.on.widgetDictationStarting` | `DictationWidget.tsx:81` | send-only |
| `widget:dictation-complete` | Widgets | `widget-manager.ts:252,258,341` | `electron.on.widgetDictationComplete` | `DictationWidget.tsx:86` | send-only |
| `widget:dictation-error` | Widgets | `widget-manager.ts:209,218,227,282,290,320,326` | `electron.on.widgetDictationError` | `DictationWidget.tsx:97` | send-only |
| `widget:insert-text` | Widgets | `widget-manager.ts:364` | `electron.on.widgetInsertText` | — | send-only |

### 2.15 Activity Log (bidirectional)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `activity:log` | Settings | `ipc.ts:869` (`ipcMain.handle`) + `sendToRenderer` | `electron.on.activityLog` | `App.tsx:790` | `console` | both |

### 2.16 Index Build (Main→Renderer push)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `index:build` | Search | `ipc.ts` (`sendToRenderer`) | `electron.on.indexBuild` | `App.tsx:818` | `StateManager` indexes | send-only |

### 2.17 Note lifecycle push (Main→Renderer)

| Channel | Owner | Handler Location | Preload Exposure | Renderer Call Sites | Service Dependency | Status |
|---|---|---|---|---|---|---|
| `note:loaded` | Notes | `ipc.ts` (`sendToRenderer`) | `electron.on.noteLoaded` | `App.tsx:749` | `StateManager` | send-only |
| `note:updated` | Notes | `ipc.ts` (`sendToRenderer`) | `electron.on.noteUpdated` | `App.tsx:754`, `NoteView.tsx:990`, `FileTree.tsx:556` | `StateManager` | send-only |
| `note:deleted` | Notes | `ipc.ts` (`sendToRenderer`) | `electron.on.noteDeleted` | `App.tsx:764` | `StateManager` | send-only |
| `notes:loaded` | Vault | `ipc.ts` (`sendToRenderer`) | `electron.on.notesLoaded` | `App.tsx:811` | `StateManager` | send-only |

### 2.18 Menu-driven push channels (Main→Renderer, `index.ts`)

| Channel | Owner | Sender Location | Preload Exposure | Renderer Call Sites | Status |
|---|---|---|---|---|---|
| `open:settings` | Settings | `index.ts:130,147` | `electron.on.openSettings` | `App.tsx:875` | send-only |
| `setup:create` | Vault | `index.ts:188` | `electron.on.setupCreate` | `App.tsx:879` | send-only |
| `setup:open` | Vault | `index.ts:194` | `electron.on.setupOpen` | `App.tsx:882` | send-only |
| `focus:search` | Search | `index.ts:240` | `electron.on.focusSearch` | `App.tsx:786` | send-only |

---

## 3. Typed Contract Summary

Contracts are defined in `src/shared/contracts/index.ts` (Phase 1.4). The table below maps each channel to its contract and summarizes request/response/error. Where the existing contract uses `z.unknown()` / `z.object({})` / generic `z.object({ error: z.string() })`, it is flagged for hardening in Phase 2.2.

| Channel | Contract | Request | Response | Error |
|---|---|---|---|---|
| `vault:open` | `VaultOpenContract` | `VaultOpenSchema` | `VaultScanResultSchema` | `{error}` \| `{canceled:true}` |
| `vault:opened` | `VaultOpenedContract` | `{}` | `{path, files[]}` | `{}` |
| `vault:open-in-new-window` | `VaultOpenInNewWindowContract` | `{path}` | `unknown` ⚠️ | `{error?}` |
| `vault:scan` | `VaultScanContract` | `{}` | `VaultScanResultSchema` | `{error?}` |
| `vault:close` | `VaultCloseContract` | `VaultCloseSchema` | `unknown` ⚠️ | `unknown` ⚠️ |
| `vault:switch` | `VaultSwitchContract` | `VaultSwitchSchema` | `VaultSwitchResultSchema` | `{error?}` |
| `vault:get-recents` | `VaultGetRecentsContract` | `VaultGetRecentsSchema` | `VaultGetRecentsResultSchema` | `unknown` ⚠️ |
| `vault:get-current` | `VaultGetCurrentContract` | `VaultGetCurrentSchema` | `unknown` ⚠️ | `unknown` ⚠️ |
| `vault:create` | `VaultCreateContract` | `VaultCreateSchema` | `VaultCreateResultSchema` | `{error?}` |
| `file:get` | `FileGetContract` | `FileGetSchema` | `FileGetResultSchema` | `{path, ast:null, error:{line,column,message}}` |
| `file:watch` | `FileWatchContract` | `FileGetSchema` | `{success, path}` | `{error}` |
| `asset:read` | `AssetReadContract` | `AssetReadSchema` | `AssetReadResultSchema` | `{path, error}` |
| `note:create` | `NoteCreateContract` | `NoteCreateSchema` | `NoteCreateResultSchema` | `{path, ast:null, error}` \| `{success, error}` |
| `note:save` | `NoteSaveContract` | `NoteSaveSchema` | `NoteSaveResultSchema` | `NoteSaveResultSchema` |
| `note:rename` | `NoteRenameContract` | `NoteRenameSchema` | `NoteRenameResultSchema` | `NoteRenameResultSchema` |
| `note:delete` | `NoteDeleteContract` | `NoteDeleteSchema` | `NoteDeleteResultSchema` | `NoteDeleteResultSchema` |
| `note:get-raw` | `NoteGetRawContract` | `NoteGetRawSchema` | `NoteGetRawResultSchema` | `{path, error}` |
| `note:export-html` | `NoteExportHtmlContract` | `NoteExportHtmlSchema` | `NoteExportHtmlResultSchema` | `NoteExportHtmlResultSchema` |
| `note:daily` | `NoteDailyContract` | `NoteDailySchema` | `NoteDailyResultSchema` | `{path, ast:null, created, error}` |
| `note:random` | `NoteRandomContract` | `NoteRandomSchema` | `NoteRandomResultSchema` | `{error}` |
| `note:compose` | `NoteComposeContract` | `NoteComposeSchema` | `NoteComposeResultSchema` | `{previewMarkdown, warning}` |
| `note:unique` | `NoteUniqueContract` | `NoteUniqueSchema` | `NoteUniqueResultSchema` | `{path, error}` |
| `note:toggle` | `NoteToggleContract` | `TaskToggleSchema` | `TaskToggleResultSchema` | `TaskToggleResultSchema` |
| `task:toggle` | `TaskToggleContract` | `TaskToggleSchema` | `TaskToggleResultSchema` | `TaskToggleResultSchema` |
| `folder:create` | `FolderCreateContract` | `FolderCreateSchema` | `FolderCreateResultSchema` | `FolderCreateResultSchema` |
| `context:query` | `ContextQueryContract` | `ContextQuerySchema` | `ContextSearchResultSchema` | `{results, error}` \| `{results, disabled, reason}` |
| `context:search` | `ContextSearchContract` | `{}` | `ContextSearchResultSchema` | `{}` |
| `context:reindex` | `ContextReindexContract` | `ContextReindexSchema` | `ContextReindexResultSchema` | `{error}` |
| `vector:status` | `VectorStatusContract` | `VectorStatusSchema` | `VectorStatusResultSchema` | `{disabled, reason, items}` |
| `search:query` | `SearchQueryContract` | `SearchQuerySchema` | `SearchResponseSchema` | `unknown` ⚠️ |
| `settings:get` | `SettingsGetContract` | `SettingsGetSchema` | `SettingsGetResultSchema` | `{success, error}` |
| `settings:set` | `SettingsSetContract` | `SettingsSetSchema` | `SettingsSetResultSchema` | `SettingsSetResultSchema` |
| `settings:getFeatureToggles` | `SettingsGetFeatureTogglesContract` | `{}` | `FeatureTogglesResultSchema` | `{toggles[]}` |
| `settings:setFeatureToggle` | `SettingsSetFeatureToggleContract` | `SetFeatureToggleSchema` | `SetFeatureToggleResultSchema` | `SetFeatureToggleResultSchema` |
| `properties:read` | `PropertiesReadContract` | `PropertiesReadSchema` | `PropertiesReadResultSchema` | `{path, properties, yaml}` |
| `properties:write` | `PropertiesWriteContract` | `PropertiesWriteSchema` | `PropertiesWriteResultSchema` | `PropertiesWriteResultSchema` |
| `favorites:get` | `FavoritesGetContract` | `FavoritesGetSchema` | `FavoritesGetResultSchema` | `{favorites[]}` |
| `favorites:toggle` | `FavoritesToggleContract` | `FavoritesToggleSchema` | `FavoritesToggleResultSchema` | `{favorites[]}` |
| `favorites:remove` | `FavoritesRemoveContract` | `FavoritesRemoveSchema` | `FavoritesRemoveResultSchema` | `{favorites[]}` |
| `bookmarks:get` | `BookmarksGetContract` | `BookmarksGetSchema` | `BookmarksGetResultSchema` | `{bookmarks{}}` |
| `bookmarks:add` | `BookmarksAddContract` | `BookmarksAddSchema` | `BookmarksAddResultSchema` | `{bookmarks{}}` |
| `bookmarks:remove` | `BookmarksRemoveContract` | `BookmarksRemoveSchema` | `BookmarksRemoveResultSchema` | `{bookmarks{}}` |
| `templates:list` | `TemplatesListContract` | `TemplatesListSchema` | `TemplatesListResultSchema` | `{templates[]}` |
| `view-state:get-fold` | `ViewStateGetFoldContract` | `ViewStateGetFoldSchema` | `z.boolean()` | `z.boolean()` ⚠️ |
| `view-state:set-fold` | `ViewStateSetFoldContract` | `ViewStateSetFoldSchema` | `z.void()` | `z.void()` ⚠️ |
| `kanban:get-data` | `KanbanGetDataContract` | `KanbanGetDataSchema` | `KanbanGetDataResultSchema` | `{statuses[], cards[]}` |
| `kanban:set-status` | `KanbanSetStatusContract` | `KanbanSetStatusSchema` | `KanbanSetStatusResultSchema` | `KanbanSetStatusResultSchema` |
| `clipboard:history-get` | `ClipboardHistoryGetContract` | `{max?}` | `ClipboardHistoryGetResultSchema` | `unknown` ⚠️ |
| `clipboard:history-clear` | `ClipboardHistoryClearContract` | `{}` | `unknown` ⚠️ | `unknown` ⚠️ |
| `clipboard:history-copy` | `ClipboardHistoryCopyContract` | `ClipboardHistoryCopySchema` | `ClipboardHistoryCopyResultSchema` | `ClipboardHistoryCopyResultSchema` |
| `pdf:open` | `PDFOpenContract` | `PDFOpenSchema` | `PDFOpenResultSchema` | `{error}` |
| `pdf:render-page` | `PDFRenderPageContract` | `PDFRenderPageSchema` | `PDFRenderPageResultSchema` | `{error}` |
| `pdf:load-annotations` | `PDFLoadAnnotationsContract` | `PDFLoadAnnotationsSchema` | `PDFLoadAnnotationsResultSchema` | `unknown` ⚠️ |
| `pdf:save-annotations` | `PDFSaveAnnotationsContract` | `PDFSaveAnnotationsSchema` | `PDFSaveAnnotationsResultSchema` | `PDFSaveAnnotationsResultSchema` |
| `dictation:start` | `DictationStartContract` | `DictationStartSchema` | `DictationStartResultSchema` | `DictationStartResultSchema` |
| `dictation:stop` | `DictationStopContract` | `DictationStopSchema` | `DictationStopResultSchema` | `DictationStopResultSchema` |
| `dictation:status` | `DictationStatusContract` | `DictationStatusSchema` | `DictationStatusResultSchema` | `{error}` |
| `dictation:download-model` | `DictationDownloadModelContract` | `DictationDownloadModelSchema` | `DictationDownloadModelResultSchema` | `DictationDownloadModelResultSchema` |
| `dictation:result` | `DictationResultContract` | `{}` | `WhisperResultSchema` | `{}` |
| `dictation:download-progress` | `DictationDownloadProgressContract` | `{}` | `DictationDownloadProgressSchema` | `{}` |
| `widget:toggle` | `WidgetToggleContract` | `WidgetToggleSchemaCanonical` | `z.void()` | `z.void()` |
| `widget:move` | `WidgetMoveContract` | `WidgetMoveSchema` | `z.void()` | `z.void()` |
| `widget:resize` | `WidgetResizeContract` | `WidgetResizeSchema` | `z.void()` | `z.void()` |
| `widget:create-note` | `WidgetCreateNoteContract` | `WidgetCreateNoteSchema` | `unknown` ⚠️ | `unknown` ⚠️ |
| `widget:fetch-title` | `WidgetFetchTitleContract` | `WidgetFetchTitleSchema` | `unknown` ⚠️ | `unknown` ⚠️ |
| `widget:open-note` | `WidgetOpenNoteContract` | `{path}` | `z.void()` | `z.void()` |
| `widget:set-shortcut` | `WidgetSetShortcutContract` | `WidgetSetShortcutSchema` | `z.void()` | `z.void()` |
| `activity:log` | `ActivityLogContract` | `ActivityLogSchema` | `{success}` \| `{error}` | `{error}` |
| `index:build` | `IndexBuildContract` | `{}` | `IndexBuildSchema` | `{}` |
| `note:loaded` | `NoteLoadedContract` | `{}` | `NoteLoadedSchema` | `{}` |
| `note:updated` | `NoteUpdatedContract` | `{}` | `NoteUpdatedSchema` | `{}` |
| `note:deleted` | `NoteDeletedContract` | `{}` | `NoteDeletedSchema` | `{}` |
| `notes:loaded` | `NotesLoadedContract` | `{}` | `NotesLoadedSchema` | `{}` |

⚠️ = contract uses `unknown` / `z.object({})` / generic error — candidate for explicit error types in Phase 2.2.

---

## 4. IPC Ownership Map

Each channel assigned to exactly one feature area.

| Feature | Channels |
|---|---|
| **Vault** | `vault:open`, `vault:opened`, `vault:open-in-new-window`, `vault:scan`, `vault:close`, `vault:switch`, `vault:get-recents`, `vault:get-current`, `vault:create`, `file:get`, `file:watch`, `asset:read`, `folder:create`, `favorites:get`, `favorites:toggle`, `favorites:remove`, `notes:loaded`, `setup:create`, `setup:open` |
| **Notes** | `note:create`, `note:save`, `note:rename`, `note:delete`, `note:get-raw`, `note:export-html`, `note:daily`, `note:random`, `note:compose`, `note:unique`, `note:toggle`, `task:toggle`, `templates:list`, `view-state:get-fold`, `view-state:set-fold`, `note:loaded`, `note:updated`, `note:deleted` |
| **Search** | `context:query`, `context:search`, `context:reindex`, `vector:status`, `search:query`, `index:build`, `focus:search` |
| **Settings** | `settings:get`, `settings:set`, `settings:getFeatureToggles`, `settings:setFeatureToggle`, `activity:log`, `open:settings` |
| **Properties** | `properties:read`, `properties:write` |
| **Bookmarks** | `bookmarks:get`, `bookmarks:add`, `bookmarks:remove` |
| **Widgets** | `kanban:get-data`, `kanban:set-status`, `clipboard:history-get`, `clipboard:history-clear`, `clipboard:history-copy`, `widget:show-clipboard`, `widget:show-dictation`, `widget:hide`, `widget:switch-mode`, `widget:get-state`, `widget:set-model`, `widget:get-model`, `widget:dictation-available`, `widget:set-mic-permission`, `widget:insert-text`, `widget:set-shortcut`, `widget:toggle`, `widget:move`, `widget:resize`, `widget:create-note`, `widget:fetch-title`, `widget:open-note`, `widget:open-note-request`, `widget:mode-changed`, `widget:dictation-starting`, `widget:dictation-complete`, `widget:dictation-error`, `widget:insert-text` (push) |
| **PDF** | `pdf:open`, `pdf:render-page`, `pdf:load-annotations`, `pdf:save-annotations` |
| **Dictation** | `dictation:start`, `dictation:stop`, `dictation:status`, `dictation:download-model`, `dictation:result`, `dictation:download-progress` |

---

## 5. Proposed IPC Folder Layout

Permanent structure for `src/main/ipc/`, organized by feature ownership. Each feature directory contains only its own IPC registrations (one `register*.ts` per feature, plus an `index.ts` aggregator). No implementation work occurs in this phase.

```
src/main/ipc/
├── index.ts                      # registerAllIPC() — calls each feature registrar
├── vault/
│   └── registerVaultIPC.ts       # vault:open, vault:opened, vault:open-in-new-window,
│                                 #   vault:scan, vault:close, vault:switch,
│                                 #   vault:get-recents, vault:get-current, vault:create,
│                                 #   file:get, file:watch, asset:read, folder:create,
│                                 #   favorites:get, favorites:toggle, favorites:remove,
│                                 #   notes:loaded, setup:create, setup:open
├── notes/
│   └── registerNotesIPC.ts       # note:create, note:save, note:rename, note:delete,
│                                 #   note:get-raw, note:export-html, note:daily,
│                                 #   note:random, note:compose, note:unique, note:toggle,
│                                 #   task:toggle, templates:list,
│                                 #   view-state:get-fold, view-state:set-fold,
│                                 #   note:loaded, note:updated, note:deleted
├── search/
│   └── registerSearchIPC.ts      # context:query, context:search, context:reindex,
│                                 #   vector:status, search:query, index:build, focus:search
├── settings/
│   └── registerSettingsIPC.ts    # settings:get, settings:set, settings:getFeatureToggles,
│                                 #   settings:setFeatureToggle, activity:log, open:settings
├── properties/
│   └── registerPropertiesIPC.ts  # properties:read, properties:write
├── bookmarks/
│   └── registerBookmarksIPC.ts   # bookmarks:get, bookmarks:add, bookmarks:remove
├── widgets/
│   └── registerWidgetsIPC.ts     # kanban:get-data, kanban:set-status,
│                                 #   clipboard:history-get, clipboard:history-clear,
│                                 #   clipboard:history-copy, widget:* (all),
│                                 #   widget:open-note-request, widget:mode-changed,
│                                 #   widget:dictation-starting, widget:dictation-complete,
│                                 #   widget:dictation-error, widget:insert-text
├── pdf/
│   └── registerPdfIPC.ts         # pdf:open, pdf:render-page, pdf:load-annotations,
│                                 #   pdf:save-annotations
└── dictation/
    └── registerDictationIPC.ts   # dictation:start, dictation:stop, dictation:status,
                                  #   dictation:download-model, dictation:result,
                                  #   dictation:download-progress
```

**Migration note:** The current `src/main/ipc.ts` (single 1942-line file) and the widget handlers in `widget-manager.ts` / `widget-service.ts` are the sources to be split into the above structure during Phase 2.2. The `registerIPCHandlers()` entry point in `index.ts` will be replaced by `registerAllIPC()` from `src/main/ipc/index.ts`.

---

## 6. Findings & Gaps (for Phase 2.2)

1. **Orphaned preload exposures (6):** `widget:toggle`, `widget:move`, `widget:resize`, `widget:create-note`, `widget:fetch-title`, `widget:open-note` are exposed in `src/preload/index.ts` but have **no registered `ipcMain.handle`** in the main process. `WidgetService` has the corresponding methods (`createNote`, `fetchTitle`, `openNote`) but they are never wired. Decision needed: wire them or remove the preload exposure.

2. **Dead push channel:** `dictation:result` is sent from `dictation-service.ts` but has no preload listener and no renderer subscriber. The widget consumes `widget:dictation-complete` instead. Candidate for removal.

3. **Generic error contracts:** Several contracts use `z.unknown()` or `z.object({ error: z.string() })` (e.g. `vault:open-in-new-window`, `vault:close`, `vault:get-recents`, `vault:get-current`, `search:query`, `clipboard:history-get`, `clipboard:history-clear`, `pdf:load-annotations`, `widget:create-note`, `widget:fetch-title`). Phase 2.2 should introduce explicit error types.

4. **Internal-only channels not in `IPCChannel` enum:** `file:watch`, `note:random`, `note:compose`, `note:unique`, `note:toggle`, `bookmarks:*`, and all `widget:*` window-control channels use raw string literals, not the `IPCChannel` enum. Several `widget:*` channels are also absent from `src/shared/channels.ts`. Recommend promoting all to the enum for type safety.

5. **`view-state` error contract is `z.boolean()`** — error and response share the same shape, which is incorrect. Should be `z.void()` for error.

6. **Handler registration split across 3 files:** `ipc.ts`, `widget-manager.ts`, `widget-service.ts`. The proposed layout consolidates widget IPC under `src/main/ipc/widgets/`.

---

## 7. Success Criteria Checklist

- [x] Every IPC channel is catalogued (Main handlers, preload exposures, renderer call sites, push channels).
- [x] Every preload exposure is documented (Section 2, Preload Exposure column).
- [x] Every handler is documented (Section 2, Handler Location column).
- [x] Typed request, response, and error contracts exist (Section 3, referencing `src/shared/contracts/index.ts`).
- [x] A feature-oriented IPC directory structure is defined (Section 5).
- [x] No handlers, preload APIs, renderer usage, or channel names were modified.

**Phase 2.1 complete. Do not begin Phase 2.2.**
