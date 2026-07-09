/**
 * settings.ts
 *
 * Settings persistence — load and save AppSettings to userData/settings.json.
 * Extracted into its own module to avoid circular imports between index.ts
 * and ipc.ts.
 *
 * Requirements: 11.7, 12.7, 12.9, 16.5
 */

import { app } from 'electron'
import { join } from 'path'
import fs from 'fs/promises'

// ---------------------------------------------------------------------------
// AppSettings — persisted to userData/settings.json
// ---------------------------------------------------------------------------

export interface AppSettings {
  lastVaultPath: string | null
  windowBounds: { x: number; y: number; width: number; height: number } | null
  theme: 'dark' | 'light' | 'system'
  /** Automatically inject `created` / `modified` timestamps in frontmatter (Req 16.5). */
  autoProperties: boolean
}

export const DEFAULT_SETTINGS: AppSettings = {
  lastVaultPath: null,
  windowBounds: null,
  theme: 'dark',
  autoProperties: true,
}

export function settingsPath(): string {
  return join(app.getPath('userData'), 'settings.json')
}

/**
 * Load settings from disk. Returns defaults if the file doesn't exist or
 * cannot be parsed.
 */
export async function loadSettings(): Promise<AppSettings> {
  try {
    const raw = await fs.readFile(settingsPath(), 'utf-8')
    const parsed = JSON.parse(raw) as Partial<AppSettings>
    return {
      lastVaultPath: parsed.lastVaultPath ?? null,
      windowBounds: parsed.windowBounds ?? null,
      theme: parsed.theme ?? 'dark',
      autoProperties: parsed.autoProperties ?? true,
    }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

/**
 * Persist settings to disk. Errors are logged but not thrown so that a save
 * failure never crashes the app.
 */
export async function saveSettings(settings: AppSettings): Promise<void> {
  try {
    await fs.writeFile(settingsPath(), JSON.stringify(settings, null, 2), 'utf-8')
  } catch (err) {
    console.error('[Settings] Failed to save settings:', err)
  }
}
