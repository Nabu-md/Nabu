/**
 * widget-service.ts
 *
 * WidgetService — owns widget lifecycle, widget registration, and widget
 * coordination.
 *
 * This service extracts the widget business logic that was previously embedded
 * inside `index.ts` (clipboard history start, widget:create-note,
 * widget:fetch-title, widget:open-note, widget:set-shortcut handlers) and
 * coordinates the existing `widgetManager` (widget-manager.ts) and
 * `ClipboardHistory` (clipboard-history.ts) singletons.
 *
 * The IPC layer and bootstrap now delegate to this service, leaving behind
 * thin wrappers. The underlying widget window management and dictation-mode
 * orchestration remain in `widget-manager.ts` (unchanged).
 *
 * This is a pure extraction: no behavior is redesigned, improved, or changed.
 *
 * Requirements: 41.4, 42.2, 42.3, 43.1, 43.2, 43.4
 */

import * as path from 'path'
import fs from 'fs/promises'
import { ipcMain, BrowserWindow } from 'electron'

import { loadSettings } from './settings'
import { vaultRegistry } from './vault-registry'
import { widgetManager, registerWidgetIPCHandlers, wireFnMonitorToWidget } from './widget-manager'
import { ClipboardHistory } from './clipboard-history'

// ---------------------------------------------------------------------------
// WidgetService
// ---------------------------------------------------------------------------

/**
 * Owns all widget lifecycle and coordination business logic.
 */
export class WidgetService {
  private clipboardHistory: ClipboardHistory | null = null

  /**
   * Register all widget IPC handlers (delegates to the existing
   * registerWidgetIPCHandlers plus the clipboard-history and widget
   * coordination handlers previously defined inline in index.ts).
   */
  registerIPCHandlers(): void {
    // Delegate the core widget window IPC handlers to the existing manager.
    registerWidgetIPCHandlers()

    // Clipboard history: start the service and register its IPC handlers.
    this.clipboardHistory = new ClipboardHistory()
    this.clipboardHistory.start()

    ipcMain.handle('clipboard:history-get', async (_event, { max }) => {
      const entries = this.clipboardHistory!.getRecent(max ?? 8)
      return { entries }
    })
    ipcMain.handle('clipboard:history-clear', async () => {
      await this.clipboardHistory!.clear()
    })
    ipcMain.handle('clipboard:history-copy', async (_event, { text }) => {
      try {
        this.clipboardHistory!.copyToClipboard(text)
        return { success: true }
      } catch (err) {
        return { success: false, error: String(err) }
      }
    })

    // Widget starts enabled by default with saved shortcut.
    loadSettings()
      .then((s) => {
        widgetManager.setEnabled(true, s.clipboardShortcut)
      })
      .catch((err) => {
        console.error('[WidgetService] Failed to load widget shortcut:', err)
        widgetManager.setEnabled(true)
      })

    // Listen for shortcut changes from the Settings panel.
    ipcMain.handle('widget:set-shortcut', async (_event, { shortcut }: { shortcut: string }) => {
      widgetManager.setShortcut(shortcut)
    })

    // Wire feature-toggle changes for clipboard-widget to WidgetManager.
    // (onWidgetToggle is registered in ipc.ts and bridges to widgetManager.)
    // The callback is supplied by the caller via setWidgetToggleCallback.
  }

  /**
   * Wire the fn-monitor to the widget (delegates to existing helper).
   * macOS only — called from index.ts after fn-monitor starts.
   */
  wireFnMonitor(): void {
    wireFnMonitorToWidget()
  }

  /**
   * Create a quick note in the active vault (widget:create-note).
   * Mirrors the previous inline handler in index.ts exactly.
   */
  async createNote({ name, content, timestamp }: { name: string; content: string; timestamp?: boolean }): Promise<{ success: boolean; path?: string; error?: string }> {
    try {
      const session = vaultRegistry.getActive()
      if (!session) return { success: false, error: 'No vault open' }
      const vaultPath = session.vaultPath
      const now = new Date()
      const safeName = timestamp
        ? `${name} ${now.toISOString().slice(0, 10)} ${now.toTimeString().slice(0, 8).replace(/:/g, '-')}`
        : name
      const filePath = path.join(vaultPath, `${safeName}.md`)
      await fs.writeFile(filePath, content, 'utf-8')
      void session.stateManager.invalidateAST(filePath)
      return { success: true, path: filePath }
    } catch (err) {
      return { success: false, error: String(err) }
    }
  }

  /**
   * Fetch a URL and extract its page title (widget:fetch-title).
   * Mirrors the previous inline handler in index.ts exactly.
   */
  async fetchTitle({ url }: { url: string }): Promise<{ title: string }> {
    try {
      const controller = new AbortController()
      const timeout = setTimeout(() => controller.abort(), 5000)
      const response = await fetch(url, { signal: controller.signal })
      clearTimeout(timeout)
      const html = await response.text()
      const match = html.match(/<title[^>]*>([^<]*)<\/title>/i)
      return { title: match ? match[1].trim() : url }
    } catch {
      return { title: url }
    }
  }

  /**
   * Tell the main window to open a file (widget:open-note).
   * Mirrors the previous inline handler in index.ts exactly.
   */
  openNote(mainWindow: BrowserWindow, { path: filePath }: { path: string }): void {
    mainWindow.webContents.send('widget:open-note-request', { path: filePath })
  }
}
