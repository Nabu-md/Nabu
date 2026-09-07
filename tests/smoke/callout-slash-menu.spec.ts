import { expect, test, type Page } from '@playwright/test'
import {
  createFixtureVaultCopy,
  openFixtureVault,
  removeFixtureVaultCopy,
} from '../helpers/fixtureVault'
import { executeCommand, openCommandPalette } from './helpers'

let tempVaultDir: string

test.beforeEach(async ({ page }, testInfo) => {
  testInfo.setTimeout(90_000)
  tempVaultDir = createFixtureVaultCopy()
  await openFixtureVault(page, tempVaultDir)
  await page.locator('[data-testid="note-list-container"]')
    .getByText('Alpha Project', { exact: true })
    .click()
  await expect(page.locator('.bn-editor')).toBeVisible({ timeout: 5_000 })
})

test.afterEach(() => {
  removeFixtureVaultCopy(tempVaultDir)
})

async function openCalloutSubmenu(page: Page) {
  await page.locator('.bn-block-content').last().click()
  await page.keyboard.press('Enter')
  await page.keyboard.type('/')
  await expect(page.locator('#bn-suggestion-menu')).toBeVisible({ timeout: 5_000 })
  await expect(page.locator('#nabu-fatal-render-error')).toHaveCount(0)
  await page.keyboard.type('call')
  const calloutItem = page.getByRole('option', { name: /Callout/i })
  await expect(calloutItem).toBeVisible({ timeout: 5_000 })
  await calloutItem.click()
  const submenu = page.locator('.nabu-slash-menu__submenu')
  await expect(submenu).toBeVisible({ timeout: 5_000 })
  return { calloutItem, submenu }
}

test('callout slash submenu opens on the right and inserts clean multiline markdown', async ({ page }) => {
  const { calloutItem, submenu } = await openCalloutSubmenu(page)
  await expect.poll(async () => {
    const parentBounds = await calloutItem.boundingBox()
    const submenuBounds = await submenu.boundingBox()
    return Boolean(parentBounds && submenuBounds && submenuBounds.x >= parentBounds.x + parentBounds.width)
  }).toBe(true)

  const submenuItems = submenu.getByRole('menuitem')
  await expect(submenuItems).toHaveCount(13)
  for (let index = 0; index < 13; index += 1) {
    await expect(submenuItems.nth(index).locator('svg')).toHaveCount(1)
  }

  await page.getByRole('menuitem', { name: 'Tip' }).click()
  const callout = page.locator('.nabu-callout[data-callout-type="tip"]')
  await expect(callout).toBeVisible()
  await expect(callout.locator('button')).toHaveCount(0)
  await expect.poll(async () => callout.evaluate((element) => {
    const style = getComputedStyle(element)
    return style.borderTopWidth === '0px' && style.borderLeftWidth === '0px'
  })).toBe(true)
  await expect.poll(async () => callout.evaluate((element) => {
    const header = element.querySelector<HTMLElement>('.nabu-callout__header')
    const body = element.querySelector<HTMLElement>('.nabu-callout__body')
    if (!header || !body) return false
    return Number.parseFloat(getComputedStyle(header).fontSize)
      > Number.parseFloat(getComputedStyle(body).fontSize)
  })).toBe(true)

  await callout.locator('.nabu-callout__body').click()
  await page.keyboard.type('First line')
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.type('Second line')
  await openCommandPalette(page)
  await executeCommand(page, 'Toggle Raw')
  const rawContent = (await page.locator('.cm-line').allTextContents()).join('\n')

  expect(rawContent).toContain('> First line\n> Second line')
  expect(rawContent).not.toMatch(/\\$/mu)
})
