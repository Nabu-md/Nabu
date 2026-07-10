/**
 * widget-manager.ts
 *
 * Manages the clipboard-widget BrowserWindow lifecycle:
 *   - Creates a frameless, transparent, always-on-top window
 *   - Loads the widget HTML as a data:text/html URL
 *   - Registers the global shortcut (default: Cmd+§) to toggle the widget
 *   - Handles widget IPC (drag, resize, clipboard copy)
 *   - Hides on blur so other apps regain focus naturally
 */

import { BrowserWindow, globalShortcut, ipcMain, screen } from 'electron'
import { join } from 'path'
import { getWidgetHTML } from './widget-template'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default keyboard shortcut to toggle the widget. */
const DEFAULT_SHORTCUT = 'CmdOrCtrl+§'

// ---------------------------------------------------------------------------
// WidgetManager
// ---------------------------------------------------------------------------

export class WidgetManager {
  private win: BrowserWindow | null = null
  private enabled: boolean = false
  private shortcut: string = DEFAULT_SHORTCUT
  /** Track whether the widget was intentionally shown (vs. being hidden). */
  private isWidgetVisible: boolean = false

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  /**
   * Enable or disable the widget. Safe to call multiple times.
   */
  setEnabled(enabled: boolean, shortcut?: string): void {
    this.enabled = enabled
    if (shortcut) this.shortcut = shortcut

    if (enabled) {
      this.createWindow()
      this.registerShortcut()
    } else {
      this.destroy()
    }
  }

  /**
   * Set a custom keyboard shortcut. Re-registers if already enabled.
   */
  setShortcut(shortcut: string): void {
    this.shortcut = shortcut
    if (this.enabled) {
      globalShortcut.unregisterAll()
      this.registerShortcut()
    }
  }

  /**
   * Toggle widget visibility programmatically.
   */
  toggle(): void {
    if (this.isWidgetVisible) {
      this.hide()
    } else {
      this.show()
    }
  }

  // ---------------------------------------------------------------------------
  // Window management
  // ---------------------------------------------------------------------------

  private createWindow(): void {
    if (this.win && !this.win.isDestroyed()) return

    this.win = new BrowserWindow({
      width: 38,
      height: 38,
      x: 10,
      y: 10,
      frame: false,
      transparent: true,
      alwaysOnTop: true,
      skipTaskbar: true,
      resizable: false,
      show: false,
      webPreferences: {
        preload: join(__dirname, '../preload/index.js'),
        contextIsolation: true,
        nodeIntegration: false,
        backgroundThrottling: false
      }
    })

    // Hide on focus loss — clicking elsewhere dismisses the widget
    this.win.on('blur', () => {
      this.hide()
    })

    // Load widget HTML
    const encoded = encodeURIComponent(getWidgetHTML())
    this.win.loadURL(`data:text/html;charset=utf-8,${encoded}`).catch((err) => {
      console.error('[WidgetManager] Failed to load widget HTML:', err)
    })

    // Prevent the widget from being killed on page unload
    this.win.on('closed', () => {
      this.win = null
    })
  }

  private show(): void {
    if (!this.win || this.win.isDestroyed()) return

    // Position at top-left of the cursor's display
    const cursor = screen.getCursorScreenPoint()
    const display = screen.getDisplayNearestPoint(cursor)
    const { x, y } = display.workArea
    this.win.setPosition(x + 12, y + 12)

    this.win.show()
    this.win.focus()
    this.win.setAlwaysOnTop(true, 'pop-up-menu')
    this.isWidgetVisible = true
  }

  private hide(): void {
    if (!this.win || this.win.isDestroyed()) return
    this.win.hide()
    this.isWidgetVisible = false
  }

  private destroy(): void {
    if (this.win && !this.win.isDestroyed()) {
      this.win.close()
    }
    this.win = null
    this.isWidgetVisible = false
    globalShortcut.unregisterAll()
  }

  // ---------------------------------------------------------------------------
  // Global shortcut
  // ---------------------------------------------------------------------------

  private registerShortcut(): void {
    // Unregister any previous shortcuts
    globalShortcut.unregisterAll()

    if (!this.shortcut) return

    const registered = globalShortcut.register(this.shortcut, () => {
      this.toggle()
    })

    if (!registered) {
      console.warn(
        `[WidgetManager] Failed to register shortcut "${this.shortcut}" — ` +
        'the key combo may not be supported. Configure a different shortcut in Settings.'
      )
    }
  }

  // ---------------------------------------------------------------------------
  // IPC handlers (registered once per WidgetManager instance)
  // ---------------------------------------------------------------------------

  registerIPCHandlers(): void {
    // Widget toggle from renderer (main window can also trigger it)
    ipcMain.handle('widget:toggle', () => {
      this.toggle()
    })

    // Move widget window by an incremental delta (for drag-to-move)
    ipcMain.handle('widget:move', (_event, { dx, dy }: { dx: number; dy: number }) => {
      const win = BrowserWindow.fromWebContents(_event.sender)
      if (win && !win.isDestroyed()) {
        const [x, y] = win.getPosition()
        win.setPosition(x + dx, y + dy)
      }
    })

    // Resize widget window from the JS side
    ipcMain.handle('widget:resize', (_event, { width, height }: { width: number; height: number }) => {
      const win = BrowserWindow.fromWebContents(_event.sender)
      if (win && !win.isDestroyed()) {
        win.setSize(Math.max(30, width), Math.max(30, height))
      }
    })
  }
}
