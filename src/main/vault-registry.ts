/**
 * vault-registry.ts
 *
 * VaultRegistry — manages multiple vault sessions for multi-vault support.
 * Each vault session contains its own StateManager, VectorManager, and VaultWatcher.
 *
 * Requirements: 22.2, 22.10
 */

import type { VaultMetadata } from '../shared/types'
import type { StateManager } from './state'
import type { VectorManager } from './vector'
import type { VaultWatcher } from './watcher'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** VaultSession holds the managers for a single open vault. */
export interface VaultSession {
  vaultId: string
  vaultPath: string
  /** StateManager for this vault — see src/main/state.ts */
  stateManager: StateManager
  /** VectorManager for this vault — see src/main/vector.ts */
  vectorManager: VectorManager
  /** VaultWatcher for this vault — see src/main/watcher.ts */
  watcher: VaultWatcher
  isActive: boolean
}

// ---------------------------------------------------------------------------
// VaultRegistry
// ---------------------------------------------------------------------------

/**
 * Manages multiple vault sessions for multi-vault support.
 * Provides backward compatibility with v1 singletons during migration.
 */
export class VaultRegistry {
  private sessions: Map<string, VaultSession> = new Map()
  private activeVaultId: string | null = null

  /**
   * Register a new vault session.
   * If the vault already exists, updates it.
   */
  register(
    vaultId: string,
    vaultPath: string,
    stateManager: StateManager,
    vectorManager: VectorManager,
    watcher: VaultWatcher
  ): VaultSession {
    const session: VaultSession = {
      vaultId,
      vaultPath,
      stateManager,
      vectorManager,
      watcher,
      isActive: false
    }

    this.sessions.set(vaultId, session)
    return session
  }

  /**
   * Get a vault session by ID.
   * Returns undefined if the vault is not open.
   */
  get(vaultId: string | undefined): VaultSession | undefined {
    if (!vaultId) {
      return this.getActive()
    }
    return this.sessions.get(vaultId)
  }

  /**
   * Set the active vault session.
   * Returns the previous active session (if any) for graceful transition.
   */
  setActive(vaultId: string | null): VaultSession | undefined {
    // Deactivate current session
    if (this.activeVaultId) {
      const current = this.sessions.get(this.activeVaultId)
      if (current) {
        current.isActive = false
      }
    }

    // Clear active if null
    if (!vaultId) {
      this.activeVaultId = null
      return undefined
    }

    // Activate new session
    const session = this.sessions.get(vaultId)
    if (session) {
      session.isActive = true
      this.activeVaultId = vaultId
      return session
    }

    return undefined
  }

  /**
   * Get the currently active vault session.
   */
  getActive(): VaultSession | undefined {
    if (!this.activeVaultId) return undefined
    return this.sessions.get(this.activeVaultId)
  }

  /**
   * Get the active vault ID.
   */
  getActiveId(): string | null {
    return this.activeVaultId
  }

  /**
   * Close a vault session and release its resources.
   * Stops the watcher and clears in-memory state.
   */
  close(vaultId: string): void {
    const session = this.sessions.get(vaultId)
    if (session) {
      session.watcher.stop()
      // Deactivate if this was the active session
      if (this.activeVaultId === vaultId) {
        this.activeVaultId = null
      }
      this.sessions.delete(vaultId)
    }
  }

  /**
   * Get all open vault metadata.
   */
  getAllVaults(): VaultMetadata[] {
    const vaults: VaultMetadata[] = []
    for (const session of this.sessions.values()) {
      const vault = session.stateManager.getCurrentVault()
      if (vault) {
        vaults.push(vault)
      }
    }
    return vaults
  }

  /**
   * Get the number of open vaults.
   */
  getVaultCount(): number {
    return this.sessions.size
  }
}

// ---------------------------------------------------------------------------
// Singleton export
// ---------------------------------------------------------------------------

export const vaultRegistry = new VaultRegistry()
