/**
 * search.ts — Search feature IPC module.
 *
 * Owns context:query, context:reindex, vector:status, search:query, and the
 * Main→Renderer push channels index:build / context:search (via sendToRenderer).
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain } from 'electron'

import { IPCChannel } from '@shared/channels'
import {
  ContextQuerySchema,
  ContextReindexSchema,
  VectorStatusSchema,
  ContextSearchResultSchema,
  VectorStatusResultSchema,
  ContextReindexResultSchema
} from '@shared/schemas'

import type { IPCContext } from './context'
import { emitActivityLog, formatZodError, normalizeError, errorToString } from './shared'

/**
 * Register all search-feature IPC handlers.
 */
export function registerSearchIPC(ctx: IPCContext): void {
  const { stateManager, vectorManager, searchService } = ctx

  // -------------------------------------------------------------------------
  // context:query — perform a semantic similarity search
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.CONTEXT_QUERY, async (_event, rawPayload) => {
    const validation = ContextQuerySchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] context:query validation failed: ${reason}`)
      return { error: reason }
    }

    const { text, excludePath } = validation.data

    try {
      const status = await vectorManager.getStatus()
      if (status.disabled) {
        return {
          results: [],
          disabled: true,
          reason: status.reason ?? 'Embedding model not loaded'
        }
      }
      if (status.items === 0) {
        return {
          results: [],
          disabled: true,
          reason: 'Vector index is empty — save some notes to populate it'
        }
      }
    } catch (err) {
      const normalized = normalizeError(err)
      emitActivityLog(
        'warn',
        `[IPC] context:query status check failed: ${errorToString(normalized)}`
      )
    }

    try {
      const rawResults = await vectorManager.search(text, 5, excludePath)
      return ContextSearchResultSchema.parse({ results: rawResults })
    } catch (err) {
      const normalized = normalizeError(err, { text, excludePath })
      const msg = `[IPC] context:query handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { results: [], error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // context:reindex — trigger full re-embed of all vault files
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.CONTEXT_REINDEX, async (_event, rawPayload) => {
    const validation = ContextReindexSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] context:reindex validation failed: ${reason}`)
      return { error: reason }
    }

    const { vaultPath } = validation.data
    const vault = stateManager.getCurrentVault()
    if (!vault) {
      return { error: 'No vault is open' }
    }
    if (vault.path !== vaultPath) {
      emitActivityLog(
        'warn',
        `[IPC] context:reindex vault path mismatch: "${vaultPath}" !== "${vault.path}"`
      )
      return { error: 'Vault path does not match currently open vault' }
    }

    try {
      const processed = await vectorManager.reindexAll(vault.files)
      return ContextReindexResultSchema.parse({ processed })
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] context:reindex handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // vector:status — return the current vector index status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VECTOR_STATUS, async (_event, rawPayload) => {
    const validation = VectorStatusSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] vector:status validation failed: ${reason}`)
      return { disabled: true, reason }
    }

    try {
      const status = await vectorManager.getStatus()
      return VectorStatusResultSchema.parse(status)
    } catch (err) {
      const normalized = normalizeError(err)
      const msg = `[IPC] vector:status handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { disabled: true, reason: errorToString(normalized), items: 0 }
    }
  })

  // -------------------------------------------------------------------------
  // search:query — execute a text search against the extended search index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SEARCH_QUERY, async (_event, rawPayload) => {
    return searchService.query(rawPayload)
  })
}
