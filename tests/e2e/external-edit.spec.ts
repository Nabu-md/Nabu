/**
 * external-edit.spec.ts
 *
 * E2E tests for external edit detection flow:
 *  - NoteView updates when file is modified externally
 *  - ActivityTimeline records an External entry
 *  - FileTree shows blue pulse (.external-edit) CSS class
 *
 * Requirements: 6.1, 6.3, 6.4, 6.6, 11.2
 */

import { test, expect } from '@playwright/test'
import path from 'path'
import fs from 'fs/promises'
import os from 'os'
import { launchApp, AppHandle } from './helpers/launchApp'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for FileTree to be populated. */
async function waitForFileTree(handle: AppHandle): Promise<void> {
  await handle.page.waitForSelector('[role="tree"]', { timeout: 10_000 })
  await handle.page.waitForFunction(
    () => {
      const tree = document.querySelector('[role="tree"]')
      return !!tree && tree.querySelectorAll('[role="button"]').length >= 2
    },
    { timeout: 10_000 }
  )
}

/** Click a file in FileTree and wait for note content to load. */
async function openFile(handle: AppHandle, fileName: string): Promise<void> {
  const fileItem = handle.page
    .locator('[role="tree"] [role="button"]')
    .filter({ hasText: fileName })
  await fileItem.click()
  await handle.page.locator('.note-content').waitFor({ timeout: 10_000 })
}

// ---------------------------------------------------------------------------
// Suite: each test gets its own isolated temp vault so writes don't interfere
// ---------------------------------------------------------------------------

let handle: AppHandle
let tmpDir: string
let indexPath: string

test.beforeEach(async () => {
  // Create a fresh temp vault for each test
  tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'nabu-e2e-ext-'))

  // Copy the fixture vault into the temp dir so we can write to it
  await fs.writeFile(path.join(tmpDir, 'index.md'), '# Original Title\n\nOriginal content here.\n')
  await fs.writeFile(path.join(tmpDir, 'linked-note.md'), '# Linked Note\n\nLinked content.\n')

  indexPath = path.join(tmpDir, 'index.md')

  handle = await launchApp(tmpDir)
  await waitForFileTree(handle)
  await openFile(handle, 'index.md')
})

test.afterEach(async () => {
  await handle.electronApp.close()
  await fs.rm(tmpDir, { recursive: true, force: true })
})

// ---------------------------------------------------------------------------
// Test 1: NoteView updates with new content after external write
// ---------------------------------------------------------------------------

test('external edit — NoteView updates with new content', async () => {
  const { page } = handle

  // Confirm original content is shown
  const noteContent = page.locator('.note-content')
  await expect(noteContent).toContainText('Original Title', { timeout: 5_000 })

  // Write new content to the file externally (simulating Claude Code, vim, etc.)
  await fs.writeFile(indexPath, '# Updated Title\n\nNew content after external edit.\n')

  // Wait for NoteView to reflect the new content.
  // The watcher has a 50ms debounce; allow up to 3s total for debounce + render.
  await expect(noteContent).toContainText('Updated Title', { timeout: 3_000 })
  await expect(noteContent).not.toContainText('Original Title', { timeout: 3_000 })
})

// ---------------------------------------------------------------------------
// Test 2: ActivityTimeline records entry with External badge
// ---------------------------------------------------------------------------

test('external edit — ActivityTimeline records entry', async () => {
  const { page } = handle

  // Confirm we're on the original content
  await expect(page.locator('.note-content')).toContainText('Original Title', {
    timeout: 5_000
  })

  // Write externally
  await fs.writeFile(indexPath, '# Updated For Timeline\n\nTimeline test content.\n')

  // Wait for NoteView to update (confirms IPC round-trip completed)
  await page.waitForSelector('.note-content', { timeout: 5_000 })
  await page.waitForFunction(
    () => document.querySelector('.note-content')?.textContent?.includes('Updated For Timeline'),
    { timeout: 5_000 }
  )

  // ActivityTimeline should have an entry with the External badge
  const timeline = page.locator('[aria-label="Activity timeline"]')
  await expect(timeline).toBeAttached({ timeout: 3_000 })

  // Look for the External badge inside the timeline
  const externalBadge = timeline.locator('[aria-label="External edit"]')
  await externalBadge.waitFor({ timeout: 5_000 })
  await expect(externalBadge).toBeVisible()

  // The entry should contain the file name
  const entryWithExternal = timeline.locator('.entry.external').first()
  await entryWithExternal.waitFor({ timeout: 5_000 })
  const entryText = await entryWithExternal.getAttribute('title')
  expect(entryText).toContain('index.md')
})

// ---------------------------------------------------------------------------
// Test 3: FileTree shows .external-edit CSS class after external write
// ---------------------------------------------------------------------------

test('external edit — FileTree blue pulse animation', async () => {
  const { page } = handle

  // Confirm original state
  await expect(page.locator('.note-content')).toContainText('Original Title', {
    timeout: 5_000
  })

  // Write externally
  await fs.writeFile(indexPath, '# Updated For Pulse\n\nPulse animation test.\n')

  // The FileTree should apply `.external-edit` to the file entry.
  // This class is added and then removed after 600ms — so we need to catch it
  // quickly. We use waitForSelector with a short timeout.
  await page.waitForSelector('.external-edit', { timeout: 5_000 })

  // Verify it's on the index.md entry specifically
  const externalEditEl = page.locator('.external-edit').first()
  const itemText = await externalEditEl.textContent()
  expect(itemText).toContain('index.md')
})
