import { test, expect } from '@playwright/test'

test('settings shows connected GitHub state with username', async ({ page }) => {
  await page.goto('http://localhost:5243/')
  await page.waitForLoadState('networkidle')
  await page.waitForTimeout(500)

  // Open settings
  await page.keyboard.press('Meta+Comma')
  await page.waitForTimeout(500)

  // Verify settings panel opened
  await expect(page.getByTestId('settings-panel')).toBeVisible()

  // Mock data starts with github connected — verify connected state
  const connected = page.getByTestId('github-connected')
  await expect(connected).toBeVisible({ timeout: 5000 })
  await expect(connected).toContainText('nabu-md')
  await expect(connected).toContainText('Connected')

  // Verify disconnect button
  await expect(page.getByTestId('github-disconnect')).toBeVisible()

  // Verify NO token input field
  await expect(page.getByTestId('settings-key-github-token')).not.toBeVisible()

  await page.screenshot({ path: 'test-results/settings-oauth-connected.png' })
})

test('disconnect shows Login with GitHub button', async ({ page }) => {
  await page.goto('http://localhost:5243/')
  await page.waitForLoadState('networkidle')
  await page.waitForTimeout(500)

  // Open settings
  await page.keyboard.press('Meta+Comma')
  await page.waitForTimeout(500)

  // Click disconnect
  const disconnectBtn = page.getByTestId('github-disconnect')
  await expect(disconnectBtn).toBeVisible({ timeout: 5000 })
  await disconnectBtn.click()
  await page.waitForTimeout(300)

  // Login button should now be visible
  const loginBtn = page.getByTestId('github-login')
  await expect(loginBtn).toBeVisible({ timeout: 3000 })
  await expect(loginBtn).toContainText('Login with GitHub')

  await page.screenshot({ path: 'test-results/settings-oauth-login.png' })
})
