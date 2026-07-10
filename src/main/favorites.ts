/**
 * favorites.ts
 *
 * Per-vault favorites persistence. Favorites are stored in `.nabu/favorites.json`
 * as a simple array of file paths. The file is read/written synchronously via
 * the main process IPC handlers.
 *
 * Requirements: 18.1, 18.2, 18.3, 18.5, 18.6
 */

import path from 'path'
import fs from 'fs/promises'

/**
 * Get the path to the favorites file for a given vault.
 */
export function favoritesFilePath(vaultPath: string): string {
  return path.join(vaultPath, '.nabu', 'favorites.json')
}

/**
 * Read the favorites list for a vault. Returns an empty array if the file
 * doesn't exist or can't be parsed.
 */
export async function readFavorites(vaultPath: string): Promise<string[]> {
  try {
    const raw = await fs.readFile(favoritesFilePath(vaultPath), 'utf-8')
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      return parsed.filter((p): p is string => typeof p === 'string')
    }
    return []
  } catch {
    return []
  }
}

/**
 * Write the favorites list for a vault. Creates the `.nabu/` directory if
 * it doesn't exist.
 */
export async function writeFavorites(vaultPath: string, favorites: string[]): Promise<void> {
  const dir = path.join(vaultPath, '.nabu')
  await fs.mkdir(dir, { recursive: true })
  await fs.writeFile(favoritesFilePath(vaultPath), JSON.stringify(favorites, null, 2), 'utf-8')
}

/**
 * Toggle a file path in the favorites list. If the path is already present,
 * it is removed; otherwise it is added. Returns the updated list.
 */
export async function toggleFavorite(vaultPath: string, filePath: string): Promise<string[]> {
  const favorites = await readFavorites(vaultPath)
  const index = favorites.indexOf(filePath)
  if (index >= 0) {
    favorites.splice(index, 1)
  } else {
    favorites.push(filePath)
  }
  await writeFavorites(vaultPath, favorites)
  return favorites
}

/**
 * Remove a file path from favorites (e.g. when a note is deleted or renamed).
 * Returns the updated list.
 */
export async function removeFavorite(vaultPath: string, filePath: string): Promise<string[]> {
  const favorites = await readFavorites(vaultPath)
  const index = favorites.indexOf(filePath)
  if (index >= 0) {
    favorites.splice(index, 1)
    await writeFavorites(vaultPath, favorites)
  }
  return favorites
}
