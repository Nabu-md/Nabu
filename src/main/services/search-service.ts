/**
 * search-service.ts
 *
 * SearchService — owns search orchestration, indexing coordination, and search
 * execution against the extended search index.
 *
 * This service extracts the search business logic that was previously embedded
 * inside `ipc.ts` (search:query handler). The IPC layer now delegates to this
 * service, leaving behind a thin wrapper.
 *
 * This is a pure extraction: no behavior is redesigned, improved, or changed.
 *
 * Requirements: 13.1, 13.2, 13.3
 */

import { SearchQuerySchema, SearchResponseSchema } from '@shared/schemas'
import { search } from '@shared/search-query'
import { emitActivityLog, formatZodError } from '../ipc/shared'

import type { StateManager } from './state'

// ---------------------------------------------------------------------------
// SearchService
// ---------------------------------------------------------------------------

/**
 * Owns all search business logic.
 *
 * The service is constructed with the StateManager that owns the extended
 * search index and the parsed ASTs used for search execution.
 */
export class SearchService {
  private stateManager: StateManager

  constructor(stateManager: StateManager) {
    this.stateManager = stateManager
  }

  /**
   * Execute a text search against the extended search index.
   * Mirrors the previous `search:query` IPC handler logic exactly.
   */
  query(rawPayload: unknown): { results: unknown[] } {
    const validation = SearchQuerySchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[SearchService] search:query validation failed: ${reason}`)
      return { results: [] }
    }

    const { query } = validation.data
    const vault = this.stateManager.getCurrentVault()
    if (!vault) {
      return { results: [] }
    }

    try {
      const results = search(
        query,
        vault.files,
        vault.path,
        this.stateManager.getExtendedIndex(),
        (p) => this.stateManager.getASTSync(p)
      )
      return SearchResponseSchema.parse({ results })
    } catch (err) {
      const msg = `[SearchService] search:query handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { results: [] }
    }
  }
}
