/**
 * state.ts
 *
 * StateManager — orchestrates vault operations, manages the AST store,
 * and coordinates the Pending_Write_Lock used to distinguish app-initiated
 * writes from external edits detected by the file-system watcher.
 *
 * Requirements: 1.2, 1.3, 2.7, 5.3, 5.4, 5.5, 5.8
 */

import fs from 'fs/promises'
import path from 'path'
import type { Root } from 'mdast'
import { visit } from 'unist-util-visit'

import { parseFile } from './parser'
import { buildGraph } from '@shared/graph'
import {
  buildFullTextIndex,
  buildTagIndex,
  removeFileFromFullTextIndex,
  removeFileFromTagIndex
} from '@shared/indexing'
import {
  buildExtendedIndex,
  updateExtendedIndexForFile,
  createEmptyIndex
} from '@shared/extended-indexing'
import type { ExtendedSearchIndex } from '@shared/extended-indexing'
import type { VaultMetadata, FileEntry, Edge } from '@shared/types'

// ---------------------------------------------------------------------------
// StateManager
// ---------------------------------------------------------------------------

export class StateManager {
  /** AST cache keyed by absolute file path */
  private astStore: Map<string, Root> = new Map()

  /**
   * Pending write lock: set before an app-initiated disk write so the watcher
   * can skip the resulting change event rather than triggering a spurious re-parse.
   */
  private pendingWrites: Map<string, { timeout: NodeJS.Timeout }> = new Map()

  /** Currently open vault, or null if no vault is open */
  private currentVault: VaultMetadata | null = null

  /**
   * In-memory inverted full-text index: word → Set of file paths.
   * Mutated incrementally by `updateIndexesForFile()`.
   */
  private fullTextIndex: Map<string, Set<string>> = new Map()

  /**
   * In-memory tag index: tag → Set of file paths.
   * Mutated incrementally by `updateIndexesForFile()`.
   */
  private tagIndex: Map<string, Set<string>> = new Map()

  /**
   * Extended search index: token positions, line snippets, unified tag index,
   * alias map, property index, and block references.
   * Mutated incrementally by `updateIndexesForFile()`.
   */
  private extendedIndex: ExtendedSearchIndex = createEmptyIndex()

  // -------------------------------------------------------------------------
  // Vault operations
  // -------------------------------------------------------------------------

  /**
   * Open a vault at `vaultPath`.
   *
   * - Scans for all `.md` files using `fs.readdir` recursive (Node 20+)
   * - Excludes `.nabu/` and any dot-prefixed path segments
   * - Sorts: folders before files, alphabetical (case-insensitive) within groups
   * - Creates `.nabu/` directory if it doesn't exist
   * - Appends `.nabu/` to `.gitignore` if the file exists but doesn't already
   *   contain the entry (prevents git pollution in tracked vaults)
   *
   * Requirements: 1.2, 1.3
   */
  async openVault(vaultPath: string): Promise<VaultMetadata> {
    const files = await this.scanVault(vaultPath)

    // Ensure .nabu/ cache directory exists
    const nabuDir = path.join(vaultPath, '.nabu')
    await fs.mkdir(nabuDir, { recursive: true })

    // Append .nabu/ to .gitignore if the file exists and lacks the entry
    const gitignorePath = path.join(vaultPath, '.gitignore')
    try {
      const gitignoreContent = await fs.readFile(gitignorePath, 'utf-8')
      // Match .nabu/ as a standalone line (with or without trailing newline)
      if (!/(^|\n)\.nabu\/(\n|$)/.test(gitignoreContent)) {
        const suffix = gitignoreContent.endsWith('\n') ? '.nabu/\n' : '\n.nabu/\n'
        await fs.appendFile(gitignorePath, suffix, 'utf-8')
      }
    } catch (err) {
      // .gitignore doesn't exist — nothing to update
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
        throw err
      }
    }

    this.currentVault = { path: vaultPath, files }
    return this.currentVault
  }

  /**
   * Scan a vault directory for `.md` files.
   *
   * Uses `fs.readdir` with `{ recursive: true, withFileTypes: true }` for a
   * single syscall (Node 20+), which keeps performance well under the 1-second
   * target for vaults with 10K files.
   */
  private async scanVault(vaultPath: string): Promise<FileEntry[]> {
    const dirents = await fs.readdir(vaultPath, {
      recursive: true,
      withFileTypes: true
    })

    const mdFiles: FileEntry[] = []

    for (const dirent of dirents) {
      if (!dirent.isFile()) continue
      if (!dirent.name.endsWith('.md')) continue

      // Build the absolute path. In Node 20 recursive readdir, `dirent.path`
      // (or `dirent.parentPath` in newer patch releases) holds the directory.
      const parentPath: string =
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (dirent as any).parentPath ?? (dirent as any).path ?? vaultPath

      const absolutePath = path.join(parentPath, dirent.name)

      // Derive the relative path to check for excluded segments
      const relativePath = path.relative(vaultPath, absolutePath)
      const segments = relativePath.split(path.sep)

      // Exclude if any segment starts with '.' (covers .nabu/, .git/, etc.)
      if (segments.some((seg) => seg.startsWith('.'))) continue

      let mtime = 0
      try {
        const stat = await fs.stat(absolutePath)
        mtime = stat.mtimeMs
      } catch {
        // File disappeared between readdir and stat — skip it
        continue
      }

      mdFiles.push({
        path: absolutePath,
        name: path.basename(dirent.name, '.md'),
        mtime
      })
    }

    return sortFileEntries(mdFiles, vaultPath)
  }

  // -------------------------------------------------------------------------
  // AST store
  // -------------------------------------------------------------------------

  /**
   * Return the cached AST for `filePath`, parsing the file on first access
   * (lazy initialisation of the AST_Store).
   *
   * Requirements: 2.7
   */
  async getAST(filePath: string): Promise<Root> {
    const cached = this.astStore.get(filePath)
    if (cached) return cached

    const { ast } = await parseFile(filePath)
    this.astStore.set(filePath, ast)
    return ast
  }

  /**
   * Invalidate the cached AST for `filePath` so the next `getAST()` call
   * re-parses the file from disk. Used by the watcher for external edits.
   */
  invalidateAST(filePath: string): void {
    this.astStore.delete(filePath)
  }

  /**
   * Synchronously return the cached AST for `filePath`, or `undefined` if it
   * has not been parsed yet.
   *
   * Requirements: 6.3
   */
  getASTSync(filePath: string): Root | undefined {
    return this.astStore.get(filePath) ?? undefined
  }

  // -------------------------------------------------------------------------
  // Index building
  // -------------------------------------------------------------------------

  /**
   * Build the full-text index, tag index, knowledge-graph edges, and extended
   * search index for the currently open vault.
   *
   * - Uses the synchronous AST accessor so no additional I/O is performed for
   *   already-cached files.
   * - Stores the built indexes in the `fullTextIndex`, `tagIndex`, and
   *   `extendedIndex` instance fields so that `updateIndexesForFile()` can
   *   perform incremental updates.
   * - Populates `edge.snippet` by finding the first `paragraph` node in the
   *   source AST, gathering all nested `text` node values, joining them, and
   *   truncating to 80 characters.
   * - Serialises the `Map<string, Set<string>>` indexes to plain
   *   `Record<string, string[]>` objects for IPC transport.
   *
   * Requirements: 2.6, 2.8, 6.3, 7.6, 8.6
   */
  async buildIndexes(): Promise<{
    ftIndex: Record<string, string[]>
    tagIndex: Record<string, string[]>
    edges: Edge[]
    extendedIndex: {
      positions: Record<string, Record<string, number[]>>
      lineSnippets: Record<string, string[]>
      tagIndex: Record<string, string[]>
      aliasIndex: Record<string, string[]>
      propertyIndex: Record<string, Record<string, string[]>>
      blockRefs: Record<string, Record<string, string>>
    }
  }> {
    const files = this.currentVault?.files ?? []
    const getAST = (p: string): Root | undefined => this.astStore.get(p)

    // Build extended index first so aliasIndex is available for graph (Req 15.2)
    this.extendedIndex = buildExtendedIndex(files, getAST)

    // Build graph edges with alias resolution
    const edges = buildGraph(files, getAST, this.extendedIndex.aliasIndex)

    // Store built indexes in instance fields for incremental updates later
    this.fullTextIndex = buildFullTextIndex(files, getAST)
    this.tagIndex = buildTagIndex(files, getAST)

    // Populate snippet for each edge
    for (const edge of edges) {
      const sourceAST = getAST(edge.source)
      if (sourceAST === undefined) continue

      // Find the first paragraph node and collect all text node values within it
      let snippet = ''
      visit(sourceAST, 'paragraph', (paraNode) => {
        if (snippet !== '') return // already found one
        const parts: string[] = []
        visit(paraNode, 'text', (textNode: { value: string }) => {
          parts.push(textNode.value)
        })
        snippet = parts.join('').slice(0, 80)
      })

      edge.snippet = snippet
    }

    // Serialise Maps → Records for IPC transport
    const ftIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.fullTextIndex) ftIndexObj[k] = Array.from(v)

    const tagIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.tagIndex) tagIndexObj[k] = Array.from(v)

    // Serialise extended index Maps → Records for IPC transport
    const extendedIndexObj = serializeExtendedIndex(this.extendedIndex)

    return { ftIndex: ftIndexObj, tagIndex: tagIndexObj, edges, extendedIndex: extendedIndexObj }
  }

  /**
   * Incrementally update the full-text index, tag index, graph edges, and
   * extended search index after a single file changes (e.g. `note:save`).
   *
   * Avoids re-tokenising all vault files on every keystroke by:
   *   1. Invalidating and re-fetching the AST for `filePath` only.
   *   2. Removing all index entries for `filePath` from the stored in-memory
   *      `fullTextIndex` and `tagIndex` (iterates all keys, removes `filePath`
   *      from the Set, prunes empty Sets).
   *   3. Re-indexing the single file using the same tokenisation / tag-parsing
   *      logic as `buildFullTextIndex` / `buildTagIndex` (by calling both with
   *      a single-element file list).
   *   4. Incrementally updating the extended search index via
   *      `updateExtendedIndexForFile`.
   *   5. Rebuilding the edges for `filePath` with refreshed snippets.
   *   6. Serialising and returning the updated indexes in the same shape as
   *      `buildIndexes()`.
   *
   * Requirements: 2.6, 2.8, 6.3, 7.6, 8.6
   */
  async updateIndexesForFile(filePath: string): Promise<{
    ftIndex: Record<string, string[]>
    tagIndex: Record<string, string[]>
    edges: Edge[]
    extendedIndex: {
      positions: Record<string, Record<string, number[]>>
      lineSnippets: Record<string, string[]>
      tagIndex: Record<string, string[]>
      aliasIndex: Record<string, string[]>
      propertyIndex: Record<string, Record<string, string[]>>
      blockRefs: Record<string, Record<string, string>>
    }
  }> {
    // 1. Invalidate stale AST and re-parse from disk
    this.invalidateAST(filePath)
    await this.getAST(filePath)

    // 2. Remove all index entries for filePath from stored Maps
    for (const [word, paths] of this.fullTextIndex) {
      paths.delete(filePath)
      if (paths.size === 0) this.fullTextIndex.delete(word)
    }
    for (const [tag, paths] of this.tagIndex) {
      paths.delete(filePath)
      if (paths.size === 0) this.tagIndex.delete(tag)
    }

    // 3. Re-index only the changed file — merge results into the stored Maps
    const singleFile = this.currentVault?.files.filter((f) => f.path === filePath) ?? []
    const getAST = (p: string): Root | undefined => this.astStore.get(p)

    const newFtEntries = buildFullTextIndex(singleFile, getAST)
    for (const [word, paths] of newFtEntries) {
      const existing = this.fullTextIndex.get(word)
      if (existing) {
        for (const p of paths) existing.add(p)
      } else {
        this.fullTextIndex.set(word, new Set(paths))
      }
    }

    const newTagEntries = buildTagIndex(singleFile, getAST)
    for (const [tag, paths] of newTagEntries) {
      const existing = this.tagIndex.get(tag)
      if (existing) {
        for (const p of paths) existing.add(p)
      } else {
        this.tagIndex.set(tag, new Set(paths))
      }
    }

    // 4. Incrementally update the extended search index (Req 2.6)
    updateExtendedIndexForFile(this.extendedIndex, filePath, getAST(filePath))

    // 5. Rebuild the complete edge list with alias resolution (Req 15.2)
    //    and refresh snippets for edges whose source is the changed file
    const allFiles = this.currentVault?.files ?? []
    const edges = buildGraph(allFiles, getAST, this.extendedIndex.aliasIndex)

    for (const edge of edges) {
      const sourceAST = getAST(edge.source)
      if (sourceAST === undefined) continue

      // Only refresh snippets for edges originating from the changed file;
      // all other edges retain their previously computed snippets.
      if (edge.source !== filePath) continue

      let snippet = ''
      visit(sourceAST, 'paragraph', (paraNode) => {
        if (snippet !== '') return
        const parts: string[] = []
        visit(paraNode, 'text', (textNode: { value: string }) => {
          parts.push(textNode.value)
        })
        snippet = parts.join('').slice(0, 80)
      })
      edge.snippet = snippet
    }

    // 6. Serialise Maps → Records for IPC transport
    const ftIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.fullTextIndex) ftIndexObj[k] = Array.from(v)

    const tagIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.tagIndex) tagIndexObj[k] = Array.from(v)

    const extendedIndexObj = serializeExtendedIndex(this.extendedIndex)

    return { ftIndex: ftIndexObj, tagIndex: tagIndexObj, edges, extendedIndex: extendedIndexObj }
  }

  // -------------------------------------------------------------------------
  // Task toggle
  // -------------------------------------------------------------------------

  /**
   * Toggle a task checkbox at `lineIndex` in `filePath`.
   *
   * Sets the Pending_Write_Lock before writing so the chokidar watcher can
   * recognise the resulting `change` event as app-initiated and skip re-parse.
   *
   * Requirements: 5.3, 5.4, 5.5
   */
  async toggleTask(filePath: string, lineIndex: number): Promise<void> {
    this.setPendingWrite(filePath)

    const content = await fs.readFile(filePath, 'utf-8')
    const lines = content.split('\n')

    if (lineIndex < 0 || lineIndex >= lines.length) {
      // Release the lock immediately since we're not writing
      this.clearPendingWrite(filePath)
      throw new Error(`Invalid line index: ${lineIndex}`)
    }

    const line = lines[lineIndex]
    // Toggle [ ] → [x] or [x] → [ ] (case-insensitive for [X] variants)
    const toggled = line.replace(/- \[ \]/, '- [x]').replace(/- \[x\]/i, '- [ ]')
    lines[lineIndex] = toggled

    await fs.writeFile(filePath, lines.join('\n'), 'utf-8')

    // Invalidate the cached AST so the next getAST() call re-parses the file
    this.astStore.delete(filePath)

    // The watcher will call clearPendingWrite() when it receives the change event.
    // The 2s timeout in setPendingWrite() acts as a safety net if the event is missed.
  }

  // -------------------------------------------------------------------------
  // Pending_Write_Lock
  // -------------------------------------------------------------------------

  /**
   * Returns true if an app-initiated write is in progress for `filePath`.
   *
   * Requirements: 5.8
   */
  hasPendingWrite(filePath: string): boolean {
    return this.pendingWrites.has(filePath)
  }

  /**
   * Acquire the Pending_Write_Lock for `filePath`.
   *
   * The lock auto-clears after 2 seconds as a safety net for missed watcher
   * events (e.g., the watcher was stopped before the change event fired).
   *
   * Requirements: 5.5, 5.8
   */
  setPendingWrite(filePath: string): void {
    // Cancel any existing timeout before overwriting the entry
    const existing = this.pendingWrites.get(filePath)
    if (existing) {
      clearTimeout(existing.timeout)
    }

    const timeout = setTimeout(() => {
      this.pendingWrites.delete(filePath)
      console.warn(
        `[StateManager] Pending write lock expired for "${filePath}" — ` +
          'watcher event may have been missed.'
      )
    }, 2000)

    this.pendingWrites.set(filePath, { timeout })
  }

  /**
   * Release the Pending_Write_Lock for `filePath` and cancel the safety-net
   * timeout.
   *
   * Requirements: 5.5, 5.8
   */
  clearPendingWrite(filePath: string): void {
    const entry = this.pendingWrites.get(filePath)
    if (entry) {
      clearTimeout(entry.timeout)
      this.pendingWrites.delete(filePath)
    }
  }

  // -------------------------------------------------------------------------
  // Accessors
  // -------------------------------------------------------------------------

  /** Return the currently open vault, or null. */
  getCurrentVault(): VaultMetadata | null {
    return this.currentVault
  }

  /** Return the extended search index for the current vault. */
  getExtendedIndex(): ExtendedSearchIndex {
    return this.extendedIndex
  }

  /**
   * Serialize the current in-memory indexes to plain objects for IPC transport
   * without rebuilding. Used after incremental updates (delete, rename).
   */
  getSerializedIndexes(): {
    ftIndex: Record<string, string[]>
    tagIndex: Record<string, string[]>
    extendedIndex: {
      positions: Record<string, Record<string, number[]>>
      lineSnippets: Record<string, string[]>
      tagIndex: Record<string, string[]>
      aliasIndex: Record<string, string[]>
      propertyIndex: Record<string, Record<string, string[]>>
      blockRefs: Record<string, Record<string, string>>
    }
  } {
    const ftIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.fullTextIndex) ftIndexObj[k] = Array.from(v)

    const tagIndexObj: Record<string, string[]> = {}
    for (const [k, v] of this.tagIndex) tagIndexObj[k] = Array.from(v)

    const extendedIndexObj = serializeExtendedIndex(this.extendedIndex)

    return { ftIndex: ftIndexObj, tagIndex: tagIndexObj, extendedIndex: extendedIndexObj }
  }

  // -------------------------------------------------------------------------
  // Incremental index removal (for delete/rename operations)
  // -------------------------------------------------------------------------

  /**
   * Remove all in-memory index entries for `filePath` without re-parsing.
   * Used by delete and rename operations where the file is no longer needed
   * in the index (or will be re-indexed under a new path).
   */
  removeFileFromIndexes(filePath: string): void {
    removeFileFromFullTextIndex(this.fullTextIndex, filePath)
    removeFileFromTagIndex(this.tagIndex, filePath)
    updateExtendedIndexForFile(this.extendedIndex, filePath, undefined)
  }

  /**
   * Update the vault file list to reflect a rename: replace `oldPath` with
   * `newPath` in `currentVault.files`.
   */
  renameFileInVault(oldPath: string, newPath: string): void {
    if (!this.currentVault) return
    const file = this.currentVault.files.find((f) => f.path === oldPath)
    if (file) {
      file.path = newPath
      file.name = path.basename(newPath, '.md')
    }
  }

  /**
   * Perform a full rename operation on the indexes:
   * 1. Remove all entries for the old path
   * 2. Update the vault file list
   * 3. Re-index the file under its new path
   *
   * Returns the updated index data for IPC transport.
   */
  async renameFile(oldPath: string, newPath: string): Promise<{
    ftIndex: Record<string, string[]>
    tagIndex: Record<string, string[]>
    edges: Edge[]
    extendedIndex: {
      positions: Record<string, Record<string, number[]>>
      lineSnippets: Record<string, string[]>
      tagIndex: Record<string, string[]>
      aliasIndex: Record<string, string[]>
      propertyIndex: Record<string, Record<string, string[]>>
      blockRefs: Record<string, Record<string, string>>
    }
  }> {
    // 1. Remove old path from all indexes
    this.removeFileFromIndexes(oldPath)

    // 2. Update vault file list
    this.renameFileInVault(oldPath, newPath)

    // 3. Re-index under new path
    return this.updateIndexesForFile(newPath)
  }
}

// ---------------------------------------------------------------------------
// Extended index serialisation helpers
// ---------------------------------------------------------------------------

/**
 * Serialise an ExtendedSearchIndex (which uses Maps and Sets) into plain
 * JSON-safe objects for IPC transport to the renderer.
 */
function serializeExtendedIndex(index: ExtendedSearchIndex): {
  positions: Record<string, Record<string, number[]>>
  lineSnippets: Record<string, string[]>
  tagIndex: Record<string, string[]>
  aliasIndex: Record<string, string[]>
  propertyIndex: Record<string, Record<string, string[]>>
  blockRefs: Record<string, Record<string, string>>
} {
  const positions: Record<string, Record<string, number[]>> = {}
  for (const [word, fileMap] of index.positions) {
    const obj: Record<string, number[]> = {}
    for (const [filePath, lines] of fileMap) obj[filePath] = lines
    positions[word] = obj
  }

  const lineSnippets: Record<string, string[]> = {}
  for (const [filePath, snippets] of index.lineSnippets) lineSnippets[filePath] = snippets

  const tagIndex: Record<string, string[]> = {}
  for (const [tag, paths] of index.tagIndex) tagIndex[tag] = Array.from(paths)

  const aliasIndex: Record<string, string[]> = {}
  for (const [alias, paths] of index.aliasIndex) aliasIndex[alias] = paths

  const propertyIndex: Record<string, Record<string, string[]>> = {}
  for (const [propName, valueMap] of index.propertyIndex) {
    const obj: Record<string, string[]> = {}
    for (const [value, paths] of valueMap) obj[value] = Array.from(paths)
    propertyIndex[propName] = obj
  }

  const blockRefs: Record<string, Record<string, string>> = {}
  for (const [filePath, refs] of index.blockRefs) {
    const obj: Record<string, string> = {}
    for (const [blockId, nodeKey] of refs) obj[blockId] = nodeKey
    blockRefs[filePath] = obj
  }

  return { positions, lineSnippets, tagIndex, aliasIndex, propertyIndex, blockRefs }
}

// ---------------------------------------------------------------------------
// Sorting helpers
// ---------------------------------------------------------------------------

/**
 * Sort file entries: folders (deeper paths first) before files at the same
 * level, alphabetical (case-insensitive) within groups.
 *
 * More precisely: entries whose relative directory is deeper (has more
 * segments) sort before shallower ones, and within the same directory entries
 * are sorted folders-before-files then alphabetically.
 */
function sortFileEntries(entries: FileEntry[], vaultPath: string): FileEntry[] {
  return entries.slice().sort((a, b) => {
    const relA = path.relative(vaultPath, a.path)
    const relB = path.relative(vaultPath, b.path)

    const partsA = relA.split(path.sep)
    const partsB = relB.split(path.sep)

    // Compare segment by segment
    const len = Math.min(partsA.length, partsB.length)
    for (let i = 0; i < len - 1; i++) {
      const cmp = partsA[i].toLowerCase().localeCompare(partsB[i].toLowerCase())
      if (cmp !== 0) return cmp
    }

    // One is in a subdirectory of the other — deeper (folder) sorts first
    if (partsA.length !== partsB.length) {
      return partsB.length - partsA.length // more segments → earlier
    }

    // Same directory: alphabetical by filename (case-insensitive)
    return partsA[partsA.length - 1]
      .toLowerCase()
      .localeCompare(partsB[partsB.length - 1].toLowerCase())
  })
}

// ---------------------------------------------------------------------------
// Singleton export
// ---------------------------------------------------------------------------


