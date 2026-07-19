/**
 * vault-service.ts
 *
 * VaultService — owns vault lifecycle, vault loading, vault closing,
 * vault path resolution, and vault coordination.
 *
 * This service extracts the vault business logic that was previously embedded
 * inside `ipc.ts` (vault:open, vault:scan, vault:close, vault:create,
 * vault:switch, vault:get-recents, vault:get-current) and `index.ts`
 * (restoreVault, NABU_TEST_VAULT open). The IPC layer and bootstrap now
 * delegate to this service, leaving behind thin wrappers.
 *
 * This is a pure extraction: no behavior is redesigned, improved, or changed.
 *
 * Requirements: 22.3, 22.5, 22.6, 22.7, 22.9, 22.10
 */

import { app, dialog, BrowserWindow } from 'electron'
import path from 'path'
import fs from 'fs/promises'

import { IPCChannel } from '../../shared/channels'
import {
  VaultOpenSchema,
  VaultCloseSchema,
  VaultSwitchSchema,
  VaultScanResultSchema,
  VaultCreateSchema
} from '../../shared/schemas'
import { loadSettings, saveSettings } from './settings'
import { vaultRegistry } from './vault-registry'
import { sendToRenderer, buildWatcherConfig, emitActivityLog, formatZodError } from '../ipc'

import type { StateManager } from './state'
import type { VectorManager } from './vector'
import type { VaultWatcher } from './watcher'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface VaultOpenOptions {
  /** Explicit vault path. If omitted, a native folder picker is shown. */
  path?: string
  /** When true, only show the native picker without opening (used on launch). */
  showPicker?: boolean
}

export interface VaultOpenResult {
  /** Parsed vault scan result on success. */
  vault?: unknown
  /** Error string on failure. */
  error?: string
  /** True when the user canceled the folder picker. */
  canceled?: boolean
}

// ---------------------------------------------------------------------------
// VaultService
// ---------------------------------------------------------------------------

/**
 * Owns all vault lifecycle business logic.
 *
 * The service is constructed with the legacy singleton managers (used during
 * the v1→v2 migration) and coordinates them with the VaultRegistry and the
 * VaultWatcher.
 */
export class VaultService {
  private stateManager: StateManager
  private vectorManager: VectorManager
  private watcher: VaultWatcher

  constructor(stateManager: StateManager, vectorManager: VectorManager, watcher: VaultWatcher) {
    this.stateManager = stateManager
    this.vectorManager = vectorManager
    this.watcher = watcher
  }

  /**
   * Copy default templates into a vault on first open.
   * Non-fatal: failures are logged but do not abort the open.
   */
  private async copyDefaultTemplates(vaultPath: string): Promise<void> {
    const templatesDir = path.join(vaultPath, '_templates')

    // Only copy on first open — skip if _templates already exists
    try {
      await fs.access(templatesDir)
      return // directory exists; nothing to do
    } catch {
      // Directory does not exist — proceed with copy
    }

    // Resolve source directory based on whether the app is packaged
    const srcDir = app.isPackaged
      ? path.join(process.resourcesPath, 'default-templates')
      : path.join(__dirname, '..', '..', '..', 'resources', 'default-templates')

    // Create the _templates directory
    await fs.mkdir(templatesDir, { recursive: true })

    // Read all .md files from the source dir and copy each to _templates/
    const dirents = await fs.readdir(srcDir, { withFileTypes: true })
    await Promise.all(
      dirents
        .filter((d) => d.isFile() && d.name.endsWith('.md'))
        .map((d) => fs.copyFile(path.join(srcDir, d.name), path.join(templatesDir, d.name)))
    )
  }

  /**
   * Register a vault session in the registry and start its file watcher.
   * Shared by open / open-in-new-window / create flows.
   */
  private registerAndWatch(vaultPath: string, vaultMeta: { files: import('../../shared/types').FileEntry[] }): void {
    vaultRegistry.register(
      vaultPath, // vaultId is the vault path
      vaultPath,
      this.stateManager,
      this.vectorManager,
      this.watcher
    )
    vaultRegistry.setActive(vaultPath)

    // Start the file watcher (uses shared config with vector embedding)
    this.watcher.start(
      buildWatcherConfig(this.stateManager, this.vectorManager, vaultPath, vaultMeta)
    )
  }

  /**
   * Trigger an index build if the StateManager supports it, pushing the result
   * to the renderer. Guarded so missing support is silently ignored.
   */
  private async triggerIndexBuild(): Promise<void> {
    try {
      const indexResult = await (this.stateManager as any).buildIndexes?.()
      if (indexResult) {
        sendToRenderer(IPCChannel.INDEX_BUILD, indexResult)
      }
    } catch {
      // buildIndexes not yet available — silently ignore
    }
  }

  /**
   * Open a vault by path (or via native folder picker when no path is given).
   * Mirrors the previous `vault:open` IPC handler logic exactly.
   */
  async openVault(options: VaultOpenOptions): Promise<VaultOpenResult> {
    const validation = VaultOpenSchema.safeParse(options ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[VaultService] vault:open validation failed: ${reason}`)
      return { error: reason }
    }

    let parsedPath = validation.data.path

    // If no path provided, show native folder picker
    if (!parsedPath) {
      const focusedWindow = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0]
      const result = await dialog.showOpenDialog(focusedWindow, {
        properties: ['openDirectory'],
        title: 'Open Vault',
        buttonLabel: 'Open'
      })

      if (result.canceled || result.filePaths.length === 0) {
        return { canceled: true }
      }

      parsedPath = result.filePaths[0]
    }

    try {
      const vaultMeta = await this.stateManager.openVault(parsedPath)

      // Copy default templates on first open (non-fatal)
      try {
        await this.copyDefaultTemplates(parsedPath)
      } catch (copyErr) {
        emitActivityLog(
          'warn',
          `[VaultService] vault:open — failed to copy default templates: ${String(copyErr)}`
        )
      }

      this.registerAndWatch(parsedPath, vaultMeta)

      const response = VaultScanResultSchema.parse(vaultMeta)

      // Trigger index build
      await this.triggerIndexBuild()

      // Notify renderer that vault was opened (via validated channel)
      sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath: parsedPath, files: vaultMeta.files })

      return { vault: response }
    } catch (err) {
      const msg = `[VaultService] vault:open handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  }

  /**
   * Re-scan the current vault and return updated metadata.
   * Mirrors the previous `vault:scan` IPC handler logic exactly.
   */
  async scanVault(): Promise<VaultOpenResult> {
    try {
      const currentVault = this.stateManager.getCurrentVault()
      if (!currentVault) {
        return { error: 'No vault is currently open' }
      }

      const vaultMeta = await this.stateManager.openVault(currentVault.path)
      const response = VaultScanResultSchema.parse(vaultMeta)

      await this.triggerIndexBuild()

      return { vault: response }
    } catch (err) {
      const msg = `[VaultService] vault:scan handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  }

  /**
   * Close a vault session (stop watcher, release state).
   * Mirrors the previous `vault:close` IPC handler logic exactly.
   */
  async closeVault(rawPayload: unknown): Promise<{ success?: boolean; error?: string }> {
    const validation = VaultCloseSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[VaultService] vault:close validation failed: ${reason}`)
      return { error: reason }
    }

    const vaultId = validation.data.vaultId

    try {
      // Close vault session in registry if vaultId provided
      if (vaultId) {
        vaultRegistry.close(vaultId)
      } else {
        // Fall back to stopping the legacy watcher
        this.watcher.stop()
      }
      return { success: true }
    } catch (err) {
      const msg = `[VaultService] vault:close handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  }

  /**
   * Create a new vault directory, seed a Welcome note, and open it.
   * Mirrors the previous `vault:create` IPC handler logic exactly.
   */
  async createVault(rawPayload: unknown): Promise<VaultOpenResult> {
    const validation = VaultCreateSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[VaultService] vault:create validation failed: ${reason}`)
      return { error: reason }
    }

    const { parentPath, name } = validation.data
    const newPath = path.join(parentPath, name)

    try {
      // Create the vault directory
      await fs.mkdir(newPath, { recursive: true })

      // Write a Welcome.md file as the initial note
      const welcomePath = path.join(newPath, 'Welcome.md')
      const welcomeContent = `# Welcome to ${name}\n\nThis is your new vault. Start writing!\n`
      this.stateManager.setPendingWrite(welcomePath)
      try {
        await fs.writeFile(welcomePath, welcomeContent, 'utf-8')
      } finally {
        this.stateManager.clearPendingWrite(welcomePath)
      }

      // Open the newly created vault
      const vaultMeta = await this.stateManager.openVault(newPath)
      const result = VaultScanResultSchema.parse(vaultMeta)

      await this.triggerIndexBuild()

      return { vault: result }
    } catch (err) {
      const msg = `[VaultService] vault:create handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  }

  /**
   * Switch to a different already-registered vault.
   * Mirrors the previous `vault:switch` IPC handler logic exactly.
   */
  async switchVault(rawPayload: unknown): Promise<{ success: boolean; error?: string }> {
    const validation = VaultSwitchSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[VaultService] vault:switch validation failed: ${reason}`)
      return { success: false, error: reason }
    }

    const { vaultId } = validation.data

    try {
      // Check if the vault is already open in the registry
      const session = vaultRegistry.get(vaultId)
      if (session) {
        vaultRegistry.setActive(vaultId)
        return { success: true }
      }

      // If not in registry, check if it's the current vault
      const currentVault = this.stateManager.getCurrentVault()
      if (currentVault && currentVault.path === vaultId) {
        return { success: true }
      }

      return { success: false, error: 'Vault not found in registry' }
    } catch (err) {
      const msg = `[VaultService] vault:switch handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { success: false, error: String(err) }
    }
  }

  /**
   * Get the current vault metadata (or null if none open).
   * Mirrors the previous `vault:get-current` IPC handler logic exactly.
   */
  getCurrentVault(): unknown {
    try {
      const vault = this.stateManager.getCurrentVault()
      if (!vault) return null
      return VaultScanResultSchema.parse(vault)
    } catch (err) {
      console.error('[VaultService] getCurrentVault error:', err)
      return null
    }
  }

  /**
   * Get the list of recently opened vaults from settings.
   * Mirrors the previous `vault:get-recents` IPC handler logic exactly.
   */
  async getRecents(): Promise<unknown> {
    try {
      const settings = await loadSettings()
      // recentVaults is already an array of { path, name, lastOpened }
      return { recents: settings.recentVaults ?? [] }
    } catch (err) {
      const msg = `[VaultService] vault:get-recents handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { recents: [] }
    }
  }

  /**
   * Open a vault in a second BrowserWindow (Req 22.7).
   * Mirrors the previous `vault:open-in-new-window` IPC handler logic exactly.
   */
  async openVaultInNewWindow(rawPayload: unknown): Promise<{ success?: boolean; path?: string; error?: string }> {
    const validation = VaultOpenSchema.safeParse(rawPayload ?? {})
    if (!validation.success) {
      const reason = formatZodError(validation.error)
      emitActivityLog('warn', `[VaultService] vault:open-in-new-window validation failed: ${reason}`)
      return { error: reason }
    }

    const vaultPath = validation.data.path
    if (!vaultPath) {
      return { error: 'No vault path provided' }
    }

    try {
      // Check path is accessible
      await fs.access(vaultPath, fs.constants.R_OK)

      // Open the vault in the registry (Req 22.7)
      const vaultMeta = await this.stateManager.openVault(vaultPath)

      // Copy default templates on first open (non-fatal)
      try {
        await this.copyDefaultTemplates(vaultPath)
      } catch (copyErr) {
        emitActivityLog(
          'warn',
          `[VaultService] vault:open-in-new-window — failed to copy default templates: ${String(copyErr)}`
        )
      }

      // Register vault session in the registry
      vaultRegistry.register(vaultPath, vaultPath, this.stateManager, this.vectorManager, this.watcher)
      vaultRegistry.setActive(vaultPath)

      // Start the file watcher for this vault
      this.watcher.start(buildWatcherConfig(this.stateManager, this.vectorManager, vaultPath, vaultMeta))

      // Create a new BrowserWindow for this vault (Req 22.7)
      const newWindow = new BrowserWindow({
        width: 1200,
        height: 800,
        show: false,
        autoHideMenuBar: false,
        webPreferences: {
          preload: path.join(__dirname, '../preload/index.js'),
          sandbox: false,
          contextIsolation: true,
          nodeIntegration: false
        }
      })

      // Load renderer in the new window
      if (process.env['VITE_DEV_SERVER_URL']) {
        await newWindow.loadURL(process.env['VITE_DEV_SERVER_URL'])
      } else {
        await newWindow.loadFile(path.join(__dirname, '../renderer/index.html'))
      }

      newWindow.on('ready-to-show', () => {
        newWindow.show()
      })

      // Send vault state to the new window
      newWindow.webContents.once('did-finish-load', () => {
        sendToRenderer(IPCChannel.NOTES_LOADED, { vaultPath, files: vaultMeta.files })
      })

      return { success: true, path: vaultPath }
    } catch (err) {
      const msg = `[VaultService] vault:open-in-new-window handler error: ${String(err)}`
      console.error(msg)
      emitActivityLog('error', msg)
      return { error: String(err) }
    }
  }

  /**
   * Attempt to reopen the last-used vault on launch.
   * Extracted from `restoreVault` in index.ts. Preserves identical behavior:
   *  - No lastVaultPath → signal renderer to show picker
   *  - Path unreadable → error dialog, clear stale path, show picker
   *  - Open succeeds → register + watch
   *  - Open fails → error dialog, clear stale path, show picker
   */
  async restoreVault(mainWindow: BrowserWindow): Promise<void> {
    const settings = await loadSettings()

    if (!settings.lastVaultPath) {
      // No previously opened vault — renderer will show the picker
      mainWindow.webContents.send(IPCChannel.VAULT_OPEN, { showPicker: true })
      return
    }

    try {
      // Check path is readable (Req 1.8)
      await fs.access(settings.lastVaultPath, fs.constants.R_OK)
    } catch {
      // Path no longer accessible — show error then fall back to picker (Req 1.8)
      await dialog
        .showMessageBox(mainWindow, {
          type: 'error',
          title: 'Vault Not Found',
          message: 'Could not reopen last vault',
          detail: `"${settings.lastVaultPath}" no longer exists or is not readable.\n\nPlease select a different vault.`,
          buttons: ['OK']
        })
        .catch(() => {})

      // Clear the stale path so we don't retry on next launch
      await saveSettings({ ...settings, lastVaultPath: null })

      // Signal renderer to show vault picker (Req 1.6, 1.8)
      mainWindow.webContents.send(IPCChannel.VAULT_OPEN, { showPicker: true })
      return
    }

    try {
      const vaultMeta = await this.stateManager.openVault(settings.lastVaultPath)

      // Start the file watcher for the restored vault (uses shared config with vector embedding)
      this.watcher.start(
        buildWatcherConfig(this.stateManager, this.vectorManager, settings.lastVaultPath, vaultMeta)
      )
    } catch (err) {
      console.error('[VaultService] Failed to open vault:', err)

      await dialog
        .showMessageBox(mainWindow, {
          type: 'error',
          title: 'Vault Error',
          message: 'Failed to open vault',
          detail: `${String(err)}\n\nPlease select a different vault.`,
          buttons: ['OK']
        })
        .catch(() => {})

      await saveSettings({ ...settings, lastVaultPath: null })
      mainWindow.webContents.send(IPCChannel.VAULT_OPEN, { showPicker: true })
    }
  }

  /**
   * Open a vault for E2E test injection (NABU_TEST_VAULT env var).
   * Extracted from index.ts. Preserves identical behavior.
   */
  async openTestVault(testVaultPath: string): Promise<void> {
    const vaultMeta = await this.stateManager.openVault(testVaultPath)
    // Start the file watcher (uses shared config with vector embedding)
    this.watcher.start(
      buildWatcherConfig(this.stateManager, this.vectorManager, testVaultPath, vaultMeta)
    )
    // Push vault state to the renderer. This may arrive before or after
    // React mounts. The renderer also polls via vault:get-current so
    // whichever path succeeds first wins.
    sendToRenderer(IPCChannel.NOTES_LOADED, {
      vaultPath: testVaultPath,
      files: vaultMeta.files
    })
  }
}
