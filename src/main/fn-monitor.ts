/**
 * fn-monitor.ts
 *
 * Bridge between the Swift CGEventTap helper and Electron's main process.
 * Spawns `fn-monitor.swift` as a child process, parses JSON events from
 * stdout, and emits typed events.
 *
 * Events:
 *   'fn-down'   → fn key pressed
 *   'fn-up'     → fn key released
 *   'nav-up'    → arrow up while fn held
 *   'nav-down'  → arrow down while fn held
 *   'nav-left'  → arrow left while fn held
 *   'nav-right' → arrow right while fn held
 *   'error'     → monitor error / permission denied
 *
 * The monitor requires Accessibility permissions.
 * If permissions are denied, it exits immediately with an error event.
 */

import { EventEmitter } from 'events'
import { spawn, type ChildProcess } from 'child_process'
import * as path from 'path'
import { app } from 'electron'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FnMonitorEvent {
  event: string
  message?: string
}

// ---------------------------------------------------------------------------
// FnMonitor
// ---------------------------------------------------------------------------

export class FnMonitor extends EventEmitter {
  private proc: ChildProcess | null = null
  private _running: boolean = false
  private restartTimer: ReturnType<typeof setTimeout> | null = null

  /** Whether the process is currently active. */
  get running(): boolean {
    return this._running
  }

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  /**
   * Start the fn monitoring process. In development mode it runs the .swift
   * file directly via the `swift` interpreter. In production it expects a
   * pre-compiled binary.
   */
  start(): void {
    if (this._running) return
    this.spawnProcess()
  }

  /** Stop the monitoring process. */
  stop(): void {
    if (this.restartTimer) {
      clearTimeout(this.restartTimer)
      this.restartTimer = null
    }
    this.killProcess()
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private spawnProcess(): void {
    // Resolve the script/binary path
    const isPackaged = app.isPackaged
    const scriptPath = isPackaged
      ? path.join(process.resourcesPath, 'fn-monitor')
      : path.join(app.getAppPath(), 'scripts', 'fn-monitor.swift')

    try {
      if (isPackaged) {
        // Pre-compiled binary
        this.proc = spawn(scriptPath, [], {
          stdio: ['pipe', 'pipe', 'pipe']
        })
      } else {
        // Run via swift interpreter
        this.proc = spawn('swift', [scriptPath], {
          stdio: ['pipe', 'pipe', 'pipe']
        })
      }
    } catch (err) {
      console.error('[FnMonitor] Failed to spawn process:', err)
      this.emit('error', { event: 'error', message: String(err) })
      return
    }

    this._running = true

    // Parse stdout JSON lines
    this.proc.stdout?.on('data', (data: Buffer) => {
      const lines = data.toString().split('\n').filter(Boolean)
      for (const line of lines) {
        try {
          const parsed = JSON.parse(line) as FnMonitorEvent
          this.emit(parsed.event, parsed)
        } catch {
          // Ignore non-JSON output
        }
      }
    })

    // Capture stderr for debugging
    this.proc.stderr?.on('data', (data: Buffer) => {
      console.error('[FnMonitor] stderr:', data.toString().trim())
    })

    // Handle exit
    this.proc.on('exit', (code, signal) => {
      this._running = false
      this.proc = null

      if (code === 1) {
        // Permission error — don't restart
        console.warn('[FnMonitor] Process exited with code 1 (likely permission denied)')
        this.emit('error', { event: 'error', message: 'Accessibility permission denied' })
      } else if (code !== 0 && signal === null) {
        // Unexpected exit — restart after a delay
        console.warn(`[FnMonitor] Process exited unexpectedly (code=${code}), restarting...`)
        this.restartTimer = setTimeout(() => this.spawnProcess(), 2000)
      }
    })

    this.proc.on('error', (err) => {
      console.error('[FnMonitor] Process error:', err)
      this._running = false
      this.emit('error', { event: 'error', message: String(err) })
    })
  }

  private killProcess(): void {
    if (this.proc) {
      try {
        this.proc.kill('SIGTERM')
      } catch {
        // process already dead
      }
      this.proc = null
    }
    this._running = false
  }
}
