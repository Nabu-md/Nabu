/**
 * watcher.ts
 *
 * VaultWatcher — monitors the vault folder for file-system changes using
 * chokidar (fsevents on macOS). Implements per-file debouncing, the
 * Pending_Write_Lock check to distinguish app-initiated writes from external
 * edits, and an automatic restart sequence for fatal watcher errors.
 *
 * Requirements: 1.4, 5.5, 6.1, 6.2, 6.7, 6.8, 6.9, 39.2
 */

import { watch as chokidarWatch, FSWatcher } from 'chokidar'
import { stateManager } from './state'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface WatcherConfig {
  vaultPath: string
  /** Patterns to ignore — default: /^\.|\.nabu/ */
  ignored: RegExp
  awaitWriteFinish: { stabilityThreshold: number }
  /** Called when a file is changed by an external editor (isExternal=true) */
  onFileChanged: (filePath: string, isExternal: boolean) => void
  /** Called when a new .md file appears in the vault */
  onFileAdded: (filePath: string) => void
  /** Called when an image file is added to trigger OCR (Req 39.2) */
  onImageAdded?: (filePath: string) => void
  /** Called when a .md file is deleted from the vault */
  onFileDeleted: (filePath: string) => void
  /** Called on watcher errors (after restart attempts are exhausted) */
  onError: (error: Error) => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Per-file debounce window in milliseconds (Requirement 6.2) */
const DEBOUNCE_MS = 50

/** Maximum automatic restart attempts before giving up (Requirement 6.8) */
const MAX_RESTART_ATTEMPTS = 3

/** Delay between restart attempts in milliseconds (Requirement 6.8) */
const RESTART_DELAY_MS = 2000

/** Error codes treated as fatal (trigger restart sequence) */
const FATAL_ERROR_CODES = new Set(['EMFILE', 'ENFILE'])

// ---------------------------------------------------------------------------
// VaultWatcher
// ---------------------------------------------------------------------------

/**
 * Manages a chokidar FSWatcher for the open vault. Responsibilities:
 *
 * - Start / stop the underlying watcher
 * - Per-file debouncing (50 ms) to coalesce rapid write sequences
 * - Pending_Write_Lock check: skip re-parse when Nabu itself wrote the file
 * - Automatic restart (up to 3 attempts, 2 s apart) on fatal errors
 * - Callback-based event forwarding to the IPC layer
 */
export class VaultWatcher {
  private watcher: FSWatcher | null = null
  private restartAttempts: number = 0
  private debounceTimers: Map<string, NodeJS.Timeout> = new Map()
  private config: WatcherConfig | null = null

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Start watching the vault at `config.vaultPath`.
   *
   * Configures chokidar to:
   * - Ignore dot-prefixed paths and the `.nabu/` cache directory
   * - Use `awaitWriteFinish` with a 50 ms stability threshold to avoid
   *   partial-read events on large files
   * - Only report events for `.md` files (other files are filtered in handlers)
   *
   * Requirements: 6.1, 6.2
   */
  start(config: WatcherConfig): void {
    // Stop any existing watcher before starting a new one so we never stack
    // two chokidar instances on top of each other (second vault:open call,
    // hot-reload, or repeated open-vault flow would otherwise cause doubled
    // events and a black-screen crash).
    this.stop()

    // Store config so restart() can reuse it
    this.config = config
    this.restartAttempts = 0
    this._startWatcher(config)
  }

  /**
   * Stop the watcher and cancel all pending debounce timers.
   */
  stop(): void {
    this._clearAllDebounceTimers()

    if (this.watcher) {
      this.watcher.close().catch((err) => {
        console.warn('[VaultWatcher] Error closing watcher:', err)
      })
      this.watcher = null
    }
  }

  // -------------------------------------------------------------------------
  // Internal — watcher lifecycle
  // -------------------------------------------------------------------------

  /**
   * Instantiate and wire up a new chokidar watcher from the given config.
   * Called both by `start()` and by `restart()`.
   */
  private _startWatcher(config: WatcherConfig): void {
    const fsWatcher = chokidarWatch(config.vaultPath, {
      ignored: config.ignored,
      persistent: true,
      ignoreInitial: true,
      awaitWriteFinish: {
        stabilityThreshold: config.awaitWriteFinish.stabilityThreshold,
        pollInterval: 100
      }
    })

    fsWatcher.on('change', (filePath: string) => {
      if (!filePath.endsWith('.md')) return
      this._scheduleChange(filePath)
    })

    fsWatcher.on('add', (filePath: string) => {
      // Handle markdown files
      if (filePath.endsWith('.md')) {
        this.handleAdd(filePath)
      }
      // Handle image files for OCR (Req 39.2)
      else if (this.config?.onImageAdded && this.isImageFile(filePath)) {
        this.handleImageAdd(filePath)
      }
    })

    fsWatcher.on('unlink', (filePath: string) => {
      if (!filePath.endsWith('.md')) return
      this.handleUnlink(filePath)
    })

    fsWatcher.on('error', (err: unknown) => {
      const error = err instanceof Error ? err : new Error(String(err))
      this._handleError(error)
    })

    this.watcher = fsWatcher
  }

  // -------------------------------------------------------------------------
  // Internal — event handlers
  // -------------------------------------------------------------------------

  /**
   * Schedule a debounced call to `handleChange`. Any existing timer for
   * `filePath` is cancelled so that only the last event in a burst fires.
   *
   * Requirements: 6.2 (50 ms per-file debounce)
   */
  private _scheduleChange(filePath: string): void {
    const existing = this.debounceTimers.get(filePath)
    if (existing) {
      clearTimeout(existing)
    }

    const timer = setTimeout(() => {
      this.debounceTimers.delete(filePath)
      this.handleChange(filePath)
    }, DEBOUNCE_MS)

    this.debounceTimers.set(filePath, timer)
  }

  /**
   * Handle a debounced `change` event.
   *
   * Checks the Pending_Write_Lock first:
   * - If the lock is set, the change was triggered by Nabu itself (task
   *   toggle, etc.) — clear the lock and skip re-parsing.
   * - If the lock is absent, the change came from an external editor —
   *   forward to the callback for re-parsing.
   *
   * Requirements: 5.5, 6.7
   */
  private handleChange(filePath: string): void {
    if (stateManager.hasPendingWrite(filePath)) {
      // App-initiated write: clear the lock, no re-parse needed
      stateManager.clearPendingWrite(filePath)
      return
    }

    // External edit: notify the IPC layer
    if (this.config) {
      this.config.onFileChanged(filePath, /* isExternal */ true)
    }
  }

  /**
   * Handle an `add` event for a new `.md` file.
   *
   * Requirements: 6.9
   */
  private handleAdd(filePath: string): void {
    if (this.config) {
      this.config.onFileAdded(filePath)
    }
  }

  /**
   * Handle an `add` event for a new image file to trigger OCR.
   *
   * Requirements: 39.2
   */
  private handleImageAdd(filePath: string): void {
    if (this.config?.onImageAdded) {
      this.config.onImageAdded(filePath)
    }
  }

  /**
   * Check if a file is an image based on extension.
   *
   * Requirements: 39.2
   */
  private isImageFile(filePath: string): boolean {
    const imageExtensions = ['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.tiff']
    const ext = filePath.toLowerCase().slice(filePath.lastIndexOf('.'))
    return imageExtensions.includes(ext)
  }

  /**
   * Handle an `unlink` event for a deleted `.md` file.
   *
   * Requirements: 6.9
   */
  private handleUnlink(filePath: string): void {
    // Cancel any pending debounce for this path since the file is gone
    const existing = this.debounceTimers.get(filePath)
    if (existing) {
      clearTimeout(existing)
      this.debounceTimers.delete(filePath)
    }

    if (this.config) {
      this.config.onFileDeleted(filePath)
    }
  }

  // -------------------------------------------------------------------------
  // Internal — error handling and restart
  // -------------------------------------------------------------------------

  /**
   * Inspect a watcher error and decide whether to trigger a restart.
   *
   * Fatal errors (EMFILE = too many open files, fsevents crash) initiate the
   * automatic restart sequence. Non-fatal errors are forwarded to `onError`.
   *
   * Requirements: 6.8
   */
  private _handleError(error: Error): void {
    const code = (error as NodeJS.ErrnoException).code ?? ''
    const isFatal = FATAL_ERROR_CODES.has(code) || this._isFsEventsCrash(error)

    if (isFatal) {
      console.error(`[VaultWatcher] Fatal watcher error (${code}), initiating restart:`, error)
      this.restart().catch((restartErr) => {
        console.error('[VaultWatcher] All restart attempts failed:', restartErr)
        if (this.config) {
          this.config.onError(error)
        }
      })
    } else {
      console.warn('[VaultWatcher] Non-fatal watcher error:', error)
      if (this.config) {
        this.config.onError(error)
      }
    }
  }

  /**
   * Heuristic to detect an fsevents crash (macOS-specific).
   * fsevents errors don't always carry standard errno codes, so we also
   * match by message patterns.
   */
  private _isFsEventsCrash(error: Error): boolean {
    const msg = error.message.toLowerCase()
    return msg.includes('fsevents') || msg.includes('kqueue') || msg.includes('inotify')
  }

  /**
   * Attempt to restart the watcher after a fatal error.
   *
   * Tries up to `MAX_RESTART_ATTEMPTS` times, waiting `RESTART_DELAY_MS`
   * between each attempt. Throws if all attempts are exhausted.
   *
   * Requirements: 6.8
   */
  async restart(): Promise<void> {
    if (!this.config) {
      throw new Error('[VaultWatcher] Cannot restart — watcher was never started.')
    }

    // Tear down the current (broken) watcher
    this._clearAllDebounceTimers()
    if (this.watcher) {
      try {
        await this.watcher.close()
      } catch {
        // Ignore close errors during restart
      }
      this.watcher = null
    }

    while (this.restartAttempts < MAX_RESTART_ATTEMPTS) {
      this.restartAttempts++
      console.log(`[VaultWatcher] Restart attempt ${this.restartAttempts}/${MAX_RESTART_ATTEMPTS}…`)

      await this._delay(RESTART_DELAY_MS)

      try {
        this._startWatcher(this.config)
        console.log(`[VaultWatcher] Restart successful on attempt ${this.restartAttempts}.`)
        return
      } catch (err) {
        console.error(`[VaultWatcher] Restart attempt ${this.restartAttempts} failed:`, err)
      }
    }

    throw new Error(
      `[VaultWatcher] Watcher failed to restart after ${MAX_RESTART_ATTEMPTS} attempts.`
    )
  }

  // -------------------------------------------------------------------------
  // Utilities
  // -------------------------------------------------------------------------

  /** Cancel all pending per-file debounce timers. */
  private _clearAllDebounceTimers(): void {
    for (const timer of this.debounceTimers.values()) {
      clearTimeout(timer)
    }
    this.debounceTimers.clear()
  }

  /** Promise-based sleep helper. */
  private _delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms))
  }
}

// ---------------------------------------------------------------------------
// Singleton export removed - unused
// ---------------------------------------------------------------------------

// Note: vaultWatcher singleton was never imported; new instances are created in index.ts
