/**
 * bookmarks.ts
 *
 * Per-vault bookmarks persistence. Bookmarks are stored in `.nabu/bookmarks.json`
 * as a collection of named bookmark lists, where each list contains file paths.
 *
 * Requirements: 18.4, 18.5, 18.6
 */

import path from 'path'
import fs from 'fs/promises'

/**
 * Structure of bookmarks file: { [listName: string]: string[] }
 * e.g., { "Reading List": ["/vault/note1.md", "/vault/note2.md"], "Ideas": [...] }
 */
export type BookmarksCollection = Record<string, string[]>

/**
 * Get the path to the bookmarks file for a given vault.
 */
export function bookmarksFilePath(vaultPath: string): string {
  return path.join(vaultPath, '.nabu', 'bookmarks.json')
}

/**
 * Read the bookmarks collection for a vault. Returns an empty object if the file
 * doesn't exist or can't be parsed.
 */
export async function readBookmarks(vaultPath: string): Promise<BookmarksCollection> {
  try {
    const raw = await fs.readFile(bookmarksFilePath(vaultPath), 'utf-8')
    const parsed = JSON.parse(raw)
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      return parsed as BookmarksCollection
    }
    return {}
  } catch {
    return {}
  }
}

/**
 * Write the bookmarks collection for a vault. Creates the `.nabu/` directory if
 * it doesn't exist.
 */
export async function writeBookmarks(
  vaultPath: string,
  collection: BookmarksCollection
): Promise<void> {
  const dir = path.join(vaultPath, '.nabu')
  await fs.mkdir(dir, { recursive: true })
  await fs.writeFile(bookmarksFilePath(vaultPath), JSON.stringify(collection, null, 2), 'utf-8')
}

/**
 * Add a file to a bookmark list. Creates the list if it doesn't exist.
 */
export async function addBookmark(
  vaultPath: string,
  listName: string,
  filePath: string
): Promise<BookmarksCollection> {
  const bookmarks = await readBookmarks(vaultPath)
  const list = bookmarks[listName] ?? []
  if (!list.includes(filePath)) {
    list.push(filePath)
  }
  bookmarks[listName] = list
  await writeBookmarks(vaultPath, bookmarks)
  return bookmarks
}

/**
 * Remove a file from a bookmark list. Deletes the list if it becomes empty.
 */
export async function removeBookmark(
  vaultPath: string,
  listName: string,
  filePath: string
): Promise<BookmarksCollection> {
  const bookmarks = await readBookmarks(vaultPath)
  const list = bookmarks[listName]
  if (list) {
    const index = list.indexOf(filePath)
    if (index >= 0) {
      list.splice(index, 1)
    }
    if (list.length === 0) {
      delete bookmarks[listName]
    }
    await writeBookmarks(vaultPath, bookmarks)
  }
  return bookmarks
}

/**
 * Remove a file from all bookmark lists (used when a note is deleted/renamed).
 */
export async function removeFileFromBookmarks(
  vaultPath: string,
  filePath: string
): Promise<BookmarksCollection> {
  const bookmarks = await readBookmarks(vaultPath)
  let changed = false
  for (const listName of Object.keys(bookmarks)) {
    const list = bookmarks[listName]
    const index = list.indexOf(filePath)
    if (index >= 0) {
      list.splice(index, 1)
      changed = true
    }
    if (list.length === 0) {
      delete bookmarks[listName]
      changed = true
    }
  }
  if (changed) {
    await writeBookmarks(vaultPath, bookmarks)
  }
  return bookmarks
}

/**
 * Rename a file path in all bookmark lists (used when a note is renamed).
 */
export async function renameFileInBookmarks(
  vaultPath: string,
  oldPath: string,
  newPath: string
): Promise<BookmarksCollection> {
  const bookmarks = await readBookmarks(vaultPath)
  let changed = false
  for (const listName of Object.keys(bookmarks)) {
    const list = bookmarks[listName]
    const index = list.indexOf(oldPath)
    if (index >= 0) {
      list[index] = newPath
      changed = true
    }
  }
  if (changed) {
    await writeBookmarks(vaultPath, bookmarks)
  }
  return bookmarks
}
