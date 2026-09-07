import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useRef, useState } from 'react'
import type { PersistedVaultList } from '../utils/vaultListStore'
import { useVaultRegistryRefresh } from './useVaultRegistryRefresh'

let registry: PersistedVaultList

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('../mock-tauri', () => ({
  isTauri: () => false,
  mockInvoke: (command: string) => command === 'load_vault_list'
    ? Promise.resolve(registry)
    : Promise.resolve(true),
}))

function useHarness() {
  const [vaults, setExtraVaults] = useState<Array<{ label: string; path: string; mounted?: boolean }>>([])
  const snapshotRef = useRef<string | null>(null)
  const refreshVaultRegistry = useVaultRegistryRefresh({
    lastPersistedSnapshotRef: snapshotRef,
    setExtraVaults,
  })
  return { refreshVaultRegistry, readSnapshot: () => snapshotRef.current, vaults }
}

describe('useVaultRegistryRefresh', () => {
  beforeEach(() => {
    registry = {
      vaults: [{ label: 'Work', path: '/work/vault', mounted: true }],
      active_vault: '/work/vault',
      default_workspace_path: '/work/vault',
      hidden_defaults: [],
    }
  })

  it('reloads externally registered vaults without changing registry selection metadata', async () => {
    const { result } = renderHook(useHarness)
    registry = {
      ...registry,
      vaults: [...registry.vaults, { label: 'Attached', path: '/attached/vault', mounted: true }],
    }

    await act(() => result.current.refreshVaultRegistry())

    expect(result.current.vaults).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: 'Attached', path: '/attached/vault', mounted: true }),
    ]))
    expect(result.current.readSnapshot()).toContain('"activeVault":"/work/vault"')
  })

  it('responds to the MCP registry event and removes the listener on unmount', async () => {
    const removeEventListener = vi.spyOn(window, 'removeEventListener')
    const { result, unmount } = renderHook(useHarness)
    registry = {
      ...registry,
      vaults: [...registry.vaults, { label: 'Cloned', path: '/cloned/vault', mounted: true }],
    }

    act(() => window.dispatchEvent(new CustomEvent('nabu:vault-registry-changed')))
    await waitFor(() => expect(result.current.vaults).toHaveLength(2))
    unmount()

    expect(removeEventListener).toHaveBeenCalledWith(
      'nabu:vault-registry-changed',
      expect.any(Function),
    )
  })
})
