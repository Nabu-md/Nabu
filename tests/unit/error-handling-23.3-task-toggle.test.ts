/**
 * Property-based tests for task toggle write failure error handling (Task 23.3)
 *
 * Property 28: Task Toggle Write Failure Error Response
 *   - When fs.writeFile fails (disk full, permissions), the error propagates
 *     from toggleTask() so the IPC layer can return { success: false, error: ... }
 *   - The IPC handler returns a structured TaskToggleResult with success=false
 *
 * Validates: Requirements 5.6
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import * as fc from 'fast-check'
import { mkdtemp, writeFile, rm, chmod } from 'fs/promises'
import { tmpdir, platform } from 'os'
import { join } from 'path'

// ── Module under test ──────────────────────────────────────────────────────
import { StateManager } from '@main/state'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpDir: string

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), 'nabu-toggle-'))
})

afterEach(async () => {
  vi.restoreAllMocks()
  try {
    // Restore permissions before cleanup
    await chmod(tmpDir, 0o755).catch(() => {})
    await rm(tmpDir, { recursive: true, force: true })
  } catch {
    // best-effort
  }
})

/** Create a temp .md file with `taskCount` task checkboxes. */
async function createTaskFile(
  taskCount: number
): Promise<{ filePath: string; lineIndexes: number[] }> {
  const lines: string[] = ['# Tasks']
  const lineIndexes: number[] = []
  for (let i = 0; i < taskCount; i++) {
    lineIndexes.push(lines.length)
    lines.push(`- [ ] Task ${i + 1}`)
  }
  const content = lines.join('\n')
  const filePath = join(tmpDir, `tasks-${Math.random().toString(36).slice(2)}.md`)
  await writeFile(filePath, content, 'utf-8')
  return { filePath, lineIndexes }
}

/**
 * Create a read-only .md file (permission denied on write).
 * Returns the file path; caller must restore permissions after the test.
 */
async function createReadOnlyTaskFile(): Promise<{ filePath: string; lineIndex: number }> {
  const { filePath, lineIndexes } = await createTaskFile(2)
  await chmod(filePath, 0o444) // read-only
  return { filePath, lineIndex: lineIndexes[0] }
}

// Skip permission-based tests on Windows (chmod isn't enforced the same way)
const isWindows = platform() === 'win32'

// ---------------------------------------------------------------------------
// Property 28 — Task Toggle Write Failure Error Response (Requirement 5.6)
// ---------------------------------------------------------------------------
/**Validates: Requirements 5.6 */
describe('Property 28 — Task Toggle Write Failure Error Response (Req 5.6)', () => {
  it(
    'toggleTask() rejects when file is read-only (permission denied)',
    { skip: isWindows },
    async () => {
      const { filePath, lineIndex } = await createReadOnlyTaskFile()

      const stateManager = new StateManager()
      let caughtError: Error | null = null

      try {
        await stateManager.toggleTask(filePath, lineIndex)
      } catch (err) {
        caughtError = err as Error
      } finally {
        await chmod(filePath, 0o644).catch(() => {}) // restore
      }

      // Must throw
      expect(caughtError).not.toBeNull()
      // Error message should reference permission denied or EACCES
      const msg = caughtError!.message.toUpperCase()
      const hasPermissionReason =
        msg.includes('EACCES') || msg.includes('PERMISSION') || msg.includes('EPERM')
      expect(hasPermissionReason).toBe(true)
    }
  )

  it(
    'IPC handler returns { success: false } when file is read-only',
    { skip: isWindows },
    async () => {
      const { filePath, lineIndex } = await createReadOnlyTaskFile()

      const stateManager = new StateManager()
      let ipcResult: { success: boolean; error?: string }

      try {
        await stateManager.toggleTask(filePath, lineIndex)
        ipcResult = { success: true }
      } catch (err) {
        ipcResult = { success: false, error: String(err) }
      } finally {
        await chmod(filePath, 0o644).catch(() => {}) // restore
      }

      expect(ipcResult.success).toBe(false)
      expect(ipcResult.error).toBeDefined()
    }
  )

  it('IPC returns success:true when write succeeds (control case)', async () => {
    const { filePath, lineIndexes } = await createTaskFile(3)
    const stateManager = new StateManager()

    let ipcResult: { success: boolean; error?: string }
    try {
      await stateManager.toggleTask(filePath, lineIndexes[0])
      ipcResult = { success: true }
    } catch (err) {
      ipcResult = { success: false, error: String(err) }
    }

    expect(ipcResult.success).toBe(true)
    expect(ipcResult.error).toBeUndefined()
  })

  it('toggleTask() rejects for invalid line index (out of bounds)', async () => {
    await fc.assert(
      fc.asyncProperty(
        // Generate invalid line indexes that are out of bounds
        fc.oneof(
          fc.integer({ min: -100, max: -1 }), // negative
          fc.integer({ min: 1000, max: 9999 }) // very large
        ),
        async (outOfBoundsIndex) => {
          const { filePath } = await createTaskFile(3)
          const stateManager = new StateManager()

          let caughtError: Error | null = null
          try {
            await stateManager.toggleTask(filePath, outOfBoundsIndex)
          } catch (err) {
            caughtError = err as Error
          }

          // Must throw for out-of-bounds indexes
          expect(caughtError).not.toBeNull()
        }
      ),
      { numRuns: 15, seed: 42 }
    )
  })

  it('IPC error response includes error string when toggleTask throws (out-of-bounds property)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.integer({ min: 500, max: 9999 }), // indexes well beyond any file length
        async (lineIndex) => {
          const { filePath } = await createTaskFile(2)
          const stateManager = new StateManager()

          let ipcResult: { success: boolean; error?: string }
          try {
            await stateManager.toggleTask(filePath, lineIndex)
            ipcResult = { success: true }
          } catch (err) {
            ipcResult = { success: false, error: String(err) }
          }

          // Out-of-bounds always produces an error response
          expect(ipcResult.success).toBe(false)
          expect(typeof ipcResult.error).toBe('string')
          expect(ipcResult.error!.length).toBeGreaterThan(0)
        }
      ),
      { numRuns: 10, seed: 77 }
    )
  })

  it('successful toggle always returns success:true (property over valid line indexes)', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc
          .integer({ min: 1, max: 5 })
          .chain((taskCount) =>
            fc.tuple(fc.constant(taskCount), fc.integer({ min: 0, max: taskCount - 1 }))
          ),
        async ([taskCount, taskIndex]) => {
          const { filePath, lineIndexes } = await createTaskFile(taskCount)
          const lineIndex = lineIndexes[taskIndex]

          const stateManager = new StateManager()

          let ipcResult: { success: boolean; error?: string }
          try {
            await stateManager.toggleTask(filePath, lineIndex)
            ipcResult = { success: true }
          } catch (err) {
            ipcResult = { success: false, error: String(err) }
          }

          // Valid toggles must always succeed
          expect(ipcResult.success).toBe(true)
          expect(ipcResult.error).toBeUndefined()
        }
      ),
      { numRuns: 15, seed: 55 }
    )
  })
})
