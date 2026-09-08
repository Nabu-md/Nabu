import { useMemo } from 'react'
import type { SidebarSelection, VaultEntry, ViewFile } from '../types'
import type { AllNotesFileVisibility } from './allNotesFileVisibility'
import { filterEntries as filterVaultEntries, getSortComparator, type SortConfig } from '../utils/noteListHelpers'
import { filterByQuery } from '../utils/changeEntries'

/** Entries grouped by their parent folder path ('' = vault root). */
export type EntriesByFolder = Map<string, VaultEntry[]>

function folderPathOf(entryPath: string): string {
  const normalized = entryPath.replaceAll('\\\\', '/')
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

/** Sort direction: title ascending, dates descending, matching NoteList defaults. */
export function sortVaultEntriesForTree(entries: VaultEntry[], sort: SortConfig | null): VaultEntry[] {
  if (!sort) {
    return [...entries].sort((a, b) => (b.modifiedAt ?? b.createdAt ?? 0) - (a.modifiedAt ?? a.createdAt ?? 0))
  }
  return [...entries].sort(getSortComparator(sort.option, sort.direction))
}

/** When the selection is a view/type/folder, the tree keeps those sections expanded. */
export function selectionTargets(selection: SidebarSelection): string[] {
  switch (selection.kind) {
    case 'folder':
      return [`folder:${selection.rootPath ?? ''}::${selection.path}`]
    case 'sectionGroup':
      return [`type:${selection.type}`]
    case 'view':
      return [`view:${selection.filename}`]
    default:
      return []
  }
}

export interface TreeExplorerData {
  entriesByFolder: EntriesByFolder
  sortedEntries: VaultEntry[]
}

/** Build memoized inputs for the explorer tree. */
export function useTreeExplorerData(
  entries: VaultEntry[],
  selection: SidebarSelection,
  sort: SortConfig | null,
  search: string,
  views: ViewFile[] | undefined,
  allNotesFileVisibility: AllNotesFileVisibility | undefined,
): TreeExplorerData {
  return useMemo(() => {
    let scoped: VaultEntry[] = entries
    // Type/view selections scope the tree to the matching entries so their
    // contents render inline under the section headers.
    if (selection.kind === 'view') {
      scoped = filterVaultEntries(entries, selection, { views, allNotesFileVisibility })
    }
    const sorted = sortVaultEntriesForTree(scoped, sort)
    return {
      entriesByFolder: indexEntriesByFolder(sorted),
      sortedEntries: filterByQuery(sorted, search.trim().toLowerCase()),
    }
  }, [allNotesFileVisibility, entries, search, selection, sort, views])
}

export const TREE_FOLDER_KEY_PREFIX = {
  folder: 'folder:',
  type: 'type:',
  view: 'view:',
} as const

export function folderKey(rootPath: string | undefined, path: string): string {
  return `${TREE_FOLDER_KEY_PREFIX.folder}${rootPath ?? ''}::${path}`
}
