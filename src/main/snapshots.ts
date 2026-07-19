/**
 * snapshots.ts
 *
 * File snapshot management for recovery from accidental edits/deletions.
 * Snapshots are stored in `.nabu/snapshots/<path>/` with per-note and per-vault caps.
 *
 * Requirements: 29.1, 29.2, 29.3, 29.4, 29.5, 29.6, 29.7, 29.8, 29.9
 */

import path from 'path'
import fs from 'fs/promises'
import { toVaultRelative } from '@shared/path'

interface Snapshot {
  timestamp: number
  content: string
  path: string
}

/**
 * Get the snapshot directory for a vault.
 */
export function getSnapshotDir(vaultPath: string): string {
  return path.join(vaultPath, '.nabu', 'snapshots')
}

/**
 * Get the snapshot file path for a note.
 */
export function getSnapshotPath(vaultPath: string, notePath: string, timestamp: number): string {
  const relativePath = notePath.replace(vaultPath, '').replace(/^\//, '').replace(/\.md$/, '')
  return path.join(getSnapshotDir(vaultPath), `${relativePath}-${timestamp}.json`)
}

/**
 * Create a snapshot before saving a note.
 * Maintains per-note cap (50) and per-vault cap (1000).
 */
export async function createSnapshot(
  vaultPath: string,
  notePath: string,
  content: string
): Promise<void> {
  const snapDir = getSnapshotDir(vaultPath)
  const timestamp = Date.now()

  try {
    // Ensure snapshot directory exists
    await fs.mkdir(snapDir, { recursive: true })

    // Create snapshot file
    const snapPath = getSnapshotPath(vaultPath, notePath, timestamp)
    const snapshot: Snapshot = {
      timestamp,
      content,
      path: notePath
    }
    await fs.writeFile(snapPath, JSON.stringify(snapshot), 'utf-8')

    // Prune old snapshots (per-note cap)
    await pruneNoteSnapshots(vaultPath, notePath, 50)

    // Prune vault-wide snapshots
    await pruneVaultSnapshots(vaultPath, 1000)
  } catch (err) {
    console.error(`[snapshots] Failed to create snapshot for "${notePath}":`, err)
  }
}

/**
 * Prune snapshots for a single note to stay under cap.
 */
async function pruneNoteSnapshots(vaultPath: string, notePath: string, cap: number): Promise<void> {
  const relativePath = toVaultRelative(vaultPath, notePath).replace(/\.md$/, '')
  const snapDir = getSnapshotDir(vaultPath)

  try {
    const files = await fs.readdir(snapDir)
    const noteSnaps = files
      .filter((f) => f.startsWith(relativePath))
      .map((f) => ({
        name: f,
        path: path.join(snapDir, f),
        timestamp: parseInt(f.split('-').pop()?.replace('.json', '') ?? '0', 10)
      }))
      .sort((a, b) => b.timestamp - a.timestamp)

    // Remove oldest snapshots beyond cap
    for (const snap of noteSnaps.slice(cap)) {
      await fs.unlink(snap.path).catch(() => {})
    }
  } catch {
    // Directory may not exist
  }
}

/**
 * Prune all snapshots in vault to stay under cap.
 */
async function pruneVaultSnapshots(vaultPath: string, cap: number): Promise<void> {
  const snapDir = getSnapshotDir(vaultPath)

  try {
    const files = await fs.readdir(snapDir)
    const allSnaps = await Promise.all(
      files.map(async (f) => {
        const snapPath = path.join(snapDir, f)
        const stat = await fs.stat(snapPath)
        return {
          path: snapPath,
          timestamp: stat.mtimeMs
        }
      })
    )

    // Sort by timestamp (oldest first)
    allSnaps.sort((a, b) => a.timestamp - b.timestamp)

    // Remove oldest if over cap
    const overBy = allSnaps.length - cap
    if (overBy > 0) {
      for (let i = 0; i < overBy; i++) {
        await fs.unlink(allSnaps[i].path).catch(() => {})
      }
    }
  } catch {
    // Directory may not exist
  }
}

/**
 * List all snapshots for a vault.
 */
export async function listSnapshots(vaultPath: string): Promise<Snapshot[]> {
  const snapDir = getSnapshotDir(vaultPath)

  try {
    const files = await fs.readdir(snapDir)
    const snapshots: Snapshot[] = []

    for (const f of files) {
      if (!f.endsWith('.json')) continue
      const snapPath = path.join(snapDir, f)
      try {
        const raw = await fs.readFile(snapPath, 'utf-8')
        const snap = JSON.parse(raw) as Snapshot
        snapshots.push(snap)
      } catch {
        // Skip corrupted snapshots
      }
    }

    return snapshots.sort((a, b) => b.timestamp - a.timestamp)
  } catch {
    return []
  }
}

/**
 * Remove all snapshots for a specific note (used when a note is deleted or renamed).
 */
export async function removeSnapshotsForNote(vaultPath: string, notePath: string): Promise<void> {
  const snapDir = getSnapshotDir(vaultPath)
  const relativePath = toVaultRelative(vaultPath, notePath).replace(/\.md$/, '')

  try {
    const files = await fs.readdir(snapDir)
    for (const f of files) {
      if (f.startsWith(relativePath)) {
        await fs.unlink(path.join(snapDir, f)).catch(() => {})
      }
    }
  } catch {
    // Directory may not exist
  }
}

/**
 * Restore a snapshot to a note or create as new note.
 */
export async function restoreSnapshot(
  vaultPath: string,
  notePath: string,
  snapshotTimestamp: number,
  asNew: boolean = false
): Promise<{ success: boolean; error?: string }> {
  const snapDir = getSnapshotDir(vaultPath)
  const relativePath = toVaultRelative(vaultPath, notePath).replace(/\.md$/, '')
  const snapPath = path.join(snapDir, `${relativePath}-${snapshotTimestamp}.json`)

  try {
    const raw = await fs.readFile(snapPath, 'utf-8')
    const snap = JSON.parse(raw) as Snapshot

    const targetPath = asNew ? `${notePath.replace(/\.md$/, '')}-restored.md` : notePath

    // Create a pre-restore snapshot first
    const currentContent = await fs.readFile(targetPath, 'utf-8').catch(() => '')
    if (currentContent) {
      await createSnapshot(vaultPath, targetPath, currentContent)
    }

    await fs.writeFile(targetPath, snap.content, 'utf-8')
    return { success: true }
  } catch (err) {
    return { success: false, error: String(err) }
  }
}
