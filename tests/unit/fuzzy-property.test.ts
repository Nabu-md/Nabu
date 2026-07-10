/**
 * fuzzy-property.test.ts
 *
 * Property-based tests for the fuzzy search ranker.
 * Verifies determinism and basic invariants (Req 5.7).
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import { fuzzySearch, type FuzzyItem } from '../../src/renderer/src/utils/fuzzy'

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/** Generate a random fuzzy item. */
const fuzzyItemArbitrary: fc.Arbitrary<FuzzyItem> = fc.record({
  name: fc.string({ minLength: 1, maxLength: 30 }),
  path: fc.string({ minLength: 1, maxLength: 50 }),
  aliases: fc.option(
    fc.array(fc.string({ minLength: 1, maxLength: 20 }), { minLength: 0, maxLength: 3 })
  ),
  keywords: fc.option(
    fc.array(fc.string({ minLength: 1, maxLength: 20 }), { minLength: 0, maxLength: 3 })
  )
})

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

describe('fuzzy determinism (property)', () => {
  it('same inputs always produce the same ranked output', () => {
    fc.assert(
      fc.property(
        fuzzyItemArbitrary,
        fuzzyItemArbitrary,
        fuzzyItemArbitrary,
        fc.string({ minLength: 0, maxLength: 15 }),
        (a, b, c, query) => {
          const items = [a, b, c]
          const result1 = fuzzySearch(query, items)
          const result2 = fuzzySearch(query, items)
          const result3 = fuzzySearch(query, items)

          // All three calls must produce identical results.
          expect(result1).toEqual(result2)
          expect(result2).toEqual(result3)
        }
      ),
      { numRuns: 100 }
    )
  })
})

// ---------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------

describe('fuzzy result invariants (property)', () => {
  it('never returns more items than maxResults', () => {
    fc.assert(
      fc.property(
        fc.array(fuzzyItemArbitrary, { minLength: 1, maxLength: 20 }),
        fc.constantFrom(1, 3, 5, 10),
        (items, maxResults) => {
          const results = fuzzySearch('test', items, { maxResults })
          expect(results.length).toBeLessThanOrEqual(maxResults)
        }
      ),
      { numRuns: 100 }
    )
  })

  it('results are sorted by score descending', () => {
    fc.assert(
      fc.property(
        fc.array(fuzzyItemArbitrary, { minLength: 1, maxLength: 20 }),
        fc.string({ minLength: 1, maxLength: 10 }),
        (items, query) => {
          const results = fuzzySearch(query, items)
          for (let i = 1; i < results.length; i++) {
            expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score)
          }
        }
      ),
      { numRuns: 100 }
    )
  })

  it('empty query returns no results', () => {
    fc.assert(
      fc.property(fc.array(fuzzyItemArbitrary, { minLength: 1, maxLength: 10 }), (items) => {
        const results = fuzzySearch('', items)
        expect(results).toHaveLength(0)
      }),
      { numRuns: 50 }
    )
  })

  it('threshold excludes low-score results', () => {
    fc.assert(
      fc.property(
        fc.array(fuzzyItemArbitrary, { minLength: 1, maxLength: 10 }),
        fc.string({ minLength: 1, maxLength: 10 }),
        (items, query) => {
          const noThreshold = fuzzySearch(query, items)
          const withThreshold = fuzzySearch(query, items, { threshold: 5 })
          // With a high threshold, we should have fewer or equal results.
          expect(withThreshold.length).toBeLessThanOrEqual(noThreshold.length)
        }
      ),
      { numRuns: 100 }
    )
  })
})
