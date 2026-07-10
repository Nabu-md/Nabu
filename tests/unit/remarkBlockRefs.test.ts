/**
 * remarkBlockRefs.test.ts
 *
 * Unit tests for the remarkBlockRefs plugin that:
 *   1. Extracts trailing `^identifier` from block-level nodes as `data.blockId`.
 *   2. Splits `[[target#^id]]` wikiLink targets into `target` + `blockRef`.
 *
 * Requirements: 20.1, 20.2, 20.5, 20.6
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import type { Root } from 'mdast'
import { remarkWikiLinks } from '../../src/main/plugins/remarkWikiLinks'
import { remarkBlockRefs } from '../../src/main/plugins/remarkBlockRefs'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createProcessor() {
  return unified().use(remarkParse).use(remarkGfm).use(remarkWikiLinks).use(remarkBlockRefs)
}

function parse(md: string): Root {
  const result = createProcessor().parse(md)
  createProcessor().runSync(result)
  return result
}

/** Find all nodes that have data.blockId set. */
function findBlockIds(ast: Root): { type: string; blockId: string; text?: string }[] {
  const results: { type: string; blockId: string; text?: string }[] = []
  function walk(nodes: any[]): void {
    for (const node of nodes) {
      if (node.data?.blockId) {
        const text = node.children?.find((c: any) => c.type === 'text')?.value
        results.push({ type: node.type, blockId: node.data.blockId, text })
      }
      if (node.children) walk(node.children)
    }
  }
  walk(ast.children)
  return results
}

/** Find all wikiLink nodes with blockRef set. */
function findBlockRefLinks(ast: Root): { target: string; blockRef: string }[] {
  const results: { target: string; blockRef: string }[] = []
  function walk(nodes: any[]): void {
    for (const node of nodes) {
      if (node.type === 'wikiLink' && node.blockRef) {
        results.push({ target: node.target, blockRef: node.blockRef })
      }
      if (node.children) walk(node.children)
    }
  }
  walk(ast.children)
  return results
}

// ---------------------------------------------------------------------------
// Tests: Block IDs
// ---------------------------------------------------------------------------

describe('remarkBlockRefs — block IDs', () => {
  it('extracts trailing ^id from a paragraph', () => {
    const ast = parse('Hello world ^my-id')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(1)
    expect(blocks[0].type).toBe('paragraph')
    expect(blocks[0].blockId).toBe('my-id')
    // The ^id should be removed from the text content
    expect(blocks[0].text).toBe('Hello world')
  })

  it('extracts trailing ^id from a heading', () => {
    const ast = parse('# Section Title ^sec1')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(1)
    expect(blocks[0].type).toBe('heading')
    expect(blocks[0].blockId).toBe('sec1')
    expect(blocks[0].text).toBe('Section Title')
  })

  it('extracts trailing ^id from a paragraph inside a list item', () => {
    // remark-parse wraps list item content in a paragraph, so the blockId
    // is attached to the paragraph node inside the listItem.
    const ast = parse('- List item text ^li-1')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(1)
    expect(blocks[0].type).toBe('paragraph')
    expect(blocks[0].blockId).toBe('li-1')
    expect(blocks[0].text).toBe('List item text')
  })

  it('extracts trailing ^id from a paragraph inside a blockquote', () => {
    // remark-parse wraps blockquote content in a paragraph, so the blockId
    // is attached to the paragraph node inside the blockquote.
    const ast = parse('> A quoted line ^quote1')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(1)
    expect(blocks[0].type).toBe('paragraph')
    expect(blocks[0].blockId).toBe('quote1')
    expect(blocks[0].text).toBe('A quoted line')
  })

  it('does not extract ^id from code blocks (code has no text children)', () => {
    // Code blocks use `value` instead of children/text, so the plugin
    // cannot extract block IDs from them.
    const ast = parse('```\ncode content\n```')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(0)
  })

  it('handles block IDs with hyphens and underscores', () => {
    const ast = parse('Content with complex-id_123 ^complex-id_456')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(1)
    expect(blocks[0].blockId).toBe('complex-id_456')
    expect(blocks[0].text).toBe('Content with complex-id_123')
  })

  it('does not extract ^id when there is no trailing identifier', () => {
    const ast = parse('Just regular text with no block id ^somewhere in the middle')
    const blocks = findBlockIds(ast)
    // The ^somewhere is not at the end of the paragraph so it should not match
    expect(blocks).toHaveLength(0)
  })

  it('does not add blockId to nodes without trailing ^id', () => {
    const ast = parse('Regular paragraph\n\nAnother paragraph')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(0)
  })

  it('handles multiple block IDs across different paragraphs', () => {
    const ast = parse('First block ^first\n\nSecond block ^second\n\nThird block ^third')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(3)
    expect(blocks[0].blockId).toBe('first')
    expect(blocks[1].blockId).toBe('second')
    expect(blocks[2].blockId).toBe('third')
  })

  it('preserves non-block-id trailing content', () => {
    const ast = parse('Text with caret^like this')
    const blocks = findBlockIds(ast)
    expect(blocks).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// Tests: Block references in wiki links
// ---------------------------------------------------------------------------

describe('remarkBlockRefs — block refs in wiki links', () => {
  it('splits [[note#^id]] into target and blockRef', () => {
    const ast = parse('Link to [[Note#^my-block]]')
    const refs = findBlockRefLinks(ast)
    expect(refs).toHaveLength(1)
    expect(refs[0].target).toBe('Note')
    expect(refs[0].blockRef).toBe('my-block')
  })

  it('preserves [[note]] links without blockRef (no hash)', () => {
    const ast = parse('Regular [[Wiki Link]]')
    const refs = findBlockRefLinks(ast)
    expect(refs).toHaveLength(0)
    // The wikiLink node should exist but without blockRef
    function walk(nodes: any[]): void {
      for (const node of nodes) {
        if (node.type === 'wikiLink') {
          expect(node.blockRef).toBeUndefined()
          expect(node.target).toBe('Wiki Link')
        }
        if (node.children) walk(node.children)
      }
    }
    walk(ast.children)
  })

  it('handles hyphens in blockRef identifier', () => {
    const ast = parse('[[Note-with-dashes#^complex-block-id]]')
    const refs = findBlockRefLinks(ast)
    expect(refs).toHaveLength(1)
    expect(refs[0].target).toBe('Note-with-dashes')
    expect(refs[0].blockRef).toBe('complex-block-id')
  })

  it('handles underscores in blockRef identifier', () => {
    const ast = parse('[[Note#^block_identifier]]')
    const refs = findBlockRefLinks(ast)
    expect(refs).toHaveLength(1)
    expect(refs[0].target).toBe('Note')
    expect(refs[0].blockRef).toBe('block_identifier')
  })

  it('preserves regular text when [[target#^id]] is followed by text', () => {
    const ast = parse('Before [[Note#^ref]] after')
    const paragraph = ast.children[0] as any
    const textNodes = paragraph.children.filter((c: any) => c.type === 'text')
    expect(textNodes.some((t: any) => t.value.includes('Before'))).toBe(true)
    expect(textNodes.some((t: any) => t.value.includes('after'))).toBe(true)
  })
})
