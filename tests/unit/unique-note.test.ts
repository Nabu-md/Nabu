/**
 * unique-note.test.ts
 *
 * Unit tests for unique note name generation.
 *
 * Requirements: 27.1, 27.2, 27.3, 27.4, 27.5
 */

import { describe, it, expect } from 'vitest'
import { generateUniqueNoteName } from '../../src/main/services/unique-note'

describe('generateUniqueNoteName', () => {
  it('generates default YYYYMMDDHHmmss format', () => {
    const result = generateUniqueNoteName('YYYYMMDDHHmmss', new Date('2026-07-08T14:30:00'))
    expect(result).toBe('20260708143000')
  })

  it('generates custom format with YYYY', () => {
    const result = generateUniqueNoteName('YYYY-MM-DD', new Date('2026-01-08T00:00:00'))
    expect(result).toBe('2026-01-08')
  })

  it('generates custom format with time components', () => {
    const result = generateUniqueNoteName('HH-mm-ss', new Date('2026-01-01T09:05:03'))
    expect(result).toBe('09-05-03')
  })
})
