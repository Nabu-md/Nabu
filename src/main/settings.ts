/**
 * settings.ts
 *
 * Settings persistence — load and save AppSettings to userData/settings.json.
 * Extracted into its own module to avoid circular imports between index.ts
 * and ipc.ts.
 *
 * Requirements: 11.7, 12.7, 12.9, 16.5, 17.3, 22.1, 22.10
 */

import { app } from 'electron'
import { join } from 'path'
import fs from 'fs/promises'

// ---------------------------------------------------------------------------
// AppSettings — persisted to userData/settings.json
// ---------------------------------------------------------------------------

export interface RecentVaultEntry {
  path: string
  name: string
  lastOpened: number
}

export interface AppSettings {
  lastVaultPath: string | null
  windowBounds: { x: number; y: number; width: number; height: number } | null
  theme: 'dark' | 'light' | 'system'
  /** Automatically inject `created` / `modified` timestamps in frontmatter (Req 16.5). */
  autoProperties: boolean
  /** Date format for daily note filenames (e.g. "YYYY-MM-DD"). Default: "YYYY-MM-DD" (Req 17.3). */
  dailyNoteDateFormat: string
  /** Folder path (relative to vault root) for daily notes. Default: "Daily" (Req 17.3). */
  dailyNoteFolder: string
  /** Template name (without .md extension) for daily notes. Empty = no template (Req 17.3). */
  dailyNoteTemplate: string
  /** List of recently opened vaults for multi-vault support (Req 22.1). */
  recentVaults: RecentVaultEntry[]
}

export const DEFAULT_SETTINGS: AppSettings = {
  lastVaultPath: null,
  windowBounds: null,
  theme: 'dark',
  autoProperties: true,
  dailyNoteDateFormat: 'YYYY-MM-DD',
  dailyNoteFolder: 'Daily',
  dailyNoteTemplate: '',
  recentVaults: [],
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
      dailyNoteDateFormat: parsed.dailyNoteDateFormat ?? 'YYYY-MM-DD',
      dailyNoteFolder: parsed.dailyNoteFolder ?? 'Daily',
      dailyNoteTemplate: parsed.dailyNoteTemplate ?? '',
      recentVaults: parsed.recentVaults ?? [],
    }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

// ---------------------------------------------------------------------------
// Recent Vault Helpers
// ---------------------------------------------------------------------------

/** Maximum number of recent vaults to store. */
const MAX_RECENT_VAULTS = 20

/**
 * Add or update a vault in the recent vaults list.
 * Migrates v1 `lastVaultPath` to the `recentVaults` list on first load.
 *
 * Requirements: 22.1, 22.10
 */
export function updateRecentVaults(
  settings: AppSettings,
  vaultPath: string,
  vaultName: string,
): AppSettings {
  const now = Date.now()
  const existing = settings.recentVaults ?? []

  // Filter out this vault if it already exists (to re-add at top)
  const filtered = existing.filter((v) => v.path !== vaultPath)

  // Add the new entry at the front
  const updated = [
    { path: vaultPath, name: vaultName, lastOpened: now },
    ...filtered,
  ].slice(0, MAX_RECENT_VAULTS)

  return { ...settings, recentVaults: updated, lastVaultPath: vaultPath }
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