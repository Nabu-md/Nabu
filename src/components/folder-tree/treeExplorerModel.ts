import { useMemo } from 'react'
import type { FolderNode, VaultEntry } from '../../types'
import { getSortComparator, type SortConfig } from '../../utils/noteListHelpers'

/** Entries grouped by their parent folder path ('' = vault root). */
export type EntriesByFolder = Map<string, VaultEntry[]>

function normalizeRoot(root: string | undefined): string | undefined {
  const trimmed = root?.trim().replace(/[\\/]+$/g, '')
  return trimmed ? trimmed : undefined
}

function folderPathOf(entryPath: string): string {
  const normalized = entryPath.replaceAll('\\', '/')
  const lastSlash = normalized.lastIndexOf('/')
  return lastSlash <= 0 ? '' : normalized.slice(0, lastSlash)
}

/**
 * Index entries by parent folder so the tree can look up children per folder.
 * `FolderNode.path` is vault-relative, so entries under the vault root are
 * indexed by their relative folder path; entries outside the root (other
 * mounted workspaces) fall back to their absolute parent folder.
 */
export function indexEntriesByFolder(entries: VaultEntry[], vaultRootPath?: string): EntriesByFolder {
  const root = normalizeRoot(vaultRootPath)
  const byFolder: EntriesByFolder = new Map()
  const add = (key: string, entry: VaultEntry) => {
    const bucket = byFolder.get(key)
    if (bucket) bucket.push(entry)
    else byFolder.set(key, [entry])
  }
  for (const entry of entries) {
    const parent = folderPathOf(entry.path)
    add(parent, entry)
    if (root && parent.startsWith(`${root}/`)) add(parent.slice(root.length + 1), entry)
    else if (root && parent === root) add('', entry)
  }
  return byFolder
}

/**
 * Resolve the entries shown inside a folder node. Tries the vault-relative
 * path first, then the absolute path under the node's (or default) vault root.
 */
export function entriesForFolderNode(
  byFolder: EntriesByFolder,
  node: FolderNode,
  defaultRootPath?: string,
): VaultEntry[] {
  const candidates: string[] = [node.path]
  const root = normalizeRoot(node.rootPath ?? defaultRootPath)
  if (root) candidates.push(node.path === '' ? root : `${root}/${node.path}`)
  for (const candidate of candidates) {
    const bucket = byFolder.get(candidate)
    if (bucket && bucket.length > 0) return bucket
  }
  return []
}

/**
 * Ensure every entry is reachable in the tree: synthesize folder nodes for
 * entry paths whose folders are missing from the backend listing.
 */
export function mergeEntryFolders(folders: FolderNode[], entries: VaultEntry[], vaultRootPath?: string): FolderNode[] {
  const root = normalizeRoot(vaultRootPath)
  if (!root) return folders
  const existing = new Set<string>()
  const collect = (nodes: FolderNode[]) => {
    for (const node of nodes) {
      existing.add(node.path)
      collect(node.children)
    }
  }
  collect(folders)

  const synthesized: FolderNode[] = []
  for (const entry of entries) {
    const parent = folderPathOf(entry.path)
    if (!parent.startsWith(`${root}/`)) continue
    const relative = parent.slice(root.length + 1)
    if (existing.has(relative)) continue
    let level = synthesized
    let acc = ''
    for (const part of relative.split('/')) {
      acc = acc ? `${acc}/${part}` : part
      let node = level.find((candidate) => candidate.path === acc)
      if (!node) {
        node = { name: part, path: acc, rootPath: root, synthetic: true, children: [] }
        level.push(node)
      }
      level = node.children
    }
  }
  if (synthesized.length === 0) return folders
  synthesized.sort((a, b) => a.name.localeCompare(b.name))
  return [...folders, ...synthesized]
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
  vaultRootPath?: string,
): EntriesByFolder {
  return useMemo(() => {
    const filtered = filterEntriesByTitle(entries, search)
    return indexEntriesByFolder(sortVaultEntriesForTree(filtered, sort), vaultRootPath)
  }, [entries, search, sort, vaultRootPath])
}
