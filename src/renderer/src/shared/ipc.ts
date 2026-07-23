import { tauriBridge as bridge } from './tauri-ipc'
import type { VaultMetadata, SearchResult } from '@shared/types'

type ContextQueryResponse = {
  results: SearchResult[]
  disabled?: boolean
  reason?: string
}

export const ipc = {
  vault: {
    ...bridge.vault,
    getCurrent(): Promise<VaultMetadata | null> {
      return bridge.vault.open() as unknown as Promise<VaultMetadata | null>
    }
  },
  file: bridge.file,
  pdf: bridge.pdf,
  dictation: bridge.dictation,
  folder: bridge.folder,
  note: bridge.note,
  favorites: bridge.favorites,
  templates: bridge.templates,
  settings: bridge.settings,
  task: bridge.task,
  context: {
    reindex: async () => {}, // TODO: Implement if needed
    status: async () => {}, // TODO: Implement if needed
    query: (text: string) =>
      bridge.search.query(text) as unknown as Promise<ContextQueryResponse>
  },
  search: bridge.search,
  on: bridge.on
}

export type { SearchResult }
