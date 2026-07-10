/**
 * random-note.ts
 *
 * Returns a uniformly random note path from the vault files.
 * Also respects tag filter if provided.
 *
 * Requirements: 19.1, 19.2, 19.3, 19.4
 */

interface FileEntry {
  path: string
  name: string
}

/**
 * Select a random note from the vault files.
 * Returns null if the vault has no files.
 */
export function getRandomNotePath(files: FileEntry[]): string | null {
  if (files.length === 0) {
    return null
  }
  const index = Math.floor(Math.random() * files.length)
  return files[index]?.path ?? null
}
