/**
 * QuickSwitcher.tsx
 *
 * Cmd+O modal for fuzzy note navigation. Fuzzy-matches the typed query
 * against file names, relative paths, and frontmatter aliases. Shows
 * recently opened notes when the query is empty.
 *
 * Requirements: 4.1, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8
 */

import React, { useEffect, useRef, useMemo, useCallback } from 'react'
import { useAppContext } from '../../shared/store'
import { fuzzySearch, type FuzzyItem, type FuzzyMatch, type FuzzyRange } from './fuzzy'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_RESULTS = 10
const DEBOUNCE_MS = 80

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Compute a relative path from the vault root. */
function relativePath(vaultPath: string, absolutePath: string): string {
  if (absolutePath.startsWith(vaultPath)) {
    let rel = absolutePath.slice(vaultPath.length)
    if (rel.startsWith('/') || rel.startsWith('\\')) rel = rel.slice(1)
    return rel
  }
  return absolutePath
}

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
// QuickSwitcher
// ---------------------------------------------------------------------------

export function QuickSwitcher(): React.JSX.Element | null {
  const { state, dispatch } = useAppContext()
  const vault = state.vault

  // --- Local state ---
  const [query, setQuery] = React.useState('')
  const [selectedIndex, setSelectedIndex] = React.useState(0)
  const [debouncedQuery, setDebouncedQuery] = React.useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const activeRef = useRef<HTMLDivElement>(null)

  // Debounce the query for ranking (Req 4.7).
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [query])

  // Reset selection when query or debounced query changes.
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

  // --- Build fuzzy items from vault files + extended index ---
  const fuzzyItems: FuzzyItem[] = useMemo(() => {
    if (!vault) return []

    // Build a reverse alias index: filePath → aliases[] (Req 15.4)
    const aliasReverse = new Map<string, string[]>()
    if (state.extendedIndex?.aliasIndex) {
      for (const [alias, paths] of state.extendedIndex.aliasIndex) {
        for (const p of paths) {
          const existing = aliasReverse.get(p)
          if (existing) {
            if (!existing.includes(alias)) existing.push(alias)
          } else {
            aliasReverse.set(p, [alias])
          }
        }
      }
    }

    return vault.files.map((file) => {
      const name = file.name.replace(/\.md$/i, '')
      const relPath = relativePath(vault.path, file.path)
      const aliases = aliasReverse.get(file.path) ?? undefined
      return { name, path: relPath, aliases }
    })
  }, [vault, state.extendedIndex])

  // --- Compute ranked results ---
  const results = useMemo((): FuzzyMatch<FuzzyItem>[] => {
    if (!debouncedQuery.trim()) {
      // Empty query → show recent notes (Req 4.6).
      // Map recent paths to FuzzyItem matches with a perfect score.
      const recentResults: FuzzyMatch<FuzzyItem>[] = []
      for (const path of state.recentNotes) {
        const file = fuzzyItems.find((f) => f.path === path || f.name === path)
        if (!file) continue

        // Find the item to get the real path
        const vaultFile = vault?.files.find(
          (f) => f.path === path || f.name.replace(/\.md$/i, '') === path
        )
        const actualPath = vaultFile ? relativePath(vault?.path ?? '', vaultFile.path) : path

        recentResults.push({
          item: { ...file, path: actualPath },
          score: 1,
          ranges: [] as FuzzyRange[],
          matchField: 'name' as const
        })
      }
      return recentResults
    }

    return fuzzySearch(debouncedQuery, fuzzyItems, {
      maxResults: MAX_RESULTS,
      threshold: 0.05
    })
  }, [debouncedQuery, fuzzyItems, state.recentNotes, vault])

  // --- Open a note ---
  const openNote = useCallback(
    (filePath: string) => {
      window.ipc.file
        .get(filePath)
        .then((fileAST) => {
          dispatch({
            type: 'FILE_LOADED',
            payload: { path: fileAST.path, ast: fileAST.ast }
          })
        })
        .catch((err) => {
          console.error('[QuickSwitcher] failed to open:', err)
        })
      dispatch({ type: 'QUICK_SWITCHER_CLOSE' })
    },
    [dispatch]
  )

  // --- Find the full file path for a result ---
  const resolveFilePath = useCallback(
    (match: FuzzyMatch): string | null => {
      if (!vault) return null

      // Try matching by name
      const byName = vault.files.find((f) => f.name.replace(/\.md$/i, '') === match.item.name)
      // Or try matching by relative path
      const byPath = vault.files.find((f) =>
        relativePath(vault.path, f.path).includes(match.item.path)
      )

      return byName?.path ?? byPath?.path ?? null
    },
    [vault]
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
            const filePath = resolveFilePath(results[selectedIndex])
            if (filePath) openNote(filePath)
          }
          break
        case 'Escape':
          e.preventDefault()
          dispatch({ type: 'QUICK_SWITCHER_CLOSE' })
          break
      }
    },
    [results, selectedIndex, openNote, resolveFilePath, dispatch]
  )

  // --- Global Esc handler (close when clicking backdrop or pressing Esc elsewhere) ---
  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        dispatch({ type: 'QUICK_SWITCHER_CLOSE' })
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [dispatch])

  // --- Render ---
  const hasQuery = query.trim().length > 0
  const showRecents = !hasQuery && state.recentNotes.length > 0
  const showEmpty = hasQuery && results.length === 0

  return (
    // Backdrop
    <div
      className="quick-switcher__backdrop"
      onClick={() => dispatch({ type: 'QUICK_SWITCHER_CLOSE' })}
      onKeyDown={() => {}}
      role="presentation"
    >
      {/* Modal */}
      <div
        className="quick-switcher"
        role="dialog"
        aria-label="Quick switcher"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={() => {}}
      >
        {/* Search input */}
        <div className="quick-switcher__input-wrapper">
          <svg
            className="quick-switcher__search-icon"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            className="quick-switcher__input"
            placeholder="Go to note…"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setSelectedIndex(0)
            }}
            onKeyDown={handleKeyDown}
            aria-label="Search notes"
            autoComplete="off"
            spellCheck={false}
          />
        </div>

        {/* Results */}
        <div className="quick-switcher__results" role="listbox" aria-label="Results">
          {showRecents && (
            <>
              <div className="quick-switcher__section-label">Recent</div>
              {results.map((match, idx) => {
                const isSelected = idx === selectedIndex
                return (
                  <QuickSwitcherItem
                    key={match.item.path}
                    match={match}
                    isSelected={isSelected}
                    onClick={() => {
                      const fp = resolveFilePath(match)
                      if (fp) openNote(fp)
                    }}
                    onHover={() => setSelectedIndex(idx)}
                    innerRef={isSelected ? activeRef : undefined}
                  />
                )
              })}
            </>
          )}

          {hasQuery && results.length > 0 && (
            <>
              {results.map((match, idx) => {
                const isSelected = idx === selectedIndex
                return (
                  <QuickSwitcherItem
                    key={match.item.path}
                    match={match}
                    isSelected={isSelected}
                    onClick={() => {
                      const fp = resolveFilePath(match)
                      if (fp) openNote(fp)
                    }}
                    onHover={() => setSelectedIndex(idx)}
                    innerRef={isSelected ? activeRef : undefined}
                  />
                )
              })}
            </>
          )}

          {showEmpty && (
            <div className="quick-switcher__empty">
              No results for <strong>{query}</strong>
            </div>
          )}

          {!hasQuery && !showRecents && (
            <div className="quick-switcher__empty">Type to search notes</div>
          )}
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// QuickSwitcherItem — single result row
// ---------------------------------------------------------------------------

interface QuickSwitcherItemProps {
  match: FuzzyMatch
  isSelected: boolean
  onClick: () => void
  onHover: () => void
  innerRef?: React.Ref<HTMLDivElement>
}

function QuickSwitcherItem({
  match,
  isSelected,
  onClick,
  onHover,
  innerRef
}: QuickSwitcherItemProps): React.JSX.Element {
  const { item, ranges } = match

  return (
    <div
      ref={innerRef}
      className={`quick-switcher__result${isSelected ? ' quick-switcher__result--selected' : ''}`}
      role="option"
      aria-selected={isSelected}
      onClick={onClick}
      onMouseEnter={onHover}
    >
      <div className="quick-switcher__result-name">
        <HighlightedText text={item.name} ranges={ranges} />
      </div>
      <div className="quick-switcher__result-path">{item.path}</div>
    </div>
  )
}

export default QuickSwitcher
