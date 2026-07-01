/**
 * launchApp.ts
 *
 * Shared helper that launches the Electron app for E2E tests using
 * Playwright's `_electron` API.
 *
 * The app is launched pointing to the pre-built output (`out/main/index.js`)
 * so E2E tests always exercise the compiled artefact, not the Vite dev server.
 *
 * Usage:
 *   const { electronApp, page } = await launchApp(vaultPath)
 *   // … run tests …
 *   await electronApp.close()
 */

import path from 'path'
import { _electron as electron, ElectronApplication, Page } from '@playwright/test'

export interface AppHandle {
  electronApp: ElectronApplication
  page: Page
}

/**
 * Path to the compiled main process entry point.
 * electron-vite outputs to `out/main/index.js`.
 */
const MAIN_PATH = path.resolve(__dirname, '../../../out/main/index.js')

/**
 * Launch the Electron app and return the first window's Page.
 *
 * @param vaultPath  Optional vault path to open on launch.  When provided it is
 *                   passed via the `ONYX_TEST_VAULT` environment variable so the
 *                   app can load it automatically without showing the native
 *                   directory picker.
 * @param extraEnv   Additional environment variables to inject.
 */
export async function launchApp(
  vaultPath?: string,
  extraEnv: Record<string, string> = {}
): Promise<AppHandle> {
  const env: Record<string, string> = {
    NODE_ENV: 'test',
    ...extraEnv
  }
  if (vaultPath) {
    env['ONYX_TEST_VAULT'] = vaultPath
  }

  const electronApp = await electron.launch({
    args: [MAIN_PATH],
    env: { ...process.env, ...env } as Record<string, string>
  })

  // Wait for the first window to appear
  const page = await electronApp.firstWindow()

  // Wait for the renderer to fully load and for React to mount.
  // 'load' ensures all scripts have run; the vault pull happens in useEffect
  // after mount so we need to wait a beat beyond domcontentloaded.
  await page.waitForLoadState('load')

  return { electronApp, page }
}

/**
 * Absolute path to the bundled test vault fixture directory.
 */
export const TEST_VAULT_PATH = path.resolve(__dirname, '../fixtures/vault')
