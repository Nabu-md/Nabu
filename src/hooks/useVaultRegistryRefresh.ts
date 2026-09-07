import { useCallback, useEffect } from 'react'
import type { Dispatch, MutableRefObject, SetStateAction } from 'react'
import type { VaultOption } from '../components/StatusBar'
import { loadVaultList } from '../utils/vaultListStore'

const VAULT_REGISTRY_CHANGED_EVENT = 'nabu:vault-registry-changed'

function persistedVault(vault: VaultOption) {
  return {
    ...vault,
    available: undefined,
    managedDefault: undefined,
  }
}

export function serializePersistedVaultSnapshot(
  vaults: VaultOption[],
  activeVault: string | null,
  hiddenDefaults: string[],
  defaultWorkspacePath: string | null,
): string {
  return JSON.stringify({
    activeVault,
    defaultWorkspacePath,
    hiddenDefaults,
    vaults: vaults.map(persistedVault),
  })
}

export function useVaultRegistryRefresh(options: {
  lastPersistedSnapshotRef: MutableRefObject<string | null>
  setExtraVaults: Dispatch<SetStateAction<VaultOption[]>>
}) {
  const { lastPersistedSnapshotRef, setExtraVaults } = options
  const refreshVaultRegistry = useCallback(async () => {
    const { activeVault, defaultWorkspacePath, hiddenDefaults, vaults } = await loadVaultList()
    lastPersistedSnapshotRef.current = serializePersistedVaultSnapshot(vaults, activeVault, hiddenDefaults, defaultWorkspacePath)
    setExtraVaults(vaults)
  }, [lastPersistedSnapshotRef, setExtraVaults])

  useEffect(() => {
    const refresh = () => { void refreshVaultRegistry() }
    window.addEventListener(VAULT_REGISTRY_CHANGED_EVENT, refresh)
    return () => window.removeEventListener(VAULT_REGISTRY_CHANGED_EVENT, refresh)
  }, [refreshVaultRegistry])

  return refreshVaultRegistry
}
