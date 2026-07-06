/**
 * vault-navigation.spec.ts
 *
 * E2E tests for vault opening and file-tree navigation.
 *
 * Requirements: 1.1, 1.2, 1.5, 11.1, 14.1
 */

import { test, expect } from '@playwright/test'
import path from 'path'
import fs from 'fs/promises'
import os from 'os'
import { launchApp, TEST_VAULT_PATH, AppHandle } from './helpers/launchApp'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait until the FileTree is populated with vault files (at least 2 items). */
async function waitForFileTree(handle: AppHandle): Promise<void> {
  await handle.page.waitForSelector('[role="tree"]', { timeout: 10_000 })
  // Also wait for vault files to be populated (IPC round-trip from vault:opened-test)
  await handle.page.waitForFunction(
    () => {
      const tree = document.querySelector('[role="tree"]')
      return !!tree && tree.querySelectorAll('[role="button"]').length >= 2
    },
    { timeout: 15_000 }
  )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

let handle: AppHandle

test.beforeEach(async () => {
  handle = await launchApp(TEST_VAULT_PATH)
})

test.afterEach(async () => {
  await handle.electronApp.close()
})

// ---------------------------------------------------------------------------
// Test 1: FileTree displays all files and folders
// ---------------------------------------------------------------------------

test('vault open — FileTree displays all files and folders', async () => {
  const { page } = handle

  // waitForFileTree waits for the tree AND for vault items to be populated
  await waitForFileTree(handle)

  // Wait for at least 3 items (subfolder + index.md + linked-note.md)
  await page.waitForFunction(
    () => {
      const tree = document.querySelector('[role="tree"]')
      if (!tree) return false
      return tree.querySelectorAll('[role="button"]').length >= 3
    },
    { timeout: 10_000 }
  )

  const treeItems = page.locator('[role="tree"] [role="button"]')
  const count = await treeItems.count()
  expect(count).toBeGreaterThanOrEqual(3)

  // Verify each file/folder name appears
  const treeText = await page.locator('[role="tree"]').textContent()
  expect(treeText).toContain('index.md')
  expect(treeText).toContain('linked-note.md')
  expect(treeText).toContain('subfolder')
})

// ---------------------------------------------------------------------------
// Test 2: Active file gets highlighted CSS class
// ---------------------------------------------------------------------------

test('vault open — active file highlighted', async () => {
  const { page } = handle

  // waitForFileTree now waits for both [role="tree"] AND vault items to load
  await waitForFileTree(handle)

  // Find and click index.md
  const fileItem = page.locator('[role="tree"] [role="button"]', { hasText: 'index.md' })
  await fileItem.waitFor({ timeout: 5_000 })
  await fileItem.click()

  // After click, the item should have aria-current="page" (as implemented in FileTree)
  await expect(fileItem).toHaveAttribute('aria-current', 'page', { timeout: 5_000 })
})

// ---------------------------------------------------------------------------
// Test 3: Folder expand/collapse
// ---------------------------------------------------------------------------

test('vault open — folder expand/collapse', async () => {
  const { page } = handle

  // waitForFileTree waits for tree AND vault items
  await waitForFileTree(handle)

  // Wait for the subfolder button to appear
  const folderItem = page
    .locator('[role="tree"] [role="button"][aria-expanded]')
    .filter({ hasText: 'subfolder' })

  await folderItem.waitFor({ timeout: 10_000 })

  // Initially folder is collapsed (aria-expanded="false")
  await expect(folderItem).toHaveAttribute('aria-expanded', 'false')

  // Click to expand
  await folderItem.click()
  await expect(folderItem).toHaveAttribute('aria-expanded', 'true', { timeout: 3_000 })

  // Click again to collapse
  await folderItem.click()
  await expect(folderItem).toHaveAttribute('aria-expanded', 'false', { timeout: 3_000 })
})

// ---------------------------------------------------------------------------
// Test 4: NoteView loads note content on file selection
// ---------------------------------------------------------------------------

test('file selection — NoteView loads note content', async () => {
  const { page } = handle

  // waitForFileTree now waits for both [role="tree"] AND vault items to load
  await waitForFileTree(handle)

  // Click index.md in the tree
  const fileItem = page.locator('[role="tree"] [role="button"]', { hasText: 'index.md' })
  await fileItem.waitFor({ timeout: 5_000 })
  await fileItem.click()

  // NoteView should show note content — wait for note-content article
  const noteContent = page.locator('.note-content')
  await noteContent.waitFor({ timeout: 10_000 })

  // The index.md heading "Welcome to Test Vault" should appear
  const headingText = await noteContent.textContent()
  expect(headingText).toContain('Welcome to Test Vault')
})

// ---------------------------------------------------------------------------
// Test 5: Performance — vault with 100 files renders FileTree within 1000ms
// ---------------------------------------------------------------------------

test('vault open with many files — completes within 1 second', async () => {
  // Create a temporary vault with 100 markdown files
  const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nabu-e2e-perf-'))
  try {
    // Create 100 .md files
    await Promise.all(
      Array.from({ length: 100 }, (_, i) =>
        fs.writeFile(
          path.join(tmpDir, `note-${String(i).padStart(3, '0')}.md`),
          `# Note ${i}\n\nContent of note ${i}.\n`
        )
      )
    )

    // Close the current app instance and open a new one with the large vault
    await handle.electronApp.close()
    handle = await launchApp(tmpDir)

    const { page } = handle

    const start = Date.now()

    // Wait for FileTree to be populated
    await page.waitForFunction(
      () => {
        const tree = document.querySelector('[role="tree"]')
        if (!tree) return false
        return tree.querySelectorAll('[role="button"]').length >= 10
      },
      { timeout: 10_000 }
    )

    const elapsed = Date.now() - start
    expect(elapsed).toBeLessThan(1000)
  } finally {
    await fs.rm(tmpDir, { recursive: true, force: true })
  }
})
