import { test } from '@playwright/test'
import { launchApp, TEST_VAULT_PATH } from './helpers/launchApp'

test('debug Cmd+Shift+F focus', async () => {
  const handle = await launchApp(TEST_VAULT_PATH)
  const { page } = handle

  const logs: string[] = []
  page.on('console', m => logs.push(m.text()))

  await page.waitForFunction(() => {
    const tree = document.querySelector('[role="tree"]')
    return !!tree && tree.querySelectorAll('[role="button"]').length >= 1
  }, undefined, { timeout: 15000 })

  // Intercept focusSearch to log when it's called
  await page.evaluate(() => {
    // Patch the input's focus/click methods
    const input = document.querySelector('[aria-label="Filter files"]') as HTMLInputElement
    if (input) {
      const origFocus = input.focus.bind(input)
      const origClick = input.click.bind(input)
      input.focus = (...args) => {
        console.log('[DEBUG] input.focus() called')
        origFocus(...args)
        console.log('[DEBUG] activeElement after focus:', document.activeElement?.tagName, document.activeElement === input)
      }
      input.click = () => {
        console.log('[DEBUG] input.click() called')
        origClick()
        console.log('[DEBUG] activeElement after click:', document.activeElement?.tagName, document.activeElement === input)
      }
    }
  })

  await page.keyboard.press('Meta+Shift+F')
  await page.waitForTimeout(600)

  console.log('Logs:\n' + logs.join('\n'))
  await handle.electronApp.close()
})
