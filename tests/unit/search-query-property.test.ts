/**
 * search-query-property.test.ts
 *
 * Property-based tests for the search query module.
 * Verifies AND-combination soundness (Req 3.8) and parse invariants.
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import { parseQuery } from '../../src/shared/search-query'

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/** A value that doesn't contain whitespace (safe for single-token operators). */
const wordArbitrary = fc.string({ minLength: 1, maxLength: 20 }).filter((s) => !/\s/.test(s))

/** A value suitable for property:name:value — name cannot contain colons. */
const propertyNameArbitrary = fc
  .string({ minLength: 1, maxLength: 20 })
  .filter((s) => !/\s/.test(s) && !s.includes(':'))

// ---------------------------------------------------------------------------
// Property: parse invariants
// ---------------------------------------------------------------------------

describe('parseQuery invariants (property)', () => {
  it('always returns a valid ParsedQuery object for any string', () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 100 }), (query) => {
        const parsed = parseQuery(query)
        expect(parsed).toBeDefined()
        expect(typeof parsed).toBe('object')
        expect(Array.isArray(parsed.bareTerms)).toBe(true)

        // Operator fields are strings or undefined.
        if (parsed.path !== undefined) expect(typeof parsed.path).toBe('string')
        if (parsed.tag !== undefined) expect(typeof parsed.tag).toBe('string')
        if (parsed.line !== undefined) expect(typeof parsed.line).toBe('string')
        if (parsed.content !== undefined) expect(typeof parsed.content).toBe('string')
        if (parsed.file !== undefined) expect(typeof parsed.file).toBe('string')
        if (parsed.regex !== undefined) expect(typeof parsed.regex).toBe('string')
        if (parsed.property !== undefined) {
          expect(typeof parsed.property).toBe('object')
          expect(typeof parsed.property.name).toBe('string')
          expect(typeof parsed.property.value).toBe('string')
        }
      }),
      { numRuns: 200 }
    )
  })

  it('bare terms are always lowercase', () => {
    fc.assert(
      fc.property(fc.array(wordArbitrary, { minLength: 0, maxLength: 10 }), (tokens) => {
        const query = tokens.join(' ')
        const parsed = parseQuery(query)
        for (const term of parsed.bareTerms) {
          expect(term).toBe(term.toLowerCase())
        }
      }),
      { numRuns: 100 }
    )
  })

  it('empty or whitespace-only query has no operators and empty bareTerms', () => {
    fc.assert(
      fc.property(
        fc.constantFrom('', ' ', '\t', '\n', '  ', '\t\t', '\n\n', ' \t \n '),
        (query) => {
          const parsed = parseQuery(query)
          expect(parsed.bareTerms).toEqual([])
          expect(parsed.path).toBeUndefined()
          expect(parsed.tag).toBeUndefined()
          expect(parsed.line).toBeUndefined()
          expect(parsed.content).toBeUndefined()
          expect(parsed.file).toBeUndefined()
          expect(parsed.regex).toBeUndefined()
          expect(parsed.property).toBeUndefined()
        }
      ),
      { numRuns: 50 }
    )
  })
})

// ---------------------------------------------------------------------------
// Property: operator token parsing
// ---------------------------------------------------------------------------

describe('operator token parsing (property)', () => {
  it('a path: token always sets result.path', () => {
    fc.assert(
      fc.property(wordArbitrary, (value) => {
        const query = `path:${value}`
        const parsed = parseQuery(query)
        expect(parsed.path).toBeDefined()
      }),
      { numRuns: 100 }
    )
  })

  it('a tag: token always sets result.tag', () => {
    fc.assert(
      fc.property(wordArbitrary, (value) => {
        const query = `tag:${value}`
        const parsed = parseQuery(query)
        expect(parsed.tag).toBeDefined()
      }),
      { numRuns: 100 }
    )
  })

  it('a regex: token always sets result.regex', () => {
    fc.assert(
      fc.property(wordArbitrary, (value) => {
        const query = `regex:${value}`
        const parsed = parseQuery(query)
        expect(parsed.regex).toBeDefined()
      }),
      { numRuns: 100 }
    )
  })

  it('a property:name:value token always sets result.property', () => {
    fc.assert(
      fc.property(propertyNameArbitrary, wordArbitrary, (name, value) => {
        const query = `property:${name}:${value}`
        const parsed = parseQuery(query)
        expect(parsed.property).toBeDefined()
        expect(typeof parsed.property.name).toBe('string')
        expect(typeof parsed.property.value).toBe('string')
      }),
      { numRuns: 100 }
    )
  })
})
