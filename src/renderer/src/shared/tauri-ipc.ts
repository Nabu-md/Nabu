import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const tauriBridge = {
  vault: {
    open: async (payload?: any) => await invoke('vault_open', { payload }),
    close: async () => await invoke('vault_close'),
    scan: async () => await invoke('vault_scan'),
    create: async (parentPath: string, name: string) => await invoke('folder_create', { payload: { path: parentPath }, name }),
  },
  note: {
    read: async (payload: any) => await invoke('note_read', { payload }),
    getRaw: async (path: string) => await invoke('note_read', { payload: { path } }),
    daily: async (path: string) => await invoke('note_daily', { path }),
    create: async (vaultPath: string, name: string, templateContent: string) => await invoke('note_create', { vaultPath, name, templateContent }),
    save: async (path: string, content: string) => await invoke('note_save', { path, content }),
    rename: async (oldPath: string, newPath: string) => await invoke('note_rename', { oldPath, newPath }),
    delete: async (path: string) => await invoke('note_delete', { path }),
  },
  file: {
    get: async (path: string, vaultId?: string) => await invoke('file_get', { path, vaultId }),
    readAsset: async (path: string) => await invoke('asset_read', { path }),
  },
  pdf: {
    open: async (path: string) => await invoke('pdf_open', { path }),
    loadAnnotations: async (path: string) => await invoke('pdf_load_annotations', { path }),
    renderPage: async (path: string, pageNumber: number, scale: number) => await invoke('pdf_render_page', { path, pageNumber, scale }),
    saveAnnotations: async (path: string, annotations: any) => await invoke('pdf_save_annotations', { path, annotations }),
  },
  search: {
    query: async (query: string) => await invoke('search_query', { query }),
  },
  settings: {
    get: async (key: string) => await invoke('settings_get', { key }),
    set: async (key: string, value: any) => await invoke('settings_set', { key, value }),
    getFeatureToggles: async () => await invoke('settings_get_feature_toggles'),
    setFeatureToggle: async (id: string, enabled: boolean) => await invoke('settings_set_feature_toggle', { id, enabled }),
  },
  task: {
    toggle: async (path: string, lineIndex: number) => await invoke('task_toggle', { path, lineIndex }),
  },
  favorites: {
    get: async (vaultPath: string) => await invoke('favorites_get', { vaultPath }),
    toggle: async (vaultPath: string, path: string) => await invoke('favorites_toggle', { vaultPath, path }),
  },
  dictation: {
    downloadModel: async (model: string) => await invoke('dictation_download_model', { model }),
  },
  widget: {
    setShortcut: async (shortcut: string) => await invoke('widget_set_shortcut', { shortcut }),
  },
  kanban: {
    setStatus: async (folderPath: string, filePath: string, newStatus: string) => await invoke('kanban_set_status', { folderPath, filePath, newStatus }),
  },
  templates: {
    list: async (vaultPath: string) => await invoke('templates_list', { vaultPath }),
  },
  on: {
    noteLoaded: (callback: (payload: any) => void) => listen('noteLoaded', (event) => callback(event.payload)),
    noteUpdated: (callback: (payload: any) => void) => listen('noteUpdated', (event) => callback(event.payload)),
    noteDeleted: (callback: (payload: any) => void) => listen('noteDeleted', (event) => callback(event.payload)),
    noteOpenRequested: (callback: (payload: any) => void) => listen('noteOpenRequested', (event) => callback(event.payload)),
    contextSearch: (callback: (payload: any) => void) => listen('contextSearch', (event) => callback(event.payload)),
    vaultOpened: (callback: (payload: any) => void) => listen('vaultOpened', (event) => callback(event.payload)),
    notesLoaded: (callback: (payload: any) => void) => listen('notesLoaded', (event) => callback(event.payload)),
    focusSearch: (callback: (payload: any) => void) => listen('focusSearch', (event) => callback(event.payload)),
    indexBuild: (callback: (payload: any) => void) => listen('indexBuild', (event) => callback(event.payload)),
    openSettings: (callback: (payload: any) => void) => listen('openSettings', (event) => callback(event.payload)),
    setupCreate: (callback: (payload: any) => void) => listen('setupCreate', (event) => callback(event.payload)),
    setupOpen: (callback: (payload: any) => void) => listen('setupOpen', (event) => callback(event.payload)),
  }
};
