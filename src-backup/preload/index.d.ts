import { z } from 'zod'
import * as C from '@shared/contracts'

// ---------------------------------------------------------------------------
// Preload type declarations (renderer global `window.electron`).
//
// Every exposed method's parameter and return types are DERIVED from the shared
// IPC contracts in `src/shared/contracts`. No interface is duplicated here; the
// renderer consumes the exact request/response shapes defined by the contracts.
// ---------------------------------------------------------------------------

type Req<T extends z.ZodTypeAny> = z.infer<T>
type Res<T extends z.ZodTypeAny> = z.infer<T>

declare global {
  interface Window {
    electron: {
      vault: {
        open(): Promise<Res<typeof C.VaultOpenContract.response>>
        close(vaultId?: string): Promise<Res<typeof C.VaultCloseContract.response>>
        switch(vaultId: string): Promise<Res<typeof C.VaultSwitchContract.response>>
        getRecents(): Promise<Res<typeof C.VaultGetRecentsContract.response>>
        getCurrent(): Promise<Res<typeof C.VaultGetCurrentContract.response>>
        create(parentPath: string, name: string): Promise<Res<typeof C.VaultCreateContract.response>>
        scan(): Promise<Res<typeof C.VaultScanContract.response>>
        openInNewWindow(path: string): Promise<Res<typeof C.VaultOpenInNewWindowContract.response>>
      }
      file: {
        get(path: string, vaultId?: string): Promise<Res<typeof C.FileGetContract.response>>
        readAsset(path: string): Promise<Res<typeof C.AssetReadContract.response>>
      }
      pdf: {
        open(path: string): Promise<Res<typeof C.PDFOpenContract.response>>
        renderPage(
          path: string,
          pageNumber: number,
          scale: number
        ): Promise<Res<typeof C.PDFRenderPageContract.response>>
        loadAnnotations(path: string): Promise<Res<typeof C.PDFLoadAnnotationsContract.response>>
        saveAnnotations(
          path: string,
          annotations: Req<typeof C.PDFSaveAnnotationsContract.request>['annotations']
        ): Promise<Res<typeof C.PDFSaveAnnotationsContract.response>>
      }
      dictation: {
        start(model?: 'base' | 'large-v3-turbo-q5'): Promise<Res<typeof C.DictationStartContract.response>>
        stop(): Promise<Res<typeof C.DictationStopContract.response>>
        status(): Promise<Res<typeof C.DictationStatusContract.response>>
        downloadModel(
          model: 'base' | 'large-v3-turbo-q5'
        ): Promise<Res<typeof C.DictationDownloadModelContract.response>>
      }
      folder: {
        create(path: string): Promise<Res<typeof C.FolderCreateContract.response>>
      }
      note: {
        create(
          vaultPath: string,
          name: string,
          templateContent?: string
        ): Promise<Res<typeof C.NoteCreateContract.response>>
        save(path: string, content: string): Promise<Res<typeof C.NoteSaveContract.response>>
        rename(oldPath: string, newPath: string): Promise<Res<typeof C.NoteRenameContract.response>>
        delete(path: string): Promise<Res<typeof C.NoteDeleteContract.response>>
        getRaw(path: string): Promise<Res<typeof C.NoteGetRawContract.response>>
        exportHtml(
          path: string,
          html: string
        ): Promise<Res<typeof C.NoteExportHtmlContract.response>>
        daily(vaultPath: string): Promise<Res<typeof C.NoteDailyContract.response>>
      }
      favorites: {
        get(vaultPath: string): Promise<Res<typeof C.FavoritesGetContract.response>>
        toggle(
          vaultPath: string,
          filePath: string
        ): Promise<Res<typeof C.FavoritesToggleContract.response>>
        remove(
          vaultPath: string,
          filePath: string
        ): Promise<Res<typeof C.FavoritesRemoveContract.response>>
      }
      templates: {
        list(vaultPath: string): Promise<Res<typeof C.TemplatesListContract.response>>
      }
      settings: {
        get(key: string): Promise<Res<typeof C.SettingsGetContract.response>>
        set(key: string, value: unknown): Promise<Res<typeof C.SettingsSetContract.response>>
        getFeatureToggles(): Promise<Res<typeof C.SettingsGetFeatureTogglesContract.response>>
        setFeatureToggle(
          id: string,
          enabled: boolean
        ): Promise<Res<typeof C.SettingsSetFeatureToggleContract.response>>
      }
      task: {
        toggle(path: string, lineIndex: number): Promise<Res<typeof C.TaskToggleContract.response>>
      }
      context: {
        query(text: string): Promise<Res<typeof C.ContextQueryContract.response>>
        reindex(vaultPath: string): Promise<Res<typeof C.ContextReindexContract.response>>
        status(): Promise<Res<typeof C.VectorStatusContract.response>>
      }
      search: {
        query(query: string): Promise<Res<typeof C.SearchQueryContract.response>>
      }
      properties: {
        read(path: string): Promise<Res<typeof C.PropertiesReadContract.response>>
        write(path: string, yaml: string): Promise<Res<typeof C.PropertiesWriteContract.response>>
      }
      viewState: {
        getFold(
          vaultPath: string,
          notePath: string,
          headingId: string
        ): Promise<Res<typeof C.ViewStateGetFoldContract.response>>
        setFold(
          vaultPath: string,
          notePath: string,
          headingId: string,
          isOpen: boolean
        ): Promise<Res<typeof C.ViewStateSetFoldContract.response>>
      }
      kanban: {
        getData(
          vaultPath: string,
          folderPath: string
        ): Promise<Res<typeof C.KanbanGetDataContract.response>>
        setStatus(
          vaultPath: string,
          filePath: string,
          status: string
        ): Promise<Res<typeof C.KanbanSetStatusContract.response>>
      }
      clipboardHistory: {
        get(max: number): Promise<Res<typeof C.ClipboardHistoryGetContract.response>>
        clear(): Promise<Res<typeof C.ClipboardHistoryClearContract.response>>
        copy(text: string): Promise<Res<typeof C.ClipboardHistoryCopyContract.response>>
      }
      widget: {
        setShortcut(shortcut: string): Promise<Res<typeof C.WidgetSetShortcutContract.response>>
      }
      on: {
        noteLoaded(
          callback: (data: Res<typeof C.NoteLoadedContract.response>) => void
        ): () => void
        noteOpenRequested(callback: (data: { path: string }) => void): () => void
        noteUpdated(
          callback: (data: Res<typeof C.NoteUpdatedContract.response>) => void
        ): () => void
        noteDeleted(
          callback: (data: Res<typeof C.NoteDeletedContract.response>) => void
        ): () => void
        contextSearch(
          callback: (data: Res<typeof C.ContextSearchContract.response>) => void
        ): () => void
        activityLog(
          callback: (data: Res<typeof C.ActivityLogContract.response>) => void
        ): () => void
        vaultOpened(
          callback: (data: Res<typeof C.VaultOpenedContract.response>) => void
        ): () => void
        notesLoaded(
          callback: (data: Res<typeof C.NotesLoadedContract.response>) => void
        ): () => void
        focusSearch(callback: () => void): () => void
        openSettings(callback: () => void): () => void
        setupCreate(callback: () => void): () => void
        setupOpen(callback: () => void): () => void
        showClipboard(callback: () => void): () => void
        indexBuild(
          callback: (data: Res<typeof C.IndexBuildContract.response>) => void
        ): () => void
        dictationDownloadProgress(
          callback: (data: Res<typeof C.DictationDownloadProgressContract.response>) => void
        ): () => void
        widgetModeChanged(callback: (data: { mode: 'clipboard' | 'dictation' }) => void): () => void
        widgetDictationStarting(callback: (data: unknown) => void): () => void
        widgetDictationComplete(callback: (data: { text: string; silent: boolean }) => void): () => void
        widgetDictationError(callback: (data: { error: string }) => void): () => void
        widgetInsertText(callback: (data: { text: string }) => void): () => void
      }
    }
  }
}
