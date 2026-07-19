/**
 * vault.ts — Vault feature IPC module.
 *
 * Owns all vault-related IPC channels plus the closely-related file, folder,
 * favorites, and bookmarks channels.
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. Handler behavior is unchanged.
 */

import { ipcMain } from 'electron'
import fs from 'fs/promises'

import { IPCChannel } from '@shared/channels'
import {
  FileGetSchema,
  FileGetResultSchema,
  FolderCreateSchema,
  FavoritesGetSchema,
  FavoritesToggleSchema,
  FavoritesRemoveSchema
} from '@shared/schemas'

import { readFavorites, toggleFavorite, removeFavorite } from '../favorites'
import { VaultService } from '../services/vault-service'

import type { IPCContext } from './context'
import {
  emitActivityLog,
  formatZodError,
  getSessionForVault,
  normalizeError,
  errorToString
} from './shared'

/**
 * Register all vault-feature IPC handlers.
 */
export function registerVaultIPC(ctx: IPCContext): void {
  const { stateManager, vectorManager, watcher } = ctx
  const vaultService = new VaultService(stateManager, vectorManager, watcher)

  // -------------------------------------------------------------------------
  // vault:get-current — renderer pulls current vault state on mount
  // -------------------------------------------------------------------------
  ipcMain.removeHandler('vault:get-current')
  ipcMain.handle('vault:get-current', async (_event) => {
    return vaultService.getCurrentVault()
  })

  // -------------------------------------------------------------------------
  // vault:open — open a vault by path, or prompt with native folder picker
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN, async (_event, rawPayload) => {
    const result = await vaultService.openVault(rawPayload ?? {})
    if (result.error) return { error: result.error }
    if (result.canceled) return { canceled: true }
    return result.vault
  })

  // -------------------------------------------------------------------------
  // vault:scan — re-scan the current vault and return updated metadata
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_SCAN, async (_event, _rawPayload) => {
    const result = await vaultService.scanVault()
    if (result.error) return { error: result.error }
    return result.vault
  })

  // -------------------------------------------------------------------------
  // vault:close — stop the watcher and release vault state
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CLOSE, async (_event, rawPayload) => {
    return vaultService.closeVault(rawPayload)
  })

  // -------------------------------------------------------------------------
  // vault:open-in-new-window — open vault in a second BrowserWindow
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN_IN_NEW_WINDOW, async (_event, rawPayload) => {
    return vaultService.openVaultInNewWindow(rawPayload)
  })

  // -------------------------------------------------------------------------
  // vault:switch — switch to a different vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_SWITCH, async (_event, rawPayload) => {
    return vaultService.switchVault(rawPayload)
  })

  // -------------------------------------------------------------------------
  // vault:get-recents — get list of recently opened vaults
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_GET_RECENTS, async (_event, _rawPayload) => {
    return vaultService.getRecents()
  })

  // -------------------------------------------------------------------------
  // vault:create — create a new vault directory and open it
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CREATE, async (_event, rawPayload) => {
    const result = await vaultService.createVault(rawPayload)
    if (result.error) return { error: result.error }
    return result.vault
  })

  // -------------------------------------------------------------------------
  // file:get — return the parsed AST for a given file path
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FILE_GET, async (_event, rawPayload) => {
    const validation = FileGetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] file:get validation failed: ${reason}`)
      return { error: reason }
    }

    const { path: filePath, vaultId } = validation.data

    try {
      const { stateManager: sm } = getSessionForVault(vaultId)
      const ast = await sm.getAST(filePath)
      const response = FileGetResultSchema.parse({ path: filePath, ast })
      return response
    } catch (err) {
      const normalized = normalizeError(err, { path: filePath })
      const msg = `[IPC] file:get handler error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return {
        path: filePath,
        ast: null,
        error: {
          line: 0,
          column: 0,
          message: errorToString(normalized)
        }
      }
    }
  })

  // -------------------------------------------------------------------------
  // file:watch — acknowledge a watch request for a specific file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FILE_WATCH, async (_event, rawPayload) => {
    const validation = FileGetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] file:watch validation failed: ${reason}`)
      return { error: reason }
    }

    // The VaultWatcher already watches the entire vault directory, so
    // individual file watch requests are acknowledged without additional action.
    return { success: true, path: validation.data.path }
  })

  // -------------------------------------------------------------------------
  // folder:create — create a new folder inside the vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FOLDER_CREATE, async (_event, rawPayload) => {
    const validation = FolderCreateSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] folder:create validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: folderPath } = validation.data

    try {
      await fs.mkdir(folderPath, { recursive: true })
      return { success: true }
    } catch (err) {
      const normalized = normalizeError(err, { path: folderPath })
      const msg = `[IPC] folder:create handler error for "${folderPath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // favorites:get — get favorites list for a vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FAVORITES_GET, async (_event, rawPayload) => {
    const validation = FavoritesGetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] favorites:get validation failed: ${reason}`)
      return { favorites: [] }
    }
    const { vaultPath } = validation.data
    try {
      const favorites = await readFavorites(vaultPath)
      return { favorites }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] favorites:get error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { favorites: [] }
    }
  })

  // -------------------------------------------------------------------------
  // favorites:toggle — toggle a file's favorite state
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FAVORITES_TOGGLE, async (_event, rawPayload) => {
    const validation = FavoritesToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] favorites:toggle validation failed: ${reason}`)
      return { favorites: [] }
    }
    const { vaultPath, filePath } = validation.data
    try {
      const favorites = await toggleFavorite(vaultPath, filePath)
      return { favorites }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, filePath })
      const msg = `[IPC] favorites:toggle error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { favorites: [] }
    }
  })

  // -------------------------------------------------------------------------
  // favorites:remove — remove a file from favorites
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.FAVORITES_REMOVE, async (_event, rawPayload) => {
    const validation = FavoritesRemoveSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] favorites:remove validation failed: ${reason}`)
      return { favorites: [] }
    }
    const { vaultPath, filePath } = validation.data
    try {
      const favorites = await removeFavorite(vaultPath, filePath)
      return { favorites }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, filePath })
      const msg = `[IPC] favorites:remove error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { favorites: [] }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:get — get bookmarks for a vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_GET, async (_event, rawPayload) => {
    const vaultPath =
      typeof rawPayload === 'object' && rawPayload !== null ? (rawPayload as any).vaultPath : ''
    if (!vaultPath) {
      return { bookmarks: {} }
    }

    try {
      const { readBookmarks } = await import('../bookmarks')
      const bookmarks = await readBookmarks(vaultPath)
      return { bookmarks }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath })
      const msg = `[IPC] bookmarks:get handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:add — add a bookmark to a list
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_ADD, async (_event, rawPayload) => {
    const { vaultPath, listName, filePath } = (rawPayload ?? {}) as {
      vaultPath?: string
      listName?: string
      filePath?: string
    }
    if (!vaultPath || !listName || !filePath) {
      return { bookmarks: {} }
    }

    try {
      const { addBookmark } = await import('../bookmarks')
      const bookmarks = await addBookmark(vaultPath, listName, filePath)
      return { bookmarks }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, listName, filePath })
      const msg = `[IPC] bookmarks:add handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:remove — remove a bookmark from a list
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_REMOVE, async (_event, rawPayload) => {
    const { vaultPath, listName, filePath } = (rawPayload ?? {}) as {
      vaultPath?: string
      listName?: string
      filePath?: string
    }
    if (!vaultPath || !listName || !filePath) {
      return { bookmarks: {} }
    }

    try {
      const { removeBookmark } = await import('../bookmarks')
      const bookmarks = await removeBookmark(vaultPath, listName, filePath)
      return { bookmarks }
    } catch (err) {
      const normalized = normalizeError(err, { vaultPath, listName, filePath })
      const msg = `[IPC] bookmarks:remove handler error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })
}
