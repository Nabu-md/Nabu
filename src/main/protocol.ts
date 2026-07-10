/**
 * protocol.ts
 *
 * Custom protocol handler for Nabu web clipper (Phase 5).
 * Handles nabu://clip?title=...&content=... URLs to create clipped notes.
 *
 * Requirements: Phase 5 (Web Clipper)
 */

import { app, protocol } from 'electron'
import { join } from 'path'
import fs from 'fs/promises'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function generateContent(title: string, content: string): string {
  const date = new Date().toISOString().slice(0, 10)
  return `# ${title}\n\n${content}\n\n---\n\nClipped ${date}\n`
}

// ---------------------------------------------------------------------------
// Protocol registration
// ---------------------------------------------------------------------------

/**
 * Register the nabu:// protocol handler.
 * Called during app initialization.
 */
export function registerNabuProtocol(): void {
  // Check if protocol is already registered
  try {
    protocol.registerFileProtocol('nabu', (_request, callback) => {
      // This will be handled via IPC when a clip request comes in
      callback({ path: '' })
    })
  } catch (err) {
    console.error('[Protocol] Protocol may already be registered:', err)
  }
}

// ---------------------------------------------------------------------------
// Clip handler
// ---------------------------------------------------------------------------

/**
 * Handle a clip request - create a new note with clipped content.
 * This is called via IPC from the renderer.
 */
export async function handleClipRequest(
  vaultPath: string,
  title: string,
  content: string
): Promise<{ success: boolean; path?: string; error?: string }> {
  if (!vaultPath) {
    return { success: false, error: 'No vault open' }
  }

  // Create a unique filename
  const date = new Date().toISOString().slice(0, 10)
  const time = new Date().toTimeString().slice(0, 5).replace(':', '-')
  const fileName = `${title.replace(/[^a-zA-Z0-9]/g, '-')}-${date}-${time}`.replace(/-+/g, '-')
  const filePath = join(vaultPath, `${fileName}.md`)

  const noteContent = generateContent(title, content)

  try {
    await fs.writeFile(filePath, noteContent, 'utf-8')
    return { success: true, path: filePath }
  } catch (err) {
    return { success: false, error: String(err) }
  }
}