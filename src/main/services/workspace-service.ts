/**
 * workspace-service.ts
 *
 * WorkspaceService — owns workspace lifecycle, workspace state, and workspace
 * persistence.
 *
 * A *workspace* is the user's working session: which vaults are open/recent and
 * which vault is active (see `docs/architecture/domain-models.md#workspace`).
 * WorkspaceService is the single owner of that session state. It coordinates
 * with VaultService (which owns the vault lifecycle) and the VaultRegistry
 * (which holds the open vault sessions) but never touches vault files
 * directly — it is intentionally independent from filesystem concerns.
 *
 * This is a pure consolidation: the workspace-state logic that was previously
 * scattered across `index.ts` (the `vault:opened` persistence listener and the
 * `restoreVault` delegation) and `settings.ts` (the `recentVaults` /
 * `lastVaultPath` helpers) is gathered here behind a single deterministic
 * interface. No behavior is redesigned or changed.
 *
 * Canonical lifecycle flow (Phase 4.2):
 *
 *   Application Startup
 *     → VaultService.open()        (vault ready)
 *     → WorkspaceService.load()    (workspace active)
 *     → Normal Operation
 *     → WorkspaceService.save()    (persist session)
 *     → VaultService.close()       (shutdown)
 *
 * Requirements: 22.1, 22.10
 */

import { appEventBus } from '@shared/events'

import { loadSettings, saveSettings, updateRecentVaults } from './settings'
import { vaultRegistry } from './vault-registry'

import type { AppSettings, RecentVaultEntry } from './settings'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * The serializable workspace state. Mirrors the `Workspace` domain model
 * (active vault, open vaults, recent vaults) without any filesystem coupling.
 */
export interface WorkspaceState {
  /** The currently focused vault (registry id == vault path), or null. */
  activeVaultId: string | null
  /** Vaults open in the current session (registry ids). */
  openVaultIds: string[]
  /** Recently opened vaults in MRU order. */
  recentVaultIds: string[]
  /** The currently active note id, or null (reserved for future use). */
  activeNoteId: string | null
}

// ---------------------------------------------------------------------------
// WorkspaceService
// ---------------------------------------------------------------------------

/**
 * Owns all workspace lifecycle business logic.
 *
 * The service is constructed with the settings persistence helpers and the
 * VaultRegistry singleton. It does not import Electron, fs, or any vault
 * file-handling code — keeping workspace concerns decoupled from the
 * filesystem.
 */
export class WorkspaceService {
  /** In-memory workspace state, hydrated by `load()`. */
  private state: WorkspaceState = {
    activeVaultId: null,
    openVaultIds: [],
    recentVaultIds: [],
    activeNoteId: null
  }

  /** Cached settings snapshot used for persistence. */
  private settings: AppSettings | null = null

  /**
   * Load (restore) the workspace state from persisted settings.
   *
   * This is the workspace restoration step of the canonical lifecycle. It
   * reads `lastVaultPath` and `recentVaults` from settings and rebuilds the
   * in-memory workspace state. It does NOT open any vault — that remains the
   * responsibility of VaultService. It only restores *which* vault should be
   * active so VaultService can act on it.
   *
   * Behavior is identical to the previous `restoreVault` prelude in index.ts:
   * the last-used vault path (if any) becomes the active candidate.
   */
  async load(): Promise<WorkspaceState> {
    const settings = await loadSettings()
    this.settings = settings

    const recentVaultIds = (settings.recentVaults ?? []).map((v: RecentVaultEntry) => v.path)

    this.state = {
      activeVaultId: settings.lastVaultPath ?? null,
      openVaultIds: settings.lastVaultPath ? [settings.lastVaultPath] : [],
      recentVaultIds,
      activeNoteId: null
    }

    return this.state
  }

  /**
   * Initialize the workspace's active vault in the registry.
   *
   * Called after VaultService has opened the active vault. It marks the
   * restored vault as the active session in the VaultRegistry so that
   * subsequent "switch vault" / "get-current" operations resolve correctly.
   *
   * No-op if `vaultId` is null or the vault is not registered.
   */
  initialize(vaultId: string | null): void {
    if (!vaultId) return
    if (vaultRegistry.get(vaultId)) {
      vaultRegistry.setActive(vaultId)
      this.state.activeVaultId = vaultId
      if (!this.state.openVaultIds.includes(vaultId)) {
        this.state.openVaultIds.push(vaultId)
      }
    }
  }

  /**
   * Persist a successfully opened vault into the workspace session.
   *
   * Updates the in-memory workspace state (active + open + recent) and writes
   * the `lastVaultPath` / `recentVaults` fields to settings. This is the
   * workspace persistence step of the canonical lifecycle.
   *
   * Behavior is identical to the previous `vault:opened` listener in index.ts
   * plus the `updateRecentVaults` helper: the opened vault becomes the active
   * and most-recent entry.
   */
  async persist(vaultPath: string, vaultName: string): Promise<void> {
    // Update in-memory state
    this.state.activeVaultId = vaultPath
    if (!this.state.openVaultIds.includes(vaultPath)) {
      this.state.openVaultIds.push(vaultPath)
    }
    if (!this.state.recentVaultIds.includes(vaultPath)) {
      this.state.recentVaultIds.unshift(vaultPath)
    }

    // Persist to settings (mirrors updateRecentVaults + saveSettings)
    const base = this.settings ?? (await loadSettings())
    const updated = updateRecentVaults(base, vaultPath, vaultName)
    this.settings = updated
    await saveSettings(updated)

    // Notify internal subscribers (services only) that the workspace changed.
    appEventBus.publish('VaultOpened', {
      vaultId: vaultPath,
      path: vaultPath,
      fileCount: 0
    })
  }

  /**
   * Return the current workspace state snapshot.
   */
  getState(): WorkspaceState {
    return { ...this.state }
  }

  /**
   * Return the restored last-vault path (or null) for VaultService to open.
   */
  getLastVaultPath(): string | null {
    return this.settings?.lastVaultPath ?? null
  }

  /**
   * Clear the stale last-vault path (used when restoration fails because the
   * path is no longer readable). Mirrors the previous `restoreVault` behavior
   * of clearing `lastVaultPath` so we don't retry on next launch.
   */
  async clearLastVaultPath(): Promise<void> {
    const base = this.settings ?? (await loadSettings())
    const updated = { ...base, lastVaultPath: null }
    this.settings = updated
    await saveSettings(updated)
    this.state.activeVaultId = null
    this.state.openVaultIds = []
  }

  /**
   * Save (persist) the current workspace session to disk.
   *
   * This is the workspace persistence step invoked during shutdown. It writes
   * the active vault path and recent vaults so the next launch can restore the
   * same session.
   */
  async save(): Promise<void> {
    const base = this.settings ?? (await loadSettings())
    const updated: AppSettings = {
      ...base,
      lastVaultPath: this.state.activeVaultId ?? base.lastVaultPath,
      recentVaults: base.recentVaults ?? []
    }
    this.settings = updated
    await saveSettings(updated)
  }

  /**
   * Cleanup the workspace session.
   *
   * Releases in-memory workspace state. Vault resource release (stopping
   * watchers, clearing state) remains the responsibility of VaultService; this
   * method only resets the workspace's view of the session.
   */
  cleanup(): void {
    this.state = {
      activeVaultId: null,
      openVaultIds: [],
      recentVaultIds: this.state.recentVaultIds,
      activeNoteId: null
    }
  }
}
