import { tauriBridge as bridge } from './tauri-ipc'
import type { VaultMetadata, SearchResult } from '@shared/types'

type ContextQueryResponse = {
  results: SearchResult[]
  disabled?: boolean
  reason?: string
}

export const ipc = {
  vault: bridge.vault,
  file: bridge.file,
  pdf: bridge.pdf,
  dictation: bridge.dictation,
  note: bridge.note,
  favorites: bridge.favorites,
  settings: bridge.settings,
  task: bridge.task,
  kanban: bridge.kanban,
  templates: bridge.templates,
  widget: bridge.widget,
  properties: bridge.properties,
  viewState: bridge.viewState,
  context: {
    reindex: async () => {}, // TODO
    status: async () => {}, // TODO
    query: (text: string) =>
      bridge.search.query(text) as unknown as Promise<ContextQueryResponse>
  },
  search: bridge.search,
  on: bridge.on
}

export type { SearchResult }
