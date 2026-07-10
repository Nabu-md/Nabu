/**
 * remarkWikiLinks.ts
 *
 * Remark plugin that transforms [[Page Name]] wiki link syntax into wikiLink nodes.
 * Validates Requirement 2.6: Transform [[Page Name]] patterns in text nodes into
 * wikiLink AST nodes with target property.
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, Text, Node } from 'mdast'

interface WikiLink extends Node {
  type: 'wikiLink'
  target: string
  resolved: boolean
}

/**
 * Regular expression to match [[Page Name]] syntax
 */
const WIKI_LINK_PATTERN = /\[\[([^\]]+)\]\]/g

/**
 * Parses text content and extracts wiki links, returning an array of mixed text and wikiLink nodes
 */
function parseTextForWikiLinks(text: string): (Text | WikiLink)[] {
  const result: (Text | WikiLink)[] = []
  let lastIndex = 0

  // Reset the regex lastIndex to ensure we start from the beginning
  WIKI_LINK_PATTERN.lastIndex = 0

  let match: RegExpExecArray | null
  while ((match = WIKI_LINK_PATTERN.exec(text)) !== null) {
    const matchStart = match.index
    const matchEnd = WIKI_LINK_PATTERN.lastIndex

    // Add text before the match
    if (matchStart > lastIndex) {
      const textBefore = text.slice(lastIndex, matchStart)
      result.push({
        type: 'text',
        value: textBefore
      } as Text)
    }

    // Add the wiki link node
    const target = match[1].trim()
    result.push({
      type: 'wikiLink',
      target,
      resolved: false // Will be set by renderer during resolution
    } as WikiLink)

    lastIndex = matchEnd
  }

  // Add remaining text after the last match
  if (lastIndex < text.length) {
    result.push({
      type: 'text',
      value: text.slice(lastIndex)
    } as Text)
  }

  return result
}

/**
 * Remark plugin that transforms [[Page Name]] syntax into wikiLink nodes
 */
export const remarkWikiLinks: Plugin<[], Root> = function () {
  return (tree: Root) => {
    // Visit all text nodes and transform wiki links
    visit(tree, 'text', (node: Text, index, parent) => {
      if (index === undefined || !parent || !('children' in parent)) {
        return
      }

      const text = node.value

      // Quick test for presence of wiki link pattern
      if (!text.includes('[[')) {
        return
      }

      // Parse the text and extract wiki links
      const newNodes = parseTextForWikiLinks(text)

      // Only replace if we actually found wiki links
      if (newNodes.length <= 1 && newNodes[0]?.type === 'text') {
        return
      }

      // Replace the text node with the new nodes
      const parentChildren = parent.children as Node[]
      parentChildren.splice(index, 1, ...(newNodes as Node[]))

      // Return the index to skip over the newly inserted nodes
      return index + newNodes.length
    })
  }
}

export default remarkWikiLinks
