/**
 * remarkCallouts.test.ts
 *
 * Tests for the remarkCallouts plugin that transforms `> [!type]` blockquotes
 * into callout AST nodes.
 *
 * Requirements: 8.1, 8.3, 8.6
 */

import { describe, it, expect } from 'vitest'
import { unified } from 'unified'
import _remarkParse from 'remark-parse'
import _remarkStringify from 'remark-stringify'
import type { Root } from 'mdast'
import { remarkCallouts, type Callout } from '../../src/main/plugins/remarkCallouts'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** CJS/ESM interop. */
function unwrap<T>(mod: any): T {
  return mod && mod.__esModule && mod.default !== undefined ? mod.default : mod
}
const remarkParse = unwrap<typeof _remarkParse>(_remarkParse)

/** Build a processor (parse + callouts, no stringify). */
function createProcessor() {
  return unified().use(remarkParse).use(remarkCallouts)
}

/** Parse markdown with the callout plugin and return the transformed AST. */
function parse(md: string): Root {
  const result = createProcessor().parse(md)
  createProcessor().runSync(result)
  return result
}

/** Find all callout nodes in an AST. */
function findCallouts(ast: Root): Callout[] {
  const callouts: Callout[] = []
  function walk(nodes: any[]): void {
    for (const node of nodes) {
      if (node.type === 'callout') {
        callouts.push(node as Callout)
      }
      if (node.children) walk(node.children)
    }
  }
  walk(ast.children)
  return callouts
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('remarkCallouts', () => {
  it('transforms a basic callout', () => {
    const ast = parse('> [!note] A note')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('note')
    expect(callouts[0].title).toBe('A note')
    expect(callouts[0].toggle).toBeUndefined()
  })

  it('transforms warning callout', () => {
    const ast = parse('> [!warning] Be careful\n>\n> Something important')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('warning')
    expect(callouts[0].title).toBe('Be careful')
    expect(callouts[0].children.length).toBeGreaterThanOrEqual(1)
  })

  it('transforms callout with + toggle (expanded)', () => {
    const ast = parse('> [!tip]+ Tips\n> Content')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('tip')
    expect(callouts[0].toggle).toBe('+')
    expect(callouts[0].title).toBe('Tips')
  })

  it('transforms callout with - toggle (collapsed)', () => {
    const ast = parse('> [!example]- Collapsed\n> Hidden content')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('example')
    expect(callouts[0].toggle).toBe('-')
  })

  it('falls back unknown types to note', () => {
    const ast = parse('> [!unknown] Something')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('note')
  })

  it('preserves body content as children', () => {
    const ast = parse('> [!quote] Famous\n>\n> First paragraph\n>\n> Second paragraph')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    // Body should contain the remaining paragraphs.
    expect(callouts[0].children.length).toBeGreaterThanOrEqual(2)
  })

  it('does not transform regular blockquotes', () => {
    const ast = parse('> Just a quote\n> Not a callout')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(0)
  })

  it('does not transform blockquotes with [! inside but not at start', () => {
    const ast = parse('> This is [!note] not a callout')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(0)
  })

  it('handles callout without title', () => {
    const ast = parse('> [!warning]\n> Just body')
    const callouts = findCallouts(ast)
    expect(callouts).toHaveLength(1)
    expect(callouts[0].calloutType).toBe('warning')
    expect(callouts[0].title).toBeUndefined()
  })

  it('supports all known callout types', () => {
    const types = [
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
    ]

    for (const type of types) {
      const ast = parse(`> [!${type}] Test`)
      const callouts = findCallouts(ast)
      expect(callouts).toHaveLength(1)
      expect(callouts[0].calloutType).toBe(type)
    }
  })

  it('removes the blockquote wrapper (callout node is a direct child)', () => {
    // After transformation, the blockquote should become a callout node.
    const ast = parse('> [!info] Info here\n> Body text')
    const firstChild = ast.children[0] as any
    expect(firstChild.type).toBe('callout')
    // No blockquote should remain.
    const blockquotes = ast.children.filter((c: any) => c.type === 'blockquote')
    expect(blockquotes).toHaveLength(0)
  })
})
