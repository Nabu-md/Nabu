/**
 * contracts/index.ts
 *
 * Request / Response / Error contracts for every IPC channel.
 *
 * Each contract is a pure type-level + schema-level description. It does NOT
 * register handlers and does NOT implement channel behavior. It is the
 * canonical description that the typed IPC registry (../ipc) references.
 *
 * Phase 1.4 — Shared Contracts & Typed IPC Framework.
 */

import { z } from 'zod'

import { IPCChannel } from '../channels'
import * as S from '../schemas/index'

// ---------------------------------------------------------------------------
// Contract shape
// ---------------------------------------------------------------------------

/**
 * A typed IPC contract. `Request`, `Response`, and `Error` are Zod schemas so
 * the same definitions can be used for both runtime validation and static
 * type inference. `metadata` is optional descriptive information.
 */
export interface IPCContract<
  R extends z.ZodTypeAny,
  S extends z.ZodTypeAny,
  E extends z.ZodTypeAny
> {
  channel: IPCChannel
  /** Incoming (Renderer → Main) payload schema. */
  request: R
  /** Successful response schema. */
  response: S
  /** Expected failure shape schema. */
  error: E
  /** Optional descriptive metadata. */
  metadata?: {
    direction: 'invoke' | 'send' | 'both'
    description?: string
  }
}

// ---------------------------------------------------------------------------
// Helper to build a contract with inferred generics
// ---------------------------------------------------------------------------

function defineContract<R extends z.ZodTypeAny, S extends z.ZodTypeAny, E extends z.ZodTypeAny>(
  contract: IPCContract<R, S, E>
): IPCContract<R, S, E> {
  return contract
}

// ---------------------------------------------------------------------------
// Vault contracts
// ---------------------------------------------------------------------------

export const VaultOpenContract = defineContract({
  channel: IPCChannel.VAULT_OPEN,
  request: S.VaultOpenSchema,
  response: S.VaultScanResultSchema,
  error: z.object({ error: z.string() }).or(z.object({ canceled: z.literal(true) })),
  metadata: {
    direction: 'invoke',
    description: 'Open a vault by path or prompt with native picker'
  }
})

export const VaultOpenedContract = defineContract({
  channel: IPCChannel.VAULT_OPENED,
  request: z.object({}),
  response: z.object({ path: z.string(), files: z.array(z.any()) }),
  error: z.object({}),
  metadata: { direction: 'send', description: 'Main → Renderer push when a vault opens' }
})

export const VaultOpenInNewWindowContract = defineContract({
  channel: IPCChannel.VAULT_OPEN_IN_NEW_WINDOW,
  request: z.object({ path: z.string() }),
  response: z.unknown(),
  error: z.object({ error: z.string() }).optional(),
  metadata: { direction: 'invoke' }
})

export const VaultScanContract = defineContract({
  channel: IPCChannel.VAULT_SCAN,
  request: z.object({}),
  response: S.VaultScanResultSchema,
  error: z.object({ error: z.string() }).optional(),
  metadata: { direction: 'invoke' }
})

export const VaultCloseContract = defineContract({
  channel: IPCChannel.VAULT_CLOSE,
  request: S.VaultCloseSchema,
  response: z.unknown(),
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const VaultSwitchContract = defineContract({
  channel: IPCChannel.VAULT_SWITCH,
  request: S.VaultSwitchSchema,
  response: S.VaultSwitchResultSchema,
  error: z.object({ error: z.string() }).optional(),
  metadata: { direction: 'invoke' }
})

export const VaultGetRecentsContract = defineContract({
  channel: IPCChannel.VAULT_GET_RECENTS,
  request: S.VaultGetRecentsSchema,
  response: S.VaultGetRecentsResultSchema,
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const VaultGetCurrentContract = defineContract({
  channel: 'vault:get-current' as IPCChannel,
  request: S.VaultGetCurrentSchema,
  response: z.unknown(),
  error: z.unknown(),
  metadata: { direction: 'invoke', description: 'Renderer pulls current vault state on mount' }
})

export const VaultCreateContract = defineContract({
  channel: IPCChannel.VAULT_CREATE,
  request: S.VaultCreateSchema,
  response: S.VaultCreateResultSchema,
  error: z.object({ error: z.string() }).optional(),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// File / note AST contracts
// ---------------------------------------------------------------------------

export const FileGetContract = defineContract({
  channel: IPCChannel.FILE_GET,
  request: S.FileGetSchema,
  response: S.FileGetResultSchema,
  error: z.object({
    path: z.string(),
    ast: z.null(),
    error: z.object({ line: z.number(), column: z.number(), message: z.string() })
  }),
  metadata: { direction: 'invoke' }
})

export const FileWatchContract = defineContract({
  channel: IPCChannel.FILE_WATCH,
  request: S.FileGetSchema,
  response: z.object({ success: z.boolean(), path: z.string() }),
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const NoteLoadedContract = defineContract({
  channel: IPCChannel.NOTE_LOADED,
  request: z.object({}),
  response: S.NoteLoadedSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

export const NoteUpdatedContract = defineContract({
  channel: IPCChannel.NOTE_UPDATED,
  request: z.object({}),
  response: S.NoteUpdatedSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

export const NoteDeletedContract = defineContract({
  channel: IPCChannel.NOTE_DELETED,
  request: z.object({}),
  response: S.NoteDeletedSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

export const NotesLoadedContract = defineContract({
  channel: IPCChannel.NOTES_LOADED,
  request: z.object({}),
  response: S.NotesLoadedSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

// ---------------------------------------------------------------------------
// Task / note toggle contracts
// ---------------------------------------------------------------------------

export const TaskToggleContract = defineContract({
  channel: IPCChannel.TASK_TOGGLE,
  request: S.TaskToggleSchema,
  response: S.TaskToggleResultSchema,
  error: S.TaskToggleResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteToggleContract = defineContract({
  channel: IPCChannel.NOTE_TOGGLE,
  request: S.TaskToggleSchema,
  response: S.TaskToggleResultSchema,
  error: S.TaskToggleResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Context / vector contracts
// ---------------------------------------------------------------------------

export const ContextQueryContract = defineContract({
  channel: IPCChannel.CONTEXT_QUERY,
  request: S.ContextQuerySchema,
  response: S.ContextSearchResultSchema,
  error: z
    .object({ results: z.array(z.unknown()), error: z.string() })
    .or(z.object({ results: z.array(z.unknown()), disabled: z.boolean(), reason: z.string() })),
  metadata: { direction: 'invoke' }
})

export const ContextSearchContract = defineContract({
  channel: IPCChannel.CONTEXT_SEARCH,
  request: z.object({}),
  response: S.ContextSearchResultSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

export const ContextReindexContract = defineContract({
  channel: IPCChannel.CONTEXT_REINDEX,
  request: S.ContextReindexSchema,
  response: S.ContextReindexResultSchema,
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const VectorStatusContract = defineContract({
  channel: IPCChannel.VECTOR_STATUS,
  request: S.VectorStatusSchema,
  response: S.VectorStatusResultSchema,
  error: z.object({ disabled: z.boolean(), reason: z.string(), items: z.number() }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Activity log contract (bidirectional)
// ---------------------------------------------------------------------------

export const ActivityLogContract = defineContract({
  channel: IPCChannel.ACTIVITY_LOG,
  request: S.ActivityLogSchema,
  response: z.object({ success: z.boolean() }).or(z.object({ error: z.string() })),
  error: z.object({ error: z.string() }),
  metadata: { direction: 'both' }
})

// ---------------------------------------------------------------------------
// Folder / note CRUD contracts
// ---------------------------------------------------------------------------

export const FolderCreateContract = defineContract({
  channel: IPCChannel.FOLDER_CREATE,
  request: S.FolderCreateSchema,
  response: S.FolderCreateResultSchema,
  error: S.FolderCreateResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteCreateContract = defineContract({
  channel: IPCChannel.NOTE_CREATE,
  request: S.NoteCreateSchema,
  response: S.NoteCreateResultSchema,
  error: z
    .object({
      path: z.string(),
      ast: z.null(),
      error: z.object({ line: z.number(), column: z.number(), message: z.string() })
    })
    .or(z.object({ success: z.boolean(), error: z.string() })),
  metadata: { direction: 'invoke' }
})

export const NoteSaveContract = defineContract({
  channel: IPCChannel.NOTE_SAVE,
  request: S.NoteSaveSchema,
  response: S.NoteSaveResultSchema,
  error: S.NoteSaveResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteRenameContract = defineContract({
  channel: IPCChannel.NOTE_RENAME,
  request: S.NoteRenameSchema,
  response: S.NoteRenameResultSchema,
  error: S.NoteRenameResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteDeleteContract = defineContract({
  channel: IPCChannel.NOTE_DELETE,
  request: S.NoteDeleteSchema,
  response: S.NoteDeleteResultSchema,
  error: S.NoteDeleteResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteGetRawContract = defineContract({
  channel: IPCChannel.NOTE_GET_RAW,
  request: S.NoteGetRawSchema,
  response: S.NoteGetRawResultSchema,
  error: z.object({ path: z.string(), error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const NoteExportHtmlContract = defineContract({
  channel: IPCChannel.NOTE_EXPORT_HTML,
  request: S.NoteExportHtmlSchema,
  response: S.NoteExportHtmlResultSchema,
  error: S.NoteExportHtmlResultSchema,
  metadata: { direction: 'invoke' }
})

export const NoteDailyContract = defineContract({
  channel: IPCChannel.NOTE_DAILY,
  request: S.NoteDailySchema,
  response: S.NoteDailyResultSchema,
  error: z.object({ path: z.string(), ast: z.null(), created: z.boolean(), error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const NoteRandomContract = defineContract({
  channel: IPCChannel.NOTE_RANDOM,
  request: S.NoteRandomSchema,
  response: S.NoteRandomResultSchema,
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const NoteComposeContract = defineContract({
  channel: IPCChannel.NOTE_COMPOSE,
  request: S.NoteComposeSchema,
  response: S.NoteComposeResultSchema,
  error: z.object({ previewMarkdown: z.string(), warning: z.string() }),
  metadata: { direction: 'invoke' }
})

export const NoteUniqueContract = defineContract({
  channel: IPCChannel.NOTE_UNIQUE,
  request: S.NoteUniqueSchema,
  response: S.NoteUniqueResultSchema,
  error: z.object({ path: z.string(), error: z.string() }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Templates contract
// ---------------------------------------------------------------------------

export const TemplatesListContract = defineContract({
  channel: IPCChannel.TEMPLATES_LIST,
  request: S.TemplatesListSchema,
  response: S.TemplatesListResultSchema,
  error: z.object({ templates: z.array(z.unknown()) }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Settings contracts
// ---------------------------------------------------------------------------

export const SettingsGetContract = defineContract({
  channel: IPCChannel.SETTINGS_GET,
  request: S.SettingsGetSchema,
  response: S.SettingsGetResultSchema,
  error: z.object({ success: z.boolean(), error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const SettingsSetContract = defineContract({
  channel: IPCChannel.SETTINGS_SET,
  request: S.SettingsSetSchema,
  response: S.SettingsSetResultSchema,
  error: S.SettingsSetResultSchema,
  metadata: { direction: 'invoke' }
})

export const SettingsGetFeatureTogglesContract = defineContract({
  channel: IPCChannel.SETTINGS_GET_FEATURE_TOGGLES,
  request: z.object({}),
  response: S.FeatureTogglesResultSchema,
  error: z.object({ toggles: z.array(z.unknown()) }),
  metadata: { direction: 'invoke' }
})

export const SettingsSetFeatureToggleContract = defineContract({
  channel: IPCChannel.SETTINGS_SET_FEATURE_TOGGLE,
  request: S.SetFeatureToggleSchema,
  response: S.SetFeatureToggleResultSchema,
  error: S.SetFeatureToggleResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Search contract
// ---------------------------------------------------------------------------

export const SearchQueryContract = defineContract({
  channel: IPCChannel.SEARCH_QUERY,
  request: S.SearchQuerySchema,
  response: S.SearchResponseSchema,
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Asset contract
// ---------------------------------------------------------------------------

export const AssetReadContract = defineContract({
  channel: IPCChannel.ASSET_READ,
  request: S.AssetReadSchema,
  response: S.AssetReadResultSchema,
  error: z.object({ path: z.string(), error: z.string() }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Index build contract (Main → Renderer push)
// ---------------------------------------------------------------------------

export const IndexBuildContract = defineContract({
  channel: IPCChannel.INDEX_BUILD,
  request: z.object({}),
  response: S.IndexBuildSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

// ---------------------------------------------------------------------------
// Properties contracts
// ---------------------------------------------------------------------------

export const PropertiesReadContract = defineContract({
  channel: IPCChannel.PROPERTIES_READ,
  request: S.PropertiesReadSchema,
  response: S.PropertiesReadResultSchema,
  error: z.object({
    path: z.string(),
    properties: z.record(z.string(), z.unknown()),
    yaml: z.string()
  }),
  metadata: { direction: 'invoke' }
})

export const PropertiesWriteContract = defineContract({
  channel: IPCChannel.PROPERTIES_WRITE,
  request: S.PropertiesWriteSchema,
  response: S.PropertiesWriteResultSchema,
  error: S.PropertiesWriteResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Favorites contracts
// ---------------------------------------------------------------------------

export const FavoritesGetContract = defineContract({
  channel: IPCChannel.FAVORITES_GET,
  request: S.FavoritesGetSchema,
  response: S.FavoritesGetResultSchema,
  error: z.object({ favorites: z.array(z.string()) }),
  metadata: { direction: 'invoke' }
})

export const FavoritesToggleContract = defineContract({
  channel: IPCChannel.FAVORITES_TOGGLE,
  request: S.FavoritesToggleSchema,
  response: S.FavoritesToggleResultSchema,
  error: z.object({ favorites: z.array(z.string()) }),
  metadata: { direction: 'invoke' }
})

export const FavoritesRemoveContract = defineContract({
  channel: IPCChannel.FAVORITES_REMOVE,
  request: S.FavoritesRemoveSchema,
  response: S.FavoritesRemoveResultSchema,
  error: z.object({ favorites: z.array(z.string()) }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Bookmarks contracts
// ---------------------------------------------------------------------------

export const BookmarksGetContract = defineContract({
  channel: IPCChannel.BOOKMARKS_GET,
  request: S.BookmarksGetSchema,
  response: S.BookmarksGetResultSchema,
  error: z.object({ bookmarks: z.record(z.string(), z.array(z.string())) }),
  metadata: { direction: 'invoke' }
})

export const BookmarksAddContract = defineContract({
  channel: IPCChannel.BOOKMARKS_ADD,
  request: S.BookmarksAddSchema,
  response: S.BookmarksAddResultSchema,
  error: z.object({ bookmarks: z.record(z.string(), z.array(z.string())) }),
  metadata: { direction: 'invoke' }
})

export const BookmarksRemoveContract = defineContract({
  channel: IPCChannel.BOOKMARKS_REMOVE,
  request: S.BookmarksRemoveSchema,
  response: S.BookmarksRemoveResultSchema,
  error: z.object({ bookmarks: z.record(z.string(), z.array(z.string())) }),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// View state contracts
// ---------------------------------------------------------------------------

export const ViewStateGetFoldContract = defineContract({
  channel: IPCChannel.VIEW_STATE_GET_FOLD,
  request: S.ViewStateGetFoldSchema,
  response: z.boolean(),
  error: z.boolean(),
  metadata: { direction: 'invoke' }
})

export const ViewStateSetFoldContract = defineContract({
  channel: IPCChannel.VIEW_STATE_SET_FOLD,
  request: S.ViewStateSetFoldSchema,
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Kanban contracts
// ---------------------------------------------------------------------------

export const KanbanGetDataContract = defineContract({
  channel: IPCChannel.KANBAN_GET_DATA,
  request: S.KanbanGetDataSchema,
  response: S.KanbanGetDataResultSchema,
  error: z.object({ statuses: z.array(z.string()), cards: z.array(z.unknown()) }),
  metadata: { direction: 'invoke' }
})

export const KanbanSetStatusContract = defineContract({
  channel: IPCChannel.KANBAN_SET_STATUS,
  request: S.KanbanSetStatusSchema,
  response: S.KanbanSetStatusResultSchema,
  error: S.KanbanSetStatusResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Clipboard history contracts
// ---------------------------------------------------------------------------

export const ClipboardHistoryGetContract = defineContract({
  channel: IPCChannel.CLIPBOARD_HISTORY_GET,
  request: z.object({ max: z.number().optional() }),
  response: S.ClipboardHistoryGetResultSchema,
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const ClipboardHistoryClearContract = defineContract({
  channel: IPCChannel.CLIPBOARD_HISTORY_CLEAR,
  request: z.object({}),
  response: z.unknown(),
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const ClipboardHistoryCopyContract = defineContract({
  channel: IPCChannel.CLIPBOARD_HISTORY_COPY,
  request: S.ClipboardHistoryCopySchema,
  response: S.ClipboardHistoryCopyResultSchema,
  error: S.ClipboardHistoryCopyResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// PDF contracts
// ---------------------------------------------------------------------------

export const PDFOpenContract = defineContract({
  channel: IPCChannel.PDF_OPEN,
  request: S.PDFOpenSchema,
  response: S.PDFOpenResultSchema,
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const PDFRenderPageContract = defineContract({
  channel: IPCChannel.PDF_RENDER_PAGE,
  request: S.PDFRenderPageSchema,
  response: S.PDFRenderPageResultSchema,
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const PDFLoadAnnotationsContract = defineContract({
  channel: IPCChannel.PDF_LOAD_ANNOTATIONS,
  request: S.PDFLoadAnnotationsSchema,
  response: S.PDFLoadAnnotationsResultSchema,
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const PDFSaveAnnotationsContract = defineContract({
  channel: IPCChannel.PDF_SAVE_ANNOTATIONS,
  request: S.PDFSaveAnnotationsSchema,
  response: S.PDFSaveAnnotationsResultSchema,
  error: S.PDFSaveAnnotationsResultSchema,
  metadata: { direction: 'invoke' }
})

// ---------------------------------------------------------------------------
// Dictation contracts
// ---------------------------------------------------------------------------

export const DictationStartContract = defineContract({
  channel: IPCChannel.DICTATION_START,
  request: S.DictationStartSchema,
  response: S.DictationStartResultSchema,
  error: S.DictationStartResultSchema,
  metadata: { direction: 'invoke' }
})

export const DictationStopContract = defineContract({
  channel: IPCChannel.DICTATION_STOP,
  request: S.DictationStopSchema,
  response: S.DictationStopResultSchema,
  error: S.DictationStopResultSchema,
  metadata: { direction: 'invoke' }
})

export const DictationStatusContract = defineContract({
  channel: IPCChannel.DICTATION_STATUS,
  request: S.DictationStatusSchema,
  response: S.DictationStatusResultSchema,
  error: z.object({ error: z.string() }),
  metadata: { direction: 'invoke' }
})

export const DictationResultContract = defineContract({
  channel: IPCChannel.DICTATION_RESULT,
  request: z.object({}),
  response: S.WhisperResultSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

export const DictationDownloadModelContract = defineContract({
  channel: IPCChannel.DICTATION_DOWNLOAD_MODEL,
  request: S.DictationDownloadModelSchema,
  response: S.DictationDownloadModelResultSchema,
  error: S.DictationDownloadModelResultSchema,
  metadata: { direction: 'invoke' }
})

export const DictationDownloadProgressContract = defineContract({
  channel: IPCChannel.DICTATION_DOWNLOAD_PROGRESS,
  request: z.object({}),
  response: S.DictationDownloadProgressSchema,
  error: z.object({}),
  metadata: { direction: 'send' }
})

// ---------------------------------------------------------------------------
// Widget contracts (clipboard-widget window)
// ---------------------------------------------------------------------------

export const WidgetToggleContract = defineContract({
  channel: 'widget:toggle' as IPCChannel,
  request: S.WidgetToggleSchemaCanonical,
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})

export const WidgetMoveContract = defineContract({
  channel: 'widget:move' as IPCChannel,
  request: S.WidgetMoveSchema,
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})

export const WidgetResizeContract = defineContract({
  channel: 'widget:resize' as IPCChannel,
  request: S.WidgetResizeSchema,
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})

export const WidgetCreateNoteContract = defineContract({
  channel: 'widget:create-note' as IPCChannel,
  request: S.WidgetCreateNoteSchema,
  response: z.unknown(),
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const WidgetFetchTitleContract = defineContract({
  channel: 'widget:fetch-title' as IPCChannel,
  request: S.WidgetFetchTitleSchema,
  response: z.unknown(),
  error: z.unknown(),
  metadata: { direction: 'invoke' }
})

export const WidgetOpenNoteContract = defineContract({
  channel: 'widget:open-note' as IPCChannel,
  request: z.object({ path: z.string() }),
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})

export const WidgetSetShortcutContract = defineContract({
  channel: 'widget:set-shortcut' as IPCChannel,
  request: S.WidgetSetShortcutSchema,
  response: z.void(),
  error: z.void(),
  metadata: { direction: 'invoke' }
})
