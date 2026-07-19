/**
 * ipc.ts
 *
 * IPC Handler Registration — registers all Renderer→Main `ipcMain.handle()`
 * channels with Zod validation, and provides `sendToRenderer()` for
 * Main→Renderer push messages.
 *
 * Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 16.1, 16.2, 16.3, 16.4, 16.6, 22.3, 22.9
 */

import { ipcMain, dialog, BrowserWindow } from 'electron'
import { ZodError } from 'zod'
import path from 'path'
import fs from 'fs/promises'

import { IPCChannel } from '../shared/channels'
import {
  // Incoming schemas (Renderer → Main)
  FileGetSchema,
  TaskToggleSchema,
  ContextQuerySchema,
  ContextReindexSchema,
  VectorStatusSchema,
  ActivityLogSchema,
  SettingsGetSchema,
  SettingsSetSchema,
  FeatureTogglesResultSchema,
  SetFeatureToggleSchema,
  SetFeatureToggleResultSchema,
  FolderCreateSchema,
  NoteCreateSchema,
  NoteSaveSchema,
  NoteRenameSchema,
  NoteDeleteSchema,
  NoteGetRawSchema,
  NoteExportHtmlSchema,
  TemplatesListSchema,
  // Outgoing schemas (Main → Renderer)
  FileGetResultSchema,
  TaskToggleResultSchema,
  ContextSearchResultSchema,
  ContextReindexResultSchema,
  VectorStatusResultSchema,
  NoteLoadedSchema,
  NoteUpdatedSchema,
  NoteDeletedSchema,
  NotesLoadedSchema,
  TemplatesListResultSchema,
  IndexBuildSchema,
  AssetReadSchema,
  PropertiesReadSchema,
  PropertiesReadResultSchema,
  PropertiesWriteSchema,
  PropertiesWriteResultSchema,
  NoteDailySchema,
  NoteDailyResultSchema,
  NoteRandomSchema,
  NoteRandomResultSchema,
  ViewStateGetFoldSchema,
  ViewStateSetFoldSchema,
  FavoritesGetSchema,
  FavoritesToggleSchema,
  FavoritesRemoveSchema,
  // Kanban schemas
  KanbanGetDataSchema,
  KanbanGetDataResultSchema,
  KanbanSetStatusSchema,
  KanbanSetStatusResultSchema,
  // Additional schemas
  NoteComposeSchema,
  NoteComposeResultSchema,
  NoteUniqueSchema,
  NoteUniqueResultSchema
} from '../shared/schemas'

import { loadSettings, saveSettings } from './services/settings'
import { substituteVariables } from './services/templates'
import { readFavorites, toggleFavorite, removeFavorite } from './favorites'
import { vaultRegistry } from './services/vault-registry'
import { enqueueOCR, createOCRCompanionNote } from './services/ocr-manager'
import { setFoldState, loadViewState } from './services/view-state'
import { readBookmarks, addBookmark, removeBookmark } from './bookmarks'
import { mergeNotes } from './services/composer'
import { generateUniqueNoteName } from './services/unique-note'
import { VaultService } from './services/vault-service'
import { SearchService } from './services/search-service'
import { PdfService } from './services/pdf-service'
import { DictationService } from './services/dictation-service'

import type { StateManager } from './services/state'
import type { VectorManager } from './services/vector'
import type { VaultWatcher, WatcherConfig } from './services/watcher'

// ---------------------------------------------------------------------------
// Legacy singleton managers — used for backward compatibility during migration
// ---------------------------------------------------------------------------

let legacyStateManager: StateManager | null = null
let legacyVectorManager: VectorManager | null = null

/**
 * Set the legacy singleton managers for backward compatibility.
 * Called from index.ts on app initialization.
 */
export function setLegacyManagers(
  stateManager: StateManager,
  vectorManager: VectorManager,
  _watcher: VaultWatcher
): void {
  legacyStateManager = stateManager
  legacyVectorManager = vectorManager
}

// ---------------------------------------------------------------------------
// Widget toggle callback — bridge between feature toggle IPC and WidgetManager
// ---------------------------------------------------------------------------

let widgetToggleCallback: ((enabled: boolean) => void) | null = null

/**
 * Register a callback that fires when the clipboard-widget feature toggle
 * changes. Called from index.ts after WidgetManager is created.
 */
export function onWidgetToggle(callback: (enabled: boolean) => void): void {
  widgetToggleCallback = callback
}

// ---------------------------------------------------------------------------
// Internal helpers to get managers from registry or legacy singletons
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// vaultId dispatch helper
// ---------------------------------------------------------------------------

/**
 * Get the appropriate session managers for the given vaultId.
 * If vaultId is omitted, returns the active vault's managers.
 * Falls back to legacy singletons during v1→v2 migration.
 * If the specified vault is not open, throws an error.
 *
 * Requirements: 22.3, 22.9
 */
function getSessionForVault(vaultId: string | undefined): {
  stateManager: StateManager
  vectorManager: VectorManager
  vaultPath: string | null
} {
  // Try the vault registry first
  const session = vaultRegistry.get(vaultId)
  if (session) {
    return {
      stateManager: session.stateManager as unknown as StateManager,
      vectorManager: session.vectorManager as unknown as VectorManager,
      vaultPath: session.vaultPath
    }
  }

  // Fallback to legacy singletons (v1 compatibility during migration)
  if (legacyStateManager && legacyVectorManager) {
    return {
      stateManager: legacyStateManager,
      vectorManager: legacyVectorManager,
      vaultPath: legacyStateManager.getCurrentVault()?.path ?? null
    }
  }

  // Try to get the active session (in case vaultId was omitted)
  const activeSession = vaultRegistry.getActive()
  if (activeSession) {
    return {
      stateManager: activeSession.stateManager as unknown as StateManager,
      vectorManager: activeSession.vectorManager as unknown as VectorManager,
      vaultPath: activeSession.vaultPath
    }
  }

  throw new Error('No vault is currently open')
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build a structured activity:log payload and broadcast it to all renderer
 * windows. Used internally for validation warnings and handler errors.
 */
export function emitActivityLog(level: 'info' | 'warn' | 'error', message: string): void {
  const payload = ActivityLogSchema.safeParse({
    level,
    message,
    timestamp: Date.now()
  })

  if (!payload.success) return // shouldn't happen with literal inputs

  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(IPCChannel.ACTIVITY_LOG, payload.data)
    }
  }
}

/**
 * Format a Zod validation error into a short readable string suitable for
 * an activity:log message.
 */
export function formatZodError(err: ZodError): string {
  return err.issues.map((e) => `${e.path.join('.')}: ${e.message}`).join('; ')
}

// ---------------------------------------------------------------------------
// Frontmatter helpers
// ---------------------------------------------------------------------------

/** Regex to match YAML frontmatter delimiters. */
const FRONTMATTER_RE = /^---\n[\s\S]*?\n---(?:\n|$)/

/** Result of extracting frontmatter from raw content. */
interface FrontmatterResult {
  yaml: string // raw YAML string (without delimiters)
  parsed: Record<string, unknown> // parsed YAML object
}

/**
 * Extract YAML frontmatter from raw markdown content.
 * Returns the raw YAML string and parsed object, or empty values if no frontmatter exists.
 */
function extractFrontmatter(content: string): FrontmatterResult {
  const match = content.match(FRONTMATTER_RE)
  if (!match) {
    return { yaml: '', parsed: {} }
  }

  const yamlStr = match[0].replace(/^---\n/, '').replace(/\n---(?:\n|$)/, '')

  try {
    // Use dynamic import for the ESM-compatible yaml package

    const { parse } = require('yaml')
    const parsed = parse(yamlStr)
    return {
      yaml: yamlStr,
      parsed:
        typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
          ? (parsed as Record<string, unknown>)
          : {}
    }
  } catch {
    return { yaml: yamlStr, parsed: {} }
  }
}

/**
 * Replace the YAML frontmatter section in raw markdown content.
 * If the content has no frontmatter, prepend one.
 * If yaml is empty, remove the frontmatter section entirely.
 */
function replaceFrontmatterRaw(raw: string, yamlStr: string): string {
  if (!yamlStr.trim()) {
    return raw.replace(FRONTMATTER_RE, '')
  }

  const yamlBlock = `---\n${yamlStr.trim()}\n---\n`

  if (FRONTMATTER_RE.test(raw)) {
    return raw.replace(FRONTMATTER_RE, yamlBlock)
  }

  // No existing frontmatter — prepend
  return yamlBlock + raw
}

/**
 * Inject or update a single frontmatter property into raw markdown content.
 *
 * When `onlyIfAbsent` is true (e.g. for `created`), the value is only set if
 * the key does not already exist — preserving user-set values (Req 16.3).
 * When `onlyIfAbsent` is false (e.g. for `modified`), the value is always
 * written, overwriting any existing value.
 *
 * Uses `extractFrontmatter` + `replaceFrontmatterRaw` to splice into content.
 * If no frontmatter exists, a minimal one is created.
 *
 * Requirements: 16.1, 16.2, 16.3, 16.4
 */
function injectAutoProperty(
  content: string,
  key: string,
  value: string,
  onlyIfAbsent: boolean
): string {
  const { parsed } = extractFrontmatter(content)

  if (onlyIfAbsent && key in parsed) {
    // Key already set by user — do not overwrite (Req 16.3)
    return content
  }

  // Set or update the property
  const updated = { ...parsed, [key]: value }

  // Use dynamic import for the ESM-compatible yaml package (same pattern as extractFrontmatter)

  const { stringify } = require('yaml')
  const newYaml = stringify(updated)
  return replaceFrontmatterRaw(content, newYaml)
}

// ---------------------------------------------------------------------------
// buildWatcherConfig
// ---------------------------------------------------------------------------

/**
 * Build a consolidated watcher configuration with vector embedding wired into
 * the add/change/delete callbacks.
 *
 * This function replaces the three previously-duplicated watcher callback sites
 * in `ipc.ts` (vault:open) and `index.ts` (restoreVault, NABU_TEST_VAULT).
 *
 * Vector embedding behaviour:
 * - onFileChanged: re-parses the file, pushes the updated AST to the renderer,
 *   then embeds the changed content (only for external edits — the watcher's
 *   internal `handleChange` already skips when `Pending_Write_Lock` is set).
 * - onFileAdded: pushes the updated file list, then reads and embeds the new
 *   file (guarded with `Pending_Write_Lock`).
 * - onFileDeleted: removes the file's vector from the Vectra index.
 *
 * Requirements: 1.1, 1.3, 1.9
 */
export function buildWatcherConfig(
  stateManager: StateManager,
  vectorManager: VectorManager,
  vaultPath: string,
  vaultMeta: { files: import('../shared/types').FileEntry[] }
): WatcherConfig {
  return {
    vaultPath,
    ignored: /^\.|\.nabu/,
    awaitWriteFinish: { stabilityThreshold: 50 },

    onFileChanged: async (filePath: string, isExternal: boolean) => {
      // Re-parse and push update to the renderer
      stateManager.invalidateAST(filePath)
      try {
        const ast = await stateManager.getAST(filePath)
        sendToRenderer(IPCChannel.NOTE_UPDATED, { path: filePath, ast, isExternal })

        // Embed the changed file. The watcher's handleChange already skips
        // when Pending_Write_Lock is set (requirement 1.9), but we guard here
        // as a belt-and-suspenders measure.
        if (!stateManager.hasPendingWrite(filePath)) {
          try {
            const content = await fs.readFile(filePath, 'utf-8')
            vectorManager.embedFile(filePath, content)
          } catch (embedErr) {
            emitActivityLog(
              'error',
              `[IPC] Failed to read file for embedding "${filePath}": ${String(embedErr)}`
            )
          }
        }
      } catch (err) {
        emitActivityLog('error', `[IPC] Failed to re-parse "${filePath}": ${String(err)}`)
      }
    },

    onFileAdded: async (filePath: string) => {
      // Push the updated file list to the renderer
      sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath, files: vaultMeta.files })

      // Embed the new file (guard with Pending_Write_Lock — app-created files
      // set the lock before writing)
      if (!stateManager.hasPendingWrite(filePath)) {
        try {
          const content = await fs.readFile(filePath, 'utf-8')
          vectorManager.embedFile(filePath, content)
        } catch (embedErr) {
          emitActivityLog(
            'error',
            `[IPC] Failed to read new file for embedding "${filePath}": ${String(embedErr)}`
          )
        }
      }
    },

    onFileDeleted: (filePath: string) => {
      // Remove from the vector index (async, non-blocking)
      vectorManager.removeFile(filePath).catch((err) => {
        emitActivityLog('error', `[IPC] Failed to remove vector for "${filePath}": ${String(err)}`)
      })
      // Notify the renderer
      sendToRenderer(IPCChannel.NOTE_DELETED, { path: filePath })
    },

    onImageAdded: async (filePath: string) => {
      // OCR processing (Req 39.2) - process image and create companion note
      try {
        const ocrResult = await enqueueOCR(filePath, vaultPath)
        if (ocrResult) {
          // Create companion .ocr.md note
          const companionPath = await createOCRCompanionNote(filePath, ocrResult, vaultPath)
          if (companionPath) {
            // Index the companion note - trigger incremental index update
            try {
              const indexResult = await (stateManager as any).updateIndexesForFile?.(companionPath)
              if (indexResult) {
                sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
              }
            } catch {
              // updateIndexesForFile not yet available — silently ignore
            }
          }
        }
      } catch (ocrErr) {
        // Graceful degradation - log but don't fail (Req 39.6)
        console.debug(`[OCR] Failed for image ${filePath}: ${String(ocrErr)}`)
      }
    },

    onError: (error: Error) => {
      emitActivityLog('error', `[IPC] Watcher error: ${error.message}`)
    }
  }
}

// ---------------------------------------------------------------------------
// sendToRenderer
// ---------------------------------------------------------------------------

/**
 * Schema map for outgoing Main→Renderer channels.
 * Used by `sendToRenderer` to validate payloads before dispatch.
 */
const outgoingSchemas: Partial<
  Record<
    IPCChannel,
    { safeParse: (data: unknown) => { success: boolean; data?: unknown; error?: ZodError } }
  >
> = {
  [IPCChannel.NOTE_LOADED]: NoteLoadedSchema,
  [IPCChannel.NOTE_UPDATED]: NoteUpdatedSchema,
  [IPCChannel.NOTE_DELETED]: NoteDeletedSchema,
  [IPCChannel.NOTES_LOADED]: NotesLoadedSchema,
  [IPCChannel.CONTEXT_SEARCH]: ContextSearchResultSchema,
  [IPCChannel.ACTIVITY_LOG]: ActivityLogSchema,
  [IPCChannel.INDEX_BUILD]: IndexBuildSchema
}

/**
 * Send a validated payload from the main process to all renderer windows on
 * the given channel.
 *
 * - Validates the payload against the channel's Zod schema before sending.
 * - On validation failure: logs a warning to activity:log, does not send.
 * - Channels not present in `outgoingSchemas` are ignored silently (Req 13.5).
 *
 * Requirements: 13.4, 13.5
 */
export function sendToRenderer(channel: IPCChannel, payload: unknown): void {
  const schema = outgoingSchemas[channel]

  // Silently ignore undeclared outgoing channels (Req 13.5)
  if (!schema) return

  const result = schema.safeParse(payload)

  if (!result.success) {
    const reason = result.error ? formatZodError(result.error as ZodError) : 'unknown'
    const msg = `[IPC] sendToRenderer validation failed on channel "${channel}": ${reason}`
    console.warn(msg)
    emitActivityLog('warn', msg)
    return
  }

  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(channel, result.data)
    }
  }
}

// ---------------------------------------------------------------------------
// registerIPCHandlers
// ---------------------------------------------------------------------------

/**
 * Register all IPC `ipcMain.handle()` channels for Renderer→Main invocations.
 *
 * Each handler:
 * 1. Parses the raw payload through the appropriate Zod schema.
 * 2. Executes the handler logic.
 * 3. Returns a validated response.
 *
 * Validation failures and handler errors are caught, logged to activity:log,
 * and a structured error response is returned so the renderer is never left
 * awaiting a rejected promise without context.
 *
 * Requirements: 13.1, 13.2, 13.3, 13.6
 */
export function registerIPCHandlers(
  stateManager: StateManager,
  vectorManager: VectorManager,
  watcher: VaultWatcher
): void {
  // Remove any previously registered handlers to avoid "handler already
  // registered" errors on hot-reload or second-window initialization.
  const channels = [
    IPCChannel.VAULT_OPEN,
    IPCChannel.VAULT_OPEN_IN_NEW_WINDOW,
    IPCChannel.VAULT_SCAN,
    IPCChannel.VAULT_CLOSE,
    IPCChannel.FILE_GET,
    IPCChannel.FILE_WATCH,
    IPCChannel.TASK_TOGGLE,
    IPCChannel.NOTE_TOGGLE,
    IPCChannel.CONTEXT_QUERY,
    IPCChannel.ACTIVITY_LOG,
    IPCChannel.SETTINGS_GET,
    IPCChannel.SETTINGS_SET,
    IPCChannel.VAULT_CREATE,
    IPCChannel.FOLDER_CREATE,
    IPCChannel.NOTE_CREATE,
    IPCChannel.NOTE_SAVE,
    IPCChannel.NOTE_RENAME,
    IPCChannel.NOTE_DELETE,
    IPCChannel.NOTE_GET_RAW,
    IPCChannel.KANBAN_GET_DATA,
    IPCChannel.KANBAN_SET_STATUS,
    IPCChannel.NOTE_EXPORT_HTML,
    IPCChannel.TEMPLATES_LIST,
    IPCChannel.ASSET_READ,
    IPCChannel.CONTEXT_REINDEX,
    IPCChannel.VECTOR_STATUS,
    IPCChannel.SEARCH_QUERY,
    IPCChannel.PROPERTIES_READ,
    IPCChannel.PROPERTIES_WRITE,
    IPCChannel.VAULT_SWITCH,
    IPCChannel.VAULT_GET_RECENTS,
    IPCChannel.VIEW_STATE_GET_FOLD,
    IPCChannel.VIEW_STATE_SET_FOLD,
    IPCChannel.BOOKMARKS_GET,
    IPCChannel.BOOKMARKS_ADD,
    IPCChannel.BOOKMARKS_REMOVE,
    IPCChannel.NOTE_COMPOSE,
    IPCChannel.NOTE_UNIQUE,
    'vault:get-current' as IPCChannel
  ]
  for (const ch of channels) {
    ipcMain.removeHandler(ch)
  }

  // -------------------------------------------------------------------------
  // Service instantiation — business logic now lives in focused services.
  // The handlers below are thin wrappers that delegate to these services.
  // -------------------------------------------------------------------------
  const vaultService = new VaultService(stateManager, vectorManager, watcher)
  const searchService = new SearchService(stateManager)
  const pdfService = new PdfService()
  const dictationService = new DictationService()

  // -------------------------------------------------------------------------
  // vault:get-current — renderer pulls current vault state on mount
  // -------------------------------------------------------------------------
  ipcMain.removeHandler('vault:get-current')
  ipcMain.handle('vault:get-current', async (_event) => {
    return vaultService.getCurrentVault()
  })

  // -------------------------------------------------------------------------
  // vault:open — open a vault by path, or prompt with native folder picker
  // Requirements: 22.5, 22.6
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
  // Requirements: 22.5, 22.6
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CLOSE, async (_event, rawPayload) => {
    return vaultService.closeVault(rawPayload)
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
      const { stateManager } = getSessionForVault(vaultId)
      const ast = await stateManager.getAST(filePath)
      const response = FileGetResultSchema.parse({ path: filePath, ast })
      return response
    } catch (err) {
      const msg = `[IPC] file:get handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return {
        path: filePath,
        ast: null,
        error: {
          line: 0,
          column: 0,
          message: String(err)
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
  // task:toggle — toggle a checkbox at the given line index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TASK_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] task:toggle validation failed: ${reason}`)
      return TaskToggleResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, lineIndex, vaultId } = validation.data

    try {
      const { stateManager } = getSessionForVault(vaultId)
      await stateManager.toggleTask(filePath, lineIndex)
      return TaskToggleResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[IPC] task:toggle handler error for "${filePath}" line ${lineIndex}: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return TaskToggleResultSchema.parse({ success: false, error: String(err) })
    }
  })

  // -------------------------------------------------------------------------
  // note:toggle — toggle a note-level item (same mechanism as task:toggle)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_TOGGLE, async (_event, rawPayload) => {
    const validation = TaskToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:toggle validation failed: ${reason}`)
      return TaskToggleResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, lineIndex } = validation.data

    try {
      await stateManager.toggleTask(filePath, lineIndex)
      return TaskToggleResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[IPC] note:toggle handler error for "${filePath}" line ${lineIndex}: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return TaskToggleResultSchema.parse({ success: false, error: String(err) })
    }
  })

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

    // Check vector index status before searching. If disabled or empty, return
    // an honest `disabled` flag so the renderer surfaces a clear message
    // instead of silently showing no results (Requirement 1.7).
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
      emitActivityLog('warn', `[IPC] context:query status check failed: ${String(err)}`)
      // Fall through to search — let it fail normally if there's a real problem
    }

    try {
      const rawResults = await vectorManager.search(text, 5, excludePath)
      return ContextSearchResultSchema.parse({ results: rawResults })
    } catch (err) {
      const msg = `[IPC] context:query handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { results: [], error: String(err) }
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
    // Verify vault path matches the open vault
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
      const msg = `[IPC] context:reindex handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // vector:status — return the current vector index status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VECTOR_STATUS, async (_event, rawPayload) => {
    // Validate payload (empty schema — just ensures correctness)
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
      const msg = `[IPC] vector:status handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { disabled: true, reason: String(err), items: 0 }
    }
  })

  // -------------------------------------------------------------------------
  // search:query — execute a text search against the extended search index
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SEARCH_QUERY, async (_event, rawPayload) => {
    return searchService.query(rawPayload)
  })

  // -------------------------------------------------------------------------
  // vault:open-in-new-window — open vault in a second BrowserWindow
  // Requirements: 22.7
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN_IN_NEW_WINDOW, async (_event, rawPayload) => {
    return vaultService.openVaultInNewWindow(rawPayload)
  })

  // -------------------------------------------------------------------------
  // vault:scan — re-scan the current vault and return updated metadata
  // -------------------------------------------------------------------------
  // properties:write — rewrite YAML frontmatter properties for a file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PROPERTIES_WRITE, async (_event, rawPayload) => {
    const validation = PropertiesWriteSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] properties:write validation failed: ${reason}`)
      return PropertiesWriteResultSchema.parse({ success: false, error: reason })
    }

    const { path: filePath, yaml: newYaml } = validation.data

    // Validate the YAML before writing
    try {
      const yaml = await import('yaml')
      yaml.parse(newYaml)
    } catch (err) {
      const reason = `Invalid YAML: ${err instanceof Error ? err.message : String(err)}`
      emitActivityLog('warn', `[IPC] properties:write rejected: ${reason}`)
      return PropertiesWriteResultSchema.parse({ success: false, error: reason })
    }

    try {
      // Read current file content
      const content = await fs.readFile(filePath, 'utf-8')
      const newContent = replaceFrontmatterRaw(content, newYaml)

      // Write under Pending_Write_Lock (same pattern as note:save)
      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, newContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      return PropertiesWriteResultSchema.parse({ success: true })
    } catch (err) {
      stateManager.clearPendingWrite(filePath)
      const msg = `[IPC] properties:write error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return PropertiesWriteResultSchema.parse({ success: false, error: String(err) })
    }
  })

  // -------------------------------------------------------------------------
  // activity:log — receive log entries from the renderer
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ACTIVITY_LOG, async (_event, rawPayload) => {
    const validation = ActivityLogSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      // Log to console only — avoid recursive loop back to renderer
      console.warn(`[IPC] activity:log validation failed: ${reason}`)
      return { error: reason }
    }

    const { level, message } = validation.data
    console[level](`[Renderer] ${message}`)
    return { success: true }
  })

  // -------------------------------------------------------------------------
  // settings:get — retrieve a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_GET, async (_event, rawPayload) => {
    const validation = SettingsGetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:get validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { key } = validation.data

    try {
      const settings = await loadSettings()
      const value = (settings as unknown as Record<string, unknown>)[key]
      return { value }
    } catch (err) {
      const msg = `[IPC] settings:get handler error for key "${key}": ${String(err)}`
      console.error(msg)
      emitActivityLog('warn', msg)
      return { success: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // settings:set — update a single settings value by key
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_SET, async (_event, rawPayload) => {
    const validation = SettingsSetSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:set validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { key, value } = validation.data

    try {
      const settings = await loadSettings()
      const updated = { ...settings, [key]: value }
      await saveSettings(updated)
      return { success: true }
    } catch (err) {
      const msg = `[IPC] settings:set handler error for key "${key}": ${String(err)}`
      console.error(msg)
      emitActivityLog('warn', msg)
      return { success: false, error: String(err) }
    }
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
      const msg = `[IPC] folder:create handler error for "${folderPath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:create — create a new note, optionally from a template
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_CREATE, async (_event, rawPayload) => {
    const validation = NoteCreateSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:create validation failed: ${reason}`)
      return { error: reason }
    }

    const { vaultPath, name, templateContent } = validation.data

    // Strip .md suffix if present, then re-append for the actual file path
    const normalisedName = name.replace(/\.md$/i, '')
    const filePath = path.join(vaultPath, normalisedName + '.md')

    // Check for existing file
    try {
      await fs.access(filePath)
      // File exists — return error
      return { success: false, error: 'A note with that name already exists' }
    } catch {
      // File does not exist — proceed
    }

    try {
      // Prepare content with template variable substitution
      const now = new Date()
      const dateStr = now.toISOString().slice(0, 10)
      const timeStr = now.toTimeString().slice(0, 5)

      const rawContent = templateContent ?? `# ${normalisedName}\n`
      let content = substituteVariables(rawContent, {
        title: normalisedName,
        date: dateStr,
        time: timeStr
      })

      // Auto-properties: inject `created` timestamp if absent (Req 16.1, 16.2)
      const settings = await loadSettings()
      if (settings.autoProperties) {
        content = injectAutoProperty(content, 'created', now.toISOString(), true)
      }

      // Write file with pending write lock
      stateManager.setPendingWrite(filePath)
      try {
        await fs.writeFile(filePath, content, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(filePath)
      }

      // Get AST for the new file and return
      const ast = await stateManager.getAST(filePath)
      const response = FileGetResultSchema.parse({ path: filePath, ast })
      return response
    } catch (err) {
      const msg = `[IPC] note:create handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return {
        path: filePath,
        ast: null,
        error: { line: 0, column: 0, message: String(err) }
      }
    }
  })

  // -------------------------------------------------------------------------
  // note:save — write updated content to an existing note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_SAVE, async (_event, rawPayload) => {
    const validation = NoteSaveSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:save validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: filePath, content } = validation.data

    try {
      // Auto-properties: inject/update `modified` timestamp (Req 16.1, 16.2)
      const settings = await loadSettings()
      const finalContent = settings.autoProperties
        ? injectAutoProperty(content, 'modified', new Date().toISOString(), false)
        : content

      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, finalContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      // Incremental index update (task 9 implements updateIndexesForFile; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).updateIndexesForFile?.(filePath)
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // updateIndexesForFile not yet available — silently ignore
      }

      // Enqueue an embedding for the saved file (Requirement 1.2).
      // VectorManager.embedFile skips empty-content notes internally (Requirement 1.8)
      // and respects the embeddingsDisabled flag, so calling it unconditionally is safe.
      vectorManager.embedFile(filePath, content)

      return { success: true }
    } catch (err) {
      // Ensure lock is released even on error
      stateManager.clearPendingWrite(filePath)
      const msg = `[IPC] note:save handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:rename — rename a note file (no PendingWriteLock — watcher handles events)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_RENAME, async (_event, rawPayload) => {
    const validation = NoteRenameSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:rename validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { oldPath, newPath: rawNewPath } = validation.data

    // Normalise: append .md if not already present
    const normalisedNewPath = rawNewPath.endsWith('.md') ? rawNewPath : rawNewPath + '.md'

    try {
      await fs.rename(oldPath, normalisedNewPath)
      return { success: true }
    } catch (err) {
      const msg = `[IPC] note:rename handler error "${oldPath}" → "${normalisedNewPath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:delete — delete a note file (no PendingWriteLock — watcher handleUnlink
  //               never checks the lock, so it has no effect)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_DELETE, async (_event, rawPayload) => {
    const validation = NoteDeleteSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:delete validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: filePath } = validation.data

    try {
      await fs.rm(filePath)

      // Full index rebuild after deletion (deleted file must be purged from all index entries)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.()
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return { success: true }
    } catch (err) {
      const msg = `[IPC] note:delete handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:get-raw — return the raw markdown string for a note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_GET_RAW, async (_event, rawPayload) => {
    const validation = NoteGetRawSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:get-raw validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { path: filePath } = validation.data

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      return { path: filePath, content }
    } catch (err) {
      const msg = `[IPC] note:get-raw handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // asset:read — read a file as a base64 data URI for sandboxed iframes
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.ASSET_READ, async (_event, rawPayload) => {
    const validation = AssetReadSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] asset:read validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { path: filePath } = validation.data

    try {
      // Read the file as a Buffer so it works for both text and binary
      const buffer = await fs.readFile(filePath)
      const ext = path.extname(filePath).toLowerCase()
      const mimeMap: Record<string, string> = {
        '.png': 'image/png',
        '.jpg': 'image/jpeg',
        '.jpeg': 'image/jpeg',
        '.gif': 'image/gif',
        '.svg': 'image/svg+xml',
        '.webp': 'image/webp',
        '.ico': 'image/x-icon',
        '.css': 'text/css',
        '.js': 'application/javascript',
        '.json': 'application/json',
        '.woff': 'font/woff',
        '.woff2': 'font/woff2',
        '.ttf': 'font/ttf'
      }
      const mime = mimeMap[ext] ?? 'application/octet-stream'
      const dataUri = `data:${mime};base64,${buffer.toString('base64')}`
      return { path: filePath, dataUri }
    } catch (err) {
      const msg = `[IPC] asset:read handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:export-html — export a note as an HTML file via save dialog
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_EXPORT_HTML, async (_event, rawPayload) => {
    const validation = NoteExportHtmlSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:export-html validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { path: notePath, html } = validation.data

    try {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0]
      const dialogResult = await dialog.showSaveDialog(focusedWindow, {
        defaultPath: notePath,
        filters: [{ name: 'HTML', extensions: ['html'] }]
      })

      if (dialogResult.canceled || !dialogResult.filePath) {
        return { success: false }
      }

      const savedPath = dialogResult.filePath
      stateManager.setPendingWrite(savedPath)
      try {
        await fs.writeFile(savedPath, html, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(savedPath)
      }

      return { success: true, savedPath }
    } catch (err) {
      const msg = `[IPC] note:export-html handler error for "${notePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
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
      const msg = `[IPC] favorites:get error: ${String(err)}`
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
      const msg = `[IPC] favorites:toggle error: ${String(err)}`
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
      const msg = `[IPC] favorites:remove error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { favorites: [] }
    }
  })

  // -------------------------------------------------------------------------
  // note:daily — open or create today's daily note
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_DAILY, async (_event, rawPayload) => {
    const validation = NoteDailySchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:daily validation failed: ${reason}`)
      return { path: '', ast: null, created: false, error: reason }
    }

    const { vaultPath } = validation.data

    try {
      const settings = await loadSettings()
      const now = new Date()

      // Derive daily note filename from configured date format (Req 17.4)
      // Supported tokens: YYYY, MM, DD (simple substitution)
      const dateFormat = settings.dailyNoteDateFormat || 'YYYY-MM-DD'
      const dateStr = dateFormat
        .replace('YYYY', String(now.getFullYear()))
        .replace('MM', String(now.getMonth() + 1).padStart(2, '0'))
        .replace('DD', String(now.getDate()).padStart(2, '0'))

      const folder = settings.dailyNoteFolder || 'Daily'
      const dirPath = path.join(vaultPath, folder)
      const filePath = path.join(dirPath, `${dateStr}.md`)

      // Check if file already exists
      let created = false
      let content: string
      try {
        await fs.access(filePath)
        // File exists — read it
        content = await fs.readFile(filePath, 'utf-8')
      } catch {
        // File does not exist — create it
        created = true

        // Ensure the daily note folder exists
        await fs.mkdir(dirPath, { recursive: true })

        // Prepare content from template or default heading
        const templateName = settings.dailyNoteTemplate || ''
        if (templateName) {
          // Look up the template file in _templates/
          const templatesDir = path.join(vaultPath, '_templates')
          const templatePath = path.join(templatesDir, `${templateName}.md`)
          try {
            const templateContent = await fs.readFile(templatePath, 'utf-8')
            const dateFormatted = now.toISOString().slice(0, 10)
            const timeFormatted = now.toTimeString().slice(0, 5)
            content = substituteVariables(templateContent, {
              title: dateStr,
              date: dateFormatted,
              time: timeFormatted
            })
          } catch {
            // Template not found — fall back to empty note
            content = `# ${dateStr}\n\n`
          }
        } else {
          content = `# ${dateStr}\n\n`
        }

        // Auto-properties: inject `created` timestamp if absent (Req 16.1)
        const dnSettings = await loadSettings()
        if (dnSettings.autoProperties) {
          content = injectAutoProperty(content, 'created', now.toISOString(), true)
        }

        stateManager.setPendingWrite(filePath)
        try {
          await fs.writeFile(filePath, content, 'utf-8')
        } finally {
          stateManager.clearPendingWrite(filePath)
        }
      }

      // Get AST and return
      const ast = await stateManager.getAST(filePath)
      return NoteDailyResultSchema.parse({
        path: filePath,
        ast,
        created
      })
    } catch (err) {
      const msg = `[IPC] note:daily handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: '', ast: null, created: false, error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:random — open a random note from the vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_RANDOM, async (_event, rawPayload) => {
    const validation = NoteRandomSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:random validation failed: ${reason}`)
      return { error: reason }
    }
    const { vaultPath, tagFilter } = validation.data
    try {
      // Get files from the vault - need to access the vault's file list
      // For now, we'll use a simple approach: get files from StateManager
      const vault = stateManager.getCurrentVault()
      if (!vault || vault.path !== vaultPath) {
        return { error: 'Vault not open' }
      }
      const files = vault.files ?? []
      // Filter by tag if provided
      let candidates = files
      if (tagFilter && tagFilter.length > 0) {
        const tagPaths = stateManager.getExtendedIndex()?.tagIndex?.get(tagFilter)
        if (tagPaths) {
          candidates = files.filter((f) => tagPaths.has(f.path))
        } else {
          candidates = []
        }
      }
      if (candidates.length === 0) {
        return { error: 'No notes found in vault' }
      }
      const randomFile = candidates[Math.floor(Math.random() * candidates.length)]
      const result = await stateManager.getAST(randomFile.path)
      return NoteRandomResultSchema.parse({
        path: randomFile.path,
        ast: result
      })
    } catch (err) {
      const msg = `[IPC] note:random error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // templates:list — list all templates in the vault's _templates directory
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.TEMPLATES_LIST, async (_event, rawPayload) => {
    const validation = TemplatesListSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] templates:list validation failed: ${reason}`)
      return { templates: [] }
    }

    const { vaultPath } = validation.data
    const templatesDir = path.join(vaultPath, '_templates')

    // Check if _templates directory exists
    try {
      await fs.access(templatesDir)
    } catch {
      // Directory does not exist — return empty list
      return { templates: [] }
    }

    try {
      const dirents = await fs.readdir(templatesDir, { withFileTypes: true })
      const mdFiles = dirents.filter((d) => d.isFile() && d.name.endsWith('.md'))

      const templates = await Promise.all(
        mdFiles.map(async (dirent) => {
          const templatePath = path.join(templatesDir, dirent.name)
          const content = await fs.readFile(templatePath, 'utf-8')
          const name = path.basename(dirent.name, '.md')
          return { name, path: templatePath, content }
        })
      )

      return TemplatesListResultSchema.parse({ templates })
    } catch (err) {
      const msg = `[IPC] templates:list handler error for vault "${vaultPath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { templates: [] }
    }
  })

  // -------------------------------------------------------------------------
  // settings:getFeatureToggles — get all feature toggles for the Settings UI
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_GET_FEATURE_TOGGLES, async (_event) => {
    try {
      const { getFeatureToggles, getDefaultState } = await import('../shared/feature-toggles')
      const toggles = getFeatureToggles()
      const result = toggles.map((t) => ({
        ...t,
        enabled: getDefaultState(t.id)
      }))
      return FeatureTogglesResultSchema.parse({ toggles: result })
    } catch (err) {
      const msg = `[IPC] settings:getFeatureToggles error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { toggles: [] }
    }
  })

  // -------------------------------------------------------------------------
  // settings:setFeatureToggle — toggle a feature on/off
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.SETTINGS_SET_FEATURE_TOGGLE, async (_event, rawPayload) => {
    const validation = SetFeatureToggleSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] settings:setFeatureToggle validation failed: ${reason}`)
      return SetFeatureToggleResultSchema.parse({ success: false, error: reason })
    }

    const { id, enabled } = validation.data

    try {
      const { setFeatureEnabled } = await import('../shared/feature-toggles')
      setFeatureEnabled(id, enabled)

      // Notify the widget manager when clipboard-widget toggles
      if (id === 'clipboard-widget' && widgetToggleCallback) {
        widgetToggleCallback(enabled)
      }

      return SetFeatureToggleResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[IPC] settings:setFeatureToggle error for "${id}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return SetFeatureToggleResultSchema.parse({ success: false, error: String(err) })
    }
  })

  // -------------------------------------------------------------------------
  // kanban:get-data — scan folder for notes with frontmatter status
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.KANBAN_GET_DATA, async (_event, rawPayload) => {
    const validation = KanbanGetDataSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] kanban:get-data validation failed: ${reason}`)
      return KanbanGetDataResultSchema.parse({ statuses: [], cards: [] })
    }

    const { vaultPath: _vaultPath, folderPath } = validation.data
    try {
      const dirents = await fs.readdir(folderPath, { withFileTypes: true })
      const mdFiles = dirents.filter((d) => d.isFile() && d.name.endsWith('.md'))

      // Collect all unique statuses and their cards
      const statusSet = new Set<string>(['Backlog', 'In Progress', 'Done'])
      const cards: Array<{ filePath: string; title: string; content: string; tags: string[]; status: string }> = []

      for (const dirent of mdFiles) {
        const filePath = path.join(folderPath, dirent.name)
        try {
          const content = await fs.readFile(filePath, 'utf-8')
          const { parsed } = extractFrontmatter(content)
          const status = parsed.status as string | undefined
          if (status) {
            statusSet.add(status)

            // Extract title (first heading or filename)
            const titleMatch = content.match(/^#\s+(.+)$/m)
            const title = titleMatch?.[1] ?? path.basename(dirent.name, '.md')

            // Extract snippet (first line of meaningful content after frontmatter)
            const contentLines = content.replace(FRONTMATTER_RE, '').trim().split('\n')
            const snippet = contentLines.find((l) => l.trim() && !l.startsWith('#'))?.slice(0, 120) ?? ''

            // Read tags from frontmatter
            const tags = Array.isArray(parsed.tags)
              ? parsed.tags.map(String)
              : typeof parsed.tag === 'string'
                ? [parsed.tag]
                : []

            cards.push({ filePath, title, content: snippet, tags, status })
          }
        } catch {
          // Skip files that can't be read
        }
      }

      return KanbanGetDataResultSchema.parse({
        statuses: Array.from(statusSet).sort(),
        cards
      })
    } catch (err) {
      const msg = `[IPC] kanban:get-data error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return KanbanGetDataResultSchema.parse({ statuses: [], cards: [] })
    }
  })

  // -------------------------------------------------------------------------
  // pdf:open — open a PDF and return metadata + page count (Req 40.1, 40.2)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_OPEN, async (_event, rawPayload) => {
    return pdfService.open(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:render-page — render a single PDF page to a base64 PNG (Req 40.2)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_RENDER_PAGE, async (_event, rawPayload) => {
    return pdfService.renderPage(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:load-annotations — load annotations for a PDF (Req 40.4)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_LOAD_ANNOTATIONS, async (_event, rawPayload) => {
    return pdfService.loadAnnotations(rawPayload)
  })

  // -------------------------------------------------------------------------
  // pdf:save-annotations — save annotations for a PDF (Req 40.5)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PDF_SAVE_ANNOTATIONS, async (_event, rawPayload) => {
    return pdfService.saveAnnotations(rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:start — start audio capture and whisper transcription (Req 41.3)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_START, async (_event, rawPayload) => {
    return dictationService.start(_event, rawPayload)
  })

  // -------------------------------------------------------------------------
  // kanban:set-status — update a note's frontmatter status property
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.KANBAN_SET_STATUS, async (_event, rawPayload) => {
    const validation = KanbanSetStatusSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] kanban:set-status validation failed: ${reason}`)
      return KanbanSetStatusResultSchema.parse({ success: false, error: reason })
    }

    const { vaultPath: _vaultPath2, filePath, status } = validation.data

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      const { parsed } = extractFrontmatter(content)

      // Merge existing properties with new status
      const merged = { ...parsed, status }
      const { stringify } = require('yaml')
      const newYamlStr = stringify(merged)
      const newContent = replaceFrontmatterRaw(content, newYamlStr)

      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, newContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      return KanbanSetStatusResultSchema.parse({ success: true })
    } catch (err) {
      stateManager.clearPendingWrite(filePath)
      const msg = `[IPC] kanban:set-status error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return KanbanSetStatusResultSchema.parse({ success: false, error: String(err) })
    }
  })

  // -------------------------------------------------------------------------
  // dictation:stop — stop dictation and return transcription (Req 41.4)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STOP, async (_event, rawPayload) => {
    return dictationService.stop(rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:status — get dictation model status (Req 42.4)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_STATUS, async (_event, rawPayload) => {
    return dictationService.status(rawPayload)
  })

  // -------------------------------------------------------------------------
  // dictation:download-model — download a dictation model (Req 42.5)
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.DICTATION_DOWNLOAD_MODEL, async (_event, rawPayload) => {
    return dictationService.downloadModel(_event, rawPayload)
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
  // view-state:get-fold — get fold state for a heading
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VIEW_STATE_GET_FOLD, async (_event, rawPayload) => {
    const validation = ViewStateGetFoldSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] view-state:get-fold validation failed: ${reason}`)
      return true // Default to open
    }

    const { vaultPath, notePath, headingId } = validation.data

    try {
      // Load view state for the note
      const state = await loadViewState(vaultPath, notePath)
      return state.foldStates[headingId] ?? true
    } catch (err) {
      const msg = `[IPC] view-state:get-fold handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return true // Default to open on error
    }
  })

  // -------------------------------------------------------------------------
  // view-state:set-fold — set fold state for a heading
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VIEW_STATE_SET_FOLD, async (_event, rawPayload) => {
    const validation = ViewStateSetFoldSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] view-state:set-fold validation failed: ${reason}`)
      return
    }

    const { vaultPath, notePath, headingId, isOpen } = validation.data

    try {
      await setFoldState(vaultPath, notePath, headingId, isOpen)
    } catch (err) {
      const msg = `[IPC] view-state:set-fold handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
    }
  })

  // -------------------------------------------------------------------------
  // properties:read — read YAML frontmatter properties from a file
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.PROPERTIES_READ, async (_event, rawPayload) => {
    const validation = PropertiesReadSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] properties:read validation failed: ${reason}`)
      return { path: '', properties: {}, yaml: '' }
    }

    const { path: filePath } = validation.data

    try {
      const content = await fs.readFile(filePath, 'utf-8')
      const { yaml, parsed } = extractFrontmatter(content)
      return PropertiesReadResultSchema.parse({
        path: filePath,
        properties: parsed,
        yaml
      })
    } catch (err) {
      const msg = `[IPC] properties:read handler error for "${filePath}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: filePath, properties: {}, yaml: '' }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:get — get bookmarks for a vault
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_GET, async (_event, rawPayload) => {
    // Simple schema validation
    const vaultPath = typeof rawPayload === 'object' && rawPayload !== null ? (rawPayload as any).vaultPath : ''
    if (!vaultPath) {
      return { bookmarks: {} }
    }

    try {
      const bookmarks = await readBookmarks(vaultPath)
      return { bookmarks }
    } catch (err) {
      const msg = `[IPC] bookmarks:get handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:add — add a bookmark to a list
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_ADD, async (_event, rawPayload) => {
    const { vaultPath, listName, filePath } = (rawPayload ?? {}) as { vaultPath?: string; listName?: string; filePath?: string }
    if (!vaultPath || !listName || !filePath) {
      return { bookmarks: {} }
    }

    try {
      const bookmarks = await addBookmark(vaultPath, listName, filePath)
      return { bookmarks }
    } catch (err) {
      const msg = `[IPC] bookmarks:add handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })

  // -------------------------------------------------------------------------
  // bookmarks:remove — remove a bookmark from a list
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.BOOKMARKS_REMOVE, async (_event, rawPayload) => {
    const { vaultPath, listName, filePath } = (rawPayload ?? {}) as { vaultPath?: string; listName?: string; filePath?: string }
    if (!vaultPath || !listName || !filePath) {
      return { bookmarks: {} }
    }

    try {
      const bookmarks = await removeBookmark(vaultPath, listName, filePath)
      return { bookmarks }
    } catch (err) {
      const msg = `[IPC] bookmarks:remove handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { bookmarks: {} }
    }
  })

  // -------------------------------------------------------------------------
  // note:compose — merge multiple notes into one
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_COMPOSE, async (_event, rawPayload) => {
    const validation = NoteComposeSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:compose validation failed: ${reason}`)
      return { previewMarkdown: '', warning: reason }
    }

    const { vaultPath, sourcePaths } = validation.data

    try {
      const vault = stateManager.getCurrentVault()
      if (!vault || vault.path !== vaultPath) {
        return { previewMarkdown: '', warning: 'Vault not open' }
      }

      const result = await mergeNotes(sourcePaths, vaultPath, vault.files)
      return NoteComposeResultSchema.parse(result)
    } catch (err) {
      const msg = `[IPC] note:compose handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { previewMarkdown: '', warning: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // note:unique — create a note with a unique timestamp name
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.NOTE_UNIQUE, async (_event, rawPayload) => {
    const validation = NoteUniqueSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] note:unique validation failed: ${reason}`)
      return { path: '', error: reason }
    }

    const { vaultPath } = validation.data

    try {
      const now = new Date()
      const uniqueName = generateUniqueNoteName('YYYYMMDDHHmmss', now)
      const filePath = path.join(vaultPath, `${uniqueName}.md`)

      // Check if file already exists
      try {
        await fs.access(filePath)
        return { path: '', error: 'Note with that name already exists' }
      } catch {
        // File doesn't exist, proceed
      }

      // Create the note
      const content = `# ${uniqueName}\n\n`
      stateManager.setPendingWrite(filePath)
      try {
        await fs.writeFile(filePath, content, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(filePath)
      }

      // Get AST for the new file
      const ast = await stateManager.getAST(filePath)
      return NoteUniqueResultSchema.parse({ path: filePath, ast })
    } catch (err) {
      const msg = `[IPC] note:unique handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { path: '', error: String(err) }
    }
  })
}
