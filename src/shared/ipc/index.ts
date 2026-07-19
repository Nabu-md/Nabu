/**
 * ipc/index.ts
 *
 * Typed IPC Registry — the canonical source of truth for all inter-process
 * communication channels.
 *
 * This module ONLY defines contracts. It does NOT register handlers and does
 * NOT implement channel behavior. During Phase 2 the existing IPC
 * implementation will be migrated to consume this registry.
 *
 * Phase 1.4 — Shared Contracts & Typed IPC Framework.
 */

import { IPCChannel } from '../channels'
import * as C from '../contracts'

// ---------------------------------------------------------------------------
// Registry entry type
// ---------------------------------------------------------------------------

/** A registered channel: its contract plus optional descriptive metadata. */
export interface RegistryEntry {
  channel: IPCChannel
  // `any` is intentional here: the registry stores heterogeneous contracts and
  // only needs structural access (channel, request/response/error schemas).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  contract: C.IPCContract<any, any, any>
  direction: 'invoke' | 'send' | 'both'
  description?: string
}

// ---------------------------------------------------------------------------
// The canonical registry
// ---------------------------------------------------------------------------

/**
 * Every IPC channel known to the application, keyed by channel identifier.
 *
 * This is the single source of truth. Adding a new channel MUST go through
 * this registry. The `contract` field references the typed request/response/
 * error definitions from `../contracts`.
 */
export const IPC_REGISTRY: Record<IPCChannel, RegistryEntry> = {
  [IPCChannel.VAULT_OPEN]: {
    channel: IPCChannel.VAULT_OPEN,
    contract: C.VaultOpenContract,
    direction: 'invoke',
    description: 'Open a vault by path or prompt with native picker'
  },
  [IPCChannel.VAULT_OPENED]: {
    channel: IPCChannel.VAULT_OPENED,
    contract: C.VaultOpenedContract,
    direction: 'send',
    description: 'Main → Renderer push when a vault opens'
  },
  [IPCChannel.VAULT_OPEN_IN_NEW_WINDOW]: {
    channel: IPCChannel.VAULT_OPEN_IN_NEW_WINDOW,
    contract: C.VaultOpenInNewWindowContract,
    direction: 'invoke'
  },
  [IPCChannel.VAULT_SCAN]: {
    channel: IPCChannel.VAULT_SCAN,
    contract: C.VaultScanContract,
    direction: 'invoke'
  },
  [IPCChannel.VAULT_CLOSE]: {
    channel: IPCChannel.VAULT_CLOSE,
    contract: C.VaultCloseContract,
    direction: 'invoke'
  },
  [IPCChannel.VAULT_SWITCH]: {
    channel: IPCChannel.VAULT_SWITCH,
    contract: C.VaultSwitchContract,
    direction: 'invoke'
  },
  [IPCChannel.VAULT_GET_RECENTS]: {
    channel: IPCChannel.VAULT_GET_RECENTS,
    contract: C.VaultGetRecentsContract,
    direction: 'invoke'
  },
  [IPCChannel.VAULT_CREATE]: {
    channel: IPCChannel.VAULT_CREATE,
    contract: C.VaultCreateContract,
    direction: 'invoke'
  },
  [IPCChannel.FILE_GET]: {
    channel: IPCChannel.FILE_GET,
    contract: C.FileGetContract,
    direction: 'invoke'
  },
  [IPCChannel.FILE_WATCH]: {
    channel: IPCChannel.FILE_WATCH,
    contract: C.FileWatchContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_LOADED]: {
    channel: IPCChannel.NOTE_LOADED,
    contract: C.NoteLoadedContract,
    direction: 'send'
  },
  [IPCChannel.NOTE_UPDATED]: {
    channel: IPCChannel.NOTE_UPDATED,
    contract: C.NoteUpdatedContract,
    direction: 'send'
  },
  [IPCChannel.NOTE_DELETED]: {
    channel: IPCChannel.NOTE_DELETED,
    contract: C.NoteDeletedContract,
    direction: 'send'
  },
  [IPCChannel.NOTES_LOADED]: {
    channel: IPCChannel.NOTES_LOADED,
    contract: C.NotesLoadedContract,
    direction: 'send'
  },
  [IPCChannel.TASK_TOGGLE]: {
    channel: IPCChannel.TASK_TOGGLE,
    contract: C.TaskToggleContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_TOGGLE]: {
    channel: IPCChannel.NOTE_TOGGLE,
    contract: C.NoteToggleContract,
    direction: 'invoke'
  },
  [IPCChannel.CONTEXT_QUERY]: {
    channel: IPCChannel.CONTEXT_QUERY,
    contract: C.ContextQueryContract,
    direction: 'invoke'
  },
  [IPCChannel.CONTEXT_SEARCH]: {
    channel: IPCChannel.CONTEXT_SEARCH,
    contract: C.ContextSearchContract,
    direction: 'send'
  },
  [IPCChannel.ACTIVITY_LOG]: {
    channel: IPCChannel.ACTIVITY_LOG,
    contract: C.ActivityLogContract,
    direction: 'both'
  },
  [IPCChannel.VIEW_STATE_GET_FOLD]: {
    channel: IPCChannel.VIEW_STATE_GET_FOLD,
    contract: C.ViewStateGetFoldContract,
    direction: 'invoke'
  },
  [IPCChannel.VIEW_STATE_SET_FOLD]: {
    channel: IPCChannel.VIEW_STATE_SET_FOLD,
    contract: C.ViewStateSetFoldContract,
    direction: 'invoke'
  },
  [IPCChannel.FOLDER_CREATE]: {
    channel: IPCChannel.FOLDER_CREATE,
    contract: C.FolderCreateContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_CREATE]: {
    channel: IPCChannel.NOTE_CREATE,
    contract: C.NoteCreateContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_SAVE]: {
    channel: IPCChannel.NOTE_SAVE,
    contract: C.NoteSaveContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_RENAME]: {
    channel: IPCChannel.NOTE_RENAME,
    contract: C.NoteRenameContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_DELETE]: {
    channel: IPCChannel.NOTE_DELETE,
    contract: C.NoteDeleteContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_GET_RAW]: {
    channel: IPCChannel.NOTE_GET_RAW,
    contract: C.NoteGetRawContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_EXPORT_HTML]: {
    channel: IPCChannel.NOTE_EXPORT_HTML,
    contract: C.NoteExportHtmlContract,
    direction: 'invoke'
  },
  [IPCChannel.TEMPLATES_LIST]: {
    channel: IPCChannel.TEMPLATES_LIST,
    contract: C.TemplatesListContract,
    direction: 'invoke'
  },
  [IPCChannel.SETTINGS_GET]: {
    channel: IPCChannel.SETTINGS_GET,
    contract: C.SettingsGetContract,
    direction: 'invoke'
  },
  [IPCChannel.SETTINGS_SET]: {
    channel: IPCChannel.SETTINGS_SET,
    contract: C.SettingsSetContract,
    direction: 'invoke'
  },
  [IPCChannel.INDEX_BUILD]: {
    channel: IPCChannel.INDEX_BUILD,
    contract: C.IndexBuildContract,
    direction: 'send'
  },
  [IPCChannel.ASSET_READ]: {
    channel: IPCChannel.ASSET_READ,
    contract: C.AssetReadContract,
    direction: 'invoke'
  },
  [IPCChannel.CONTEXT_REINDEX]: {
    channel: IPCChannel.CONTEXT_REINDEX,
    contract: C.ContextReindexContract,
    direction: 'invoke'
  },
  [IPCChannel.VECTOR_STATUS]: {
    channel: IPCChannel.VECTOR_STATUS,
    contract: C.VectorStatusContract,
    direction: 'invoke'
  },
  [IPCChannel.SEARCH_QUERY]: {
    channel: IPCChannel.SEARCH_QUERY,
    contract: C.SearchQueryContract,
    direction: 'invoke'
  },
  [IPCChannel.PROPERTIES_READ]: {
    channel: IPCChannel.PROPERTIES_READ,
    contract: C.PropertiesReadContract,
    direction: 'invoke'
  },
  [IPCChannel.PROPERTIES_WRITE]: {
    channel: IPCChannel.PROPERTIES_WRITE,
    contract: C.PropertiesWriteContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_DAILY]: {
    channel: IPCChannel.NOTE_DAILY,
    contract: C.NoteDailyContract,
    direction: 'invoke'
  },
  [IPCChannel.FAVORITES_GET]: {
    channel: IPCChannel.FAVORITES_GET,
    contract: C.FavoritesGetContract,
    direction: 'invoke'
  },
  [IPCChannel.FAVORITES_TOGGLE]: {
    channel: IPCChannel.FAVORITES_TOGGLE,
    contract: C.FavoritesToggleContract,
    direction: 'invoke'
  },
  [IPCChannel.FAVORITES_REMOVE]: {
    channel: IPCChannel.FAVORITES_REMOVE,
    contract: C.FavoritesRemoveContract,
    direction: 'invoke'
  },
  [IPCChannel.BOOKMARKS_GET]: {
    channel: IPCChannel.BOOKMARKS_GET,
    contract: C.BookmarksGetContract,
    direction: 'invoke'
  },
  [IPCChannel.BOOKMARKS_ADD]: {
    channel: IPCChannel.BOOKMARKS_ADD,
    contract: C.BookmarksAddContract,
    direction: 'invoke'
  },
  [IPCChannel.BOOKMARKS_REMOVE]: {
    channel: IPCChannel.BOOKMARKS_REMOVE,
    contract: C.BookmarksRemoveContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_RANDOM]: {
    channel: IPCChannel.NOTE_RANDOM,
    contract: C.NoteRandomContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_COMPOSE]: {
    channel: IPCChannel.NOTE_COMPOSE,
    contract: C.NoteComposeContract,
    direction: 'invoke'
  },
  [IPCChannel.NOTE_UNIQUE]: {
    channel: IPCChannel.NOTE_UNIQUE,
    contract: C.NoteUniqueContract,
    direction: 'invoke'
  },
  [IPCChannel.SETTINGS_GET_FEATURE_TOGGLES]: {
    channel: IPCChannel.SETTINGS_GET_FEATURE_TOGGLES,
    contract: C.SettingsGetFeatureTogglesContract,
    direction: 'invoke'
  },
  [IPCChannel.SETTINGS_SET_FEATURE_TOGGLE]: {
    channel: IPCChannel.SETTINGS_SET_FEATURE_TOGGLE,
    contract: C.SettingsSetFeatureToggleContract,
    direction: 'invoke'
  },
  [IPCChannel.KANBAN_GET_DATA]: {
    channel: IPCChannel.KANBAN_GET_DATA,
    contract: C.KanbanGetDataContract,
    direction: 'invoke'
  },
  [IPCChannel.KANBAN_SET_STATUS]: {
    channel: IPCChannel.KANBAN_SET_STATUS,
    contract: C.KanbanSetStatusContract,
    direction: 'invoke'
  },
  [IPCChannel.CLIPBOARD_HISTORY_GET]: {
    channel: IPCChannel.CLIPBOARD_HISTORY_GET,
    contract: C.ClipboardHistoryGetContract,
    direction: 'invoke'
  },
  [IPCChannel.CLIPBOARD_HISTORY_CLEAR]: {
    channel: IPCChannel.CLIPBOARD_HISTORY_CLEAR,
    contract: C.ClipboardHistoryClearContract,
    direction: 'invoke'
  },
  [IPCChannel.CLIPBOARD_HISTORY_COPY]: {
    channel: IPCChannel.CLIPBOARD_HISTORY_COPY,
    contract: C.ClipboardHistoryCopyContract,
    direction: 'invoke'
  },
  [IPCChannel.PDF_OPEN]: {
    channel: IPCChannel.PDF_OPEN,
    contract: C.PDFOpenContract,
    direction: 'invoke'
  },
  [IPCChannel.PDF_RENDER_PAGE]: {
    channel: IPCChannel.PDF_RENDER_PAGE,
    contract: C.PDFRenderPageContract,
    direction: 'invoke'
  },
  [IPCChannel.PDF_LOAD_ANNOTATIONS]: {
    channel: IPCChannel.PDF_LOAD_ANNOTATIONS,
    contract: C.PDFLoadAnnotationsContract,
    direction: 'invoke'
  },
  [IPCChannel.PDF_SAVE_ANNOTATIONS]: {
    channel: IPCChannel.PDF_SAVE_ANNOTATIONS,
    contract: C.PDFSaveAnnotationsContract,
    direction: 'invoke'
  },
  [IPCChannel.DICTATION_START]: {
    channel: IPCChannel.DICTATION_START,
    contract: C.DictationStartContract,
    direction: 'invoke'
  },
  [IPCChannel.DICTATION_STOP]: {
    channel: IPCChannel.DICTATION_STOP,
    contract: C.DictationStopContract,
    direction: 'invoke'
  },
  [IPCChannel.DICTATION_STATUS]: {
    channel: IPCChannel.DICTATION_STATUS,
    contract: C.DictationStatusContract,
    direction: 'invoke'
  },
  [IPCChannel.DICTATION_RESULT]: {
    channel: IPCChannel.DICTATION_RESULT,
    contract: C.DictationResultContract,
    direction: 'send'
  },
  [IPCChannel.DICTATION_DOWNLOAD_MODEL]: {
    channel: IPCChannel.DICTATION_DOWNLOAD_MODEL,
    contract: C.DictationDownloadModelContract,
    direction: 'invoke'
  },
  [IPCChannel.DICTATION_DOWNLOAD_PROGRESS]: {
    channel: IPCChannel.DICTATION_DOWNLOAD_PROGRESS,
    contract: C.DictationDownloadProgressContract,
    direction: 'send'
  }
}

// ---------------------------------------------------------------------------
// Channels not yet in the IPCChannel enum (string-literal channels)
// ---------------------------------------------------------------------------

/**
 * Channels that exist in the preload/main but are not (yet) members of the
 * `IPCChannel` enum. They are registered here as string-literal keys so the
 * registry remains the complete source of truth. These will be folded into the
 * enum during Phase 2 migration.
 */
export const IPC_REGISTRY_EXTRA: Record<string, RegistryEntry> = {
  'vault:get-current': {
    channel: 'vault:get-current' as IPCChannel,
    contract: C.VaultGetCurrentContract,
    direction: 'invoke',
    description: 'Renderer pulls current vault state on mount'
  },
  'widget:toggle': {
    channel: 'widget:toggle' as IPCChannel,
    contract: C.WidgetToggleContract,
    direction: 'invoke'
  },
  'widget:move': {
    channel: 'widget:move' as IPCChannel,
    contract: C.WidgetMoveContract,
    direction: 'invoke'
  },
  'widget:resize': {
    channel: 'widget:resize' as IPCChannel,
    contract: C.WidgetResizeContract,
    direction: 'invoke'
  },
  'widget:create-note': {
    channel: 'widget:create-note' as IPCChannel,
    contract: C.WidgetCreateNoteContract,
    direction: 'invoke'
  },
  'widget:fetch-title': {
    channel: 'widget:fetch-title' as IPCChannel,
    contract: C.WidgetFetchTitleContract,
    direction: 'invoke'
  },
  'widget:open-note': {
    channel: 'widget:open-note' as IPCChannel,
    contract: C.WidgetOpenNoteContract,
    direction: 'invoke'
  },
  'widget:set-shortcut': {
    channel: 'widget:set-shortcut' as IPCChannel,
    contract: C.WidgetSetShortcutContract,
    direction: 'invoke'
  }
}

// ---------------------------------------------------------------------------
// Lookup helpers (pure, side-effect free)
// ---------------------------------------------------------------------------

/** All registered channels (enum + extra) as a single iterable map. */
export const ALL_IPC_ENTRIES: RegistryEntry[] = [
  ...Object.values(IPC_REGISTRY),
  ...Object.values(IPC_REGISTRY_EXTRA)
]

/** Look up a registry entry by channel identifier (enum or string). */
export function getIPCEntry(channel: IPCChannel | string): RegistryEntry | undefined {
  if (typeof channel === 'string' && (channel as IPCChannel) in IPC_REGISTRY) {
    return IPC_REGISTRY[channel as IPCChannel]
  }
  return IPC_REGISTRY_EXTRA[channel]
}

/** True if the channel is registered in the canonical registry. */
export function isRegisteredChannel(channel: IPCChannel | string): boolean {
  return getIPCEntry(channel) !== undefined
}
