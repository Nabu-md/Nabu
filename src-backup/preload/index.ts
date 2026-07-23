/**
 * preload/index.ts — migration bridge
 *
 * Expose a minimal `window.electron` surface typed from the shared IPC
 * contracts so the existing renderer can keep typechecking while the actual
 * command layer moves to Tauri `invoke`.
 */

import { contextBridge, ipcRenderer, IpcRendererEvent } from 'electron'
import { z } from 'zod'
import { IPCChannel } from '@shared/channels'
import * as C from '@shared/contracts'

// ---------------------------------------------------------------------------
// Type helpers — derive request/response types directly from the shared IPC
// contracts so the preload layer is a thin, type-safe bridge. No contract is
// invented here; every type is inferred from `src/shared/contracts`.
// ---------------------------------------------------------------------------

type Req<T extends z.ZodTypeAny> = z.infer<T>
type Res<T extends z.ZodTypeAny> = z.infer<T>

// ---------------------------------------------------------------------------
// Invoke helpers
// ---------------------------------------------------------------------------

/** Invoke a channel and return a value typed by its contract response. */
function invoke<R extends z.ZodTypeAny, S extends z.ZodTypeAny, E extends z.ZodTypeAny>(
  contract: C.IPCContract<R, S, E>,
  payload: Req<R>
): Promise<Res<S>> {
  return ipcRenderer.invoke(contract.channel, payload) as Promise<Res<S>>
}

/** Invoke a channel whose request is `z.object({})` (no payload needed). */
function invokeVoid<R extends z.ZodTypeAny, S extends z.ZodTypeAny, E extends z.ZodTypeAny>(
  contract: C.IPCContract<R, S, E>
): Promise<Res<S>> {
  return ipcRenderer.invoke(contract.channel, {}) as Promise<Res<S>>
}

// ---------------------------------------------------------------------------
// Build the electron API object
// ---------------------------------------------------------------------------

const electronAPI = {
  vault: {
    open: (): Promise<Res<typeof C.VaultOpenContract.response>> =>
      invokeVoid(C.VaultOpenContract),
    close: (vaultId?: string): Promise<Res<typeof C.VaultCloseContract.response>> =>
      ipcRenderer.invoke(IPCChannel.VAULT_CLOSE, { vaultId }),
    switch: (vaultId: string): Promise<Res<typeof C.VaultSwitchContract.response>> =>
      invoke(C.VaultSwitchContract, { vaultId }),
    getRecents: (): Promise<Res<typeof C.VaultGetRecentsContract.response>> =>
      invokeVoid(C.VaultGetRecentsContract),
    getCurrent: (): Promise<Res<typeof C.VaultGetCurrentContract.response>> =>
      ipcRenderer.invoke('vault:get-current', {}),
    create: (
      parentPath: string,
      name: string
    ): Promise<Res<typeof C.VaultCreateContract.response>> =>
      invoke(C.VaultCreateContract, { parentPath, name }),
    scan: (): Promise<Res<typeof C.VaultScanContract.response>> =>
      invokeVoid(C.VaultScanContract),
    openInNewWindow: (path: string): Promise<Res<typeof C.VaultOpenInNewWindowContract.response>> =>
      invoke(C.VaultOpenInNewWindowContract, { path })
  },
  file: {
    get: (path: string, vaultId?: string): Promise<Res<typeof C.FileGetContract.response>> =>
      invoke(C.FileGetContract, { path, vaultId }),
    readAsset: (path: string): Promise<Res<typeof C.AssetReadContract.response>> =>
      invoke(C.AssetReadContract, { path })
  },
  pdf: {
    open: (path: string): Promise<Res<typeof C.PDFOpenContract.response>> =>
      invoke(C.PDFOpenContract, { path }),
    renderPage: (
      path: string,
      pageNumber: number,
      scale: number
    ): Promise<Res<typeof C.PDFRenderPageContract.response>> =>
      invoke(C.PDFRenderPageContract, { path, pageNumber, scale }),
    loadAnnotations: (
      path: string
    ): Promise<Res<typeof C.PDFLoadAnnotationsContract.response>> =>
      invoke(C.PDFLoadAnnotationsContract, { path }),
    saveAnnotations: (
      path: string,
      annotations: Res<typeof C.PDFSaveAnnotationsContract.request>['annotations']
    ): Promise<Res<typeof C.PDFSaveAnnotationsContract.response>> =>
      invoke(C.PDFSaveAnnotationsContract, { path, annotations })
  },
  dictation: {
    start: (
      model?: 'base' | 'large-v3-turbo-q5'
    ): Promise<Res<typeof C.DictationStartContract.response>> =>
      invoke(C.DictationStartContract, { model }),
    stop: (): Promise<Res<typeof C.DictationStopContract.response>> =>
      invokeVoid(C.DictationStopContract),
    status: (): Promise<Res<typeof C.DictationStatusContract.response>> =>
      invokeVoid(C.DictationStatusContract),
    downloadModel: (
      model: 'base' | 'large-v3-turbo-q5'
    ): Promise<Res<typeof C.DictationDownloadModelContract.response>> =>
      invoke(C.DictationDownloadModelContract, { model })
  },
  folder: {
    create: (path: string): Promise<Res<typeof C.FolderCreateContract.response>> =>
      invoke(C.FolderCreateContract, { path })
  },
  note: {
    create: (
      vaultPath: string,
      name: string,
      templateContent?: string
    ): Promise<Res<typeof C.NoteCreateContract.response>> =>
      invoke(C.NoteCreateContract, { vaultPath, name, templateContent }),
    save: (path: string, content: string): Promise<Res<typeof C.NoteSaveContract.response>> =>
      invoke(C.NoteSaveContract, { path, content }),
    rename: (
      oldPath: string,
      newPath: string
    ): Promise<Res<typeof C.NoteRenameContract.response>> =>
      invoke(C.NoteRenameContract, { oldPath, newPath }),
    delete: (path: string): Promise<Res<typeof C.NoteDeleteContract.response>> =>
      invoke(C.NoteDeleteContract, { path }),
    getRaw: (path: string): Promise<Res<typeof C.NoteGetRawContract.response>> =>
      invoke(C.NoteGetRawContract, { path }),
    exportHtml: (
      path: string,
      html: string
    ): Promise<Res<typeof C.NoteExportHtmlContract.response>> =>
      invoke(C.NoteExportHtmlContract, { path, html }),
    daily: (vaultPath: string): Promise<Res<typeof C.NoteDailyContract.response>> =>
      invoke(C.NoteDailyContract, { vaultPath })
  },
  favorites: {
    get: (vaultPath: string): Promise<Res<typeof C.FavoritesGetContract.response>> =>
      invoke(C.FavoritesGetContract, { vaultPath }),
    toggle: (
      vaultPath: string,
      filePath: string
    ): Promise<Res<typeof C.FavoritesToggleContract.response>> =>
      invoke(C.FavoritesToggleContract, { vaultPath, filePath }),
    remove: (
      vaultPath: string,
      filePath: string
    ): Promise<Res<typeof C.FavoritesRemoveContract.response>> =>
      invoke(C.FavoritesRemoveContract, { vaultPath, filePath })
  },
  templates: {
    list: (vaultPath: string): Promise<Res<typeof C.TemplatesListContract.response>> =>
      invoke(C.TemplatesListContract, { vaultPath })
  },
  settings: {
    get: (key: string): Promise<Res<typeof C.SettingsGetContract.response>> =>
      invoke(C.SettingsGetContract, { key }),
    set: (key: string, value: unknown): Promise<Res<typeof C.SettingsSetContract.response>> =>
      invoke(C.SettingsSetContract, { key, value }),
    getFeatureToggles: (): Promise<Res<typeof C.SettingsGetFeatureTogglesContract.response>> =>
      invokeVoid(C.SettingsGetFeatureTogglesContract),
    setFeatureToggle: (
      id: string,
      enabled: boolean
    ): Promise<Res<typeof C.SettingsSetFeatureToggleContract.response>> =>
      invoke(C.SettingsSetFeatureToggleContract, { id, enabled })
  },
  task: {
    toggle: (path: string, lineIndex: number): Promise<Res<typeof C.TaskToggleContract.response>> =>
      invoke(C.TaskToggleContract, { path, lineIndex })
  },
  context: {
    query: (text: string): Promise<Res<typeof C.ContextQueryContract.response>> =>
      invoke(C.ContextQueryContract, { text }),
    reindex: (vaultPath: string): Promise<Res<typeof C.ContextReindexContract.response>> =>
      invoke(C.ContextReindexContract, { vaultPath }),
    status: (): Promise<Res<typeof C.VectorStatusContract.response>> =>
      invokeVoid(C.VectorStatusContract)
  },
  search: {
    query: (queryString: string): Promise<Res<typeof C.SearchQueryContract.response>> =>
      invoke(C.SearchQueryContract, { query: queryString })
  },
  properties: {
    read: (path: string): Promise<Res<typeof C.PropertiesReadContract.response>> =>
      invoke(C.PropertiesReadContract, { path }),
    write: (path: string, yaml: string): Promise<Res<typeof C.PropertiesWriteContract.response>> =>
      invoke(C.PropertiesWriteContract, { path, yaml })
  },
  viewState: {
    getFold: (
      vaultPath: string,
      notePath: string,
      headingId: string
    ): Promise<Res<typeof C.ViewStateGetFoldContract.response>> =>
      invoke(C.ViewStateGetFoldContract, { vaultPath, notePath, headingId }),
    setFold: (
      vaultPath: string,
      notePath: string,
      headingId: string,
      isOpen: boolean
    ): Promise<Res<typeof C.ViewStateSetFoldContract.response>> =>
      invoke(C.ViewStateSetFoldContract, { vaultPath, notePath, headingId, isOpen })
  },
  kanban: {
    getData: (
      vaultPath: string,
      folderPath: string
    ): Promise<Res<typeof C.KanbanGetDataContract.response>> =>
      invoke(C.KanbanGetDataContract, { vaultPath, folderPath }),
    setStatus: (
      vaultPath: string,
      filePath: string,
      status: string
    ): Promise<Res<typeof C.KanbanSetStatusContract.response>> =>
      invoke(C.KanbanSetStatusContract, { vaultPath, filePath, status })
  },
  clipboardHistory: {
    get: (max: number): Promise<Res<typeof C.ClipboardHistoryGetContract.response>> =>
      ipcRenderer.invoke(IPCChannel.CLIPBOARD_HISTORY_GET, { max }),
    clear: (): Promise<Res<typeof C.ClipboardHistoryClearContract.response>> =>
      invokeVoid(C.ClipboardHistoryClearContract),
    copy: (text: string): Promise<Res<typeof C.ClipboardHistoryCopyContract.response>> =>
      invoke(C.ClipboardHistoryCopyContract, { text })
  },
  widget: {
    setShortcut: (shortcut: string): Promise<Res<typeof C.WidgetSetShortcutContract.response>> =>
      invoke(C.WidgetSetShortcutContract, { shortcut })
  },
  on: {
    noteLoaded: (
      callback: (data: Res<typeof C.NoteLoadedContract.response>) => void
    ): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: Res<typeof C.NoteLoadedContract.response>): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.NOTE_LOADED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_LOADED, listener)
    },
    noteOpenRequested: (callback: (data: { path: string }) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: { path: string }): void => callback(data)
      ipcRenderer.on('widget:open-note-request', listener)
      return () => ipcRenderer.removeListener('widget:open-note-request', listener)
    },
    noteUpdated: (
      callback: (data: Res<typeof C.NoteUpdatedContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.NoteUpdatedContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTE_UPDATED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_UPDATED, listener)
    },
    noteDeleted: (
      callback: (data: Res<typeof C.NoteDeletedContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.NoteDeletedContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTE_DELETED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_DELETED, listener)
    },
    contextSearch: (
      callback: (data: Res<typeof C.ContextSearchContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.ContextSearchContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.CONTEXT_SEARCH, listener)
      return () => ipcRenderer.removeListener(IPCChannel.CONTEXT_SEARCH, listener)
    },
    activityLog: (
      callback: (data: Res<typeof C.ActivityLogContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.ActivityLogContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.ACTIVITY_LOG, listener)
      return () => ipcRenderer.removeListener(IPCChannel.ACTIVITY_LOG, listener)
    },
    vaultOpened: (
      callback: (data: Res<typeof C.VaultOpenedContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.VaultOpenedContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.VAULT_OPENED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.VAULT_OPENED, listener)
    },
    notesLoaded: (
      callback: (data: Res<typeof C.NotesLoadedContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.NotesLoadedContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTES_LOADED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTES_LOADED, listener)
    },
    focusSearch: (callback: () => void): (() => void) => {
      const listener = (): void => callback()
      ipcRenderer.on('focus:search', listener)
      return () => ipcRenderer.removeListener('focus:search', listener)
    },
    openSettings: (callback: () => void): (() => void) => {
      const listener = (): void => callback()
      ipcRenderer.on('open:settings', listener)
      return () => ipcRenderer.removeListener('open:settings', listener)
    },
    setupCreate: (callback: () => void): (() => void) => {
      const listener = (): void => callback()
      ipcRenderer.on('setup:create', listener)
      return () => ipcRenderer.removeListener('setup:create', listener)
    },
    setupOpen: (callback: () => void): (() => void) => {
      const listener = (): void => callback()
      ipcRenderer.on('setup:open', listener)
      return () => ipcRenderer.removeListener('setup:open', listener)
    },
    showClipboard: (callback: () => void): (() => void) => {
      const listener = (): void => callback()
      ipcRenderer.on('widget:show-clipboard', listener)
      return () => ipcRenderer.removeListener('widget:show-clipboard', listener)
    },
    indexBuild: (
      callback: (data: Res<typeof C.IndexBuildContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.IndexBuildContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.INDEX_BUILD, listener)
      return () => ipcRenderer.removeListener(IPCChannel.INDEX_BUILD, listener)
    },
    dictationDownloadProgress: (
      callback: (data: Res<typeof C.DictationDownloadProgressContract.response>) => void
    ): (() => void) => {
      const listener = (
        _event: IpcRendererEvent,
        data: Res<typeof C.DictationDownloadProgressContract.response>
      ): void => callback(data)
      ipcRenderer.on(IPCChannel.DICTATION_DOWNLOAD_PROGRESS, listener)
      return () => ipcRenderer.removeListener(IPCChannel.DICTATION_DOWNLOAD_PROGRESS, listener)
    },
    // Widget channels (for the dictation/clipboard widget window)
    widgetModeChanged: (callback: (data: { mode: 'clipboard' | 'dictation' }) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: { mode: 'clipboard' | 'dictation' }): void =>
        callback(data)
      ipcRenderer.on('widget:mode-changed', listener)
      return () => ipcRenderer.removeListener('widget:mode-changed', listener)
    },
    widgetDictationStarting: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:dictation-starting', listener)
      return () => ipcRenderer.removeListener('widget:dictation-starting', listener)
    },
    widgetDictationComplete: (callback: (data: { text: string; silent: boolean }) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: { text: string; silent: boolean }): void =>
        callback(data)
      ipcRenderer.on('widget:dictation-complete', listener)
      return () => ipcRenderer.removeListener('widget:dictation-complete', listener)
    },
    widgetDictationError: (callback: (data: { error: string }) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: { error: string }): void => callback(data)
      ipcRenderer.on('widget:dictation-error', listener)
      return () => ipcRenderer.removeListener('widget:dictation-error', listener)
    },
    widgetInsertText: (callback: (data: { text: string }) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: { text: string }): void => callback(data)
      ipcRenderer.on('widget:insert-text', listener)
      return () => ipcRenderer.removeListener('widget:insert-text', listener)
    }
  }
}

if (process.contextIsolated) {
  try {
    contextBridge.exposeInMainWorld('electron', electronAPI)
  } catch (error) {
    console.error(error)
  }
} else {
  // @ts-ignore (define in dts)
  window.electron = electronAPI
}
