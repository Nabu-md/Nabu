/**
 * graph-utils.ts — Graph view mode utilities
 *
 * Provides computation of tag co-occurrence graphs for GraphView tags mode.
 *
 * Requirements: 38.1, 38.2, 38.3, 38.4, 38.5, 38.6
 */

import type { ExtendedSearchIndex } from './extended-indexing'
import type { FileEntry } from './types'

/** Node in the tag co-occurrence graph */
export interface TagGraphNode {
  id: string
  /** The tag name (e.g., "project/nabu") */
  label: string
  /** Number of notes carrying this tag */
  count: number
  /** Radius for rendering (derived from count) */
  radius: number
}

/** Edge in the tag co-occurrence graph */
export interface TagGraphEdge {
  source: string
  target: string
  /** Number of notes where both tags co-occur */
  cooccurrence: number
}

/** Palette colors for tag nodes (same palette as tab groups - Req 38.4) */
export type TagNodeColor =
  'blue' | 'red' | 'green' | 'yellow' | 'purple' | 'orange' | 'cyan' | 'pink'

// Color palette for deterministic tag node coloring (Req 38.4)
const TAG_COLORS: TagNodeColor[] = [
  'blue',
  'red',
  'green',
  'yellow',
  'purple',
  'orange',
  'cyan',
  'pink'
]

/**
 * Compute a tag co-occurrence graph from the extended index.
 *
 * For each tag, creates a node with count = number of notes carrying that tag.
 * For each pair of tags that appear on the same note, creates an edge.
 *
 * Requirements: 38.2, 38.3
 */
export function computeTagGraph(
  index: ExtendedSearchIndex,
  files: FileEntry[]
): { nodes: TagGraphNode[]; edges: TagGraphEdge[] } {
  const tagIndex = index.tagIndex
  const fileCount = files.length

  // Count notes per tag
  const nodes: TagGraphNode[] = []
  for (const [tag, paths] of tagIndex) {
    const count = paths.size
    const radius = computeTagNodeRadius(count, fileCount)
    nodes.push({ id: tag, label: tag, count, radius })
  }

  // Build co-occurrence map: for each file, find all tags it has
  const fileToTags = new Map<string, string[]>()
  for (const [tag, paths] of tagIndex) {
    for (const path of paths) {
      const tags = fileToTags.get(path) ?? []
      if (!fileToTags.has(path)) {
        fileToTags.set(path, tags)
      }
      // Add tag to this file's tag list
      const tagsList = fileToTags.get(path)
      if (tagsList) {
        tagsList.push(tag)
      }
    }
  }

  // For each file, create edges between all tag pairs
  const edges: TagGraphEdge[] = []
  const edgeSet = new Set<string>()
  for (const [, tags] of fileToTags) {
    for (let i = 0; i < tags.length; i++) {
      for (let j = i + 1; j < tags.length; j++) {
        const t1 = tags[i]
        const t2 = tags[j]
        // Create canonical edge key (sorted to avoid duplicates)
        const [a, b] = t1 < t2 ? [t1, t2] : [t2, t1]
        const key = `${a}|${b}`
        if (!edgeSet.has(key)) {
          edgeSet.add(key)
          // Count co-occurrence
          const cooccurrence = countTagCooccurrence(tagIndex, t1, t2)
          edges.push({ source: a, target: b, cooccurrence })
        }
      }
    }
  }

  return { nodes, edges }
}

/**
 * Compute node radius based on note count.
 *
 * Minimum radius: 4, Maximum radius: 20.
 * Radius scales logarithmically with count.
 *
 * Requirements: 38.4
 */
export function computeTagNodeRadius(count: number, maxFiles: number): number {
  if (count <= 0) return 4
  if (maxFiles <= 0) return 8

  // Scale logarithmically: log(count) / log(maxFiles) * (max - min) + min
  const ratio = Math.log(1 + count) / Math.log(1 + maxFiles)
  return 4 + ratio * 16 // 4 to 20
}

/**
 * Get deterministic color for a tag based on its name hash.
 *
 * Uses the same palette as tab groups (Req 38.4).
 *
 * Requirements: 38.4
 */
export function getTagNodeColor(tag: string): TagNodeColor {
  let hash = 0
  for (let i = 0; i < tag.length; i++) {
    const char = tag.charCodeAt(i)
    hash = (hash << 5) - hash + char
    hash |= 0 // Convert to 32-bit integer
  }
  const index = Math.abs(hash) % TAG_COLORS.length
  return TAG_COLORS[index]
}

/**
 * Get display label for a tag (shortened for namespaced tags).
 *
 * For `parent/child/grandchild`, returns `grandchild` with full tag in tooltip.
 *
 * Requirements: 38.4
 */
export function getTagDisplayLabel(tag: string): string {
  const lastSlash = tag.lastIndexOf('/')
  return lastSlash >= 0 ? tag.slice(lastSlash + 1) : tag
}

/**
 * Get the N most recently modified notes for a given tag.
 *
 * Used for the hover tooltip in tag graph view.
 * Returns files sorted by mtime descending, limited to maxNotes.
 *
 * Requirements: 38.4, 38.6
 */
export function getTagRecentNotes(
  tag: string,
  files: FileEntry[],
  tagIndex: Map<string, Set<string>>,
  maxNotes: number = 3
): FileEntry[] {
  const taggedPaths = tagIndex.get(tag)
  if (!taggedPaths) return []

  // Filter files to only those with this tag
  const taggedFiles = files.filter((f) => taggedPaths.has(f.path))

  // Sort by mtime descending (most recent first)
  return taggedFiles.sort((a, b) => (b.mtime ?? 0) - (a.mtime ?? 0)).slice(0, maxNotes)
}

/**
 * Count how many notes have both tags.
 */
function countTagCooccurrence(
  tagIndex: Map<string, Set<string>>,
  tag1: string,
  tag2: string
): number {
  const paths1 = tagIndex.get(tag1)
  const paths2 = tagIndex.get(tag2)
  if (!paths1 || !paths2) return 0

  let count = 0
  for (const path of paths1) {
    if (paths2.has(path)) count++
  }
  return count
}

// ---------------------------------------------------------------------------
// Block reference graph (Req 38.6)
// ---------------------------------------------------------------------------

/**
 * Regex for extracting block-reference links of the form `[[Note Name#^blockId]]`
 * (and the embed variant `![[Note Name#^blockId]]`). Captures the target note
 * name (group 1) and the block id (group 2).
 */
export const BLOCK_REF_LINK_RE = /!?\[\[([^\]]+?)#\^([\w-]+)\]\]/g

/** A node in the block reference graph. */
export interface BlockGraphNode {
  /** Stable id: for notes it is the file path; for blocks it is `path#^blockId`. */
  id: string
  /** Display label (note name, or the block id for block nodes). */
  label: string
  /** `true` for block nodes, `false` for note nodes. */
  isBlock: boolean
  /** Owning note path (equal to `id` when this is a note node). */
  ownerPath: string
  /** Block id (only for block nodes). */
  blockId?: string
  /** Line number where the block is defined (1-based, only for block nodes). */
  line?: number
}

/** An edge in the block reference graph. */
export interface BlockGraphEdge {
  source: string
  target: string
}

/**
 * Compute the block reference graph from the extended index.
 *
 * The extended index records block *definitions* (`blockRefs`:
 * `filePath → blockId → "L{line}"`). This function turns those definitions
 * into block nodes and connects each block to its owning note.
 *
 * Cross-note *references* (notes that link to `[[note#^id]]`) are supplied by
 * the caller via `blockRefLinks` (source note path → list of target
 * `{ note, blockId }` pairs) which the renderer derives by scanning raw note
 * content through the existing `note:get-raw` IPC. This keeps the computation
 * pure and free of I/O while reusing the already-available index data.
 *
 * Requirements: 38.6
 */
export function computeBlockGraph(
  blockRefs: Record<string, Record<string, string>>,
  blockRefLinks: Array<{ source: string; targetNote: string; blockId: string }>
): { nodes: BlockGraphNode[]; edges: BlockGraphEdge[] } {
  const nodeMap = new Map<string, BlockGraphNode>()
  const edges: BlockGraphEdge[] = []
  const edgeSet = new Set<string>()

  const ensureNoteNode = (filePath: string): BlockGraphNode => {
    let node = nodeMap.get(filePath)
    if (!node) {
      node = {
        id: filePath,
        label: filePath.split('/').pop() ?? filePath,
        isBlock: false,
        ownerPath: filePath
      }
      nodeMap.set(filePath, node)
    }
    return node
  }

  // 1. Block definition nodes + note→block hierarchy edges
  for (const [filePath, refs] of Object.entries(blockRefs)) {
    const noteNode = ensureNoteNode(filePath)
    for (const [blockId, nodeKey] of Object.entries(refs)) {
      const line = parseLineFromNodeKey(nodeKey)
      const blockNodeId = `${filePath}#^${blockId}`
      nodeMap.set(blockNodeId, {
        id: blockNodeId,
        label: blockId,
        isBlock: true,
        ownerPath: filePath,
        blockId,
        line
      })
      const key = `${noteNode.id}|${blockNodeId}`
      if (!edgeSet.has(key)) {
        edgeSet.add(key)
        edges.push({ source: noteNode.id, target: blockNodeId })
      }
    }
  }

  // 2. Cross-note block reference edges
  for (const link of blockRefLinks) {
    const sourceNote = ensureNoteNode(link.source)
    const targetBlockId = `${link.targetNote}#^${link.blockId}`
    // Only draw a reference edge if the target block is actually defined somewhere
    if (!nodeMap.has(targetBlockId)) continue
    const key = `${sourceNote.id}|${targetBlockId}`
    if (!edgeSet.has(key)) {
      edgeSet.add(key)
      edges.push({ source: sourceNote.id, target: targetBlockId })
    }
  }

  return { nodes: Array.from(nodeMap.values()), edges }
}

/**
 * Parse the 1-based line number out of a node position key of the form `L{line}`.
 * Returns `undefined` when the key is not in the expected format.
 */
function parseLineFromNodeKey(nodeKey: string): number | undefined {
  const match = nodeKey.match(/^L(\d+)$/)
  return match ? Number(match[1]) : undefined
}

/**
 * Extract block-reference links (`[[Note#^id]]`) from raw markdown content.
 *
 * Returns a list of `{ targetNote, blockId }` pairs found in `content`. The
 * caller is responsible for associating these with the source note path.
 *
 * Requirements: 38.6
 */
export function extractBlockRefLinks(content: string): Array<{
  targetNote: string
  blockId: string
}> {
  const links: Array<{ targetNote: string; blockId: string }> = []
  const seen = new Set<string>()
  for (const match of content.matchAll(BLOCK_REF_LINK_RE)) {
    const targetNote = match[1].trim()
    const blockId = match[2]
    const key = `${targetNote}#^${blockId}`
    if (seen.has(key)) continue
    seen.add(key)
    links.push({ targetNote, blockId })
  }
  return links
}
