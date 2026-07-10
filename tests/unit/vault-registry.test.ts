/**
 * vault-registry.test.ts
 *
 * Unit tests for the VaultRegistry multi-vault management.
 *
 * Requirements: 22.2, 22.10
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { VaultRegistry, type VaultSession } from '../../src/main/vault-registry'
import type { VaultMetadata, FileEntry } from '../../src/shared/types'

// Mock types for testing - partial interfaces
type MockStateManager = {
  getCurrentVault(): VaultMetadata | null
  getExtendedIndex(): unknown
  invalidateAST(_path: string): void
  hasPendingWrite(_path: string): boolean
}

type MockVectorManager = {
  getStatus(): Promise<{ disabled: boolean; reason: string | null; items: number }>
}

type MockWatcher = {
  stop(): void
}

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

function createMockStateManager(vaultPath: string, files: FileEntry[] = []): MockStateManager {
  return {
    getCurrentVault: (): VaultMetadata => ({ path: vaultPath, files }),
    getExtendedIndex: () => null,
    invalidateAST: () => {},
    hasPendingWrite: () => false
  }
}

function createMockVectorManager(): MockVectorManager {
  return {
    getStatus: async () => ({ disabled: false, reason: null, items: 0 })
  }
}

function createMockWatcher(): MockWatcher {
  return {
    stop: () => {}
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('VaultRegistry', () => {
  let registry: VaultRegistry

  beforeEach(() => {
    registry = new VaultRegistry()
  })

  describe('register', () => {
    it('registers a new vault session', () => {
      const session = registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      expect(session.vaultId).toBe('vault-1')
      expect(session.vaultPath).toBe('/vault/one')
      expect(session.isActive).toBe(false)
    })

    it('overwrites an existing session with the same ID', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.register(
        'vault-1',
        '/vault/one-updated',
        createMockStateManager('/vault/one-updated') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      expect(registry.getVaultCount()).toBe(1)
    })
  })

  describe('get', () => {
    it('returns undefined for non-existent vault', () => {
      expect(registry.get('nonexistent')).toBeUndefined()
    })

    it('returns undefined when no vault is active and no ID given', () => {
      expect(registry.get(undefined)).toBeUndefined()
    })

    it('returns a vault session by ID', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      const session = registry.get('vault-1')
      expect(session?.vaultId).toBe('vault-1')
      expect(session?.vaultPath).toBe('/vault/one')
    })
  })

  describe('setActive / getActive', () => {
    it('sets and gets the active vault session', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      const active = registry.setActive('vault-1')
      expect(active?.isActive).toBe(true)
      expect(active?.vaultId).toBe('vault-1')

      const retrieved = registry.getActive()
      expect(retrieved?.vaultId).toBe('vault-1')
    })

    it('deactivates previous active session when switching', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      registry.setActive('vault-1')
      registry.setActive('vault-2')

      const session1 = registry.get('vault-1')
      const session2 = registry.get('vault-2')

      expect(session1?.isActive).toBe(false)
      expect(session2?.isActive).toBe(true)
    })

    it('clears active vault when set to null', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.setActive('vault-1')
      registry.setActive(null)

      expect(registry.getActive()).toBeUndefined()
      expect(registry.getActiveId()).toBeNull()
    })
  })

  describe('close', () => {
    it('closes a vault session and stops its watcher', () => {
      let stopCalled = false
      const mockWatcher = {
        stop: () => {
          stopCalled = true
        }
      }

      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        mockWatcher as VaultSession['watcher']
      )
      registry.setActive('vault-1')

      registry.close('vault-1')

      expect(stopCalled).toBe(true)
      expect(registry.get('vault-1')).toBeUndefined()
    })

    it('deactivates closed session if it was active', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one') as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.setActive('vault-1')
      registry.close('vault-1')

      expect(registry.getActiveId()).toBeNull()
    })
  })

  describe('getAllVaults', () => {
    it('returns metadata for all open vaults', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one', []) as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two', []) as VaultSession['stateManager'],
        createMockVectorManager() as VaultSession['vectorManager'],
        createMockWatcher() as VaultSession['watcher']
      )

      const vaults = registry.getAllVaults()
      expect(vaults).toHaveLength(2)
    })
  })
})
