import { useMemo } from 'react'
import type { VaultEntry } from '../../types'
import { getSortComparator, type SortConfig } from '../../utils/noteListHelpers'

/** Entries grouped by their parent folder path ('' = vault root). */
export type EntriesByFolder = Map<string, VaultEntry[]>

function folderPathOf(entryPath: string): string {
  const normalized = entryPath.replaceAll('\\', '/')
  const lastSlash = normalized.lastIndexOf('/')
  return lastSlash <= 0 ? '' : normalized.slice(0, lastSlash)
}

/** Index entries by parent folder so the tree can look up children per folder. */
export function indexEntriesByFolder(entries: VaultEntry[]): EntriesByFolder {
  const byFolder: EntriesByFolder = new Map()
  for (const entry of entries) {
    const folder = folderPathOf(entry.path)
    const bucket = byFolder.get(folder)
    if (bucket) bucket.push(entry)
    else byFolder.set(folder, [entry])
  }
  return byFolder
}

/** Sort entries for the tree: null sort falls back to modified-descending. */
export function sortVaultEntriesForTree(entries: VaultEntry[], sort: SortConfig | null): VaultEntry[] {
  if (!sort) {
    return [...entries].sort((a, b) => (b.modifiedAt ?? b.createdAt ?? 0) - (a.modifiedAt ?? a.createdAt ?? 0))
  }
  return [...entries].sort(getSortComparator(sort.option, sort.direction))
}

/** Substring filter over titles; empty query returns everything. */
export function filterEntriesByTitle(entries: VaultEntry[], query: string): VaultEntry[] {
  const needle = query.trim().toLowerCase()
  if (!needle) return entries
  return entries.filter((entry) => entry.title.toLowerCase().includes(needle))
}

/** Memoized explorer inputs: folder-indexed, sorted, search-filtered entries. */
export function useTreeExplorerData(
  entries: VaultEntry[],
  sort: SortConfig | null,
  search: string,
): EntriesByFolder {
  return useMemo(() => {
    const filtered = filterEntriesByTitle(entries, search)
    return indexEntriesByFolder(sortVaultEntriesForTree(filtered, sort))
  }, [entries, search, sort])
}
