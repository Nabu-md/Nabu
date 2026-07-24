/**
 * widgets.ts — Widgets feature IPC module.
 *
 * Owns kanban:* channels, clipboard:history-* channels, and the widget window
 * control channels (widget:show-clipboard, widget:show-dictation, widget:hide,
 * widget:switch-mode, widget:get-state, widget:set-model, widget:get-model,
 * widget:dictation-available, widget:set-mic-permission, widget:insert-text,
 * widget:set-shortcut).
 *
 * This is a pure structural extraction from the previous monolithic
 * `src/main/ipc.ts` and `src/main/services/widget-manager.ts`
 * (registerWidgetIPCHandlers). Handler behavior is unchanged.
 *
 * Lifecycle ownership: every widget transition is delegated to the single
 * authoritative owner, `widgetManager`. Initialization (Persist + Restore)
 * is performed exclusively via `widgetManager.initialize()`; no other
 * caller duplicates the loadSettings → setEnabled sequence.
 */

import { ipcMain } from 'electron'
import path from 'path'
import fs from 'fs/promises'

import { IPCChannel } from '@shared/channels'
import {
  KanbanGetDataSchema,
  KanbanGetDataResultSchema,
  KanbanSetStatusSchema,
  KanbanSetStatusResultSchema
} from '@shared/schemas'

import { widgetManager } from '../services/widget-manager'
import type { WidgetMode } from '../services/widget-manager'
import type { WhisperModel } from '../services/whisper'
import { ClipboardHistory } from '../services/clipboard-history'
import { loadSettings, saveSettings } from '../services/settings'

import type { IPCContext } from './context'
import {
  emitActivityLog,
  formatZodError,
  extractFrontmatter,
  replaceFrontmatterRaw,
  normalizeError,
  errorToString
} from './shared'

/**
 * Register all widgets-feature IPC handlers.
 */
export function registerWidgetsIPC(_ctx: IPCContext): void {
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

      const statusSet = new Set<string>(['Backlog', 'In Progress', 'Done'])
      const cards: Array<{
        filePath: string
        title: string
        content: string
        tags: string[]
        status: string
      }> = []

      for (const dirent of mdFiles) {
        const filePath = path.join(folderPath, dirent.name)
        try {
          const content = await fs.readFile(filePath, 'utf-8')
          const { parsed } = extractFrontmatter(content)
          const status = parsed.status as string | undefined
          if (status) {
            statusSet.add(status)

            const titleMatch = content.match(/^#\s+(.+)$/m)
            const title = titleMatch?.[1] ?? path.basename(dirent.name, '.md')

            const contentLines = content
              .replace(/^---\n[\s\S]*?\n---(?:\n|$)/, '')
              .trim()
              .split('\n')
            const snippet =
              contentLines.find((l) => l.trim() && !l.startsWith('#'))?.slice(0, 120) ?? ''

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
      const normalized = normalizeError(err, { folderPath })
      const msg = `[IPC] kanban:get-data error: ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return KanbanGetDataResultSchema.parse({ statuses: [], cards: [] })
    }
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

      const merged = { ...parsed, status }
      const { stringify } = require('yaml')
      const newYamlStr = stringify(merged)
      const newContent = replaceFrontmatterRaw(content, newYamlStr)

      const stateManager = await getStateManager()
      stateManager.setPendingWrite(filePath)
      await fs.writeFile(filePath, newContent, 'utf-8')
      stateManager.invalidateAST(filePath)
      stateManager.clearPendingWrite(filePath)

      return KanbanSetStatusResultSchema.parse({ success: true })
    } catch (err) {
      const normalized = normalizeError(err, { filePath, status })
      const msg = `[IPC] kanban:set-status error for "${filePath}": ${errorToString(normalized)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return KanbanSetStatusResultSchema.parse({ success: false, error: errorToString(normalized) })
    }
  })

  // -------------------------------------------------------------------------
  // clipboard:history-get — get recent clipboard entries
  // -------------------------------------------------------------------------
  const clipboardHistory = new ClipboardHistory()
  clipboardHistory.start()

  ipcMain.handle(IPCChannel.CLIPBOARD_HISTORY_GET, async (_event, { max }) => {
    const entries = clipboardHistory.getRecent(max ?? 8)
    return { entries }
  })

  ipcMain.handle(IPCChannel.CLIPBOARD_HISTORY_CLEAR, async () => {
    await clipboardHistory.clear()
  })

  ipcMain.handle(IPCChannel.CLIPBOARD_HISTORY_COPY, async (_event, { text }) => {
    try {
      clipboardHistory.copyToClipboard(text)
      return { success: true }
    } catch (err) {
      const normalized = normalizeError(err)
      return { success: false, error: errorToString(normalized) }
    }
  })

  // -------------------------------------------------------------------------
  // Widget window-control channels
  // -------------------------------------------------------------------------
  ipcMain.handle('widget:show-clipboard', () => {
    widgetManager.show('clipboard')
  })

  ipcMain.handle('widget:show-dictation', () => {
    widgetManager.show('dictation')
  })

  ipcMain.handle('widget:hide', () => {
    widgetManager.hide()
  })

  ipcMain.handle('widget:switch-mode', (_event, mode: WidgetMode) => {
    widgetManager.switchMode(mode)
  })

  ipcMain.handle('widget:get-state', () => {
    return widgetManager.getState()
  })

  ipcMain.handle('widget:set-model', (_event, model: WhisperModel) => {
    widgetManager.setModel(model)
  })

  ipcMain.handle('widget:get-model', () => {
    return widgetManager.getModel()
  })

  ipcMain.handle('widget:dictation-available', async () => {
    return await widgetManager.isDictationAvailable()
  })

  ipcMain.handle('widget:set-mic-permission', (_event, granted: boolean) => {
    widgetManager.setMicPermission(granted)
  })

  ipcMain.handle('widget:insert-text', (_event, text: string) => {
    widgetManager.insertTextAtCursor(text)
  })

  ipcMain.handle('widget:set-shortcut', async (_event, { shortcut }: { shortcut: string }) => {
    // Single authoritative entry for shortcut changes: update in-memory state
    // via the lifecycle owner AND persist to settings so both paths agree.
    widgetManager.setShortcut(shortcut)
    const current = await loadSettings()
    await saveSettings({ ...current, clipboardShortcut: shortcut })
  })

  // Widget lifecycle initialization (Persist + Restore) is owned exclusively
  // by WidgetManager.initialize(). This is the single init path; no other
  // caller duplicates the loadSettings → setEnabled sequence.
  widgetManager.initialize().catch((err) => {
    console.error('[WidgetsIPC] Widget initialization failed:', err)
  })
}

/**
 * Resolve the active StateManager for kanban:set-status pending-write handling.
 * Mirrors the previous inline handler which used the module-level
 * `stateManager` from `registerIPCHandlers`.
 */
async function getStateManager(): Promise<import('../services/state').StateManager> {
  const { vaultRegistry } = await import('../services/vault-registry')
  const session = vaultRegistry.getActive()
  if (session) {
    return session.stateManager as unknown as import('../services/state').StateManager
  }
  // Fall back to legacy singleton set via setLegacyManagers.
  const { getLegacyStateManager } = await import('./shared')
  const sm = getLegacyStateManager()
  if (!sm) {
    throw new Error('No vault is currently open')
  }
  return sm
}
