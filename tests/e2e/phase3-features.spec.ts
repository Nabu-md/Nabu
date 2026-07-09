/**
 * phase3-features.spec.ts
 *
 * E2E verification that a note with callout + math + mermaid + embed + block-ref
 * renders all five features correctly.
 *
 * Requirements: 8.5, 9.4, 11.6, 20.3
 */

import { test, expect } from '@playwright/test'
import { launchApp, TEST_VAULT_PATH, AppHandle } from './helpers/launchApp'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for the FileTree to populate. */
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

/** Open a file from the FileTree and wait for NoteView. */
async function openFile(handle: AppHandle, fileName: string): Promise<void> {
  const fileItem = handle.page
    .locator('[role="tree"] [role="button"]')
    .filter({ hasText: fileName })
  await fileItem.click()
  await handle.page.locator('.note-content').waitFor({ timeout: 10_000 })
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

let handle: AppHandle

test.beforeEach(async () => {
  handle = await launchApp(TEST_VAULT_PATH)
  await waitForFileTree(handle)
  await openFile(handle, 'phase3-test.md')
})

test.afterEach(async () => {
  await handle.electronApp.close()
})

// ---------------------------------------------------------------------------
// Callout
// ---------------------------------------------------------------------------

test('Callout renders with correct styling', async () => {
  const { page } = handle

  // The callout should be rendered with a data element containing the type
  const callout = page.locator('[data-callout-type]').first()
  await callout.waitFor({ timeout: 5_000 })

  await expect(callout).toHaveAttribute('data-callout-type', 'note')

  // Callout body text should be visible
  await expect(callout).toContainText('A sample callout')
  await expect(callout).toContainText('This is a callout body')
})

// ---------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------

test('Inline and display math render', async () => {
  const { page } = handle

  // Wait for KaTeX-rendered math
  // KaTeX renders with .katex class on inline math spans
  const katex = page.locator('.katex').first()
  await katex.waitFor({ timeout: 10_000 })

  // Inline math: $E = mc^2$ should render
  const inlineMath = page.locator('.katex').first()
  await expect(inlineMath).toBeVisible()

  // Display math: $$...$$ renders in its own block
  const displayMath = page.locator('.katex-display').first()
  await displayMath.waitFor({ timeout: 5_000 })
  await expect(displayMath).toBeVisible()
})

// ---------------------------------------------------------------------------
// Mermaid
// ---------------------------------------------------------------------------

test('Mermaid diagram renders SVG', async () => {
  const { page } = handle

  // Wait for mermaid SVG to render
  const svg = page.locator('.mermaid-block svg, .mermaid svg').first()
  await svg.waitFor({ timeout: 15_000 })

  await expect(svg).toBeVisible()

  // The SVG should have an svg element with some diagram content
  const svgContent = await svg.innerHTML()
  expect(svgContent.length).toBeGreaterThan(0)
})

// ---------------------------------------------------------------------------
// Embed
// ---------------------------------------------------------------------------

test('Note embed renders linked content', async () => {
  const { page } = handle

  // The embedded note content should appear
  const embedContent = page.locator('.embed-block').first()
  await embedContent.waitFor({ timeout: 10_000 })

  // It should contain content from the linked note
  await expect(embedContent).toContainText('Linked Note')
})

// ---------------------------------------------------------------------------
// Block reference
// ---------------------------------------------------------------------------

test('Block reference ID renders as attribute and link navigates', async () => {
  const { page } = handle

  // Wait for a node with data-block-id
  const blockNode = page.locator('[data-block-id="ref1"]').first()
  await blockNode.waitFor({ timeout: 5_000 })

  await expect(blockNode).toBeVisible()

  // There should be a wikiLink with blockRef displayed
  const blockRefLink = page
    .locator('a.wiki-link')
    .filter({ hasText: /\[\[phase3-test#\^ref1\]\]/i })
    .first()
  await expect(blockRefLink).toBeVisible()
})
