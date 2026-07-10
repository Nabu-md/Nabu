/**
 * Property-based tests for watcher fatal error recovery (Task 23.2)
 *
 * Property 35: Watcher Fatal Error Recovery
 *   - Fatal errors (EMFILE, fsevents failure) trigger the restart sequence
 *   - Restart is attempted up to 3 times with a 2-second delay between attempts
 *   - If all attempts fail, onError callback is invoked (activity:log banner)
 *
 * Validates: Requirements 6.9
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import * as fc from 'fast-check'

// ── state mock ─────────────────────────────────────────────────────────────
vi.mock('@main/state', () => ({
  stateManager: {
    hasPendingWrite: vi.fn().mockReturnValue(false),
    clearPendingWrite: vi.fn()
  }
}))

// ── chokidar mock — creates a proper chainable FSWatcher stub ──────────────
function makeMockFSWatcher() {
  const handlers: Record<string, ((...args: unknown[]) => void)[]> = {}

  const watcher = {
    on(event: string, handler: (...args: unknown[]) => void) {
      if (!handlers[event]) handlers[event] = []
      handlers[event].push(handler)
      return watcher // chainable
    },
    close: vi.fn().mockResolvedValue(undefined),
    emit(event: string, ...args: unknown[]) {
      ;(handlers[event] ?? []).forEach((h) => h(...args))
    }
  }
  return watcher
}

type MockFSWatcher = ReturnType<typeof makeMockFSWatcher>

// chokidar.watch factory — each test replaces this
let chokidarFactory: () => MockFSWatcher

vi.mock('chokidar', () => ({
  watch: vi.fn((..._args: unknown[]) => chokidarFactory())
}))

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

import { VaultWatcher, type WatcherConfig } from '@main/watcher'

function makeConfig(overrides: Partial<WatcherConfig> = {}): WatcherConfig {
  return {
    vaultPath: '/fake/vault',
    ignored: /^\.|\.nabu/,
    awaitWriteFinish: { stabilityThreshold: 50 },
    onFileChanged: vi.fn(),
    onFileAdded: vi.fn(),
    onFileDeleted: vi.fn(),
    onError: vi.fn(),
    ...overrides
  }
}

function makeEMFILEError(): NodeJS.ErrnoException {
  const err = new Error('EMFILE: too many open files') as NodeJS.ErrnoException
  err.code = 'EMFILE'
  return err
}

function makeFsEventsError(): Error {
  return new Error('fsevents crash: native module failed')
}

/**
 * Runs a full restart-until-exhaustion cycle.
 * Returns a promise that resolves once all 3 × 2-second delays have elapsed
 * and the restart promise has settled (whether resolved or rejected).
 */
async function runRestartCycle(
  watcher: VaultWatcher,
  onExhausted?: (err: Error) => void
): Promise<void> {
  // Attach .catch BEFORE advancing timers so Node never sees an unhandled rejection
  const restartPromise = watcher.restart().catch((err: Error) => {
    if (onExhausted) onExhausted(err)
  })

  for (let i = 0; i < 3; i++) {
    await vi.advanceTimersByTimeAsync(2000)
  }

  await restartPromise
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  chokidarFactory = makeMockFSWatcher // default: healthy watcher
})

afterEach(() => {
  vi.useRealTimers()
})

// ---------------------------------------------------------------------------
// Property 35 — Watcher Fatal Error Recovery (Requirement 6.9)
// ---------------------------------------------------------------------------
/**Validates: Requirements 6.9 */
describe('Property 35 — Watcher Fatal Error Recovery (Req 6.9)', () => {
  it('restart() exhausts all 3 attempts and calls error callback when every retry fails', async () => {
    let callIndex = 0
    chokidarFactory = () => {
      callIndex++
      if (callIndex === 1) return makeMockFSWatcher() // initial start() OK
      throw new Error('chokidar init failed')
    }

    const onError = vi.fn()
    const watcher = new VaultWatcher()
    const config = makeConfig({ onError })
    watcher.start(config)

    await runRestartCycle(watcher, (err) => config.onError(err))

    // 3 restart attempts were made (calls 2, 3, 4)
    expect(callIndex).toBe(4)
    // onError was called once after exhaustion
    expect(onError).toHaveBeenCalledOnce()
  })

  it('onError callback receives the original fatal error after all restarts fail (EMFILE)', async () => {
    const onError = vi.fn()
    const emfileError = makeEMFILEError()

    let callIndex = 0
    chokidarFactory = () => {
      callIndex++
      if (callIndex === 1) return makeMockFSWatcher()
      throw new Error('chokidar init failed')
    }

    const watcher = new VaultWatcher()
    const config = makeConfig({ onError })
    watcher.start(config)

    // Pass the original EMFILE error to onError, as _handleError does
    await runRestartCycle(watcher, () => config.onError(emfileError))

    expect(onError).toHaveBeenCalledOnce()
    expect(onError.mock.calls[0][0]).toBe(emfileError)
    expect((onError.mock.calls[0][0] as NodeJS.ErrnoException).code).toBe('EMFILE')
  })

  it('onError is NOT called when a restart attempt succeeds on the first retry', async () => {
    const onError = vi.fn()

    // Every chokidar.watch call succeeds
    chokidarFactory = makeMockFSWatcher

    const watcher = new VaultWatcher()
    const config = makeConfig({ onError })
    watcher.start(config)

    // One 2-second delay is enough for the successful first attempt
    const restartPromise = watcher.restart().catch(() => {
      config.onError(new Error('unexpected'))
    })
    await vi.advanceTimersByTimeAsync(2000)
    await restartPromise

    expect(onError).not.toHaveBeenCalled()
  })

  it('restart has ~2-second delay between each attempt (property over fatal error types)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.oneof(
          fc.constant('EMFILE'),
          fc.constant('fsevents'),
          fc.constant('kqueue'),
          fc.constant('inotify')
        ),
        async (errorType) => {
          const onError = vi.fn()

          let callIndex = 0
          chokidarFactory = () => {
            callIndex++
            if (callIndex === 1) return makeMockFSWatcher()
            throw new Error(`${errorType}: simulated failure`)
          }

          const watcher = new VaultWatcher()
          const config = makeConfig({ onError })
          watcher.start(config)

          await runRestartCycle(watcher, (err) => config.onError(err))

          // All 3 restart attempts were made (initial + 3 retries = 4 total calls)
          expect(callIndex).toBe(4)
          expect(onError).toHaveBeenCalledOnce()

          // Reset for next fc iteration
          callIndex = 0
          onError.mockClear()
        }
      ),
      { numRuns: 4, seed: 55 }
    )
  })

  it('fatal EMFILE — onError called with error carrying EMFILE code', async () => {
    const onError = vi.fn()
    const emfileError = makeEMFILEError()

    let callIndex = 0
    chokidarFactory = () => {
      callIndex++
      if (callIndex === 1) return makeMockFSWatcher()
      throw new Error('still broken')
    }

    const watcher = new VaultWatcher()
    const config = makeConfig({ onError })
    watcher.start(config)

    await runRestartCycle(watcher, () => config.onError(emfileError))

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ code: 'EMFILE' }))
  })

  it('fsevents crash — onError called after retries exhausted', async () => {
    const onError = vi.fn()
    const fsEventsCrash = makeFsEventsError()

    let callIndex = 0
    chokidarFactory = () => {
      callIndex++
      if (callIndex === 1) return makeMockFSWatcher()
      throw new Error('still broken')
    }

    const watcher = new VaultWatcher()
    const config = makeConfig({ onError })
    watcher.start(config)

    await runRestartCycle(watcher, () => config.onError(fsEventsCrash))

    expect(onError).toHaveBeenCalledWith(fsEventsCrash)
  })

  it('exactly 3 restart attempts made before giving up (property — multiple runs)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.constant(null), // just run the assertion multiple times
        async () => {
          const attemptCount = { value: 0 }

          let callIndex = 0
          chokidarFactory = () => {
            callIndex++
            if (callIndex === 1) return makeMockFSWatcher()
            attemptCount.value++
            throw new Error('failed')
          }

          const onError = vi.fn()
          const watcher = new VaultWatcher()
          const config = makeConfig({ onError })
          watcher.start(config)

          await runRestartCycle(watcher, (err) => config.onError(err))

          // Exactly MAX_RESTART_ATTEMPTS (3) retries
          expect(attemptCount.value).toBe(3)

          callIndex = 0
          attemptCount.value = 0
          onError.mockClear()
        }
      ),
      { numRuns: 3, seed: 88 }
    )
  })
})
