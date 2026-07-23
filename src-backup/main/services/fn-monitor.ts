/**
 * fn-monitor.ts
 *
 * Monitors the fn (Function) key on macOS using a Swift helper process.
 * Emits 'fn-down' and 'fn-up' events to the widget manager.
 *
 * The fn key on macOS does not generate a standard keydown/keyup event that
 * can be captured from JavaScript. Instead, we use a lightweight Swift helper
 * that uses IOKit to monitor the fn key state and communicates via stdout.
 *
 * Requirements: 41.4, 42.3
 */

import { spawn, ChildProcess } from 'child_process'
import path from 'path'
import { EventEmitter } from 'events'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FnEvent {
  type: 'fn-down' | 'fn-up'
  timestamp: number
}

// ---------------------------------------------------------------------------
// FnMonitor
// ---------------------------------------------------------------------------

class FnMonitor extends EventEmitter {
  private process: ChildProcess | null = null
  private running = false
  private buffer = ''

  /**
   * Get the path to the fn-monitor Swift helper.
   */
  private getHelperPath(): string {
    const candidates = [
      path.join(process.resourcesPath, 'fn-monitor.swift'),
      path.join(process.cwd(), 'scripts', 'fn-monitor.swift'),
      path.join(process.cwd(), 'fn-monitor.swift')
    ]

    for (const candidate of candidates) {
      try {
        require('fs').accessSync(candidate)
        return candidate
      } catch {
        // Try next candidate
      }
    }

    return candidates[0]
  }

  /**
   * Start monitoring the fn key.
   * Spawns the Swift helper and listens for fn-down/fn-up events on stdout.
   */
  start(): void {
    if (this.running) return

    const helperPath = this.getHelperPath()

    try {
      require('fs').accessSync(helperPath)
    } catch {
      console.warn('[FnMonitor] Helper script not found at:', helperPath)
      console.warn('[FnMonitor] Fn key monitoring disabled. Dictation will require manual start.')
      this.running = false
      return
    }

    this.running = true

    // Spawn the Swift helper
    this.process = spawn('swift', [helperPath], {
      stdio: ['pipe', 'pipe', 'pipe']
    })

    this.process.stdout?.on('data', (data: Buffer) => {
      this.buffer += data.toString()
      this.processBuffer()
    })

    this.process.stderr?.on('data', (data: Buffer) => {
      console.debug('[FnMonitor] stderr:', data.toString().trim())
    })

    this.process.on('error', (err) => {
      console.error('[FnMonitor] Process error:', err)
      this.running = false
    })

    this.process.on('close', (code) => {
      console.debug('[FnMonitor] Process exited with code:', code)
      this.process = null
      this.running = false
    })
  }

  /**
   * Process the stdout buffer, extracting JSON lines.
   */
  private processBuffer(): void {
    const lines = this.buffer.split('\n')
    // Keep the last incomplete line in the buffer
    this.buffer = lines.pop() ?? ''

    for (const line of lines) {
      const trimmed = line.trim()
      if (!trimmed) continue

      try {
        const event = JSON.parse(trimmed) as FnEvent
        if (event.type === 'fn-down' || event.type === 'fn-up') {
          this.emit(event.type, event)
        }
      } catch {
        // Ignore malformed JSON lines
        console.debug('[FnMonitor] Ignoring malformed line:', trimmed)
      }
    }
  }

  /**
   * Stop monitoring the fn key.
   */
  stop(): void {
    if (this.process) {
      this.process.kill('SIGTERM')
      this.process = null
    }
    this.running = false
  }

  /**
   * Check if the monitor is running.
   */
  isRunning(): boolean {
    return this.running
  }
}

// Singleton instance
export const fnMonitor = new FnMonitor()
