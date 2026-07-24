/**
 * shared.ts — Shared IPC helpers used by all feature IPC modules.
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts`. No behavior is changed.
 */

import { BrowserWindow } from 'electron'
import { ZodError } from 'zod'
import fs from 'fs/promises'

import { IPCChannel } from '@shared/channels'
import {
  ActivityLogSchema,
  NoteLoadedSchema,
  NoteUpdatedSchema,
  NoteDeletedSchema,
  NotesLoadedSchema,
  ContextSearchResultSchema,
  IndexBuildSchema
} from '@shared/schemas'

import { vaultRegistry } from '../services/vault-registry'
import type { StateManager } from '../services/state'
import type { VectorManager } from '../services/vector'
import type { VaultWatcher, WatcherConfig } from '../services/watcher'

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

/** Exposed for the settings module to invoke the widget toggle bridge. */
export function getWidgetToggleCallback(): ((enabled: boolean) => void) | null {
  return widgetToggleCallback
}

/** Exposed for feature modules that need the legacy singleton StateManager. */
export function getLegacyStateManager(): StateManager | null {
  return legacyStateManager
}

// ---------------------------------------------------------------------------
// vaultId dispatch helper
// ---------------------------------------------------------------------------

/**
 * Get the appropriate session managers for the given vaultId.
 * If vaultId is omitted, returns the active vault's managers.
 * Falls back to legacy singletons during v1→v2 migration.
 * If the specified vault is not open, throws an error.
 */
export function getSessionForVault(vaultId: string | undefined): {
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
// Phase 2.4 — Canonical exception → structured error mapping
// ---------------------------------------------------------------------------

/**
 * Canonical structured IPC error shape.
 *
 * Every IPC failure is normalized into this shape before being serialized into
 * the channel's existing contract `error` field. The fields mirror the shared
 * error contract intent established in Phase 2.1:
 *   - `code`      machine-readable error code (e.g. "EACCES", "ENOENT", or a
 *                 semantic code such as "HANDLER_ERROR")
 *   - `message`   human-readable description
 *   - `category`  coarse failure class ("validation" | "io" | "runtime" |
 *                 "unknown") used for consistent cross-channel grouping
 *   - `details`   optional diagnostic payload (original error name, stack
 *                 snippet, or contextual fields) — never internal secrets
 */
export interface NormalizedError {
  code: string
  message: string
  category: 'validation' | 'io' | 'runtime' | 'unknown'
  details?: Record<string, unknown>
}

/**
 * Map an arbitrary thrown value into the canonical {@link NormalizedError}.
 *
 * This is the single normalization point for exception handling across all IPC
 * handlers. It preserves diagnostic information (error name, message, and a
 * truncated stack) without leaking raw internal implementation details, and
 * classifies the failure into a consistent `category` so equivalent failures
 * produce equivalent structured responses across every channel.
 *
 * Pure and side-effect free.
 */
export function normalizeError(err: unknown, context?: Record<string, unknown>): NormalizedError {
  if (err instanceof ZodError) {
    return {
      code: 'VALIDATION_ERROR',
      message: formatZodError(err),
      category: 'validation',
      details: { issues: err.issues }
    }
  }

  if (err instanceof Error) {
    // Classify common Node.js system errors by their `code` when present.
    const category: NormalizedError['category'] =
      typeof (err as NodeJS.ErrnoException).code === 'string' ? 'io' : 'runtime'

    return {
      code: (err as NodeJS.ErrnoException).code ?? 'HANDLER_ERROR',
      message: err.message,
      category,
      details: {
        name: err.name,
        ...(err.stack ? { stack: err.stack.split('\n').slice(0, 3).join('\n') } : {}),
        ...(context ?? {})
      }
    }
  }

  // Non-Error thrown values (strings, objects, etc.) — coerce safely.
  const fallback = typeof err === 'string' ? err : JSON.stringify(err)
  return {
    code: 'HANDLER_ERROR',
    message: fallback,
    category: 'unknown',
    details: context
  }
}

/**
 * Serialize a {@link NormalizedError} into the string form expected by the
 * many channels whose contract `error` field is `z.string()`.
 *
 * Keeps the human-readable message first, then appends the machine code so
 * consumers can still branch on it, while preserving the exact string-typed
 * contract shape the renderer already relies on.
 */
export function errorToString(err: NormalizedError): string {
  return err.code === 'HANDLER_ERROR' ? err.message : `[${err.code}] ${err.message}`
}

// ---------------------------------------------------------------------------
// Frontmatter helpers
// ---------------------------------------------------------------------------

/** Regex to match YAML frontmatter delimiters. */
const FRONTMATTER_RE = /^---\n[\s\S]*?\n---(?:\n|$)/

/** Result of extracting frontmatter from raw content. */
export interface FrontmatterResult {
  yaml: string // raw YAML string (without delimiters)
  parsed: Record<string, unknown> // parsed YAML object
}

/**
 * Extract YAML frontmatter from raw markdown content.
 */
export function extractFrontmatter(content: string): FrontmatterResult {
  const match = content.match(FRONTMATTER_RE)
  if (!match) {
    return { yaml: '', parsed: {} }
  }

  const yamlStr = match[0].replace(/^---\n/, '').replace(/\n---(?:\n|$)/, '')

  try {
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
 */
export function replaceFrontmatterRaw(raw: string, yamlStr: string): string {
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
 */
export function injectAutoProperty(
  content: string,
  key: string,
  value: string,
  onlyIfAbsent: boolean
): string {
  const { parsed } = extractFrontmatter(content)

  if (onlyIfAbsent && key in parsed) {
    return content
  }

  const updated = { ...parsed, [key]: value }

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
 */
export function buildWatcherConfig(
  stateManager: StateManager,
  vectorManager: VectorManager,
  vaultPath: string,
  vaultMeta: { files: import('@shared/types').FileEntry[] }
): WatcherConfig {
  return {
    vaultPath,
    ignored: /^\.|\.nabu/,
    awaitWriteFinish: { stabilityThreshold: 50 },
    stateManager,

    onFileChanged: async (filePath: string, isExternal: boolean) => {
      stateManager.invalidateAST(filePath)
      try {
        const ast = await stateManager.getAST(filePath)
        sendToRenderer(IPCChannel.NOTE_UPDATED, { path: filePath, ast, isExternal })

        // Update text indexes for external edits (Phase 7.2 fix)
        if (!stateManager.hasPendingWrite(filePath)) {
          try {
            const indexResult = await (stateManager as any).updateIndexesForFile?.(filePath)
            if (indexResult) {
              sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
            }
          } catch {
            // updateIndexesForFile not yet available — silently ignore
          }

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
      sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath, files: vaultMeta.files })

      // Update text indexes for newly added files (Phase 7.2 fix)
      if (!stateManager.hasPendingWrite(filePath)) {
        try {
          const indexResult = await (stateManager as any).updateIndexesForFile?.(filePath)
          if (indexResult) {
            sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
          }
        } catch {
          // updateIndexesForFile not yet available — silently ignore
        }

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
      // Remove from text indexes (Phase 7.2 fix)
      stateManager.removeFileFromIndexes(filePath)

      // Remove from vector index
      vectorManager.removeFile(filePath).catch((err) => {
        emitActivityLog('error', `[IPC] Failed to remove vector for "${filePath}": ${String(err)}`)
      })

      sendToRenderer(IPCChannel.NOTE_DELETED, { path: filePath })
    },

    onImageAdded: async (filePath: string) => {
      try {
        const { enqueueOCR, createOCRCompanionNote } = await import('../services/ocr-manager')
        const ocrResult = await enqueueOCR(filePath, vaultPath)
        if (ocrResult) {
          const companionPath = await createOCRCompanionNote(filePath, ocrResult, vaultPath)
          if (companionPath) {
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
 */
export function sendToRenderer(channel: IPCChannel, payload: unknown): void {
  const schema = outgoingSchemas[channel]

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
