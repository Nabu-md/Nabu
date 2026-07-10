/**
 * CommandPalette.tsx
 *
 * Cmd+P modal that reads the command registry and fuzzy-filters commands
 * by label, id, and keywords. Selecting a command runs its action and
 * closes the palette.
 *
 * Requirements: 5.1, 5.4, 5.5
 */

import React, { useEffect, useRef, useMemo, useCallback } from 'react'
import { useAppContext } from '../App'
import { getCommands, type Command } from '../commands/registry'
import { matchScore, type FuzzyRange } from '../utils/fuzzy'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEBOUNCE_MS = 80
const MAX_RESULTS = 20

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ScoredCommand {
  command: Command
  score: number
  ranges: FuzzyRange[]
}

// ---------------------------------------------------------------------------
// HighlightedText
// ---------------------------------------------------------------------------

/** Highlight matched ranges in a string using `<mark>`. */
function HighlightedText({
  text,
  ranges
}: {
  text: string
  ranges: { start: number; end: number }[]
}): React.JSX.Element {
  if (ranges.length === 0) return <>{text}</>

  const parts: React.JSX.Element[] = []
  let lastEnd = 0

  for (const r of ranges) {
    if (r.start > lastEnd) {
      parts.push(<span key={`t-${lastEnd}`}>{text.slice(lastEnd, r.start)}</span>)
    }
    parts.push(
      <mark key={`m-${r.start}`} className="quick-switcher__mark">
        {text.slice(r.start, r.end)}
      </mark>
    )
    lastEnd = r.end
  }

  if (lastEnd < text.length) {
    parts.push(<span key={`t-${lastEnd}`}>{text.slice(lastEnd)}</span>)
  }

  return <>{parts}</>
}

// ---------------------------------------------------------------------------
// CommandPalette
// ---------------------------------------------------------------------------

export function CommandPalette(): React.JSX.Element | null {
  const { dispatch } = useAppContext()

  // --- Local state ---
  const [query, setQuery] = React.useState('')
  const [selectedIndex, setSelectedIndex] = React.useState(0)
  const [debouncedQuery, setDebouncedQuery] = React.useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const activeRef = useRef<HTMLDivElement>(null)

  // Debounce the query for ranking.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [query])

  // Reset selection when debounced query changes.
  useEffect(() => {
    setSelectedIndex(0)
  }, [debouncedQuery])

  // Auto-focus the input on mount.
  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // Scroll selected result into view.
  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  // --- Compute ranked results ---
  const results = useMemo((): ScoredCommand[] => {
    const commands = getCommands()
    if (!debouncedQuery.trim()) {
      // Empty query — show all commands (Req 5.1).
      return commands.slice(0, MAX_RESULTS).map((c) => ({
        command: c,
        score: 1,
        ranges: []
      }))
    }

    const scored: ScoredCommand[] = []

    for (const cmd of commands) {
      let bestScore = -1
      let bestRanges: FuzzyRange[] = []

      // Match against label.
      const labelMatch = matchScore(debouncedQuery, cmd.label)
      if (labelMatch && labelMatch.score > bestScore) {
        bestScore = labelMatch.score
        bestRanges = labelMatch.ranges
      }

      // Match against id (e.g. "edit.toggle").
      const idMatch = matchScore(debouncedQuery, cmd.id)
      if (idMatch && idMatch.score > bestScore) {
        bestScore = idMatch.score
        bestRanges = idMatch.ranges
      }

      // Match against keywords.
      if (cmd.keywords) {
        for (const kw of cmd.keywords) {
          const kwMatch = matchScore(debouncedQuery, kw)
          if (kwMatch && kwMatch.score > bestScore) {
            bestScore = kwMatch.score
            bestRanges = kwMatch.ranges
          }
        }
      }

      if (bestScore >= 0) {
        scored.push({ command: cmd, score: bestScore, ranges: bestRanges })
      }
    }

    // Sort by score descending, then by label ascending for determinism.
    scored.sort((a, b) => {
      if (a.score !== b.score) return b.score - a.score
      return a.command.label.localeCompare(b.command.label)
    })

    return scored.slice(0, MAX_RESULTS)
  }, [debouncedQuery])

  // --- Execute a command and close ---
  const executeCommand = useCallback(
    (cmd: Command) => {
      cmd.run()
      dispatch({ type: 'COMMAND_PALETTE_CLOSE' })
    },
    [dispatch]
  )

  // --- Keyboard navigation ---
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setSelectedIndex((p) => Math.min(p + 1, results.length - 1))
          break
        case 'ArrowUp':
          e.preventDefault()
          setSelectedIndex((p) => Math.max(p - 1, 0))
          break
        case 'Enter':
          e.preventDefault()
          if (results[selectedIndex]) {
            executeCommand(results[selectedIndex].command)
          }
          break
        case 'Escape':
          e.preventDefault()
          dispatch({ type: 'COMMAND_PALETTE_CLOSE' })
          break
      }
    },
    [results, selectedIndex, executeCommand, dispatch]
  )

  // --- Global Esc handler ---
  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        dispatch({ type: 'COMMAND_PALETTE_CLOSE' })
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [dispatch])

  // --- Render ---
  return (
    // Backdrop
    <div
      className="quick-switcher__backdrop"
      onClick={() => dispatch({ type: 'COMMAND_PALETTE_CLOSE' })}
      onKeyDown={() => {}}
      role="presentation"
    >
      {/* Modal */}
      <div
        className="quick-switcher"
        role="dialog"
        aria-label="Command palette"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={() => {}}
      >
        {/* Search input */}
        <div className="quick-switcher__input-wrapper">
          <input
            ref={inputRef}
            type="text"
            className="quick-switcher__input"
            placeholder="Type a command…"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setSelectedIndex(0)
            }}
            onKeyDown={handleKeyDown}
            aria-label="Search commands"
            autoComplete="off"
            spellCheck={false}
          />
        </div>

        {/* Results */}
        <div className="quick-switcher__results" role="listbox" aria-label="Commands">
          {results.map((result, idx) => {
            const isSelected = idx === selectedIndex
            return (
              <div
                key={result.command.id}
                ref={isSelected ? activeRef : undefined}
                className={`quick-switcher__result${isSelected ? ' quick-switcher__result--selected' : ''}`}
                role="option"
                aria-selected={isSelected}
                onClick={() => executeCommand(result.command)}
                onMouseEnter={() => setSelectedIndex(idx)}
                onKeyDown={() => {}}
              >
                <div className="quick-switcher__result-name">
                  <HighlightedText text={result.command.label} ranges={result.ranges} />
                </div>
                <div className="quick-switcher__result-path">{result.command.id}</div>
              </div>
            )
          })}

          {results.length === 0 && (
            <div className="quick-switcher__empty">
              No command matches <strong>{query}</strong>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default CommandPalette
