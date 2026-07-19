/**
 * ipc.ts — Renderer-side typed IPC wrapper.
 *
 * This module is the SINGLE boundary between the renderer and the preload bridge.
 * It consumes `window.electron` (whose surface is derived from the shared IPC
 * contracts in `src/shared/contracts`) and re-exposes a fully-typed API.
 *
 * The preload layer is a thin bridge that derives every method's parameter and
 * return types from the shared contracts. Two contracts are intentionally weak in
 * Phase 2.1 (`vault:get-current` response is `z.unknown()`; `context:query`
 * error union leaks into the response inference). Those weak spots are resolved
 * HERE, at the only place where the renderer meets the bridge — never inside
 * feature/business code, and never by duplicating contract definitions.
 */

import type { VaultMetadata, SearchResult } from '@shared/types'

/**
 * Canonical shape of a `context:query` response.
 *
 * `ContextSearchResult` (from `@shared/schemas`) is the intended shape, but its
 * inference is polluted by the `ContextQueryContract` error union, producing
 * `ContextSearchResult | ContextSearchResult[]`. We re-state the canonical object
 * shape here ONLY to normalize that weak contract at the renderer boundary — this
 * is not a new contract, it mirrors `ContextSearchResultSchema` exactly.
 */
type ContextQueryResponse = {
  results: SearchResult[]
  disabled?: boolean
  reason?: string
}

/** The preload bridge, already typed from the shared IPC contracts. */
const bridge = window.electron

export const ipc = {
  vault: {
    ...bridge.vault,
    /**
     * `vault:get-current` contract response is `z.unknown()` (Phase 2.1 weak
     * contract). The main handler actually returns `VaultMetadata | null`, so we
     * narrow the bridge's `unknown` to the real shape at this boundary.
     */
    getCurrent(): Promise<VaultMetadata | null> {
      return bridge.vault.getCurrent() as Promise<VaultMetadata | null>
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
    reindex: bridge.context.reindex,
    status: bridge.context.status,
    /**
     * `context:query` contract response inference is a union (the error union of
     * `ContextQueryContract` leaks into the response type). We normalize it to the
     * canonical `ContextSearchResult` shape at this boundary.
     */
    query: (text: string) =>
      bridge.context.query(text) as unknown as Promise<ContextQueryResponse>
  },
  search: bridge.search,
  properties: bridge.properties,
  viewState: bridge.viewState,
  kanban: bridge.kanban,
  clipboardHistory: bridge.clipboardHistory,
  widget: bridge.widget,
  on: bridge.on
}

export type { SearchResult }
