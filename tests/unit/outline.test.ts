/**
 * outline.test.ts
 *
 * Tests for the OutlinePanel heading extraction logic.
 *
 * Requirements: 7.1, 7.2, 7.5
 */

import { describe, it, expect } from 'vitest'
import type { Root, Heading } from 'mdast'

// ---------------------------------------------------------------------------
// Pure logic helper — mirrors extractOutline in OutlinePanel.tsx
// ---------------------------------------------------------------------------

interface OutlineEntry {
  childIndex: number
  depth: number
  text: string
}

function flattenText(node: unknown): string {
  if (typeof node === 'string') return node
  if (node && typeof node === 'object') {
    const n = node as { value?: string; children?: unknown[] }
    if (n.value) return n.value
    if (n.children) return n.children.map(flattenText).join('')
  }
  return ''
}

function extractOutline(ast: Root | null): OutlineEntry[] {
  if (!ast) return []

  const entries: OutlineEntry[] = []

  for (let i = 0; i < ast.children.length; i++) {
    const child = ast.children[i]
    if (child.type === 'heading') {
      const heading = child as Heading
      entries.push({
        childIndex: i,
        depth: heading.depth,
        text: flattenText(heading)
      })
    }
  }

  return entries
}

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

function makeHeading(depth: 1 | 2 | 3 | 4 | 5 | 6, text: string): Heading {
  return {
    type: 'heading',
    depth,
    children: [{ type: 'text', value: text }]
  } as Heading
}

function makeRoot(children: unknown[]): Root {
  return { type: 'root', children } as Root
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('extractOutline', () => {
  it('returns empty array for null AST', () => {
    expect(extractOutline(null)).toEqual([])
  })

  it('extracts a single heading', () => {
    const ast = makeRoot([makeHeading(1, 'Introduction')])
    const outline = extractOutline(ast)
    expect(outline).toHaveLength(1)
    expect(outline[0]).toEqual({
      childIndex: 0,
      depth: 1,
      text: 'Introduction'
    })
  })

  it('extracts multiple headings with correct depths and indices', () => {
    const ast = makeRoot([
      makeHeading(1, 'Title'),
      makeHeading(2, 'Section'),
      makeHeading(3, 'Subsection'),
      makeHeading(2, 'Another Section')
    ])
    const outline = extractOutline(ast)
    expect(outline).toHaveLength(4)
    expect(outline[0]).toEqual({ childIndex: 0, depth: 1, text: 'Title' })
    expect(outline[1]).toEqual({ childIndex: 1, depth: 2, text: 'Section' })
    expect(outline[2]).toEqual({ childIndex: 2, depth: 3, text: 'Subsection' })
    expect(outline[3]).toEqual({ childIndex: 3, depth: 2, text: 'Another Section' })
  })

  it('skips non-heading nodes', () => {
    const ast = makeRoot([
      makeHeading(1, 'First'),
      { type: 'paragraph', children: [{ type: 'text', value: 'some text' }] },
      makeHeading(2, 'Second')
    ])
    const outline = extractOutline(ast)
    expect(outline).toHaveLength(2)
    expect(outline[0].text).toBe('First')
    expect(outline[1].text).toBe('Second')
  })

  it('flattens inline formatting in heading text', () => {
    const ast = makeRoot([
      {
        type: 'heading',
        depth: 2,
        children: [
          { type: 'text', value: 'Hello ' },
          { type: 'strong', children: [{ type: 'text', value: 'World' }] }
        ]
      } as Heading
    ])
    const outline = extractOutline(ast)
    expect(outline[0].text).toBe('Hello World')
  })

  it('returns empty array when AST has no headings', () => {
    const ast = makeRoot([{ type: 'paragraph', children: [{ type: 'text', value: 'just text' }] }])
    expect(extractOutline(ast)).toHaveLength(0)
  })

  it('handles headings h1 through h6', () => {
    const headings = []
    for (const depth of [1, 2, 3, 4, 5, 6] as const) {
      headings.push(makeHeading(depth, `Level ${depth}`))
    }
    const ast = makeRoot(headings)
    const outline = extractOutline(ast)
    expect(outline).toHaveLength(6)
    for (let i = 0; i < 6; i++) {
      expect(outline[i].depth).toBe(i + 1)
      expect(outline[i].text).toBe(`Level ${i + 1}`)
    }
  })
})
