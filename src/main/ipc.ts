/**
 * ipc.ts
 *
 * IPC Handler Registration — registers all Renderer→Main `ipcMain.handle()`
 * channels with Zod validation, and provides `sendToRenderer()` for
 * Main→Renderer push messages.
 *
 * Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 16.1, 16.2, 16.3, 16.4, 16.6, 22.3, 22.9
 */

import { ipcMain, dialog, BrowserWindow, app } from 'electron'
import { ZodError } from 'zod'
import path from 'path'
import fs from 'fs/promises'
import { join } from 'path'

import { IPCChannel } from '../shared/channels'
import {
  // Incoming schemas (Renderer → Main)
  VaultOpenSchema,
  VaultCloseSchema,
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
  VaultCreateSchema,
  FolderCreateSchema,
  NoteCreateSchema,
  NoteSaveSchema,
  NoteRenameSchema,
  NoteDeleteSchema,
  NoteGetRawSchema,
  NoteExportHtmlSchema,
  TemplatesListSchema,
  // Outgoing schemas (Main → Renderer)
  VaultScanResultSchema,
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
  SearchQuerySchema,
  SearchResponseSchema,
  PropertiesWriteSchema,
  PropertiesWriteResultSchema,
  NoteDailySchema,
  NoteDailyResultSchema,
  NoteRandomSchema,
  NoteRandomResultSchema,
  FavoritesGetSchema,
  FavoritesToggleSchema,
  FavoritesRemoveSchema
} from '../shared/schemas'

import { search } from '../shared/search-query'

import { loadSettings, saveSettings } from './settings'
import { substituteVariables } from './templates'
import { readFavorites, toggleFavorite, removeFavorite } from './favorites'
import { vaultRegistry } from './vault-registry'

import type { StateManager } from './state'
import type { VectorManager } from './vector'
import type { VaultWatcher, WatcherConfig } from './watcher'

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
function emitActivityLog(level: 'info' | 'warn' | 'error', message: string): void {
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
function formatZodError(err: ZodError): string {
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
// copyDefaultTemplates
// ---------------------------------------------------------------------------

/**
 * Copy default template `.md` files from the bundled `default-templates`
 * resource directory into `<vaultPath>/_templates/` on first open.
 *
 * - Only copies if `_templates/` does not already exist (first-open guard).
 * - Resolves the source directory from `process.resourcesPath` when packaged,
 *   or from the local `resources/` folder in development.
 * - Failures are non-fatal: a warning is emitted to activity:log and the
 *   vault open / create flow continues normally.
 *
 * Requirements: 9.3
 */
async function copyDefaultTemplates(vaultPath: string): Promise<void> {
  const templatesDir = path.join(vaultPath, '_templates')

  // Only copy on first open — skip if _templates already exists
  try {
    await fs.access(templatesDir)
    return // directory exists; nothing to do
  } catch {
    // Directory does not exist — proceed with copy
  }

  // Resolve source directory based on whether the app is packaged
  const srcDir = app.isPackaged
    ? path.join(process.resourcesPath, 'default-templates')
    : path.join(__dirname, '..', '..', '..', 'resources', 'default-templates')

  // Create the _templates directory
  await fs.mkdir(templatesDir, { recursive: true })

  // Read all .md files from the source dir and copy each to _templates/
  const dirents = await fs.readdir(srcDir, { withFileTypes: true })
  await Promise.all(
    dirents
      .filter((d) => d.isFile() && d.name.endsWith('.md'))
      .map((d) => fs.copyFile(path.join(srcDir, d.name), path.join(templatesDir, d.name)))
  )
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
    IPCChannel.NOTE_EXPORT_HTML,
    IPCChannel.TEMPLATES_LIST,
    IPCChannel.ASSET_READ,
    IPCChannel.CONTEXT_REINDEX,
    IPCChannel.VECTOR_STATUS,
    IPCChannel.SEARCH_QUERY,
    IPCChannel.PROPERTIES_READ,
    IPCChannel.PROPERTIES_WRITE,
    'vault:get-current' as IPCChannel
  ]
  for (const ch of channels) {
    ipcMain.removeHandler(ch)
  }

  // -------------------------------------------------------------------------
  // vault:get-current — renderer pulls current vault state on mount
  // -------------------------------------------------------------------------
  ipcMain.removeHandler('vault:get-current')
  ipcMain.handle('vault:get-current', async (_event) => {
    try {
      const vault = stateManager.getCurrentVault()
      if (!vault) return null
      return VaultScanResultSchema.parse(vault)
    } catch (err) {
      console.error('[IPC] vault:get-current error:', err)
      return null
    }
  })

  // -------------------------------------------------------------------------
  // vault:open — open a vault by path, or prompt with native folder picker
  // Requirements: 22.5, 22.6
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN, async (_event, rawPayload) => {
    let parsedPath: string | undefined

    // Validate incoming payload (path is optional)
    const validation = VaultOpenSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] vault:open validation failed: ${reason}`)
      return { error: reason }
    }

    parsedPath = validation.data.path

    // If no path provided, show native folder picker
    if (!parsedPath) {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0]
      const result = await dialog.showOpenDialog(focusedWindow, {
        properties: ['openDirectory'],
        title: 'Open Vault',
        buttonLabel: 'Open'
      })

      if (result.canceled || result.filePaths.length === 0) {
        return { canceled: true }
      }

      parsedPath = result.filePaths[0]
    }

    try {
      const vaultMeta = await stateManager.openVault(parsedPath)

      // Copy default templates on first open (non-fatal)
      try {
        await copyDefaultTemplates(parsedPath)
      } catch (copyErr) {
        emitActivityLog(
          'warn',
          `[IPC] vault:open — failed to copy default templates: ${String(copyErr)}`
        )
      }

      // Register vault session in the registry (Req 22.5)
      // For now, we use the legacy singleton managers as the default session
      // This will be enhanced when we fully migrate to per-vault managers
      vaultRegistry.register(
        parsedPath, // vaultId is the vault path
        parsedPath,
        stateManager,
        vectorManager,
        watcher
      )
      vaultRegistry.setActive(parsedPath)

      // Start the file watcher (uses shared config with vector embedding)
      watcher.start(buildWatcherConfig(stateManager, vectorManager, parsedPath, vaultMeta))

      const response = VaultScanResultSchema.parse(vaultMeta)

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.()
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      // Notify renderer that vault was opened (via validated channel)
      sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath: parsedPath, files: vaultMeta.files })

      return response
    } catch (err) {
      const msg = `[IPC] vault:open handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // vault:scan — re-scan the current vault and return updated metadata
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_SCAN, async (_event, _rawPayload) => {
    try {
      const currentVault = stateManager.getCurrentVault()
      if (!currentVault) {
        return { error: 'No vault is currently open' }
      }

      const vaultMeta = await stateManager.openVault(currentVault.path)
      const response = VaultScanResultSchema.parse(vaultMeta)

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.()
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return response
    } catch (err) {
      const msg = `[IPC] vault:scan handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  })

  // -------------------------------------------------------------------------
  // vault:close — stop the watcher and release vault state
  // Requirements: 22.5, 22.6
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_CLOSE, async (_event, rawPayload) => {
    const validation = VaultCloseSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] vault:close validation failed: ${reason}`)
      return { error: reason }
    }

    const vaultId = validation.data.vaultId

    try {
      // Close vault session in registry if vaultId provided
      if (vaultId) {
        vaultRegistry.close(vaultId)
      } else {
        // Fall back to stopping the legacy watcher
        watcher.stop()
      }
      return { success: true }
    } catch (err) {
      const msg = `[IPC] vault:close handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
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
    const validation = SearchQuerySchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] search:query validation failed: ${reason}`)
      return { results: [] }
    }

    const { query } = validation.data
    const vault = stateManager.getCurrentVault()
    if (!vault) {
      return { results: [] }
    }

    try {
      const results = search(query, vault.files, vault.path, stateManager.getExtendedIndex(), (p) =>
        stateManager.getASTSync(p)
      )
      return SearchResponseSchema.parse({ results })
    } catch (err) {
      const msg = `[IPC] search:query handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { results: [] }
    }
  })

  // -------------------------------------------------------------------------
  // vault:open-in-new-window — open vault in a second BrowserWindow
  // Requirements: 22.7
  // -------------------------------------------------------------------------
  ipcMain.handle(IPCChannel.VAULT_OPEN_IN_NEW_WINDOW, async (_event, rawPayload) => {
    const validation = VaultOpenSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] vault:open-in-new-window validation failed: ${reason}`)
      return { error: reason }
    }

    const vaultPath = validation.data.path
    if (!vaultPath) {
      return { error: 'No vault path provided' }
    }

    try {
      // Check path is accessible
      await fs.access(vaultPath, fs.constants.R_OK)

      // Open the vault in the registry (Req 22.7)
      const vaultMeta = await stateManager.openVault(vaultPath)

      // Copy default templates on first open (non-fatal)
      try {
        await copyDefaultTemplates(vaultPath)
      } catch (copyErr) {
        emitActivityLog(
          'warn',
          `[IPC] vault:open-in-new-window — failed to copy default templates: ${String(copyErr)}`
        )
      }

      // Register vault session in the registry
      vaultRegistry.register(vaultPath, vaultPath, stateManager, vectorManager, watcher)
      vaultRegistry.setActive(vaultPath)

      // Start the file watcher for this vault
      watcher.start(buildWatcherConfig(stateManager, vectorManager, vaultPath, vaultMeta))

      // Create a new BrowserWindow for this vault (Req 22.7)
      const newWindow = new BrowserWindow({
        width: 1200,
        height: 800,
        show: false,
        autoHideMenuBar: false,
        webPreferences: {
          preload: join(__dirname, '../preload/index.js'),
          sandbox: false,
          contextIsolation: true,
          nodeIntegration: false
        }
      })

      // Load renderer in the new window
      if (process.env['VITE_DEV_SERVER_URL']) {
        await newWindow.loadURL(process.env['VITE_DEV_SERVER_URL'])
      } else {
        await newWindow.loadFile(join(__dirname, '../renderer/index.html'))
      }

      newWindow.on('ready-to-show', () => {
        newWindow.show()
      })

      // Send vault state to the new window
      newWindow.webContents.once('did-finish-load', () => {
        sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath, files: vaultMeta.files })
      })

      return { success: true, path: vaultPath }
    } catch (err) {
      const msg = `[IPC] vault:open-in-new-window handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
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
    const validation = VaultCreateSchema.safeParse(rawPayload)
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[IPC] vault:create validation failed: ${reason}`)
      return { error: reason }
    }

    const { parentPath, name } = validation.data
    const newPath = path.join(parentPath, name)

    try {
      // Create the vault directory
      await fs.mkdir(newPath, { recursive: true })

      // Write a Welcome.md file as the initial note
      const welcomePath = path.join(newPath, 'Welcome.md')
      const welcomeContent = `# Welcome to ${name}\n\nThis is your new vault. Start writing!\n`
      stateManager.setPendingWrite(welcomePath)
      try {
        await fs.writeFile(welcomePath, welcomeContent, 'utf-8')
      } finally {
        stateManager.clearPendingWrite(welcomePath)
      }

      // Open the newly created vault
      const vaultMeta = await stateManager.openVault(newPath)
      const result = VaultScanResultSchema.parse(vaultMeta)

      // Trigger index build (task 9 implements buildIndexes; guard with try/catch)
      try {
        const indexResult = await (stateManager as any).buildIndexes?.()
        if (indexResult) {
          sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
        }
      } catch {
        // buildIndexes not yet available — silently ignore
      }

      return result
    } catch (err) {
      const msg = `[IPC] vault:create handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
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
      return SetFeatureToggleResultSchema.parse({ success: true })
    } catch (err) {
      const msg = `[IPC] settings:setFeatureToggle error for "${id}": ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return SetFeatureToggleResultSchema.parse({ success: false, error: String(err) })
    }
  })
}
