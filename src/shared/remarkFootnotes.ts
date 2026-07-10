/**
 * remarkFootnotes.ts
 *
 * Extract footnote references and definitions from AST.
 * Supports `[^label]` references and `[^label]:` definitions.
 *
 * Requirements: 31.1, 31.2, 31.3, 31.4, 31.5, 31.6
 */

import { visit } from 'unist-util-visit'
import type { Node } from 'unist'

export interface FootnoteDefinition {
  type: 'footnoteDefinition'
  label: string
  children: Node[]
}

export interface FootnoteReference {
  type: 'footnoteReference'
  label: string
}

/**
 * Extract footnote references and definitions from AST.
 */
export function extractFootnotes(ast: Node): {
  references: FootnoteReference[]
  definitions: FootnoteDefinition[]
} {
  const references: FootnoteReference[] = []
  const definitions: FootnoteDefinition[] = []

  visit(ast, (node: Node) => {
    if (node.type === 'footnoteReference') {
      const ref = node as { label?: string }
      references.push({
        type: 'footnoteReference',
        label: ref.label ?? ''
      })
    }
    if (node.type === 'footnoteDefinition') {
      const def = node as { label?: string; children?: Node[] }
      definitions.push({
        type: 'footnoteDefinition',
        label: def.label ?? '',
        children: def.children ?? []
      })
    }
  })

  return { references, definitions }
}
