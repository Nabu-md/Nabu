/**
 * Property-based tests for substituteVariables()
 *
 * Validates: Requirements 9.5
 */

import { describe, it } from 'vitest'
import * as fc from 'fast-check'
import { substituteVariables } from '../../src/main/templates'

// ---- Generators ----

// A token slot is one of the three known variables or empty string
const tokenArb = fc.constantFrom('{{title}}', '{{date}}', '{{time}}', '')

// Arbitrary vars record with bounded strings
const varsArb = fc.record({
  title: fc.string({ maxLength: 10 }),
  date: fc.string({ maxLength: 10 }),
  time: fc.string({ maxLength: 10 })
})

// Arbitrary fixed text segment (no `{{` sequences to avoid accidental token collisions)
const segmentArb = fc
  .string({ maxLength: 20 })
  .filter((s) => !s.includes('{{') && !s.includes('}}'))

describe('substituteVariables — property-based tests', () => {
  /**
   * Property 1 — Substitution completeness
   * For any template containing any mix of {{title}}, {{date}}, {{time}} tokens
   * plus random surrounding text, the output must not contain the literal token
   * strings {{title}}, {{date}}, or {{time}}.
   */
  it('Property 1: substitution completeness — output contains no unresolved {{...}} tokens', () => {
    fc.assert(
      fc.property(
        // Build a template by interleaving text segments and token slots
        fc.array(segmentArb, { minLength: 0, maxLength: 5 }),
        fc.array(tokenArb, { minLength: 0, maxLength: 5 }),
        varsArb,
        (segments, tokens, vars) => {
          // Interleave segments and tokens into one template string
          const parts: string[] = []
          const len = Math.max(segments.length, tokens.length)
          for (let i = 0; i < len; i++) {
            if (i < segments.length) parts.push(segments[i])
            if (i < tokens.length) parts.push(tokens[i])
          }
          const template = parts.join('')
          const result = substituteVariables(template, vars)

          return (
            !result.includes('{{title}}') &&
            !result.includes('{{date}}') &&
            !result.includes('{{time}}')
          )
        }
      )
    )
  })

  /**
   * Property 2 — Content preservation
   * Non-variable text segments survive substitution unchanged.
   * Build a template as: seg0 + token0 + seg1 + token1 + seg2 …
   * Every fixed segment must appear in the output.
   */
  it('Property 2: content preservation — fixed segments are preserved in output', () => {
    fc.assert(
      fc.property(
        // At least 1 segment so there is something to check
        fc.array(segmentArb, { minLength: 1, maxLength: 5 }),
        fc.array(tokenArb, { minLength: 0, maxLength: 4 }),
        varsArb,
        (segments, tokens, vars) => {
          // Build template: seg[0] token[0] seg[1] token[1] …
          const parts: string[] = []
          for (let i = 0; i < segments.length; i++) {
            parts.push(segments[i])
            if (i < tokens.length) parts.push(tokens[i])
          }
          const template = parts.join('')
          const result = substituteVariables(template, vars)

          // Every fixed segment must appear in the result
          return segments.every((seg) => result.includes(seg))
        }
      )
    )
  })

  /**
   * Property 3 — Idempotence
   * Calling substituteVariables on a string that contains no {{...}} tokens
   * returns the same string.
   */
  it('Property 3: idempotence — substituting an already-substituted string is a no-op', () => {
    fc.assert(
      fc.property(
        // Build any template, substitute once, then substitute again
        fc.array(segmentArb, { minLength: 0, maxLength: 5 }),
        fc.array(tokenArb, { minLength: 0, maxLength: 5 }),
        varsArb,
        (segments, tokens, vars) => {
          const parts: string[] = []
          const len = Math.max(segments.length, tokens.length)
          for (let i = 0; i < len; i++) {
            if (i < segments.length) parts.push(segments[i])
            if (i < tokens.length) parts.push(tokens[i])
          }
          const template = parts.join('')
          const firstPass = substituteVariables(template, vars)
          const secondPass = substituteVariables(firstPass, vars)

          return firstPass === secondPass
        }
      )
    )
  })
})
