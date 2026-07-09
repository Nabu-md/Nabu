/**
 * recent-vaults.test.ts
 *
 * Unit tests for the recent vaults list in settings.
 *
 * Requirements: 22.1, 22.10
 */

import { describe, it, expect } from 'vitest'
import { updateRecentVaults, DEFAULT_SETTINGS, AppSettings, RecentVaultEntry } from '../../src/main/settings'

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

function createSettings(overrides?: Partial<AppSettings>): AppSettings {
  return {
    lastVaultPath: null,
    windowBounds: null,
    theme: 'dark',
    autoProperties: true,
    dailyNoteDateFormat: 'YYYY-MM-DD',
    dailyNoteFolder: 'Daily',
    dailyNoteTemplate: '',
    recentVaults: [],
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// updateRecentVaults
// ---------------------------------------------------------------------------

describe('updateRecentVaults', () => {
  it('adds a new vault to an empty list', () => {
    const settings = createSettings()
    const result = updateRecentVaults(settings, '/vaults/personal', 'Personal')

    expect(result.recentVaults).toHaveLength(1)
    expect(result.recentVaults![0]).toEqual({
      path: '/vaults/personal',
      name: 'Personal',
      lastOpened: expect.any(Number),
    })
    expect(result.lastVaultPath).toBe('/vaults/personal')
  })

  it('moves an existing vault to the front of the list', () => {
    const now = 1000000
    const settings = createSettings({
      recentVaults: [
        { path: '/vaults/personal', name: 'Personal', lastOpened: now },
        { path: '/vaults/work', name: 'Work', lastOpened: now + 100 },
      ],
    })

    const result = updateRecentVaults(settings, '/vaults/personal', 'Personal')

    expect(result.recentVaults).toHaveLength(2)
    expect(result.recentVaults![0].path).toBe('/vaults/personal')
    expect(result.recentVaults![1].path).toBe('/vaults/work')
  })

  it('caps the list at the maximum number of entries', () => {
    const now = 1000000
    const manyVaults: RecentVaultEntry[] = []
    for (let i = 0; i < 25; i++) {
      manyVaults.push({ path: `/vaults/vault-${i}`, name: `Vault ${i}`, lastOpened: now + i })
    }

    const settings = createSettings({ recentVaults: manyVaults })
    const result = updateRecentVaults(settings, '/vaults/new', 'New')

    expect(result.recentVaults).toHaveLength(20)
    expect(result.recentVaults![0].path).toBe('/vaults/new')
    expect(result.recentVaults![19].path).toBe('/vaults/vault-18')
  })

  it('preserves other settings when updating', () => {
    const settings = createSettings({
      theme: 'light',
      autoProperties: false,
    })

    const result = updateRecentVaults(settings, '/vaults/test', 'Test')

    expect(result.theme).toBe('light')
    expect(result.autoProperties).toBe(false)
  })
})

describe('DEFAULT_SETTINGS', () => {
  it('includes recentVaults as an empty array', () => {
    expect(DEFAULT_SETTINGS.recentVaults).toEqual([])
  })
})