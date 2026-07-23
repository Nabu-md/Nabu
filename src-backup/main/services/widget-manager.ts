/**
 * widget-manager.ts
 *
 * Manages the always-on-top clipboard/dictation widget window.
 * The widget is a transparent, frameless BrowserWindow that appears
 * when the fn key is held (or triggered via shortcut).
 *
 * Modes:
 * - 'clipboard': Shows clipboard history (existing behavior)
 * - 'dictation': Shows waveform animation for audio dictation
 *
 * Requirements: 41.4, 42.2, 42.3, 43.1, 43.2, 43.4
 */

import { BrowserWindow, screen } from 'electron'
import { join } from 'path'
import { fnMonitor } from './fn-monitor'
import { appEventBus } from '@shared/events'
import { loadSettings } from './settings'
import {
  startDictation,
  stopDictation,
  isDictationActive,
  isWhisperBinaryAvailable,
  ensureModelAvailable,
  getModelStatus,
  WhisperModel
} from './whisper'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type WidgetMode = 'clipboard' | 'dictation'

export interface WidgetState {
  visible: boolean
  mode: WidgetMode
  dictationActive: boolean
  silenceTimer: ReturnType<typeof setTimeout> | null
  micPermissionCached: boolean
  micPermissionGranted: boolean
  whisperCrashCount: number
  maxWhisperCrashRetries: number
  currentModel: WhisperModel
  currentShortcut: string
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WIDGET_WIDTH = 400
const WIDGET_HEIGHT = 200
const SILENCE_TIMEOUT_MS = 15000 // 15 seconds silence detection
const MAX_WHISPER_CRASH_RETRIES = 2

// ---------------------------------------------------------------------------
// Widget Manager
// ---------------------------------------------------------------------------

class WidgetManager {
  private widgetWindow: BrowserWindow | null = null
  private state: WidgetState = {
    visible: false,
    mode: 'clipboard',
    dictationActive: false,
    silenceTimer: null,
    micPermissionCached: false,
    micPermissionGranted: false,
    whisperCrashCount: 0,
    maxWhisperCrashRetries: MAX_WHISPER_CRASH_RETRIES,
    currentModel: 'base',
    currentShortcut: 'CmdOrCtrl+§'
  }

  /**
   * Create the widget window if it doesn't exist.
   * The widget is always-on-top, transparent, and frameless.
   */
  private createWidgetWindow(): void {
    if (this.widgetWindow && !this.widgetWindow.isDestroyed()) return

    this.widgetWindow = new BrowserWindow({
      width: WIDGET_WIDTH,
      height: WIDGET_HEIGHT,
      show: false,
      frame: false,
      transparent: true,
      alwaysOnTop: true,
      skipTaskbar: true,
      resizable: false,
      hasShadow: false,
      webPreferences: {
        preload: join(__dirname, '../preload/index.js'),
        sandbox: false,
        contextIsolation: true,
        nodeIntegration: false
      }
    })

    // Load the widget HTML
    if (process.env['VITE_DEV_SERVER_URL']) {
      this.widgetWindow
        .loadURL(`${process.env['VITE_DEV_SERVER_URL']}#/widget`)
        .catch((err) => console.error('[Widget] Failed to load dev URL:', err))
    } else {
      this.widgetWindow
        .loadFile(join(__dirname, '../renderer/index.html'), { hash: '/widget' })
        .catch((err) => console.error('[Widget] Failed to load:', err))
    }

    this.widgetWindow.on('closed', () => {
      this.widgetWindow = null
    })

    // Notify internal subscribers (services only) that the widget window is ready.
    appEventBus.publish('WidgetRegistered', {
      widgetId: 'clipboard-dictation-widget',
      kind: 'clipboard'
    })

    // Prevent the widget from stealing focus
    this.widgetWindow.on('blur', () => {
      // Keep the widget visible even when not focused
    })
  }

  /**
   * Show the widget at the center of the screen.
   */
  show(mode: WidgetMode = 'clipboard'): void {
    this.createWidgetWindow()
    if (!this.widgetWindow) return

    this.state.mode = mode
    this.state.visible = true

    // Center the widget on the screen
    const cursor = screen.getCursorScreenPoint()
    const display = screen.getDisplayNearestPoint(cursor)
    const { x, y, width } = display.workArea
    const widgetX = Math.round(x + width / 2 - WIDGET_WIDTH / 2)
    const widgetY = Math.round(y + 100) // 100px from top

    this.widgetWindow.setPosition(widgetX, widgetY)
    this.widgetWindow.show()
    this.widgetWindow.setAlwaysOnTop(true, 'floating')

    // Send mode to widget
    this.widgetWindow.webContents.send('widget:mode-changed', { mode })

    if (mode === 'dictation') {
      this.startDictationMode()
    }
  }

  /**
   * Hide the widget.
   */
  hide(): void {
    if (this.state.dictationActive) {
      this.stopDictationMode()
    }

    this.clearSilenceTimer()
    this.state.visible = false

    if (this.widgetWindow && !this.widgetWindow.isDestroyed()) {
      this.widgetWindow.hide()
    }
  }

  /**
   * Toggle the widget visibility.
   */
  toggle(mode?: WidgetMode): void {
    if (this.state.visible) {
      this.hide()
    } else {
      this.show(mode ?? 'clipboard')
    }
  }

  /**
   * Switch the widget to a different mode without closing/reopening.
   */
  switchMode(mode: WidgetMode): void {
    if (this.state.dictationActive) {
      this.stopDictationMode()
    }

    this.state.mode = mode

    if (this.widgetWindow && !this.widgetWindow.isDestroyed()) {
      this.widgetWindow.webContents.send('widget:mode-changed', { mode })
    }

    if (mode === 'dictation') {
      this.startDictationMode()
    }
  }

  /**
   * Start dictation mode: begin audio capture + whisper transcription.
   */
  private async startDictationMode(): Promise<void> {
    // Check microphone permission
    if (this.state.micPermissionCached && !this.state.micPermissionGranted) {
      this.sendToWidget('widget:dictation-error', {
        error:
          'Microphone access required. Enable in System Settings > Privacy & Security > Microphone.'
      })
      return
    }

    // Check whisper binary
    if (!isWhisperBinaryAvailable()) {
      this.sendToWidget('widget:dictation-error', {
        error: 'Whisper binary not found. Please reinstall Nabu.'
      })
      return
    }

    // Ensure model is available
    const modelResult = await ensureModelAvailable(this.state.currentModel)
    if (!modelResult.available) {
      this.sendToWidget('widget:dictation-error', {
        error: `Model download failed: ${modelResult.error}`
      })
      return
    }

    this.state.dictationActive = true
    this.state.whisperCrashCount = 0

    // Start silence detection timer
    this.startSilenceTimer()

    // Notify widget that dictation is starting
    this.sendToWidget('widget:dictation-starting', {})

    try {
      // Start dictation (spawns mic-capture + whisper, pipes mic → whisper)
      const result = await startDictation(this.state.currentModel)

      // Clear silence timer
      this.clearSilenceTimer()

      // Check if transcription is empty (silence)
      if (!result.text || result.text.trim() === '') {
        // Silent: do NOT insert anything, just hide
        this.sendToWidget('widget:dictation-complete', { text: '', silent: true })
        this.hide()
        return
      }

      // Send transcription result to widget
      this.sendToWidget('widget:dictation-complete', {
        text: result.text,
        segments: result.segments
      })

      // Insert text at cursor position
      this.insertTextAtCursor(result.text)

      // Auto-hide after successful transcription
      setTimeout(() => this.hide(), 500)
    } catch (err) {
      this.clearSilenceTimer()
      const errorMsg = String(err)

      // Handle whisper crash
      if (errorMsg.includes('Whisper failed') || errorMsg.includes('process error')) {
        this.handleWhisperCrash()
        return
      }

      // Handle microphone permission denied
      if (errorMsg.includes('Microphone permission denied')) {
        this.state.micPermissionCached = true
        this.state.micPermissionGranted = false
        this.sendToWidget('widget:dictation-error', {
          error:
            'Microphone access required. Enable in System Settings > Privacy & Security > Microphone.'
        })
        return
      }

      // Generic error
      this.sendToWidget('widget:dictation-error', { error: errorMsg })
    }
  }

  /**
   * Stop dictation mode.
   */
  private stopDictationMode(): void {
    this.clearSilenceTimer()
    this.state.dictationActive = false

    if (isDictationActive()) {
      stopDictation()
    }
  }

  /**
   * Handle a whisper process crash.
   * Retry up to maxWhisperCrashRetries times.
   */
  private handleWhisperCrash(): void {
    this.state.whisperCrashCount++

    if (this.state.whisperCrashCount <= this.state.maxWhisperCrashRetries) {
      console.warn(
        `[Widget] Whisper crashed (attempt ${this.state.whisperCrashCount}/${this.state.maxWhisperCrashRetries}). Restarting...`
      )
      // Retry dictation
      this.startDictationMode().catch((err) => {
        console.error('[Widget] Dictation retry failed:', err)
        this.sendToWidget('widget:dictation-error', {
          error: 'Dictation unavailable — whisper process error.'
        })
      })
    } else {
      console.error('[Widget] Whisper crashed too many times. Disabling dictation.')
      this.sendToWidget('widget:dictation-error', {
        error: 'Dictation unavailable — whisper process error.'
      })
    }
  }

  /**
   * Start the silence detection timer.
   * If the user holds fn but doesn't speak for 15 seconds, auto-finish.
   */
  private startSilenceTimer(): void {
    this.clearSilenceTimer()
    this.state.silenceTimer = setTimeout(() => {
      console.debug('[Widget] Silence timeout reached. Auto-finishing dictation.')
      this.stopDictationMode()
      this.sendToWidget('widget:dictation-complete', { text: '', silent: true })
      this.hide()
    }, SILENCE_TIMEOUT_MS)
  }

  /**
   * Clear the silence detection timer.
   */
  private clearSilenceTimer(): void {
    if (this.state.silenceTimer) {
      clearTimeout(this.state.silenceTimer)
      this.state.silenceTimer = null
    }
  }

  /**
   * Insert transcribed text at the current cursor position.
   * Uses the existing injectKey pattern or sends text via IPC to the active note's editor.
   */
  insertTextAtCursor(text: string): void {
    // Send to all renderer windows to insert at cursor
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed() && win !== this.widgetWindow) {
        win.webContents.send('widget:insert-text', { text })
      }
    }
  }

  /**
   * Send a message to the widget window.
   */
  private sendToWidget(channel: string, data: unknown): void {
    if (this.widgetWindow && !this.widgetWindow.isDestroyed()) {
      this.widgetWindow.webContents.send(channel, data)
    }
  }

  /**
   * Get the current widget state.
   */
  getState(): WidgetState {
    return { ...this.state }
  }

  /**
   * Check if the widget is visible.
   */
  isVisible(): boolean {
    return this.state.visible
  }

  /**
   * Get the current mode.
   */
  getMode(): WidgetMode {
    return this.state.mode
  }

  /**
   * Set the current dictation model.
   */
  setModel(model: WhisperModel): void {
    this.state.currentModel = model
  }

  /**
   * Get the current dictation model.
   */
  getModel(): WhisperModel {
    return this.state.currentModel
  }

  /**
   * Set microphone permission state.
   */
  setMicPermission(granted: boolean): void {
    this.state.micPermissionCached = true
    this.state.micPermissionGranted = granted
  }

  /**
   * Check if dictation is available (whisper binary + model).
   */
  async isDictationAvailable(): Promise<{ available: boolean; error?: string }> {
    if (!isWhisperBinaryAvailable()) {
      return { available: false, error: 'Whisper binary not found' }
    }

    const modelStatus = await getModelStatus()
    if (!modelStatus.installed) {
      return { available: false, error: 'Model not installed' }
    }

    return { available: true }
  }

  /**
   * Clean up resources.
   */
  destroy(): void {
    this.remove()
  }

  /**
   * Remove the widget (lifecycle terminal state).
   * Hides and closes the widget window, releasing all resources.
   * This is the single authoritative removal path.
   */
  remove(): void {
    this.hide()
    if (this.widgetWindow && !this.widgetWindow.isDestroyed()) {
      this.widgetWindow.close()
      this.widgetWindow = null
    }
  }

  /**
   * Enable or disable the widget.
   * When enabled, the widget can be shown via shortcut or fn key.
   */
  setEnabled(enabled: boolean, shortcut?: string): void {
    if (enabled) {
      this.start(shortcut)
    } else {
      this.stop()
    }
  }

  /**
   * Start the widget (register global shortcut if provided).
   */
  private start(shortcut?: string): void {
    if (shortcut) {
      this.state.currentShortcut = shortcut
    }
    // Widget is ready to be shown
  }

  /**
   * Stop the widget.
   */
  private stop(): void {
    this.hide()
  }

  /**
   * Set the keyboard shortcut for the widget (in-memory state owner).
   * Persistence of the shortcut is owned by the settings layer; this method
   * is the single in-memory owner and is the only place the runtime shortcut
   * is mutated.
   */
  setShortcut(shortcut: string): void {
    this.state.currentShortcut = shortcut
  }

  /**
   * Initialize / restore the widget lifecycle owner.
   *
   * This is the single authoritative entry point for the Persist + Restore
   * lifecycle stages. It loads the persisted shortcut from settings and
   * enables the widget. All other callers (IPC handlers, bootstrap) must
   * route initialization through this method rather than duplicating the
   * loadSettings → setEnabled sequence.
   */
  async initialize(): Promise<void> {
    try {
      const settings = await loadSettings()
      this.setEnabled(true, settings.clipboardShortcut)
    } catch (err) {
      console.error('[WidgetManager] Failed to initialize widget:', err)
      this.setEnabled(true)
    }
  }
}

// Singleton instance
export const widgetManager = new WidgetManager()

// ---------------------------------------------------------------------------
// Fn key event wiring
// ---------------------------------------------------------------------------

/**
 * Wire the fn-monitor events to the widget manager.
 * Called from index.ts after the fn-monitor is started.
 */
export function wireFnMonitorToWidget(): void {
  fnMonitor.on('fn-down', () => {
    // Show the widget in dictation mode when fn is held
    widgetManager.show('dictation')
  })

  fnMonitor.on('fn-up', () => {
    // If dictation is active, stop it (whisper will finish transcription)
    // The widget will auto-hide after transcription is complete
    if (widgetManager.getState().dictationActive) {
      stopDictation()
    }
  })
}
