/**
 * graph-utils.ts — Graph view mode utilities
 *
 * Provides computation of tag co-occurrence graphs for GraphView tags mode.
 *
 * Requirements: 38.1, 38.2, 38.3, 38.4, 38.5, 38.6
 */

import type { ExtendedSearchIndex } from './extended-indexing'
import type { FileEntry } from './types'

/** All supported graph modes */
export type GraphMode = 'files' | 'tags' | 'blocks'

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

  // Build tag -> files map (already in the index)
  // Count notes per tag
  const tagCounts = new Map<string, number>()
  for (const [, paths] of tagIndex) {
    tagCounts.set(tagIndex.entries().next().value?.[0] ?? '', paths.size)
  }

  // Actually compute counts correctly
  const nodes: TagGraphNode[] = []
  for (const [tag, paths] of tagIndex) {
    const count = paths.size
    const radius = computeTagNodeRadius(count, fileCount)
    nodes.push({ id: tag, label: tag, count, radius })
  }

  // Compute co-occurrence edges
  const edges: TagGraphEdge[] = []
  const tagPairs = new Map<string, Map<string, number>>()

  // For each note, count co-occurring tag pairs
  for (const [tag, paths] of tagIndex) {
    for (const path of paths) {
      if (!tagPairs.has(path)) {
        tagPairs.set(path, new Map())
      }
    }
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
