import { describe, it, expect } from 'vitest'
import { fuzzyMatch, searchRank, bestSearchRank } from './fuzzyMatch'

describe('fuzzyMatch', () => {
  it('matches exact string', () => {
    const result = fuzzyMatch('hello', 'hello')
    expect(result.match).toBe(true)
    expect(result.score).toBeGreaterThan(0)
  })

  it('matches case-insensitively', () => {
    expect(fuzzyMatch('hello', 'Hello World').match).toBe(true)
  })

  it('matches subsequence chars in order', () => {
    expect(fuzzyMatch('cnt', 'Create New Type').match).toBe(true)
  })

  it('rejects when chars are not all present', () => {
    expect(fuzzyMatch('xyz', 'hello').match).toBe(false)
  })

  it('rejects when chars are out of order', () => {
    expect(fuzzyMatch('ba', 'abc').match).toBe(false)
  })

  it('returns higher score for consecutive matches', () => {
    const consecutive = fuzzyMatch('com', 'Commit & Push')
    const scattered = fuzzyMatch('cmt', 'Commit & Push')
    expect(consecutive.score).toBeGreaterThan(scattered.score)
  })

  it('gives bonus for word-start matches', () => {
    const wordStart = fuzzyMatch('cp', 'Commit Push')
    const midWord = fuzzyMatch('om', 'Commit Push')
    expect(wordStart.score).toBeGreaterThan(midWord.score)
  })

  it('matches empty query against any string', () => {
    expect(fuzzyMatch('', 'anything').match).toBe(true)
  })

  it('handles empty target', () => {
    expect(fuzzyMatch('a', '').match).toBe(false)
  })
})

describe('searchRank', () => {
  it('returns 0 for exact match', () => {
    expect(searchRank('Engineering', 'Engineering')).toBe(0)
  })

  it('returns 0 for case-insensitive exact match', () => {
    expect(searchRank('engineering', 'Engineering')).toBe(0)
  })

  it('returns 1 for prefix match', () => {
    expect(searchRank('Engineering', 'Engineering Ideas')).toBe(1)
  })

  it('returns 1 for case-insensitive prefix match', () => {
    expect(searchRank('quarter', 'Quarter Review')).toBe(1)
  })

  it('returns 2 for non-prefix fuzzy match', () => {
    expect(searchRank('Ideas', 'Engineering Ideas')).toBe(2)
  })
})

describe('bestSearchRank', () => {
  it('returns 0 for title exact match', () => {
    expect(bestSearchRank('Engineering', 'Engineering', [])).toBe(0)
  })

  it('returns 0 for title exact match with aliases present', () => {
    expect(bestSearchRank('Engineering', 'Engineering', ['refactor', 'cleanup'])).toBe(0)
  })

  it('returns 1 for alias exact match (never 0)', () => {
    expect(bestSearchRank('ref', 'Engineering Notes', ['ref'])).toBe(1)
  })

  it('ranks alias exact match above title prefix match', () => {
    expect(bestSearchRank('ref', 'Reference Manual', ['ref'])).toBe(1)
    // title prefix would be rank 2, alias exact is rank 1
  })

  it('returns 2 for title prefix match (no alias boost)', () => {
    expect(bestSearchRank('Engineering', 'Engineering Ideas', [])).toBe(2)
  })

  it('returns 3 for alias prefix match only', () => {
    expect(bestSearchRank('ref', 'Something Else', ['refactor'])).toBe(3)
  })

  it('returns 4 when nothing matches as exact or prefix', () => {
    expect(bestSearchRank('ideas', 'Engineering Ideas', ['thoughts'])).toBe(4)
  })

  it('title exact match always beats alias exact match', () => {
    const titleExact = bestSearchRank('Engineering', 'Engineering', ['refactor'])
    const aliasExact = bestSearchRank('Engineering', 'Engineering Ideas', ['Engineering'])
    expect(titleExact).toBeLessThan(aliasExact)
  })

  it('handles trimmed whitespace in title and query', () => {
    expect(bestSearchRank('Engineering ', ' Engineering', [])).toBe(0)
  })
})
