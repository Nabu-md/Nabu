import { test, expect } from '@playwright/test'

test.describe('Telemetry consent dialog', () => {
  test('dialog does not appear when consent was already given', async ({ page }) => {
    // Default mock settings have telemetry_consent: false
    // The consent dialog should NOT appear (only appears when null)
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await expect(page.getByText('Help improve Nabu')).not.toBeVisible({ timeout: 5000 })
  })

  test('privacy toggles are visible in settings panel', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Open settings via keyboard shortcut
    await page.keyboard.press('Meta+,')
    await expect(page.getByTestId('settings-panel')).toBeVisible({ timeout: 5000 })

    // Privacy section should be present
    await expect(page.getByText('Privacy & Telemetry')).toBeVisible()
    await expect(page.getByTestId('settings-crash-reporting')).toBeVisible()
    await expect(page.getByTestId('settings-analytics')).toBeVisible()
  })
})
