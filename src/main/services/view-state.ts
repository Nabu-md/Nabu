/**
 * view-state.ts
 *
 * Persistent view state management for Nabu.
 * Stores fold states, cursor positions, and other UI preferences per note.
 *
 * Requirements: Phase 2 (Collapsible Headings)
 */

import { join } from 'path'
import fs from 'fs/promises'
import { toVaultRelative } from '@shared/path'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ViewState {
  foldStates: Record<string, boolean> // headingId → isOpen
}

// In-memory cache for quick access during session
const viewStateCache = new Map<string, ViewState>()

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Get the .nabu/view-state directory path for a vault.
 */
function getViewStateDir(vaultPath: string): string {
  return join(vaultPath, '.nabu', 'view-state')
}

/**
 * Get the view state file path for a specific note.
 */
function getViewStateFile(vaultPath: string, notePath: string): string {
  const dir = getViewStateDir(vaultPath)
  // Convert note path to a safe filename (relative to vault).
  // Canonical vault-relative resolution (Phase 4.3) replaces the previous
  // `notePath.replace(vaultPath, '')` which was unsafe for vault names that
  // are substrings of later path segments.
  const relativePath = toVaultRelative(vaultPath, notePath).replace(/\//g, '--')
  return join(dir, `${relativePath}.json`)
}

/**
 * Ensure the view-state directory exists.
 */
async function ensureViewStateDir(vaultPath: string): Promise<void> {
  const dir = getViewStateDir(vaultPath)
  try {
    await fs.access(dir)
  } catch {
    await fs.mkdir(dir, { recursive: true })
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Load view state for a note from disk.
 * Falls back to cache if available, or empty state if not found.
 */
export async function loadViewState(vaultPath: string, notePath: string): Promise<ViewState> {
  const cacheKey = `${vaultPath}:${notePath}`

  // Check memory cache first
  if (viewStateCache.has(cacheKey)) {
    return viewStateCache.get(cacheKey)!
  }

  // Try to load from disk
  try {
    const filePath = getViewStateFile(vaultPath, notePath)
    const content = await fs.readFile(filePath, 'utf-8')
    const state = JSON.parse(content) as ViewState
    viewStateCache.set(cacheKey, state)
    return state
  } catch {
    // Return empty state if file doesn't exist
    const emptyState: ViewState = { foldStates: {} }
    viewStateCache.set(cacheKey, emptyState)
    return emptyState
  }
}

/**
 * Save view state for a note to disk.
 * Merges with existing state if present.
 */
export async function saveViewState(
  vaultPath: string,
  notePath: string,
  state: Partial<ViewState>
): Promise<void> {
  const cacheKey = `${vaultPath}:${notePath}`

  // Update cache
  const existing = viewStateCache.get(cacheKey) ?? { foldStates: {} }
  const merged = { ...existing, ...state }
  viewStateCache.set(cacheKey, merged)

  // Save to disk
  try {
    await ensureViewStateDir(vaultPath)
    const filePath = getViewStateFile(vaultPath, notePath)
    await fs.writeFile(filePath, JSON.stringify(merged, null, 2), 'utf-8')
  } catch (err) {
    console.error(`[ViewState] Failed to save view state for ${notePath}:`, err)
  }
}

/**
 * Set a fold state for a specific heading.
 */
export async function setFoldState(
  vaultPath: string,
  notePath: string,
  headingId: string,
  isOpen: boolean
): Promise<void> {
  const cacheKey = `${vaultPath}:${notePath}`

  // Update cache
  const existing = viewStateCache.get(cacheKey) ?? { foldStates: {} }
  const newFoldStates = { ...existing.foldStates, [headingId]: isOpen }
  viewStateCache.set(cacheKey, { foldStates: newFoldStates })

  // Save to disk
  await saveViewState(vaultPath, notePath, { foldStates: newFoldStates })
}

/**
 * Remove view state for a specific file (used when a note is deleted or renamed).
 * Clears both the in-memory cache and the persisted file.
 */
export async function clearViewStateForFile(vaultPath: string, notePath: string): Promise<void> {
  const cacheKey = `${vaultPath}:${notePath}`
  viewStateCache.delete(cacheKey)

  try {
    const filePath = getViewStateFile(vaultPath, notePath)
    await fs.unlink(filePath).catch(() => {})
  } catch {
    // File may not exist — ignore
  }
}