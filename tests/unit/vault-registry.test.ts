/**
 * vault-registry.test.ts
 *
 * Unit tests for the VaultRegistry multi-vault management.
 *
 * Requirements: 22.2, 22.10
 */

import { describe, it, expect, beforeEach } from 'vitest'
import { VaultRegistry, type VaultSession } from '../../src/main/vault-registry'
import type { VaultMetadata } from '../../src/shared/types'
import type { FileEntry } from '../../src/shared/types'

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

function createMockStateManager(vaultPath: string, files: FileEntry[] = []): VaultSession['stateManager'] {
  return {
    getCurrentVault: (): VaultMetadata => ({ path: vaultPath, files }),
  }
}

function createMockVectorManager(): VaultSession['vectorManager'] {
  return {
    getStatus: async () => ({ disabled: false, reason: null, items: 0 }),
  }
}

function createMockWatcher() {
  return {
    stop: () => {},
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
      )

      expect(session.vaultId).toBe('vault-1')
      expect(session.vaultPath).toBe('/vault/one')
      expect(session.isActive).toBe(false)
    })

    it('overwrites an existing session with the same ID', () => {
      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
      )
      registry.register(
        'vault-1',
        '/vault/one-updated',
        createMockStateManager('/vault/one-updated'),
        createMockVectorManager(),
        createMockWatcher(),
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two'),
        createMockVectorManager(),
        createMockWatcher(),
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two'),
        createMockVectorManager(),
        createMockWatcher(),
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
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
      const mockWatcher = { stop: () => { stopCalled = true } }

      registry.register(
        'vault-1',
        '/vault/one',
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        mockWatcher,
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
        createMockStateManager('/vault/one'),
        createMockVectorManager(),
        createMockWatcher(),
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
        createMockStateManager('/vault/one', []),
        createMockVectorManager(),
        createMockWatcher(),
      )
      registry.register(
        'vault-2',
        '/vault/two',
        createMockStateManager('/vault/two', []),
        createMockVectorManager(),
        createMockWatcher(),
      )

      const vaults = registry.getAllVaults()
      expect(vaults).toHaveLength(2)
    })
  })
})