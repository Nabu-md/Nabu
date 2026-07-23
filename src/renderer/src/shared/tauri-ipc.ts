import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const tauriBridge = {
  vault: {
    open: async (payload: { path: string }) => await invoke<any>('vault_open', { payload }),
    close: async () => await invoke<any>('vault_close'),
    scan: async () => await invoke<any>('vault_scan'),
    create: async (parentPath: string, name: string) => await invoke<void>('folder_create', { payload: { path: parentPath }, name }),
  },
  note: {
    read: async (payload: { path: string }) => await invoke<string>('note_read', { payload }),
    getRaw: async (path: string) => await invoke<string>('note_read', { payload: { path } }),
    daily: async (path: string) => await invoke<string>('note_daily', { path }),
    create: async (vaultPath: string, name: string, templateContent: string) => await invoke<import('../../../shared/types').FileEntry>('note_create', { vaultPath, name, templateContent }),
    save: async (path: string, content: string) => await invoke<{ success: boolean; error: string | null }>('note_save', { path, content }),
    rename: async (oldPath: string, newPath: string) => await invoke<import('../../../shared/types').FileEntry>('note_rename', { oldPath, newPath }),
    delete: async (path: string) => await invoke<void>('note_delete', { payload: { path } }),
    exportHtml: async (path: string, html: string) => await invoke<{ success: boolean; error: string | null }>('note_export_html', { path, html }),
  },
  file: {
    get: async (path: string, vaultId?: string) => await invoke<import('../../../shared/types').FileAST>('file_get', { path, vaultId }),
    readAsset: async (path: string) => await invoke<Uint8Array>('asset_read', { path }),
  },
  pdf: {
    open: async (path: string) => await invoke<{ totalPages: number; metadata: { title: string; author: string }; error?: string }>('pdf_open', { path }),
    loadAnnotations: async (path: string) => await invoke<{ annotations: import('../../../shared/types').PDFAnnotation[] }>('pdf_load_annotations', { path }),
    saveAnnotations: async (path: string, annotations: import('../../../shared/types').PDFAnnotation[]) => await invoke<void>('pdf_save_annotations', { path, annotations }),
    renderPage: async (path: string, pageNumber: number, scale: number) => await invoke<{ dataUri: string; error?: string }>('pdf_render_page', { path, pageNumber, scale }),
  },
  search: {
    query: async (query: string) => await invoke('search_query', { query }),
  },
  settings: {
    get: async (key: string) => await invoke('settings_get', { key }),
    set: async (key: string, value: any) => await invoke('settings_set', { key, value }),
    getFeatureToggles: async () => await invoke('settings_get_feature_toggles'),
    setFeatureToggle: async (id: string, enabled: boolean) => await invoke<{ success: boolean; error?: string }>('settings_set_feature_toggle', { id, enabled }),
  },
  task: {
    toggle: async (path: string, lineIndex: number) => await invoke('task_toggle', { path, lineIndex }),
  },
  favorites: {
    get: async (vaultPath: string) => await invoke('favorites_get', { vaultPath }),
    toggle: async (vaultPath: string, path: string) => await invoke('favorites_toggle', { vaultPath, path }),
  },
  kanban: {
    getData: async (folderPath: string, vaultPath: string) => await invoke<{ statuses: string[]; cards: any[] }>('kanban_get_data', { folderPath, vaultPath }),
    setStatus: async (folderPath: string, filePath: string, newStatus: string) => await invoke<void>('kanban_set_status', { folderPath, filePath, newStatus }),
  },
  widget: {
    setShortcut: async (shortcut: string) => await invoke<void>('widget_set_shortcut', { shortcut }),
    setDictationProgress: async (progress: number) => await invoke<void>('widget_set_dictation_progress', { progress }),
    setDictationError: async (error: string) => await invoke<void>('widget_set_dictation_error', { error }),
    setDictationComplete: async (result: string) => await invoke<void>('widget_set_dictation_complete', { result }),
  },
  dictation: {
    downloadModel: async (model: string) => await invoke<void>('dictation_download_model', { model }),
    start: async () => await invoke<void>('dictation_start'),
    stop: async () => await invoke<void>('dictation_stop'),
  },
  templates: {
    list: async (vaultPath: string) => await invoke<import('../../../shared/types').Template[]>('templates_list', { vaultPath }),
  },
  properties: {
    write: async (path: string, yaml: string) => await invoke('properties_write', { path, yaml }),
  },
  viewState: {
    setFold: async (vaultPath: string, filePath: string, headingId: string, isOpen: boolean) => await invoke('view_state_set_fold', { vaultPath, filePath, headingId, isOpen }),
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
    activityLog: (callback: (payload: any) => void) => listen('activityLog', (event) => callback(event.payload)),
    widgetModeChanged: (callback: (payload: any) => void) => listen('widgetModeChanged', (event) => callback(event.payload)),
    widgetDictationStarting: (callback: (payload: any) => void) => listen('widgetDictationStarting', (event) => callback(event.payload)),
    widgetDictationComplete: (callback: (payload: any) => void) => listen('widgetDictationComplete', (event) => callback(event.payload)),
    widgetDictationError: (callback: (payload: any) => void) => listen('widgetDictationError', (event) => callback(event.payload)),
  }
};
