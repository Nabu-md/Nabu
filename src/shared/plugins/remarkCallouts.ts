/**
 * remarkCallouts.ts
 *
 * Remark plugin that transforms blockquote nodes starting with `> [!TYPE]`
 * into callout AST nodes. Supports `[!type]+` (expanded toggle) and
 * `[!type]-` (collapsed toggle) suffixes.
 *
 * Requirements: 8.1, 8.3, 8.6
 */

import { visit } from 'unist-util-visit'
import type { Plugin } from 'unified'
import type { Root, Blockquote, Paragraph, Text } from 'mdast'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Callout {
  type: 'callout'
  /** The callout type (e.g. "note", "warning", "tip"). Lowercased. */
  calloutType: string
  /** Optional title text after the type declaration. */
  title?: string
  /**
   * Collapsible suffix:
   *   '+' → expanded by default, toggleable
   *   '-' → collapsed by default, toggleable
   *   undefined → not collapsible
   */
  toggle: '+' | '-' | undefined
  /** The callout body (paragraphs, lists, etc.). */
  children: unknown[]
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Matches a callout marker at the start of text: [!type], [!type]+, or [!type]- */
const CALLOUT_RE = /^\[!([A-Za-z][\w/_-]*)\]([+-]?)([\s\S]*)$/

/** Known valid callout types (lowercased). Unknown types fall back to "note". */
const VALID_TYPES = new Set([
  'note',
  'info',
  'tip',
  'success',
  'warning',
  'danger',
  'error',
  'question',
  'example',
  'quote',
  'abstract'
])

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Canonicalise a callout type — lowercase, fall back to 'note' for unknown. */
function canonicalType(raw: string): string {
  const lower = raw.toLowerCase()
  return VALID_TYPES.has(lower) ? lower : 'note'
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/**
 * Remark plugin that converts blockquote nodes with callout markers
 * (`> [!TYPE]`) into custom `callout` nodes.
 *
 * The plugin runs after remark-parse has already turned `>` blockquotes
 * into `blockquote` nodes, so we visit blockquote nodes and check whether
 * their first paragraph's first text child matches the callout pattern.
 */
export const remarkCallouts: Plugin<[], Root> = function () {
  return (tree: Root) => {
    visit(tree, 'blockquote', (node: Blockquote, index: number | undefined, parent: unknown) => {
      if (index === undefined || !parent || !('children' in (parent as { children?: unknown[] }))) {
        return
      }

      const children = node.children
      if (children.length === 0) return

      // The first child must be a paragraph.
      const firstChild = children[0]
      if (firstChild.type !== 'paragraph') return

      const paragraph = firstChild as Paragraph
      if (paragraph.children.length === 0) return

      // The first child of the paragraph must be text.
      const firstText = paragraph.children[0]
      if (firstText.type !== 'text') return

      const text = (firstText as Text).value
      const match = text.match(CALLOUT_RE)
      if (!match) return

      const calloutType = canonicalType(match[1])
      const toggle = match[2] === '+' || match[2] === '-' ? match[2] : undefined

      // Split the remainder into title (first line) and body content.
      const remainder = match[3]
      const newlineIdx = remainder.indexOf('\n')
      const title =
        (newlineIdx === -1 ? remainder.trim() : remainder.slice(0, newlineIdx).trim()) || undefined

      // Build callout body from:
      //   1. The remaining paragraph children (text nodes after the marker)
      //   2. Any trailing text after the first newline in the marker text node
      //   3. The rest of the blockquote children (paragraphs 2..n)
      const bodyChildren: unknown[] = []

      // (a) Remaining text nodes in the first paragraph (e.g. bold/italic after marker).
      const remainingParagraph = [...paragraph.children.slice(1)]
      if (remainingParagraph.length > 0) {
        bodyChildren.push({ type: 'paragraph', children: remainingParagraph })
      }

      // (b) Body continuation text after the first newline in the marker text node.
      if (newlineIdx !== -1) {
        const bodyText = remainder.slice(newlineIdx + 1)
        if (bodyText.trim()) {
          // Split by double-newlines to approximate paragraphs.
          const bodyParts = bodyText.split(/\n\n+/)
          for (const part of bodyParts) {
            if (part.trim()) {
              bodyChildren.push({
                type: 'paragraph',
                children: [{ type: 'text', value: part.trim() }]
              })
            }
          }
        }
      }

      // (c) The rest of the blockquote children (paragraphs 2..n).
      bodyChildren.push(...children.slice(1))

      // Build the callout node.
      const calloutNode: Callout = {
        type: 'callout',
        calloutType,
        title,
        toggle,
        children: bodyChildren
      }

      // Replace the blockquote node with the callout node.
      const parentChildren = (parent as { children: unknown[] }).children
      parentChildren.splice(index, 1, calloutNode)
    })
  }
}

export default remarkCallouts
