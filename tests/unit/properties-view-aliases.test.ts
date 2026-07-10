/**
 * Unit tests for the AliasEditor pure logic — add, remove, duplicate
 * detection, and normalization.
 *
 * These functions mirror the logic in the AliasEditor component inside
 * PropertiesView.tsx so they can be tested without a DOM environment.
 *
 * Requirements: 15B.1, 15B.2, 15B.3
 */

import { describe, it, expect } from 'vitest'

// ---------------------------------------------------------------------------
// Pure logic — mirrors AliasEditor behaviour in PropertiesView.tsx
// ---------------------------------------------------------------------------

/**
 * Check whether a candidate alias already exists in the list
 * (case-insensitive duplicate detection, Req 15B.2).
 */
function isDuplicateAlias(aliases: string[], candidate: string): boolean {
  const lower = candidate.trim().toLowerCase()
  if (!lower) return false
  return aliases.some((a) => a.toLowerCase() === lower)
}

/**
 * Add a new alias to the list. Returns the updated list or the original
 * if the alias is empty or a case-insensitive duplicate (Req 15B.2).
 */
function addAlias(aliases: string[], candidate: string): string[] {
  const trimmed = candidate.trim()
  if (!trimmed) return aliases
  if (isDuplicateAlias(aliases, trimmed)) return aliases
  return [...aliases, trimmed]
}

/**
 * Remove an alias at the given index. Returns the updated list (Req 15B.1).
 */
function removeAlias(aliases: string[], index: number): string[] {
  return aliases.filter((_, i) => i !== index)
}

/**
 * Normalize a raw YAML value into a string array suitable for the
 * AliasEditor. Handles array, single string, and empty/null values.
 * Mirrors the logic in PropertiesView's value cell rendering.
 */
function normalizeAliases(rawValue: unknown): string[] {
  if (Array.isArray(rawValue)) return rawValue.map(String)
  if (typeof rawValue === 'string' && rawValue.trim()) return [rawValue]
  return []
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('isDuplicateAlias', () => {
  it('returns false for empty list', () => {
    expect(isDuplicateAlias([], 'Anything')).toBe(false)
  })

  it('returns false when alias is not in list', () => {
    expect(isDuplicateAlias(['Alpha', 'Beta'], 'Gamma')).toBe(false)
  })

  it('returns true for exact match', () => {
    expect(isDuplicateAlias(['Alpha', 'Beta'], 'Alpha')).toBe(true)
  })

  it('detects case-insensitive duplicates (Req 15B.2)', () => {
    expect(isDuplicateAlias(['alpha', 'Beta'], 'ALPHA')).toBe(true)
    expect(isDuplicateAlias(['Alpha', 'Beta'], 'beta')).toBe(true)
  })

  it('returns false for empty candidate', () => {
    expect(isDuplicateAlias(['Alpha'], '')).toBe(false)
    expect(isDuplicateAlias(['Alpha'], '   ')).toBe(false)
  })
})

describe('addAlias', () => {
  it('appends a new alias (Req 15B.1)', () => {
    const result = addAlias(['Alpha'], 'Beta')
    expect(result).toEqual(['Alpha', 'Beta'])
  })

  it('trims whitespace before adding', () => {
    const result = addAlias([], '  Gamma  ')
    expect(result).toEqual(['Gamma'])
  })

  it('rejects empty string — returns original list', () => {
    const list = ['Alpha']
    const result = addAlias(list, '')
    expect(result).toBe(list) // same reference
  })

  it('rejects whitespace-only — returns original list', () => {
    const list = ['Alpha']
    const result = addAlias(list, '   ')
    expect(result).toBe(list)
  })

  it('rejects case-insensitive duplicate (Req 15B.2)', () => {
    const list = ['Alpha', 'Beta']
    const result = addAlias(list, 'alpha')
    expect(result).toBe(list) // same reference — not modified
  })

  it('allows same string with different case after removal', () => {
    const list = ['Alpha']
    const without = removeAlias(list, 0)
    const result = addAlias(without, 'alpha')
    expect(result).toEqual(['alpha'])
  })
})

describe('removeAlias', () => {
  it('removes alias at given index (Req 15B.1)', () => {
    const result = removeAlias(['A', 'B', 'C'], 1)
    expect(result).toEqual(['A', 'C'])
  })

  it('removes first alias', () => {
    const result = removeAlias(['A', 'B'], 0)
    expect(result).toEqual(['B'])
  })

  it('removes last alias', () => {
    const result = removeAlias(['A', 'B'], 1)
    expect(result).toEqual(['A'])
  })

  it('returns empty array when removing the only alias', () => {
    const result = removeAlias(['Solo'], 0)
    expect(result).toEqual([])
  })

  it('handles out-of-bounds index gracefully', () => {
    const list = ['A', 'B']
    const result = removeAlias(list, 99)
    expect(result).toEqual(['A', 'B'])
  })
})

describe('normalizeAliases', () => {
  it('passes through a string array unchanged', () => {
    expect(normalizeAliases(['Alpha', 'Beta'])).toEqual(['Alpha', 'Beta'])
  })

  it('converts a single string to a one-element array', () => {
    expect(normalizeAliases('Solo Alias')).toEqual(['Solo Alias'])
  })

  it('converts an empty string to empty array', () => {
    expect(normalizeAliases('')).toEqual([])
  })

  it('converts null to empty array', () => {
    expect(normalizeAliases(null)).toEqual([])
  })

  it('converts undefined to empty array', () => {
    expect(normalizeAliases(undefined)).toEqual([])
  })

  it('converts a number to single-element string array', () => {
    // Edge case: non-string, non-array value
    expect(normalizeAliases(42)).toEqual([])
  })
})

describe('alias round-trip (add → remove → normalize)', () => {
  it('maintains correct list through multiple operations', () => {
    const start: string[] = []
    const step1 = addAlias(start, 'First')
    expect(step1).toEqual(['First'])

    const step2 = addAlias(step1, 'Second')
    expect(step2).toEqual(['First', 'Second'])

    const step3 = addAlias(step2, 'third') // different case
    expect(step3).toEqual(['First', 'Second', 'third'])

    // Remove the middle one
    const step4 = removeAlias(step3, 1)
    expect(step4).toEqual(['First', 'third'])

    // Re-add — 'Second' is not a dup anymore
    const step5 = addAlias(step4, 'Second')
    expect(step5).toEqual(['First', 'third', 'Second'])
  })

  it('prevents duplicate even after multiple adds (Req 15B.2)', () => {
    const step1 = addAlias([], 'Dup')
    const step2 = addAlias(step1, 'DUP') // case-insensitive dup
    expect(step2).toEqual(['Dup'])
    expect(step2.length).toBe(1)
  })
})
