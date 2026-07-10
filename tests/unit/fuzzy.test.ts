/**
 * fuzzy.test.ts
 *
 * Unit tests for the shared fuzzy-ranking utility.
 *
 * Requirements: 4.2, 5.7
 */

import { describe, it, expect } from 'vitest'
import { matchScore, fuzzySearch, type FuzzyItem } from '../../src/renderer/src/utils/fuzzy'

// ---------------------------------------------------------------------------
// matchScore — core matching algorithm
// ---------------------------------------------------------------------------

describe('matchScore', () => {
  it('returns null for empty query', () => {
    expect(matchScore('', 'hello')).toBeNull()
  })

  it('returns null for empty target', () => {
    expect(matchScore('hello', '')).toBeNull()
  })

  it('returns null when query characters not in order', () => {
    expect(matchScore('cb', 'abc')).toBeNull()
  })

  it('matches characters in order (case-insensitive)', () => {
    const result = matchScore('hl', 'Hello')
    expect(result).not.toBeNull()
    expect(result!.score).toBeGreaterThan(0)
  })

  it('returns match ranges for exact match', () => {
    const result = matchScore('abc', 'abc')
    expect(result).not.toBeNull()
    expect(result!.ranges).toEqual([{ start: 0, end: 3 }])
  })

  it('returns non-overlapping ranges for non-consecutive matches', () => {
    const result = matchScore('hlo', 'hello')
    expect(result).not.toBeNull()
    // h at 0, l at 2, o at 4 → ranges: [0,1], [2,3], [4,5]
    expect(result!.ranges).toEqual([
      { start: 0, end: 1 },
      { start: 2, end: 3 },
      { start: 4, end: 5 }
    ])
  })

  it('returns a single range for consecutive matches', () => {
    const result = matchScore('wor', 'HelloWorld')
    expect(result).not.toBeNull()
    // w at 5, o at 6, r at 7 → range [5, 8]
    expect(result!.ranges).toEqual([{ start: 5, end: 8 }])
  })

  it('bonuses sequential matches', () => {
    // 'ab' consecutive in 'abXYZ' should score higher than 'ab' in 'aXbYZ'
    const seq = matchScore('ab', 'abXYZ')!
    const nonSeq = matchScore('ab', 'aXbYZ')!
    expect(seq.score).toBeGreaterThan(nonSeq.score)
  })

  it('bonuses start-of-word matches', () => {
    // 'h' at position 0 scores higher than 'h' after 'a'
    const start = matchScore('h', 'hello')!
    const later = matchScore('h', 'ahello')!
    expect(start.score).toBeGreaterThan(later.score)
  })

  it('bonuses word-boundary matches after separator', () => {
    // 'w' in 'hello_world' should have a word-boundary bonus at index 6
    const boundary = matchScore('w', 'hello_world')!
    expect(boundary.score).toBeGreaterThan(0)
  })

  it('bonuses camelCase boundaries', () => {
    const result = matchScore('w', 'helloWorld')!
    expect(result.score).toBeGreaterThan(0)
  })

  it('handles special characters in target', () => {
    const result = matchScore('one', 'some/file-name_one.md')
    expect(result).not.toBeNull()
    expect(result!.score).toBeGreaterThan(0)
  })

  it('returns null when query is longer than target', () => {
    expect(matchScore('abcdef', 'abc')).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// fuzzySearch — aggregate search across items
// ---------------------------------------------------------------------------

describe('fuzzySearch', () => {
  const items: FuzzyItem[] = [
    { name: 'index', path: 'src/index.ts', aliases: ['main'] },
    { name: 'App', path: 'src/App.tsx' },
    { name: 'styles', path: 'src/assets/styles.css' },
    { name: 'readme', path: 'README.md', aliases: ['docs'] },
    { name: 'vite-config', path: 'vite.config.ts' }
  ]

  it('returns empty array for empty query', () => {
    expect(fuzzySearch('', items)).toEqual([])
  })

  it('matches by name', () => {
    const results = fuzzySearch('app', items)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].item.name).toBe('App')
    expect(results[0].matchField).toBe('name')
  })

  it('matches by path', () => {
    const results = fuzzySearch('tsx', items)
    expect(results.length).toBeGreaterThan(0)
    // At least "App" should match via path "src/App.tsx" or name
    expect(results.some((r) => r.item.name === 'App')).toBe(true)
  })

  it('matches by alias', () => {
    // 'main' is an alias of 'index'
    const results = fuzzySearch('main', items)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].item.name).toBe('index')
    expect(results[0].matchField).toBe('alias')
  })

  it('name matches outrank path matches', () => {
    // Search for 'index' — name match for 'index' beats path match for 'vite-config'
    const results = fuzzySearch('index', items)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].item.name).toBe('index')
  })

  it('sorts by score descending', () => {
    const results = fuzzySearch('a', items)
    for (let i = 1; i < results.length; i++) {
      expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score)
    }
  })

  it('returns deterministic ordering for equal scores', () => {
    // Create items with identical score potential
    const equalItems: FuzzyItem[] = [
      { name: 'alpha', path: 'alpha.md' },
      { name: 'beta', path: 'beta.md' }
    ]
    const first = fuzzySearch('a', equalItems)
    const second = fuzzySearch('a', equalItems)
    expect(first.map((r) => r.item.name)).toEqual(second.map((r) => r.item.name))
  })

  it('respects maxResults option', () => {
    const results = fuzzySearch('a', items, { maxResults: 2 })
    expect(results.length).toBeLessThanOrEqual(2)
  })

  it('respects threshold option', () => {
    // A high threshold should exclude weak matches.
    const results = fuzzySearch('a', items, { threshold: 999 })
    expect(results).toEqual([])
  })

  it('returns match ranges for highlighting', () => {
    const results = fuzzySearch('app', items)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].ranges.length).toBeGreaterThan(0)
    for (const range of results[0].ranges) {
      expect(range.start).toBeGreaterThanOrEqual(0)
      expect(range.end).toBeGreaterThan(range.start)
    }
  })

  it('matches against keywords', () => {
    const keywordItems: FuzzyItem[] = [
      { name: 'editor', path: 'editor.tsx', keywords: ['edit', 'view', 'code'] },
      { name: 'note', path: 'note.tsx', keywords: ['note', 'document'] }
    ]
    const results = fuzzySearch('code', keywordItems)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].item.name).toBe('editor')
    expect(results[0].matchField).toBe('keyword')
  })

  it('handles items with no aliases gracefully', () => {
    const results = fuzzySearch('app', items)
    // Should not throw, should still work.
    expect(results.length).toBeGreaterThan(0)
  })

  it('handles empty items array', () => {
    expect(fuzzySearch('test', [])).toEqual([])
  })

  it('accepts items with name equal to query exactly', () => {
    const exactItems: FuzzyItem[] = [
      { name: 'search', path: 'search.ts' },
      { name: 'search-results', path: 'results.ts' }
    ]
    const results = fuzzySearch('search', exactItems)
    expect(results.length).toBeGreaterThan(0)
    expect(results[0].item.name).toBe('search') // exact name match
  })
})

// ---------------------------------------------------------------------------
// Property-based invariants
// ---------------------------------------------------------------------------

describe('fuzzySearch invariants', () => {
  it('is deterministic for same input', () => {
    const items: FuzzyItem[] = [
      { name: 'alpha', path: 'a.md', aliases: ['first'] },
      { name: 'beta', path: 'b.md', aliases: ['second'] },
      { name: 'gamma', path: 'c.md' }
    ]
    const queries = ['', 'a', 'be', 'gamma', 'xyz']

    for (const q of queries) {
      const first = fuzzySearch(q, items)
      const second = fuzzySearch(q, items)
      expect(first.map((r) => r.item.name)).toEqual(second.map((r) => r.item.name))
    }
  })

  it('results are sorted by score descending', () => {
    const items: FuzzyItem[] = [
      { name: 'abc', path: 'abc.md' },
      { name: 'bcd', path: 'bcd.md' },
      { name: 'cde', path: 'cde.md' },
      { name: 'def', path: 'def.md' }
    ]
    const results = fuzzySearch('a', items)
    for (let i = 1; i < results.length; i++) {
      expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score)
    }
  })

  it('maxResults never exceeds length', () => {
    const items: FuzzyItem[] = [
      { name: 'apple', path: 'a.md' },
      { name: 'banana', path: 'b.md' },
      { name: 'cherry', path: 'c.md' }
    ]
    const results = fuzzySearch('a', items, { maxResults: 2 })
    expect(results.length).toBeLessThanOrEqual(2)
  })
})
