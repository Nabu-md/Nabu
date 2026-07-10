/**
 * footnotes.test.ts
 *
 * Unit tests for footnote extraction logic.
 *
 * Requirements: 31.1, 31.2, 31.3, 31.4, 31.5, 31.6
 */

import { describe, it, expect } from 'vitest'
import { extractFootnotes } from '../../src/shared/remarkFootnotes'

describe('extractFootnotes', () => {
  it('returns empty arrays for AST without footnotes', () => {
    const ast = { type: 'root', children: [] }
    const result = extractFootnotes(ast as unknown as Parameters<typeof extractFootnotes>[0])
    expect(result.references).toEqual([])
    expect(result.definitions).toEqual([])
  })

  it('extracts footnote references', () => {
    const ast = {
      type: 'root',
      children: [
        {
          type: 'paragraph',
          children: [
            { type: 'text', value: 'Some text ' },
            { type: 'footnoteReference', label: 'note1' },
            { type: 'text', value: ' more text' }
          ]
        }
      ]
    }
    const result = extractFootnotes(ast as unknown as Parameters<typeof extractFootnotes>[0])
    expect(result.references).toHaveLength(1)
    expect(result.references[0].label).toBe('note1')
  })

  it('extracts footnote definitions', () => {
    const ast = {
      type: 'root',
      children: [
        {
          type: 'footnoteDefinition',
          label: 'fn1',
          children: [{ type: 'text', value: 'First footnote' }]
        },
        {
          type: 'footnoteDefinition',
          label: 'fn2',
          children: [{ type: 'text', value: 'Second footnote' }]
        }
      ]
    }
    const result = extractFootnotes(ast as unknown as Parameters<typeof extractFootnotes>[0])
    expect(result.definitions).toHaveLength(2)
    expect(result.definitions[0].label).toBe('fn1')
    expect(result.definitions[1].label).toBe('fn2')
  })
})
