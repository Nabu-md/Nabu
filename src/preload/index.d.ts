import { Root } from 'mdast'
import {
  VaultMetadata,
  FileAST,
  FileEntry,
  SearchResult,
  ActivityEntry,
  Edge,
  Template
} from '@shared/types'
import type { SearchResponse, PDFAnnotationType } from '@shared/schemas'

declare global {
  interface Window {
    electron: {
      vault: {
        open(): Promise<VaultMetadata>
        close(vaultId?: string): Promise<{ success: boolean }>
        switch(vaultId: string): Promise<{ success: boolean }>
        getRecents(): Promise<{
          recents: Array<{ path: string; name: string; lastOpened: number }>
        }>
        getCurrent(): Promise<VaultMetadata | null>
        create(parentPath: string, name: string): Promise<VaultMetadata>
        scan(): Promise<VaultMetadata>
        openInNewWindow(path: string): Promise<{ success: boolean; path?: string; error?: string }>
      }
      file: {
        get(path: string, vaultId?: string): Promise<FileAST>
        readAsset(path: string): Promise<{ path: string; dataUri?: string; error?: string }>
      }
      pdf: {
        open(path: string): Promise<{
          totalPages: number
          metadata: { title?: string; author?: string; subject?: string; keywords?: string }
          error?: string
        }>
        renderPage(
          path: string,
          pageNumber: number,
          scale: number
        ): Promise<{
          pageNumber: number
          dataUri: string
          width: number
          height: number
          error?: string
        }>
        loadAnnotations(path: string): Promise<{
          annotations: PDFAnnotationType[]
          error?: string
        }>
        saveAnnotations(
          path: string,
          annotations: PDFAnnotationType[]
        ): Promise<{ success: boolean; error?: string }>
      }
      dictation: {
        start(model?: 'base' | 'large-v3-turbo-q5'): Promise<{ success: boolean; error?: string }>
        stop(): Promise<{
          success: boolean
          result?: { text: string; segments: { start: number; end: number; text: string }[] }
          error?: string
        }>
        status(): Promise<{
          available: boolean
          modelStatus?: {
            model: 'base' | 'large-v3-turbo-q5'
            installed: boolean
            downloading: boolean
            downloadProgress: number
          }
          error?: string
        }>
        downloadModel(
          model: 'base' | 'large-v3-turbo-q5'
        ): Promise<{ success: boolean; error?: string }>
      }
      folder: {
        create(path: string): Promise<{ success: boolean; error?: string }>
      }
      note: {
        create(vaultPath: string, name: string, templateContent?: string): Promise<FileAST>
        save(path: string, content: string): Promise<{ success: boolean; error?: string }>
        rename(oldPath: string, newPath: string): Promise<{ success: boolean; error?: string }>
        delete(path: string): Promise<{ success: boolean; error?: string }>
        getRaw(path: string): Promise<{ path: string; content?: string; error?: string }>
        exportHtml(
          path: string,
          html: string
        ): Promise<{ success: boolean; savedPath?: string; error?: string }>
        daily(
          vaultPath: string
        ): Promise<{ path: string; ast: Root; created: boolean; error?: string }>
      }
      favorites: {
        get(vaultPath: string): Promise<{ favorites: string[] }>
        toggle(vaultPath: string, filePath: string): Promise<{ favorites: string[] }>
        remove(vaultPath: string, filePath: string): Promise<{ favorites: string[] }>
      }
      templates: {
        list(vaultPath: string): Promise<{ templates: Template[] }>
      }
      settings: {
        get(key: string): Promise<{ value?: unknown }>
        set(key: string, value: unknown): Promise<{ success: boolean; error?: string }>
        getFeatureToggles(): Promise<{
          toggles: Array<{ id: string; label: string; description: string; enabled: boolean }>
        }>
        setFeatureToggle(
          id: string,
          enabled: boolean
        ): Promise<{
          success: boolean
          error?: string
        }>
      }
      task: {
        toggle(path: string, lineIndex: number): Promise<void>
      }
      context: {
        query(
          text: string
        ): Promise<
          SearchResult[] | { results: SearchResult[]; disabled?: boolean; reason?: string }
        >
        reindex(vaultPath: string): Promise<{ processed: number; error?: string }>
        status(): Promise<{ disabled: boolean; reason: string | null; items: number }>
      }
      search: {
        query(query: string): Promise<SearchResponse>
      }
      properties: {
        read(
          path: string
        ): Promise<{ path: string; properties: Record<string, unknown>; yaml: string }>
        write(path: string, yaml: string): Promise<{ success: boolean; error?: string }>
      }
      viewState: {
        getFold(
          vaultPath: string,
          notePath: string,
          headingId: string
        ): Promise<boolean>
        setFold(
          vaultPath: string,
          notePath: string,
          headingId: string,
          isOpen: boolean
        ): Promise<void>
      }
      kanban: {
        getData(
          vaultPath: string,
          folderPath: string
        ): Promise<{
          statuses: string[]
          cards: Array<{
            filePath: string
            title: string
            content: string
            tags: string[]
            status: string
          }>
        }>
        setStatus(
          vaultPath: string,
          filePath: string,
          status: string
        ): Promise<{ success: boolean; error?: string }>
      }
      clipboardHistory: {
        get(max: number): Promise<{ entries: Array<{ id: string; text: string; timestamp: number }> }>
        clear(): Promise<void>
        copy(text: string): Promise<{ success: boolean; error?: string }>
      }
      widget: {
        toggle(): Promise<void>
        move(dx: number, dy: number): Promise<void>
        resize(opts: { width: number; height: number }): Promise<void>
        createNote(opts: { name: string; content: string; timestamp?: boolean }): Promise<{ success: boolean; error?: string; path?: string }>
        fetchTitle(url: string): Promise<{ title: string }>
        openNote(path: string): Promise<void>
        setShortcut(shortcut: string): Promise<void>
      }
      on: {
        noteLoaded(callback: (data: { path: string; ast: Root }) => void): () => void
        noteOpenRequested(callback: (data: { path: string }) => void): () => void
        noteUpdated(
          callback: (data: { path: string; ast: Root; isExternal: boolean }) => void
        ): () => void
        noteDeleted(callback: (data: { path: string }) => void): () => void
        contextSearch(callback: (data: unknown) => void): () => void
        activityLog(callback: (entry: ActivityEntry) => void): () => void
        vaultOpened(callback: (data: VaultMetadata) => void): () => void
        notesLoaded(
          callback: (data: { vaultPath?: string; files: FileEntry[] }) => void
        ): () => void
        focusSearch(callback: () => void): () => void
        openSettings(callback: () => void): () => void
        setupCreate(callback: () => void): () => void
        setupOpen(callback: () => void): () => void
        indexBuild(callback: (data: unknown) => void): () => void
        showClipboard(callback: () => void): () => void
        dictationDownloadProgress(
          callback: (data: { model: 'base' | 'large-v3-turbo-q5'; progress: number }) => void
        ): () => void
        widgetModeChanged(callback: (data: { mode: 'clipboard' | 'dictation' }) => void): () => void
        widgetDictationStarting(callback: () => void): () => void
        widgetDictationComplete(
          callback: (data: { text: string; silent: boolean }) => void
        ): () => void
        widgetDictationError(callback: (data: { error: string }) => void): () => void
        widgetInsertText(callback: (data: { text: string }) => void): () => void
      }
    }
  }
}