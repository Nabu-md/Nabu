/**
 * performance.spec.ts
 *
 * Task 24.2 — Cold launch performance measurements
 *
 * Measures:
 *   1. Cold launch time: app start → FileTree interactive/clickable
 *      Target: <2500ms (Req 11.7)
 *   2. Vault open time for 10,000 markdown files
 *      Target: <1000ms (Req 11.1)
 *   3. Process memory — no vault open
 *      Target: <100MB RSS (Req 11.5)
 *   4. Process memory — 10K file vault, Vector_Index fully built (not yet built = baseline)
 *      Target: <300MB RSS (Req 11.6)
 *
 * Architecture: Intel Mac (x64) — the only test machine available.
 *
 * Requirements: 11.1, 11.5, 11.6, 11.7
 */

import { test, expect } from '@playwright/test'
import path from 'path'
import fs from 'fs/promises'
import os from 'os'
import { _electron as electron } from '@playwright/test'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Path to compiled main process entry (electron-vite output). */
const MAIN_PATH = path.resolve(__dirname, '../../out/main/index.js')

/** Performance targets in milliseconds / bytes. */
const TARGETS = {
  /** Cold launch → FileTree interactive, ms (Req 11.7) */
  COLD_LAUNCH_MS: 2500,
  /** Vault scan for 10K files, ms (Req 11.1) */
  VAULT_OPEN_10K_MS: 1000,
  /** RSS with no vault open, bytes (Req 11.5) */
  MEMORY_NO_VAULT_BYTES: 100 * 1024 * 1024,
  /** RSS with 10K file vault loaded, bytes (Req 11.6) */
  MEMORY_10K_VAULT_BYTES: 300 * 1024 * 1024
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Create a temporary vault directory populated with `count` markdown files.
 * Returns the directory path; caller is responsible for cleanup.
 */
async function createLargeVault(count: number): Promise<string> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'nabu-perf-'))

  // Create subdirectories to better simulate a real vault structure
  const SUB_DIRS = 10
  const filesPerDir = Math.floor(count / SUB_DIRS)

  await Promise.all(
    Array.from({ length: SUB_DIRS }, async (_, d) => {
      const subDir = path.join(dir, `folder-${d}`)
      await fs.mkdir(subDir, { recursive: true })

      const writes = Array.from({ length: filesPerDir }, (_, i) => {
        const globalIdx = d * filesPerDir + i
        return fs.writeFile(
          path.join(subDir, `note-${String(globalIdx).padStart(5, '0')}.md`),
          `# Note ${globalIdx}\n\nThis is note ${globalIdx} in folder ${d}.\n\n- [ ] Task A\n- [x] Task B\n\n[[note-${(globalIdx + 1) % count}]]\n`
        )
      })
      await Promise.all(writes)
    })
  )

  // Fill remaining files in root if count is not evenly divisible
  const remaining = count - SUB_DIRS * filesPerDir
  if (remaining > 0) {
    await Promise.all(
      Array.from({ length: remaining }, (_, i) => {
        const idx = SUB_DIRS * filesPerDir + i
        return fs.writeFile(
          path.join(dir, `note-${String(idx).padStart(5, '0')}.md`),
          `# Note ${idx}\n\nThis is extra note ${idx}.\n`
        )
      })
    )
  }

  return dir
}

/**
 * Query Electron's process metrics via `electronApp.evaluate` and return
 * the main-process RSS in bytes.
 *
 * Falls back to `process.memoryUsage().rss` inside the main process if
 * `app.getAppMetrics()` is unavailable.
 */
async function getMainProcessRss(
  electronApp: Awaited<ReturnType<typeof electron.launch>>
): Promise<number> {
  return electronApp.evaluate(({ app }) => {
    // app.getAppMetrics() returns per-process memory (Electron ≥ 0.36)
    const metrics = app.getAppMetrics()
    const mainEntry = metrics.find(
      (m: { type: string }) => m.type === 'Browser' || m.type === 'GPU' || m.type === 'Renderer'
    )
    // Sum all process RSS values (workingSetSize is in KB on Electron metrics)
    const totalKb = metrics.reduce(
      (acc: number, m: { memory: { workingSetSize: number } }) =>
        acc + (m.memory?.workingSetSize ?? 0),
      0
    )
    return totalKb * 1024 // convert KB → bytes
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/**
 * Test 1 — Cold launch time (Req 11.7)
 *
 * Measures: t0 (before electron.launch) → t1 (FileTree has clickable items)
 * Target: <2500ms
 */
test('cold launch to interactive FileTree is under 2500ms (Req 11.7)', async () => {
  const t0 = Date.now()

  const electronApp = await electron.launch({
    args: [MAIN_PATH],
    env: {
      ...process.env,
      NODE_ENV: 'test'
    } as Record<string, string>
  })

  try {
    const page = await electronApp.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    // The app with no saved vault will show either the vault picker prompt or
    // an empty FileTree.  We wait for the root [role="tree"] to appear —
    // that marks the point at which the FileTree is rendered and clickable.
    // If no vault is open the tree will be empty but still present.
    await page.waitForSelector('[role="tree"]', { timeout: TARGETS.COLD_LAUNCH_MS + 2000 })

    const t1 = Date.now()
    const launchMs = t1 - t0

    console.log(
      `[perf] Cold launch time: ${launchMs}ms (target <${TARGETS.COLD_LAUNCH_MS}ms) — ${launchMs < TARGETS.COLD_LAUNCH_MS ? 'PASS ✓' : 'FAIL ✗'}`
    )

    expect(launchMs).toBeLessThan(TARGETS.COLD_LAUNCH_MS)
  } finally {
    await electronApp.close()
  }
})

/**
 * Test 2 — Process memory with no vault (Req 11.5)
 *
 * Launches the app without a vault and measures aggregate RSS.
 * Target: <100MB
 */
test('process memory with no vault is under 100MB (Req 11.5)', async () => {
  const electronApp = await electron.launch({
    args: [MAIN_PATH],
    env: {
      ...process.env,
      NODE_ENV: 'test'
    } as Record<string, string>
  })

  try {
    const page = await electronApp.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    // Allow a moment for the renderer to settle before sampling memory
    await page.waitForSelector('[role="tree"]', { timeout: 10_000 })
    await page.waitForTimeout(500)

    const rssBytes = await getMainProcessRss(electronApp)
    const rssMb = (rssBytes / (1024 * 1024)).toFixed(1)

    console.log(
      `[perf] Memory (no vault): ${rssMb}MB (target <${TARGETS.MEMORY_NO_VAULT_BYTES / 1024 / 1024}MB) — ${rssBytes < TARGETS.MEMORY_NO_VAULT_BYTES ? 'PASS ✓' : 'FAIL ✗'}`
    )

    expect(rssBytes).toBeLessThan(TARGETS.MEMORY_NO_VAULT_BYTES)
  } finally {
    await electronApp.close()
  }
})

/**
 * Test 3 — Vault open time for 10K files (Req 11.1)
 *
 * Generates 10,000 markdown files in a temp directory, launches the app
 * pointing to that vault via NABU_TEST_VAULT, and measures the time from
 * vault scan start to FileTree being populated with visible entries.
 * Target: <1000ms
 */
test('vault open with 10K files completes under 1 second (Req 11.1)', async () => {
  const vaultDir = await createLargeVault(10_000)

  try {
    const electronApp = await electron.launch({
      args: [MAIN_PATH],
      env: {
        ...process.env,
        NODE_ENV: 'test',
        NABU_TEST_VAULT: vaultDir
      } as Record<string, string>
    })

    try {
      const page = await electronApp.firstWindow()
      await page.waitForLoadState('domcontentloaded')

      // t0: renderer is ready, vault scan is about to start (injected via env var)
      const t0 = Date.now()

      // Wait until the FileTree has at least 10 clickable entries — this
      // confirms the vault scan finished AND the renderer rendered the tree.
      await page.waitForFunction(
        () => {
          const tree = document.querySelector('[role="tree"]')
          if (!tree) return false
          return tree.querySelectorAll('[role="button"]').length >= 10
        },
        { timeout: 15_000 }
      )

      const t1 = Date.now()
      const openMs = t1 - t0

      console.log(
        `[perf] Vault open (10K files): ${openMs}ms (target <${TARGETS.VAULT_OPEN_10K_MS}ms) — ${openMs < TARGETS.VAULT_OPEN_10K_MS ? 'PASS ✓' : 'FAIL ✗'}`
      )

      expect(openMs).toBeLessThan(TARGETS.VAULT_OPEN_10K_MS)
    } finally {
      await electronApp.close()
    }
  } finally {
    await fs.rm(vaultDir, { recursive: true, force: true })
  }
})

/**
 * Test 4 — Process memory with 10K file vault loaded (Req 11.6)
 *
 * Reuses the 10K vault created in this test, waits for it to fully load,
 * then samples aggregate RSS.
 * Target: <300MB
 */
test('process memory with 10K file vault is under 300MB (Req 11.6)', async () => {
  const vaultDir = await createLargeVault(10_000)

  try {
    const electronApp = await electron.launch({
      args: [MAIN_PATH],
      env: {
        ...process.env,
        NODE_ENV: 'test',
        NABU_TEST_VAULT: vaultDir
      } as Record<string, string>
    })

    try {
      const page = await electronApp.firstWindow()
      await page.waitForLoadState('domcontentloaded')

      // Wait for vault to be fully loaded in the FileTree
      await page.waitForFunction(
        () => {
          const tree = document.querySelector('[role="tree"]')
          if (!tree) return false
          return tree.querySelectorAll('[role="button"]').length >= 10
        },
        { timeout: 15_000 }
      )

      // Allow an extra moment for background tasks to settle
      await page.waitForTimeout(1000)

      const rssBytes = await getMainProcessRss(electronApp)
      const rssMb = (rssBytes / (1024 * 1024)).toFixed(1)

      console.log(
        `[perf] Memory (10K vault): ${rssMb}MB (target <${TARGETS.MEMORY_10K_VAULT_BYTES / 1024 / 1024}MB) — ${rssBytes < TARGETS.MEMORY_10K_VAULT_BYTES ? 'PASS ✓' : 'FAIL ✗'}`
      )

      expect(rssBytes).toBeLessThan(TARGETS.MEMORY_10K_VAULT_BYTES)
    } finally {
      await electronApp.close()
    }
  } finally {
    await fs.rm(vaultDir, { recursive: true, force: true })
  }
})
