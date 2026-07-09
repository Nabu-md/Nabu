/**
 * remarkEmbeds.test.ts
 *
 * Unit tests for the remarkEmbeds plugin that transforms `![[target]]` syntax
 * into embed AST nodes.
 *
 * Requirements: 11.1, 11.7
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import type { Root } from 'mdast'
import { remarkEmbeds, type EmbedNode } from '../../src/main/plugins/remarkEmbeds'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createProcessor() {
  return unified().use(remarkParse).use(remarkEmbeds)
}

function parse(md: string): Root {
  const result = createProcessor().parse(md)
  createProcessor().runSync(result)
  return result
}

function findEmbeds(ast: Root): EmbedNode[] {
  const embeds: EmbedNode[] = []
  function walk(nodes: any[]): void {
    for (const node of nodes) {
      if (node.type === 'embed') embeds.push(node as EmbedNode)
      if (node.children) walk(node.children)
    }
  }
  walk(ast.children)
  return embeds
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('remarkEmbeds', () => {
  it('transforms ![[image.png]] into embed node', () => {
    const ast = parse('Here is ![[image.png]] inline.')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('image.png')
  })

  it('transforms ![[note]] embed', () => {
    const ast = parse('![[note]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('note')
  })

  it('handles multiple embeds in the same paragraph', () => {
    const ast = parse('![[a.png]] and ![[b.png]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(2)
    expect(embeds[0].target).toBe('a.png')
    expect(embeds[1].target).toBe('b.png')
  })

  it('preserves text before and after embeds', () => {
    const ast = parse('Before ![[mid.png]] after')
    const paragraph = ast.children[0] as any
    expect(paragraph.children).toHaveLength(3)
    expect(paragraph.children[0].type).toBe('text')
    expect(paragraph.children[0].value).toBe('Before ')
    expect(paragraph.children[1].type).toBe('embed')
    expect(paragraph.children[2].type).toBe('text')
    expect(paragraph.children[2].value).toBe(' after')
  })

  it('trims whitespace from embed target', () => {
    const ast = parse('![[  padded  ]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('padded')
  })

  it('does not transform normal text without embed syntax', () => {
    const ast = parse('Just regular text with no embeds.')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(0)
  })

  it('does not transform wiki links (without the ! prefix)', () => {
    const ast = parse('[[Just a wiki link]] not an embed')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(0)
  })

  it('handles embeds with special characters in target', () => {
    const ast = parse('![[my-file_v2.1.md]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('my-file_v2.1.md')
  })

  it('handles consecutive embeds with no text between', () => {
    const ast = parse('![[a]]![[b]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(2)
    expect(embeds[0].target).toBe('a')
    expect(embeds[1].target).toBe('b')
  })

  it('handles embed at the start of content', () => {
    const ast = parse('![[start.png]] trailing text')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('start.png')
  })

  it('handles embed at the end of content', () => {
    const ast = parse('Leading text ![[end.png]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('end.png')
  })

  it('handles embeds inside headings', () => {
    const ast = parse('# Heading with ![[embed.png]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('embed.png')
  })

  it('handles embeds inside list items', () => {
    const ast = parse('- List with ![[embed.png]]')
    const embeds = findEmbeds(ast)
    expect(embeds).toHaveLength(1)
    expect(embeds[0].target).toBe('embed.png')
  })
})
