/**
 * command-palette.test.ts
 *
 * Tests for the CommandPalette filtering logic.
 * The component itself follows the same pattern as QuickSwitcher (no DOM tests),
 * so we verify the integration of getCommands() with matchScore() which is
 * the core logic the palette depends on.
 *
 * Requirements: 5.1, 5.4, 5.5
 */

import { describe, it, expect, beforeEach } from 'vitest'
import {
  resetRegistry,
  registerCommand,
  getCommands,
  type Command
} from '../../src/renderer/src/commands/registry'
import { matchScore } from '../../src/renderer/src/utils/fuzzy'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Score a query against each command's label/id/keywords, return the best. */
function scoreCommand(
  cmd: Command,
  query: string
): { score: number; field: 'label' | 'id' | 'keyword' } | null {
  let best: { score: number; field: 'label' | 'id' | 'keyword' } | null = null

  const labelMatch = matchScore(query, cmd.label)
  if (labelMatch) {
    best = { score: labelMatch.score, field: 'label' }
  }

  const idMatch = matchScore(query, cmd.id)
  if (idMatch && (!best || idMatch.score > best.score)) {
    best = { score: idMatch.score, field: 'id' }
  }

  if (cmd.keywords) {
    for (const kw of cmd.keywords) {
      const kwMatch = matchScore(query, kw)
      if (kwMatch && (!best || kwMatch.score > best.score)) {
        best = { score: kwMatch.score, field: 'keyword' }
      }
    }
  }

  return best
}

/** Filter and rank all registered commands against a query. */
function filterCommands(query: string): Array<{ command: Command; score: number }> {
  if (!query.trim()) {
    return getCommands().map((c) => ({ command: c, score: 1 }))
  }

  const scored = getCommands()
    .map((cmd) => {
      const result = scoreCommand(cmd, query)
      return result ? { command: cmd, score: result.score, field: result.field } : null
    })
    .filter((r): r is NonNullable<typeof r> => r !== null)

  scored.sort((a, b) => {
    if (a.score !== b.score) return b.score - a.score
    return a.command.label.localeCompare(b.command.label)
  })

  return scored.map(({ command, score }) => ({ command, score }))
}

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const testCommands: Command[] = [
  {
    id: 'edit.toggle',
    label: 'Toggle edit / view mode',
    keywords: ['edit', 'view', 'toggle'],
    run: () => {}
  },
  { id: 'graph.toggle', label: 'Toggle graph view', keywords: ['graph', 'toggle'], run: () => {} },
  {
    id: 'search.toggle',
    label: 'Toggle search panel',
    keywords: ['search', 'find'],
    run: () => {}
  },
  {
    id: 'switcher.open',
    label: 'Go to note',
    keywords: ['switcher', 'quick', 'open', 'navigate'],
    run: () => {}
  },
  {
    id: 'settings.open',
    label: 'Open settings',
    keywords: ['settings', 'preferences', 'config'],
    run: () => {}
  },
  {
    id: 'note.create',
    label: 'Create new note',
    keywords: ['new', 'create', 'note'],
    run: () => {}
  }
]

beforeEach(() => {
  resetRegistry()
  for (const cmd of testCommands) {
    registerCommand(cmd)
  }
})

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('CommandPalette filtering', () => {
  it('returns all commands when query is empty', () => {
    const results = filterCommands('')
    expect(results).toHaveLength(testCommands.length)
  })

  it('filters by label text', () => {
    const results = filterCommands('toggle')
    // Should match "Toggle edit/view mode", "Toggle graph view", "Toggle search panel"
    expect(results.length).toBeGreaterThanOrEqual(3)
    for (const r of results) {
      expect(r.command.label.toLowerCase()).toContain('toggle')
    }
  })

  it('filters by command id', () => {
    const results = filterCommands('graph')
    expect(results.length).toBeGreaterThanOrEqual(1)
    expect(results[0].command.id).toBe('graph.toggle')
  })

  it('filters by keyword', () => {
    const results = filterCommands('preferences')
    expect(results.length).toBeGreaterThanOrEqual(1)
    expect(results[0].command.id).toBe('settings.open')
  })

  it('returns empty array for non-matching query', () => {
    const results = filterCommands('xyznonexistent')
    expect(results).toHaveLength(0)
  })

  it('sorts by score descending', () => {
    const results = filterCommands('toggle')
    for (let i = 1; i < results.length; i++) {
      expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score)
    }
  })

  it('breaks ties by label alphabetically', () => {
    const results = filterCommands('note')
    for (let i = 1; i < results.length; i++) {
      if (results[i - 1].score === results[i].score) {
        expect(
          results[i - 1].command.label.localeCompare(results[i].command.label)
        ).toBeLessThanOrEqual(0)
      }
    }
  })

  it('partial query matches id segments', () => {
    // "toggle" should match "edit.toggle" via the "toggle" part of the id
    const results = filterCommands('toggle')
    const editToggle = results.find((r) => r.command.id === 'edit.toggle')
    expect(editToggle).toBeDefined()
  })
})
