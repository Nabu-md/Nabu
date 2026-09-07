import { test, expect, type Page } from '@playwright/test'
import { executeCommand, openCommandPalette } from './helpers'

interface ContributionAction {
  activation: 'Enter' | 'Space'
  label: string
  url: string
}

const CONTRIBUTION_ACTIONS: ContributionAction[] = [
  {
    activation: 'Enter',
    label: 'Check out Refactoring',
    url: 'https://refactoring.fm/?utm_source=nabu&utm_medium=app&utm_campaign=refactoring',
  },
  {
    activation: 'Enter',
    label: 'Open Codacy',
    url: 'https://codacy.com/nabu?utm_source=nabu&utm_medium=app&utm_campaign=refactoring',
  },
  {
    activation: 'Space',
    label: 'Open CodeScene',
    url: 'https://codescene.com/?utm_source=nabu&utm_medium=app&utm_campaign=refactoring',
  },
  {
    activation: 'Enter',
    label: 'Open CircleCI',
    url: 'https://circleci.com/?utm_source=nabu&utm_medium=app&utm_campaign=refactoring',
  },
  {
    activation: 'Space',
    label: 'Open Unblocked',
    url: 'https://getunblocked.com/?utm_source=nabu&utm_medium=app&utm_campaign=refactoring',
  },
  {
    activation: 'Enter',
    label: 'how I develop Nabu',
    url: 'https://refactoring.fm/p/introducing-the-nabu-alliance',
  },
  { activation: 'Enter', label: 'Open Product Board', url: 'https://nabu.canny.io/' },
  { activation: 'Space', label: 'Open Discussions', url: 'https://github.com/refactoringhq/nabu/discussions' },
  { activation: 'Enter', label: 'Open PRs', url: 'https://github.com/refactoringhq/nabu/pulls' },
  { activation: 'Space', label: 'Open Guide', url: 'https://github.com/refactoringhq/nabu/blob/main/CONTRIBUTING.md' },
  { activation: 'Enter', label: 'Open Issues', url: 'https://github.com/refactoringhq/nabu/issues' },
]

async function expectOpenedUrl(page: Page, url: string): Promise<void> {
  await expect.poll(async () => page.evaluate(() => (
    window as typeof window & { __nabuOpenedUrls: string[] }
  ).__nabuOpenedUrls)).toContain(url)
}

test.describe('Contribute modal', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const openedUrls: string[] = []
      Object.defineProperty(window, '__nabuOpenedUrls', {
        configurable: true,
        value: openedUrls,
      })
      window.open = ((url?: string | URL | undefined) => {
        openedUrls.push(String(url ?? ''))
        return null
      }) as typeof window.open

      const copiedBundles: string[] = []
      Object.defineProperty(window, '__nabuCopiedBundles', {
        configurable: true,
        value: copiedBundles,
      })
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            copiedBundles.push(text)
          },
        },
      })
    })

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('[data-testid="sidebar-top-nav"]')).toBeVisible({ timeout: 10_000 })
  })

  test('Cmd+K opens Contribute, keyboard actions work, and Escape restores the opener @smoke', async ({ page }) => {
    await openCommandPalette(page)
    await executeCommand(page, 'Contribute')

    await expect(page.getByTestId('feedback-dialog')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Contribute to Nabu' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Check out Refactoring' })).toBeFocused()

    for (const [index, action] of CONTRIBUTION_ACTIONS.entries()) {
      if (index > 0) await page.keyboard.press('Tab')
      await expect(page.getByRole('button', { name: action.label })).toBeFocused()
      await page.keyboard.press(action.activation)
      await expectOpenedUrl(page, action.url)
    }

    await page.keyboard.press('Tab')
    await expect(page.getByRole('button', { name: 'Copy Diagnostics' })).toBeFocused()
    await page.keyboard.press('Space')
    await expect.poll(async () => page.evaluate(() => (window as typeof window & { __nabuCopiedBundles: string[] }).__nabuCopiedBundles.length)).toBe(1)

    await page.keyboard.press('Escape')
    await expect(page.getByTestId('feedback-dialog')).not.toBeVisible()
    await expect(page.locator('input[placeholder="Type a command..."]')).toBeVisible()
    await expect(page.locator('input[placeholder="Type a command..."]')).toBeFocused()
  })
})
