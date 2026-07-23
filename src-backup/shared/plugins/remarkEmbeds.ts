/**
 * remarkEmbeds.ts
 *
 * Remark plugin that transforms `![[target]]` embed syntax into `embed`
 * AST nodes. Must be placed *before* the wikiLink plugin so that
 * `![[target]]` is consumed as an embed and not partially matched as
 * text `!` + wikiLink `[[target]]`.
 *
 * Requirements: 11.1, 11.7
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, Text, Node } from 'mdast'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface EmbedNode extends Node {
  type: 'embed'
  target: string
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Matches `![[target]]` syntax anywhere in text. */
const EMBED_PATTERN = /!\[\[([^\]]+)\]\]/g

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Parse text content for `![[target]]` patterns and return an array of
 * mixed text and embed nodes.
 */
function parseTextForEmbeds(text: string): (Text | EmbedNode)[] {
  const result: (Text | EmbedNode)[] = []
  let lastIndex = 0

  EMBED_PATTERN.lastIndex = 0

  let match: RegExpExecArray | null
  while ((match = EMBED_PATTERN.exec(text)) !== null) {
    const matchStart = match.index
    const matchEnd = EMBED_PATTERN.lastIndex

    // Text before the match
    if (matchStart > lastIndex) {
      result.push({ type: 'text', value: text.slice(lastIndex, matchStart) } as Text)
    }

    // Embed node
    const target = match[1].trim()
    result.push({ type: 'embed', target } as EmbedNode)

    lastIndex = matchEnd
  }

  // Remaining text after the last match
  if (lastIndex < text.length) {
    result.push({ type: 'text', value: text.slice(lastIndex) } as Text)
  }

  return result
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/**
 * Remark plugin that transforms `![[target]]` into `embed` AST nodes.
 */
export const remarkEmbeds: Plugin<[], Root> = function () {
  return (tree: Root) => {
    visit(tree, 'text', (node: Text, index, parent) => {
      if (index === undefined || !parent || !('children' in parent)) {
        return
      }

      const text = node.value

      if (!text.includes('![[')) {
        return
      }

      const newNodes = parseTextForEmbeds(text)

      // Only replace if we actually found embeds
      if (newNodes.length <= 1 && newNodes[0]?.type === 'text') {
        return
      }

      const parentChildren = parent.children as Node[]
      parentChildren.splice(index, 1, ...(newNodes as Node[]))

      return index + newNodes.length
    })
  }
}

export default remarkEmbeds
