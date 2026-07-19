/**
 * clipboard-history.ts
 *
 * Clipboard watcher service that polls `Electron.clipboard.readText()` to
 * build a persistent history of copied text items.
 *
 * - Polls every 500 ms while active (only when there's a Nabu window open)
 * - Dedup: consecutive identical copies are collapsed into one entry
 * - Persisted to userData/clipboard-history.json (max ~50 entries)
 *
 * Used by the clipboard widget for multi-clipboard paste.
 */

import { app, clipboard } from 'electron'
import { join } from 'path'
import fs from 'fs/promises'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ClipboardEntry {
  /** UUID-like short id for React key usage. */
  id: string
  /** The copied text content. */
  text: string
  /** Unix-epoch timestamp (ms) when the copy was detected. */
  timestamp: number
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default max entries kept in history. */
const DEFAULT_MAX_ENTRIES = 50

/** Poll interval in ms. */
const POLL_INTERVAL_MS = 500

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8)
}

// ---------------------------------------------------------------------------
// ClipboardHistory
// ---------------------------------------------------------------------------

export class ClipboardHistory {
  private history: ClipboardEntry[] = []
  private lastText: string = ''
  private pollTimer: ReturnType<typeof setInterval> | null = null
  private storagePath: string
  private maxEntries: number

  constructor(maxEntries: number = DEFAULT_MAX_ENTRIES) {
    this.maxEntries = maxEntries
    this.storagePath = join(app.getPath('userData'), 'clipboard-history.json')
  }

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  /** Start polling the clipboard. */
  start(): void {
    if (this.pollTimer) return
    this.load().catch(() => {})
    this.lastText = clipboard.readText()
    this.pollTimer = setInterval(() => this.poll(), POLL_INTERVAL_MS)
  }

  /** Stop polling the clipboard. */
  stop(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer)
      this.pollTimer = null
    }
  }

  // ---------------------------------------------------------------------------
  // Public API
  // ---------------------------------------------------------------------------

  /** Get the current history (newest first). */
  getHistory(): ClipboardEntry[] {
    return [...this.history]
  }

  /** Get the last N entries (newest first). */
  getRecent(max: number): ClipboardEntry[] {
    return this.history.slice(0, max)
  }

  /** Clear all history and persist. */
  async clear(): Promise<void> {
    this.history = []
    this.lastText = ''
    await this.save().catch(() => {})
  }

  /**
   * Write text to the system clipboard and add it to history without
   * the poller re-adding it.
   */
  copyToClipboard(text: string): void {
    clipboard.writeText(text)
    this.addEntry(text)
    this.save().catch((err) =>
      console.error('[ClipboardHistory] Failed to persist after copy:', err)
    )
  }

  /** Manually add an entry (used when the widget copies to clipboard). */
  async push(text: string): Promise<void> {
    if (!text || text === this.lastText) return
    this.addEntry(text)
    await this.save().catch(() => {})
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private poll(): void {
    const text = clipboard.readText()
    if (!text || text === this.lastText) return
    this.lastText = text
    this.addEntry(text)
    this.save().catch((err) =>
      console.error('[ClipboardHistory] Failed to persist history:', err)
    )
  }

  private addEntry(text: string): void {
    // Update lastText so the poller won't re-add this entry
    this.lastText = text

    const entry: ClipboardEntry = {
      id: generateId(),
      text,
      timestamp: Date.now()
    }
    this.history.unshift(entry)

    // Trim to max entries
    if (this.history.length > this.maxEntries) {
      this.history = this.history.slice(0, this.maxEntries)
    }
  }

  // ---------------------------------------------------------------------------
  // Persistence
  // ---------------------------------------------------------------------------

  private async load(): Promise<void> {
    try {
      const raw = await fs.readFile(this.storagePath, 'utf-8')
      const parsed = JSON.parse(raw)
      if (Array.isArray(parsed)) {
        this.history = parsed as ClipboardEntry[]
        // Restore lastText so we don't re-add the latest entry
        if (this.history.length > 0) {
          this.lastText = this.history[0].text
        }
      }
    } catch {
      // File doesn't exist or is corrupt — start fresh
      this.history = []
    }
  }

  private async save(): Promise<void> {
    const dir = join(app.getPath('userData'))
    // Ensure directory exists
    await fs.mkdir(dir, { recursive: true }).catch(() => {})
    await fs.writeFile(this.storagePath, JSON.stringify(this.history, null, 2), 'utf-8')
  }
}
