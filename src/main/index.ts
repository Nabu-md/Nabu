/**
 * index.ts
 *
 * App entry point — manages the Electron application lifecycle, BrowserWindow
 * creation, macOS menu registration (with keyboard shortcuts), settings
 * persistence, and vault restore on launch.
 *
 * Requirements: 1.1, 1.6, 1.7, 1.8, 14.1, 14.2, 14.3, 14.4, 14.5
 */

import {
  app,
  BrowserWindow,
  Menu,
  dialog,
  shell,
  ipcMain,
  MenuItemConstructorOptions
} from 'electron'
import * as path from 'path'

import { StateManager } from './services/state'
import { VectorManager } from './services/vector'
import { VaultWatcher } from './services/watcher'
import { registerIPCHandlers, onWidgetToggle } from './ipc'
import { IPCChannel } from '@shared/channels'
import { loadSettings, saveSettings } from './services/settings'
import { fnMonitor } from './services/fn-monitor'
import { registerWidgetIPCHandlers, wireFnMonitorToWidget, widgetManager } from './services/widget-manager'
import type { AppSettings } from './services/settings'
import { VaultService } from './services/vault-service'
import { WidgetService } from './services/widget-service'

// ---------------------------------------------------------------------------
// createWindow
// ---------------------------------------------------------------------------

/**
 * Create and return the main BrowserWindow.
 *
 * - Default size: 1200 × 800
 * - Loads the renderer via the Vite dev server URL in development, or the
 *   built index.html in production.
 * - Preload script is loaded from the compiled output directory.
 *
 * Requirements: 1.1
 */
export function createWindow(bounds?: AppSettings['windowBounds']): BrowserWindow {
  const mainWindow = new BrowserWindow({
    width: bounds?.width ?? 1200,
    height: bounds?.height ?? 800,
    x: bounds?.x,
    y: bounds?.y,
    show: false,
    autoHideMenuBar: false,
    webPreferences: {
      preload: path.join(__dirname, '../preload/index.js'),
      sandbox: false,
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  mainWindow.on('ready-to-show', () => {
    mainWindow.show()
  })

  // Persist window bounds when the user moves/resizes the window
  const persistBounds = (): void => {
    const b = mainWindow.getBounds()
    loadSettings()
      .then((s) => saveSettings({ ...s, windowBounds: b }))
      .catch((err) => console.error('[Settings] Failed to persist window bounds:', err))
  }
  mainWindow.on('resize', persistBounds)
  mainWindow.on('move', persistBounds)

  // Open external links in the system browser
  mainWindow.webContents.setWindowOpenHandler((details) => {
    shell.openExternal(details.url).catch(() => {})
    return { action: 'deny' }
  })

  // Load renderer: dev server URL or built file
  // electron-vite v5 exposes the renderer dev server URL via ELECTRON_RENDERER_URL
  // (older electron-vite versions used VITE_DEV_SERVER_URL).
  const rendererDevUrl = process.env['ELECTRON_RENDERER_URL'] ?? process.env['VITE_DEV_SERVER_URL']
  if (rendererDevUrl) {
    mainWindow.loadURL(rendererDevUrl).catch((err) => {
      console.error('[Window] Failed to load dev server URL:', err)
    })
  } else {
    mainWindow.loadFile(path.join(__dirname, '../renderer/index.html')).catch((err) => {
      console.error('[Window] Failed to load renderer HTML:', err)
    })
  }

  return mainWindow
}

// ---------------------------------------------------------------------------
// registerMenu
// ---------------------------------------------------------------------------

/**
 * Build and set the macOS application menu.
 *
 * Keyboard shortcuts are registered via `accelerator` in the Electron Menu
 * API — not DOM keydown handlers — as required by Requirement 14.5.
 *
 * Shortcuts:
 *   Cmd+O         → vault folder picker (Req 14.1)
 *   Cmd+W         → close main window  (Req 14.2)
 *   Cmd+,         → open Preferences   (Req 14.3)
 *   Cmd+Shift+F   → focus FileTree search input (Req 14.4)
 *
 * Requirements: 14.1, 14.2, 14.3, 14.4, 14.5
 */
export function registerMenu(mainWindow: BrowserWindow): void {
  const isMac = process.platform === 'darwin'

  const appMenuItems: MenuItemConstructorOptions[] = isMac
    ? [
        { role: 'about' },
        { type: 'separator' },
        {
          label: 'Preferences…',
          accelerator: 'CmdOrCtrl+,',
          click: (): void => {
            mainWindow.webContents.send('open:settings')
          }
        },
        { type: 'separator' },
        { role: 'services' },
        { type: 'separator' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit' }
      ]
    : [
        {
          label: 'Preferences…',
          accelerator: 'CmdOrCtrl+,',
          click: (): void => {
            mainWindow.webContents.send('open:settings')
          }
        }
      ]

  const template: MenuItemConstructorOptions[] = [
    // ---- Application menu (macOS only) ----
    ...(isMac ? [{ label: app.name, submenu: appMenuItems }] : []),

    // ---- File menu ----
    {
      label: 'File',
      submenu: [
        {
          label: 'Open Vault…',
          accelerator: 'CmdOrCtrl+O',
          click: (): void => {
            // Req 14.1 — trigger vault:open with no path (shows native picker)
            mainWindow.webContents
              .executeJavaScript(`window.__nabuOpenVault && window.__nabuOpenVault()`)
              .catch(() => {
                // Fallback: invoke vault:open directly via IPC emulation
                dialog
                  .showOpenDialog(mainWindow, {
                    properties: ['openDirectory'],
                    title: 'Open Vault',
                    buttonLabel: 'Open'
                  })
                  .then(async (result) => {
                    if (result.canceled || result.filePaths.length === 0) return
                    mainWindow.webContents.send(IPCChannel.VAULT_OPEN, {
                      path: result.filePaths[0]
                    })
                  })
                  .catch((err) => console.error('[Menu] Open Vault dialog error:', err))
              })
          }
        },
        {
          label: 'Create Vault…',
          click: (): void => {
            mainWindow.webContents.send('setup:create')
          }
        },
        {
          label: 'Switch Vault',
          click: (): void => {
            mainWindow.webContents.send('setup:open')
          }
        },
        { type: 'separator' },
        {
          // v2: open vault in a new window (Requirement 22.7)
          label: 'Open in New Window',
          click: async (): Promise<void> => {
            const result = await dialog
              .showOpenDialog(mainWindow, {
                properties: ['openDirectory'],
                title: 'Open Vault in New Window',
                buttonLabel: 'Open'
              })
              .catch(() => ({ canceled: true, filePaths: [] }))
            if (result.canceled || result.filePaths.length === 0) return
            mainWindow.webContents.send(IPCChannel.VAULT_OPEN_IN_NEW_WINDOW, {
              path: result.filePaths[0]
            })
          }
        },
        { type: 'separator' },
        {
          label: 'Close Window',
          accelerator: 'CmdOrCtrl+W',
          click: (): void => {
            // Req 14.2 — close the main window
            mainWindow.close()
          }
        }
      ]
    },

    // ---- View menu ----
    {
      label: 'View',
      submenu: [
        {
          label: 'Search in File Tree',
          accelerator: 'CmdOrCtrl+Shift+F',
          click: (): void => {
            // Req 14.4 — send IPC event to renderer to focus FileTree search input
            // Send directly (not via sendToRenderer) to bypass Zod schema validation
            // which enforces the context:search results shape.
            for (const win of BrowserWindow.getAllWindows()) {
              if (!win.isDestroyed()) {
                win.webContents.send('focus:search')
              }
            }
          }
        },
        { type: 'separator' },
        { role: 'reload' },
        { role: 'forceReload' },
        { role: 'toggleDevTools' },
        { type: 'separator' },
        { role: 'resetZoom' },
        { role: 'zoomIn' },
        { role: 'zoomOut' },
        { type: 'separator' },
        { role: 'togglefullscreen' }
      ]
    },

    // ---- Window menu ----
    {
      label: 'Window',
      submenu: [
        { role: 'minimize' },
        { role: 'zoom' },
        ...(isMac
          ? [
              { type: 'separator' as const },
              { role: 'front' as const },
              { type: 'separator' as const },
              { role: 'window' as const }
            ]
          : [{ role: 'close' as const }])
      ]
    }
  ]

  const menu = Menu.buildFromTemplate(template)
  Menu.setApplicationMenu(menu)
}

// ---------------------------------------------------------------------------
// restoreVault
// ---------------------------------------------------------------------------

/**
 * Attempt to reopen the last-used vault on launch.
 *
 * - Loads `lastVaultPath` from settings
 * - Checks readability via `fs.access`
 * - If readable: calls `stateManager.openVault()` and pushes `notes:loaded`
 * - If invalid/missing: shows an error dialog and signals the renderer to
 *   display the vault picker
 *
 * Requirements: 1.7, 1.8
 */
async function restoreVault(vaultService: VaultService, mainWindow: BrowserWindow): Promise<void> {
  await vaultService.restoreVault(mainWindow)
}

// ---------------------------------------------------------------------------
// Persist last vault path when vault:open resolves
// ---------------------------------------------------------------------------

/**
 * Listen for successful vault:open results from the IPC layer and persist the
 * chosen path to settings so it can be restored on the next launch.
 *
 * Requirements: 1.7
 */
function registerVaultPersistence(): void {
  ipcMain.on('vault:opened', (_event, vaultPath: string) => {
    loadSettings()
      .then((s) => saveSettings({ ...s, lastVaultPath: vaultPath }))
      .catch((err) => console.error('[Settings] Failed to persist vault path:', err))
  })
}

// ---------------------------------------------------------------------------
// app.whenReady
// ---------------------------------------------------------------------------

app.whenReady().then(async () => {
  // macOS minimum version guard (13.0 Ventura)
  if (process.platform === 'darwin') {
    const release = process.getSystemVersion()
    const [major] = release.split('.').map(Number)
    if (major < 13) {
      dialog.showErrorBox(
        'macOS Version Not Supported',
        'Nabu requires macOS 13.0 (Ventura) or later.'
      )
      app.quit()
      return
    }
  }

  // ---- Instantiate core modules ----
  let stateManager: StateManager
  let vectorManager: VectorManager
  let watcher: VaultWatcher

  try {
    stateManager = new StateManager()
    vectorManager = new VectorManager()
    watcher = new VaultWatcher()
  } catch (err) {
    dialog.showErrorBox(
      'Initialization Error',
      `Failed to initialize core modules:\n\n${String(err)}`
    )
    app.quit()
    return
  }

  // ---- Register IPC handlers ----
  try {
    registerIPCHandlers(stateManager, vectorManager, watcher)
  } catch (err) {
    console.error('[App] Failed to register IPC handlers:', err)
    dialog.showErrorBox('IPC Error', `Failed to register IPC handlers:\n\n${String(err)}`)
    app.quit()
    return
  }

  // ---- Register widget IPC handlers (dictation/clipboard widget) ----
  try {
    registerWidgetIPCHandlers()
  } catch (err) {
    console.error('[App] Failed to register widget IPC handlers:', err)
  }

  // ---- Start fn-monitor for dictation (macOS only) ----
  if (process.platform === 'darwin') {
    try {
      fnMonitor.start()
      wireFnMonitorToWidget()
    } catch (err) {
      console.error('[App] Failed to start fn-monitor:', err)
    }
  }

  // ---- Register vault path persistence listener ----
  registerVaultPersistence()

  // ---- Create window (restore saved bounds) ----
  const settings = await loadSettings()
  const mainWindow = createWindow(settings.windowBounds ?? undefined)

  // ---- Register macOS application menu ----
  registerMenu(mainWindow)

  // ---- Deferred: initialise vector index after window is visible ----
  mainWindow.once('ready-to-show', () => {
    const nabuDir = path.join(app.getPath('userData'), '.nabu')
    // Resolve model path based on packaging state (Req 12.4):
    //   - Packaged: models are extracted to <Resources>/models/ by electron-builder extraResources
    //   - Development: models live under resources/models/ in the repo root
    const modelPath = app.isPackaged
      ? path.join(process.resourcesPath, 'models', 'bge-micro-v2')
      : path.join(__dirname, '..', '..', '..', 'resources', 'models', 'bge-micro-v2')

    vectorManager
      .initialize({ indexPath: nabuDir, modelPath })
      .catch((err) => console.error('[App] Vector manager init failed:', err))

    // ---- Widget service: start history service, register widget IPC handlers ----
    // All widget lifecycle / coordination business logic now lives in
    // WidgetService; bootstrap only initializes and delegates.
    const widgetService = new WidgetService()
    widgetService.registerIPCHandlers()

    // Wire feature-toggle changes for clipboard-widget to WidgetManager
    onWidgetToggle((enabled: boolean) => {
      widgetManager.setEnabled(enabled)
    })
  })

  // ---- Restore last vault once the renderer is ready ----
  // VaultService owns the vault lifecycle business logic; bootstrap delegates.
  const vaultService = new VaultService(stateManager, vectorManager, watcher)
  mainWindow.webContents.once('did-finish-load', () => {
    // Support NABU_TEST_VAULT env var for E2E test injection (bypasses persisted settings)
    const testVaultPath = process.env['NABU_TEST_VAULT']
    if (testVaultPath) {
      vaultService
        .openTestVault(testVaultPath)
        .catch((err) => {
          console.error('[App] NABU_TEST_VAULT open failed:', err)
        })
      return
    }
    restoreVault(vaultService, mainWindow).catch((err) => {
      console.error('[App] restoreVault error:', err)
    })
  })

  // Re-create window on macOS dock click when no windows are open
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

// Quit on all windows closed (except macOS — menu bar stays active)
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})
