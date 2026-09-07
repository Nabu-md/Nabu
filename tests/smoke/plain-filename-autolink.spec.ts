import { expect, test, type Page } from '@playwright/test'
import {
  createFixtureVaultCopy,
  openFixtureVault,
  removeFixtureVaultCopy,
} from '../helpers/fixtureVault'
import { executeCommand, openCommandPalette } from './helpers'

let tempVaultDir: string

async function openNote(page: Page, title: string) {
  await page.locator('[data-testid="note-list-container"]').getByText(title, { exact: true }).click()
  await expect(page.locator('.bn-editor')).toBeVisible({ timeout: 5_000 })
}

async function openRawMode(page: Page) {
  await openCommandPalette(page)
  await executeCommand(page, 'Toggle Raw')
  await expect(page.locator('.cm-content')).toBeVisible({ timeout: 5_000 })
}

async function openBlockNoteMode(page: Page) {
  await openCommandPalette(page)
  await executeCommand(page, 'Toggle Raw')
  await expect(page.locator('.bn-editor')).toBeVisible({ timeout: 5_000 })
}

async function getRawEditorContent(page: Page): Promise<string> {
  return page.evaluate(() => {
    const el = document.querySelector('.cm-content')
    if (!el) return ''

    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- CodeMirror view is attached to the DOM node.
    const view = (el as any).cmTile?.view
    if (!view) return el.textContent ?? ''

    return view.state.doc.toString() as string
  })
}

async function setRawEditorContent(page: Page, content: string) {
  await page.evaluate((nextContent) => {
    const el = document.querySelector('.cm-content')
    if (!el) return

    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- CodeMirror view is attached to the DOM node.
    const view = (el as any).cmTile?.view
    if (!view) return

    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: nextContent,
      },
    })
  }, content)
}

async function appendPlainFilenameParagraph(page: Page, value: string) {
  const lastBlock = page.locator('.bn-block-content').last()
  await expect(lastBlock).toBeVisible({ timeout: 5_000 })
  await lastBlock.click()
  await page.keyboard.press('End')
  await page.keyboard.press('Enter')
  await page.keyboard.type(`${value} `)
  await page.waitForTimeout(700)
}

test.beforeEach(async ({ page }, testInfo) => {
  testInfo.setTimeout(90_000)
  tempVaultDir = createFixtureVaultCopy()
  await openFixtureVault(page, tempVaultDir)
})

test.afterEach(async () => {
  removeFixtureVaultCopy(tempVaultDir)
})

test('plain filename-like text typed in BlockNote stays plain through raw-mode round trips', async ({ page }) => {
  await openNote(page, 'Note B')
  await appendPlainFilenameParagraph(page, 'AGENTS.md')
  await appendPlainFilenameParagraph(page, 'docs/README.md')

  await openRawMode(page)

  const raw = await getRawEditorContent(page)
  expect(raw).toContain('AGENTS.md')
  expect(raw).toContain('docs/README.md')
  expect(raw).not.toMatch(/\[AGENTS\.md\]\(/)
  expect(raw).not.toMatch(/\[docs\/README\.md\]\(/)
})

test('explicit markdown links still render and round-trip intentionally', async ({ page }) => {
  await openNote(page, 'Note B')
  await openRawMode(page)

  const raw = await getRawEditorContent(page)
  const intentionalLink = '[Nabu Docs](https://example.com/docs)'
  await setRawEditorContent(page, `${raw}\n\n${intentionalLink}\n`)
  await page.waitForTimeout(700)

  await openBlockNoteMode(page)
  await expect(page.locator('.bn-editor a[href="https://example.com/docs"]').last()).toContainText('Nabu Docs')

  await openRawMode(page)
  await expect.poll(() => getRawEditorContent(page)).toContain(intentionalLink)
})
