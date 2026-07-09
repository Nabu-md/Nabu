import { contextBridge, ipcRenderer, IpcRendererEvent } from 'electron'
import { IPCChannel } from '../shared/channels'

// Build the electron API object
const electronAPI = {
  vault: {
    open: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VAULT_OPEN),
    close: (): Promise<void> => ipcRenderer.invoke(IPCChannel.VAULT_CLOSE),
    getCurrent: (): Promise<unknown> => ipcRenderer.invoke('vault:get-current'),
    create: (parentPath: string, name: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VAULT_CREATE, { parentPath, name }),
    scan: (): Promise<unknown> => ipcRenderer.invoke(IPCChannel.VAULT_SCAN, {}),
  },
  file: {
    get: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FILE_GET, { path }),
    readAsset: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.ASSET_READ, { path }),
  },
  folder: {
    create: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.FOLDER_CREATE, { path }),
  },
  note: {
    create: (
      vaultPath: string,
      name: string,
      templateContent?: string
    ): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.NOTE_CREATE, {
        vaultPath,
        name,
        templateContent,
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
  },
  templates: {
    list: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.TEMPLATES_LIST, { vaultPath }),
  },
  settings: {
    get: (key: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SETTINGS_GET, { key }),
    set: (key: string, value: unknown): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SETTINGS_SET, { key, value }),
  },
  task: {
    toggle: (path: string, lineIndex: number): Promise<void> =>
      ipcRenderer.invoke(IPCChannel.TASK_TOGGLE, { path, lineIndex }),
  },
  context: {
    query: (text: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CONTEXT_QUERY, { text }),
    reindex: (vaultPath: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.CONTEXT_REINDEX, { vaultPath }),
    status: (): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.VECTOR_STATUS, {}),
  },
  search: {
    query: (queryString: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.SEARCH_QUERY, { query: queryString }),
  },
  properties: {
    read: (path: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PROPERTIES_READ, { path }),
    write: (path: string, yaml: string): Promise<unknown> =>
      ipcRenderer.invoke(IPCChannel.PROPERTIES_WRITE, { path, yaml }),
  },
  on: {
    noteLoaded: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.NOTE_LOADED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_LOADED, listener)
    },
    noteUpdated: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.NOTE_UPDATED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_UPDATED, listener)
    },
    noteDeleted: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.NOTE_DELETED, listener)
      return () => ipcRenderer.removeListener(IPCChannel.NOTE_DELETED, listener)
    },
    contextSearch: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.CONTEXT_SEARCH, listener)
      return () =>
        ipcRenderer.removeListener(IPCChannel.CONTEXT_SEARCH, listener)
    },
    activityLog: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.ACTIVITY_LOG, listener)
      return () => ipcRenderer.removeListener(IPCChannel.ACTIVITY_LOG, listener)
    },
    vaultOpened: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on('vault:opened-test', listener)
      return () => ipcRenderer.removeListener('vault:opened-test', listener)
    },
    notesLoaded: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
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
    indexBuild: (callback: (data: unknown) => void): (() => void) => {
      const listener = (_event: IpcRendererEvent, data: unknown): void =>
        callback(data)
      ipcRenderer.on(IPCChannel.INDEX_BUILD, listener)
      return () => ipcRenderer.removeListener(IPCChannel.INDEX_BUILD, listener)
    },
  },
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
