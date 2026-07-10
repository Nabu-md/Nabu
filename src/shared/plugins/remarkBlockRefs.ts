/**
 * remarkBlockRefs.ts
 *
 * Remark plugin that supports Obsidian-style block references.
 *
 * Two transformations:
 *   1. Trailing `^identifier` on block-level nodes (paragraph, heading,
 *      listItem, blockquote) is extracted and stored as `data.blockId`.
 *      The `^identifier` is removed from the text content.
 *   2. `[[target#^id]]` wiki-link targets are split so the wikiLink node
 *      receives `target` and `blockRef` fields separately.
 *
 * Must be placed *after* the wikiLink plugin in the pipeline so that
 * [[target#^id]] has already been turned into a wikiLink node.
 *
 * Requirements: 20.1, 20.2, 20.5, 20.6
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, Node, Text } from 'mdast'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Matches a trailing `^identifier` on a text value. */
const TRAILING_ID_RE = /\s*\^([\w-]+)$/

/** Checks whether a node type is a block that can carry a block ID. */
const BLOCK_TYPES = new Set([
  'paragraph',
  'heading',
  'listItem',
  'blockquote',
  'code',
  'table',
  'thematicBreak'
])

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

export const remarkBlockRefs: Plugin<[], Root> = function () {
  return (tree: Root) => {
    // ── Pass 1: Block IDs on block-level nodes ──────────────────────────
    visit(tree, Array.from(BLOCK_TYPES), (node: Node) => {
      if (!('children' in node) || !Array.isArray(node.children)) {
        return
      }

      // Look for a trailing ^id in the last text child
      const children = node.children as Node[]
      const lastChild = children[children.length - 1]
      if (!lastChild || lastChild.type !== 'text') return

      const textNode = lastChild as Text & { value: string }
      const textValue = textNode.value
      const match = textValue.match(TRAILING_ID_RE)
      if (!match) return

      // Strip the `^id` from the text
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ;(lastChild as any).value = textValue.slice(0, match.index)

      // Store the block id on the parent node
      const data = (node.data as Record<string, unknown>) ?? {}
      data.blockId = match[1]
      node.data = data
    })

    // ── Pass 2: Block refs in wikiLink targets ──────────────────────────
    visit(tree, 'wikiLink', (node: Node) => {
      const linkNode = node as { target?: string; blockRef?: string }
      if (!linkNode.target) return

      // Allow both `target#^id` and `target#^id-with-hyphens`
      const hashIdx = linkNode.target.lastIndexOf('#^')
      if (hashIdx === -1) return

      linkNode.blockRef = linkNode.target.slice(hashIdx + 2)
      linkNode.target = linkNode.target.slice(0, hashIdx)
    })
  }
}

export default remarkBlockRefs
