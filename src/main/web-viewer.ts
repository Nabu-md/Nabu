/**
 * web-viewer.ts
 *
 * Embedded browser view for external links.
 * Opens URLs in BrowserView instead of system browser.
 *
 * Requirements: 34.1, 34.2, 34.3, 34.4, 34.5, 34.6
 */

import { BrowserWindow } from 'electron'

interface WebViewerSession {
  window: BrowserWindow
  url: string
}

const activeViewers = new Map<string, WebViewerSession>()

/**
 * Check if a URL is safe to load in the web viewer.
 */
export function isWebViewerUrl(url: string): boolean {
  // Block file:// and localhost URLs
  if (url.startsWith('file://') || url.includes('localhost')) {
    return false
  }
  return true
}

/**
 * Open a URL in the web viewer.
 */
export function openWebViewer(
  parentWindow: BrowserWindow,
  url: string,
  vaultId: string
): BrowserWindow | null {
  if (!isWebViewerUrl(url)) {
    return null
  }

  const viewer = new BrowserWindow({
    width: 1024,
    height: 768,
    parent: parentWindow,
    webPreferences: {
      contextIsolation: true,
      sandbox: true
    }
  })

  viewer.loadURL(url)
  activeViewers.set(vaultId, { window: viewer, url })

  return viewer
}

/**
 * Close web viewer for a vault.
 */
export function closeWebViewer(vaultId: string): void {
  const session = activeViewers.get(vaultId)
  if (session) {
    session.window.close()
    activeViewers.delete(vaultId)
  }
}

/**
 * Clear session state when viewer closes.
 */
export function cleanupWebViewer(vaultId: string): void {
  activeViewers.delete(vaultId)
}
