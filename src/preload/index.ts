import { contextBridge, ipcRenderer, IpcRendererEvent } from 'electron'
import { IPCChannel } from '@shared/channels'

// Build the electron API object
const electronAPI = {
  vault: {
    open: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VAULT_OPEN),
    close: (vaultId?: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VAULT_CLOSE, { vaultId }),
    switch: (vaultId: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VAULT_SWITCH, { vaultId }),
    getRecents: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VAULT_GET_RECENTS, {}),
    getCurrent: (): Promise<unknown> => ipcRenderer.invoke('vault:get-current'),
    create: (parentPath: string, name: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VAULT_CREATE, { parentPath, name }),
    scan: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VAULT_SCAN, {}),
    openInNewWindow: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VAULT_OPEN_IN_NEW_WINDOW, { path })
  },
  file: {
    get: (path: string, vaultId?: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FILE_GET, { path, vaultId }),
    readAsset: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.ASSET_READ, { path })
  },
  pdf: {
    open: (path: string): Promise<unknown> => ipcRenderer.invoke(IPCChannel.PDF_OPEN, { path }),
    renderPage: (path: string, pageNumber: number, scale: number): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PDF_RENDER_PAGE, { path, pageNumber, scale }),
    loadAnnotations: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PDF_LOAD_ANNOTATIONS, { path }),
    saveAnnotations: (path: string, annotations: unknown[]): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PDF_SAVE_ANNOTATIONS, { path, annotations })
  },
  dictation: {
    start: (model?: 'base' | 'large-v3-turbo-q5'): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.DICTATION_START, { model }),
    stop: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.DICTATION_STOP, {}),
    status: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.DICTATION_STATUS, {}),
    downloadModel: (model: 'base' | 'large-v3-turbo-q5'): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.DICTATION_DOWNLOAD_MODEL, { model })
  },
  folder: {
    create: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FOLDER_CREATE, { path })
  },
  note: {
    create: (vaultPath: string, name: string, templateContent?: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_CREATE, {
        vaultPath,
        name,
        templateContent
      }),
    save: (path: string, content: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_SAVE, { path, content }),
    rename: (oldPath: string, newPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_RENAME, { oldPath, newPath }),
    delete: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_DELETE, { path }),
    getRaw: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_GET_RAW, { path }),
    exportHtml: (path: string, html: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_EXPORT_HTML, { path, html }),
    daily: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_DAILY, { vaultPath })
  },
  favorites: {
    get: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FAVORITES_GET, { vaultPath }),
    toggle: (vaultPath: string, filePath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FAVORITES_TOGGLE, { vaultPath, filePath }),
    remove: (vaultPath: string, filePath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FAVORITES_REMOVE, { vaultPath, filePath })
  },
  templates: {
    list: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.TEMPLATES_LIST, { vaultPath })
  },
  settings: {
    get: (key: string): Promise<unknown> => ipcRenderer.invoke(IPCChannel.SETTINGS_GET, { key }),
    set: (key: string, value: unknown): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SETTINGS_SET, { key, value }),
    getFeatureToggles: (): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SETTINGS_GET_FEATURE_TOGGLES),
    setFeatureToggle: (id: string, enabled: boolean): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SETTINGS_SET_FEATURE_TOGGLE, { id, enabled })
  },
  task: {
    toggle: (path: string, lineIndex: number): Promise<void> =>
      ipcRenderer.invoke(IPCChannel.TASK_TOGGLE, { path, lineIndex })
  },
  context: {
    query: (text: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CONTEXT_QUERY, { text }),
    reindex: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CONTEXT_REINDEX, { vaultPath }),
    status: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VECTOR_STATUS, {})
  },
  search: {
    query: (queryString: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SEARCH_QUERY, { query: queryString })
  },
  properties: {
    read: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PROPERTIES_READ, { path }),
    write: (path: string, yaml: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PROPERTIES_WRITE, { path, yaml })
  },
  viewState: {
    getFold: (vaultPath: string, notePath: string, headingId: string): Promise<boolean> =>
      ipcRenderer.invoke(IPCChannel.VIEW_STATE_GET_FOLD, { vaultPath, notePath, headingId }),
    setFold: (vaultPath: string, notePath: string, headingId: string, isOpen: boolean): Promise<void> =>
      ipcRenderer.invoke(IPCChannel.VIEW_STATE_SET_FOLD, { vaultPath, notePath, headingId, isOpen })
  },
  kanban: {
    getData: (vaultPath: string, folderPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.KANBAN_GET_DATA, { vaultPath, folderPath }),
    setStatus: (vaultPath: string, filePath: string, status: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.KANBAN_SET_STATUS, { vaultPath, filePath, status })
  },
  clipboardHistory: {
    get: (max: number): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CLIPBOARD_HISTORY_GET, { max }),
    clear: (): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CLIPBOARD_HISTORY_CLEAR, {}),
    copy: (text: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CLIPBOARD_HISTORY_COPY, { text })
  },
  widget: {
    toggle: (): Promise<void> => ipcRenderer.invoke('widget:toggle', {}),
    move: (dx: number, dy: number): Promise<void> =>
      ipcRenderer.invoke('widget:move', { dx, dy }),
    resize: (opts: { width: number; height: number }): Promise<void> =>
      ipcRenderer.invoke('widget:resize', opts),
    createNote: (opts: { name: string; content: string; timestamp?: boolean }): Promise<unknown> =>
      ipcRenderer.invoke('widget:create-note', opts),
    fetchTitle: (url: string): Promise<unknown> =>
      ipcRenderer.invoke('widget:fetch-title', { url }),
    openNote: (path: string): Promise<void> =>
      ipcRenderer.invoke('widget:open-note', { path }),
    setShortcut: (shortcut: string): Promise<void> =>
      ipcRenderer.invoke('widget:set-shortcut', { shortcut })
  },
  on: {
    noteLoaded: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTE_LOADED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_LOADED, listener)
    },
    noteOpenRequested: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:open-note-request', listener)
      return () => ipcRenderer.removeListener('widget:open-note-request', listener)
    },
    noteUpdated: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTE_UPDATED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_UPDATED, listener)
    },
    noteDeleted: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.NOTE_DELETED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_DELETED, listener)
    },
    contextSearch: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.CONTEXT_SEARCH, listener)
      return () => ipcRenderer.removeListener(IPCChannel.CONTEXT_SEARCH, listener)
    },
    activityLog: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.ACTIVITY_LOG, listener)
      return () => ipcRenderer.removeListener(IPCChannel.ACTIVITY_LOG, listener)
    },
    vaultOpened: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.VAULT_OPENED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.VAULT_OPENED, listener)
    },
    notesLoaded: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
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
    indexBuild: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.INDEX_BUILD, listener)
      return () => ipcRenderer.removeListener(IPCChannel.INDEX_BUILD, listener)
    },
    dictationDownloadProgress: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on(IPCChannel.DICTATION_DOWNLOAD_PROGRESS, listener)
      return () => ipcRenderer.removeListener(IPCChannel.DICTATION_DOWNLOAD_PROGRESS, listener)
    },
    // Widget channels (for the dictation/clipboard widget window)
    widgetModeChanged: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:mode-changed', listener)
      return () => ipcRenderer.removeListener('widget:mode-changed', listener)
    },
    widgetDictationStarting: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:dictation-starting', listener)
      return () => ipcRenderer.removeListener('widget:dictation-starting', listener)
    },
    widgetDictationComplete: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:dictation-complete', listener)
      return () => ipcRenderer.removeListener('widget:dictation-complete', listener)
    },
    widgetDictationError: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
      ipcRenderer.on('widget:dictation-error', listener)
      return () => ipcRenderer.removeListener('widget:dictation-error', listener)
    },
    widgetInsertText: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void => callback(data)
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
